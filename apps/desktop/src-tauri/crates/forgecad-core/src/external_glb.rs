//! Strict, read-only external GLB reference contract.
//!
//! User supplied bytes never become a ShapeProgram, production geometry or
//! executable input.  This module only accepts a bounded, self-contained GLB
//! 2.0 container and records facts that can be read back from those bytes.

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    semantic_sha256, ActiveDesignSnapshot, AgentAssetVersion, AssetStage, AssetVersionStatus,
    CoreError, CoreResult, ObjectRecord,
};

pub const EXTERNAL_GLB_REFERENCE_ROLE: &str = "external_reference_glb";
pub const EXTERNAL_GLB_ARTIFACT_PROFILE_ID: &str = "external_reference";
pub const MAX_IMPORTED_GLB_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_IMPORTED_GLB_TRIANGLES: u64 = 250_000;
const MAX_ENCODED_GLB_BYTES: usize = 44_739_244;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImportExternalGlbRequest {
    pub client_request_id: String,
    pub project_id: String,
    pub domain_pack_id: String,
    pub file_name: String,
    pub glb_base64: String,
    #[serde(default = "default_import_summary")]
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ImportedGlbInspection {
    pub sha256: String,
    pub byte_size: u64,
    pub triangle_count: u64,
    pub bounds_mm: [f64; 3],
    pub mesh_count: u64,
    pub primitive_count: u64,
    pub material_count: u64,
    pub node_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ImportExternalGlbResponse {
    pub asset_version: AgentAssetVersion,
    pub inspection: ImportedGlbInspection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ImportedGlbRecord {
    pub import_id: String,
    pub project_id: String,
    pub asset_version_id: String,
    pub domain_pack_id: String,
    pub file_name: String,
    pub object_path: String,
    pub sha256: String,
    pub byte_size: u64,
    pub triangle_count: u64,
    pub bounds_mm: [f64; 3],
    pub mesh_count: u64,
    pub primitive_count: u64,
    pub material_count: u64,
    pub node_count: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExternalGlbImportBundleReadback {
    pub response: ImportExternalGlbResponse,
    pub imported_glb: ImportedGlbRecord,
    pub object: ObjectRecord,
    /// Present only while this imported Version is still the active Snapshot.
    /// Historical idempotency replay remains valid after later immutable work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<ActiveDesignSnapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ValidatedExternalGlb {
    pub bytes: Vec<u8>,
    pub inspection: ImportedGlbInspection,
}

impl ImportExternalGlbRequest {
    pub(crate) fn validate_and_decode(&self) -> CoreResult<ValidatedExternalGlb> {
        require_text("client_request_id", &self.client_request_id, 120)?;
        require_text("project_id", &self.project_id, 160)?;
        require_text("file_name", &self.file_name, 180)?;
        require_text("summary", &self.summary, 500)?;
        if !valid_domain_pack_id(&self.domain_pack_id) {
            return Err(CoreError::invalid_data(
                "DOMAIN_PACK_INVALID",
                "GLB import domain_pack_id must use a stable pack_* identity.",
            ));
        }
        if self.glb_base64.len() < 16 || self.glb_base64.len() > MAX_ENCODED_GLB_BYTES {
            return Err(CoreError::invalid_data(
                "GLB_BASE64_INVALID",
                "GLB import payload is outside the bounded raw-base64 contract.",
            ));
        }
        if self.glb_base64.starts_with("data:") || self.glb_base64.contains("://") {
            return Err(CoreError::invalid_data(
                "GLB_BASE64_INVALID",
                "GLB import payload must be raw base64, not a URL or data URI.",
            ));
        }
        let bytes = BASE64_STANDARD.decode(&self.glb_base64).map_err(|_| {
            CoreError::invalid_data(
                "GLB_BASE64_INVALID",
                "GLB import payload is not canonical base64.",
            )
        })?;
        let inspection = inspect_external_glb(&bytes)?;
        Ok(ValidatedExternalGlb { bytes, inspection })
    }

    /// Matches the historical Python idempotency contract exactly.  The
    /// client_request_id is transport identity and deliberately not semantic.
    pub(crate) fn request_hash(&self) -> CoreResult<String> {
        let encoded_sha256 = sha256_bytes(self.glb_base64.as_bytes());
        semantic_sha256(&json!({
            "project_id": self.project_id,
            "domain_pack_id": self.domain_pack_id,
            "file_name": self.file_name,
            "summary": self.summary,
            "glb_sha256": encoded_sha256,
        }))
    }
}

impl ImportedGlbInspection {
    pub fn validate(&self) -> CoreResult<()> {
        validate_sha256(&self.sha256)?;
        if self.byte_size < 20
            || self.byte_size > MAX_IMPORTED_GLB_BYTES as u64
            || self.triangle_count == 0
            || self.triangle_count > MAX_IMPORTED_GLB_TRIANGLES
            || self.mesh_count == 0
            || self.primitive_count == 0
            || self
                .bounds_mm
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(CoreError::invalid_data(
                "GLB_IMPORT_INSPECTION_INVALID",
                "Imported GLB inspection is outside the read-only reference contract.",
            ));
        }
        Ok(())
    }
}

impl ImportedGlbRecord {
    pub fn inspection(&self) -> ImportedGlbInspection {
        ImportedGlbInspection {
            sha256: self.sha256.clone(),
            byte_size: self.byte_size,
            triangle_count: self.triangle_count,
            bounds_mm: self.bounds_mm,
            mesh_count: self.mesh_count,
            primitive_count: self.primitive_count,
            material_count: self.material_count,
            node_count: self.node_count,
        }
    }

    pub fn validate(&self) -> CoreResult<()> {
        require_stable_id("import_id", &self.import_id, "glbimport_")?;
        require_identifier("project_id", &self.project_id)?;
        require_stable_id("asset_version_id", &self.asset_version_id, "assetver_")?;
        if !valid_domain_pack_id(&self.domain_pack_id) {
            return Err(CoreError::invalid_data(
                "DOMAIN_PACK_INVALID",
                "Imported GLB record has an invalid Domain Pack identity.",
            ));
        }
        require_text("file_name", &self.file_name, 180)?;
        require_text("created_at", &self.created_at, 128)?;
        self.inspection().validate()?;
        let expected_path = format!(
            "objects/sha256/{}/{}/{}.glb",
            &self.sha256[..2],
            &self.sha256[2..4],
            self.sha256
        );
        if self.object_path != expected_path || self.file_name.contains(['/', '\\', '\0']) {
            return Err(CoreError::invalid_data(
                "GLB_IMPORT_RECORD_INVALID",
                "Imported GLB record contains a non-canonical object path or display filename.",
            ));
        }
        Ok(())
    }
}

pub fn inspect_external_glb(bytes: &[u8]) -> CoreResult<ImportedGlbInspection> {
    inspect_external_glb_inner(bytes).map_err(|message| {
        CoreError::invalid_data(
            "GLB_IMPORT_REJECTED",
            format!("External GLB was rejected: {message}"),
        )
    })
}

fn inspect_external_glb_inner(bytes: &[u8]) -> Result<ImportedGlbInspection, &'static str> {
    let (document, binary) = parse_glb_chunks(bytes)?;
    if document
        .get("asset")
        .and_then(|asset| asset.get("version"))
        .and_then(Value::as_str)
        != Some("2.0")
    {
        return Err("asset.version must be exactly 2.0");
    }
    if uses_unsupported_compression(&document) {
        return Err("compressed mesh extensions are not accepted");
    }

    let buffers = document
        .get("buffers")
        .and_then(Value::as_array)
        .filter(|items| items.len() == 1)
        .ok_or("exactly one embedded buffer is required")?;
    let buffer = buffers[0]
        .as_object()
        .ok_or("buffer metadata must be an object")?;
    if buffer.get("uri").is_some_and(|value| !value.is_null()) {
        return Err("external buffer URIs are not accepted");
    }
    let declared_buffer_length = buffer
        .get("byteLength")
        .and_then(Value::as_u64)
        .ok_or("buffer byteLength is required")?;
    if declared_buffer_length == 0 || declared_buffer_length > binary.len() as u64 {
        return Err("buffer byteLength exceeds the BIN chunk");
    }

    let images = document
        .get("images")
        .map(|value| value.as_array().ok_or("images must be an array"))
        .transpose()?
        .cloned()
        .unwrap_or_default();
    if images
        .iter()
        .any(|image| !image.is_object() || image.get("uri").is_some_and(|value| !value.is_null()))
    {
        return Err("external image URIs are not accepted");
    }

    let accessors = document
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or("accessors are required")?;
    let views = document
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or("bufferViews are required")?;
    let meshes = document
        .get("meshes")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or("at least one mesh is required")?;

    let mut triangle_count = 0_u64;
    let mut primitive_count = 0_u64;
    let mut minimum = [f64::INFINITY; 3];
    let mut maximum = [f64::NEG_INFINITY; 3];
    for mesh in meshes {
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .filter(|items| !items.is_empty())
            .ok_or("every mesh must contain a primitive")?;
        for primitive in primitives {
            if primitive.get("mode").and_then(Value::as_u64).unwrap_or(4) != 4 {
                return Err("only triangle primitives are accepted");
            }
            let position_index = primitive
                .get("attributes")
                .and_then(|value| value.get("POSITION"))
                .and_then(Value::as_u64)
                .ok_or("every primitive requires POSITION")?
                as usize;
            let position = validate_accessor(
                accessors,
                views,
                &binary,
                declared_buffer_length,
                position_index,
            )?;
            if position.get("componentType").and_then(Value::as_u64) != Some(5126)
                || position.get("type").and_then(Value::as_str) != Some("VEC3")
            {
                return Err("POSITION must be a float VEC3 accessor");
            }
            let lower = finite_vec3(position.get("min"))?;
            let upper = finite_vec3(position.get("max"))?;
            if (0..3).any(|axis| lower[axis] > upper[axis]) {
                return Err("POSITION min exceeds max");
            }
            for axis in 0..3 {
                minimum[axis] = minimum[axis].min(lower[axis]);
                maximum[axis] = maximum[axis].max(upper[axis]);
            }

            let index_count = if let Some(index) = primitive.get("indices") {
                let index = index.as_u64().ok_or("indices must name an accessor")? as usize;
                let accessor =
                    validate_accessor(accessors, views, &binary, declared_buffer_length, index)?;
                if accessor.get("type").and_then(Value::as_str) != Some("SCALAR")
                    || !matches!(
                        accessor.get("componentType").and_then(Value::as_u64),
                        Some(5121 | 5123 | 5125)
                    )
                {
                    return Err("indices must be an unsigned SCALAR accessor");
                }
                accessor
                    .get("count")
                    .and_then(Value::as_u64)
                    .ok_or("indices count is required")?
            } else {
                position
                    .get("count")
                    .and_then(Value::as_u64)
                    .ok_or("POSITION count is required")?
            };
            if index_count == 0 || index_count % 3 != 0 {
                return Err("triangle index count must be positive and divisible by three");
            }
            triangle_count = triangle_count
                .checked_add(index_count / 3)
                .ok_or("triangle count overflow")?;
            primitive_count = primitive_count
                .checked_add(1)
                .ok_or("primitive count overflow")?;
        }
    }
    if triangle_count == 0 || triangle_count > MAX_IMPORTED_GLB_TRIANGLES {
        return Err("triangle count is outside the 250000 limit");
    }
    let bounds_mm = std::array::from_fn(|axis| {
        let value = (maximum[axis] - minimum[axis]) * 1000.0;
        (value * 10_000.0).round() / 10_000.0
    });
    let inspection = ImportedGlbInspection {
        sha256: sha256_bytes(bytes),
        byte_size: bytes.len() as u64,
        triangle_count,
        bounds_mm,
        mesh_count: meshes.len() as u64,
        primitive_count,
        material_count: document
            .get("materials")
            .and_then(Value::as_array)
            .map_or(0, |items| items.len() as u64),
        node_count: document
            .get("nodes")
            .and_then(Value::as_array)
            .map_or(0, |items| items.len() as u64),
    };
    inspection.validate().map_err(|_| "inspection is invalid")?;
    Ok(inspection)
}

fn parse_glb_chunks(bytes: &[u8]) -> Result<(Value, Vec<u8>), &'static str> {
    if bytes.len() < 20 || bytes.len() > MAX_IMPORTED_GLB_BYTES || bytes.get(..4) != Some(b"glTF") {
        return Err("container size or magic is invalid");
    }
    if read_u32(bytes, 4)? != 2 || read_u32(bytes, 8)? as usize != bytes.len() {
        return Err("container version or declared length is invalid");
    }
    let mut cursor = 12_usize;
    let mut document = None;
    let mut binary = None;
    let mut chunk_index = 0_usize;
    while cursor < bytes.len() {
        if cursor + 8 > bytes.len() {
            return Err("chunk header exceeds the container");
        }
        let length = read_u32(bytes, cursor)? as usize;
        let kind = read_u32(bytes, cursor + 4)?;
        if length % 4 != 0 {
            return Err("chunk length is not 4-byte aligned");
        }
        let start = cursor + 8;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or("chunk exceeds the container")?;
        match kind {
            0x4e4f534a if document.is_none() && chunk_index == 0 => {
                let json_bytes = trim_glb_json_padding(&bytes[start..end]);
                document = Some(
                    serde_json::from_slice::<Value>(json_bytes)
                        .map_err(|_| "JSON chunk is invalid")?,
                );
            }
            0x004e4942 if binary.is_none() && document.is_some() => {
                binary = Some(bytes[start..end].to_vec())
            }
            0x4e4f534a | 0x004e4942 => {
                return Err("JSON or BIN chunk is duplicated or out of order")
            }
            _ => {}
        }
        cursor = end;
        chunk_index += 1;
    }
    if cursor != bytes.len() {
        return Err("container has trailing bytes");
    }
    let document = document
        .filter(Value::is_object)
        .ok_or("one JSON object chunk is required")?;
    let binary = binary
        .filter(|value| !value.is_empty())
        .ok_or("one non-empty BIN chunk is required")?;
    Ok((document, binary))
}

fn validate_accessor<'a>(
    accessors: &'a [Value],
    views: &[Value],
    binary: &[u8],
    declared_buffer_length: u64,
    accessor_index: usize,
) -> Result<&'a Value, &'static str> {
    let accessor = accessors
        .get(accessor_index)
        .filter(|value| value.is_object())
        .ok_or("accessor reference is invalid")?;
    if accessor.get("sparse").is_some() {
        return Err("sparse accessors are not accepted");
    }
    let view_index = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or("accessor bufferView is required")? as usize;
    let view = views
        .get(view_index)
        .filter(|value| value.is_object())
        .ok_or("bufferView reference is invalid")?;
    if view.get("buffer").and_then(Value::as_u64).unwrap_or(0) != 0 {
        return Err("bufferView must reference the embedded buffer");
    }
    let component_type = accessor
        .get("componentType")
        .and_then(Value::as_u64)
        .ok_or("accessor componentType is required")?;
    let component_size = match component_type {
        5120 | 5121 => 1_u64,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        _ => return Err("accessor componentType is unsupported"),
    };
    let component_count = match accessor.get("type").and_then(Value::as_str) {
        Some("SCALAR") => 1_u64,
        Some("VEC2") => 2,
        Some("VEC3") => 3,
        Some("VEC4") => 4,
        _ => return Err("accessor type is unsupported"),
    };
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0)
        .ok_or("accessor count must be positive")?;
    let element_size = component_size
        .checked_mul(component_count)
        .ok_or("accessor element size overflow")?;
    let stride = view
        .get("byteStride")
        .and_then(Value::as_u64)
        .unwrap_or(element_size);
    if stride < element_size {
        return Err("bufferView stride is too small");
    }
    let accessor_offset = accessor
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let view_offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0);
    let view_length = view
        .get("byteLength")
        .and_then(Value::as_u64)
        .ok_or("bufferView byteLength is required")?;
    let required = accessor_offset
        .checked_add(
            count
                .saturating_sub(1)
                .checked_mul(stride)
                .ok_or("accessor range overflow")?,
        )
        .and_then(|value| value.checked_add(element_size))
        .ok_or("accessor range overflow")?;
    let view_end = view_offset
        .checked_add(view_length)
        .ok_or("bufferView range overflow")?;
    if required > view_length
        || view_end > declared_buffer_length
        || view_offset
            .checked_add(required)
            .is_none_or(|value| value > binary.len() as u64)
    {
        return Err("accessor exceeds its bufferView or embedded buffer");
    }
    Ok(accessor)
}

fn uses_unsupported_compression(document: &Value) -> bool {
    const UNSUPPORTED: [&str; 2] = ["KHR_draco_mesh_compression", "EXT_meshopt_compression"];
    let declared = ["extensionsUsed", "extensionsRequired"]
        .into_iter()
        .any(|field| {
            document
                .get(field)
                .and_then(Value::as_array)
                .is_some_and(|extensions| {
                    extensions
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|extension| UNSUPPORTED.contains(&extension))
                })
        });
    let primitive_extension = document
        .get("meshes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|mesh| mesh.get("primitives").and_then(Value::as_array))
        .flatten()
        .filter_map(|primitive| primitive.get("extensions").and_then(Value::as_object))
        .any(|extensions| {
            UNSUPPORTED
                .iter()
                .any(|name| extensions.contains_key(*name))
        });
    let view_extension = document
        .get("bufferViews")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|view| view.get("extensions").and_then(Value::as_object))
        .any(|extensions| {
            UNSUPPORTED
                .iter()
                .any(|name| extensions.contains_key(*name))
        });
    declared || primitive_extension || view_extension
}

fn finite_vec3(value: Option<&Value>) -> Result<[f64; 3], &'static str> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or("POSITION min/max must be VEC3")?;
    let result = std::array::from_fn(|index| values[index].as_f64().unwrap_or(f64::NAN));
    if result.iter().all(|value| value.is_finite()) {
        Ok(result)
    } else {
        Err("POSITION min/max must be finite")
    }
}

fn trim_glb_json_padding(mut bytes: &[u8]) -> &[u8] {
    while bytes.last().is_some_and(|byte| matches!(*byte, b' ' | 0)) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, &'static str> {
    let raw: [u8; 4] = bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .ok_or("container integer is truncated")?;
    Ok(u32::from_le_bytes(raw))
}

pub(crate) fn build_external_asset_version(
    request: &ImportExternalGlbRequest,
    inspection: &ImportedGlbInspection,
    asset_version_id: String,
    version_no: u64,
    created_at: &str,
) -> CoreResult<AgentAssetVersion> {
    inspection.validate()?;
    let part_id = "part_1_imported_model";
    let bounds = inspection.bounds_mm.map(|value| value.max(0.1));
    let version = AgentAssetVersion {
        asset_version_id,
        project_id: request.project_id.clone(),
        parent_asset_version_id: None,
        version_no,
        status: AssetVersionStatus::Committed,
        summary: request.summary.clone(),
        stage: AssetStage::SegmentedConcept,
        plan_id: "external_glb_import".into(),
        direction_id: "external_reference".into(),
        domain_pack_id: request.domain_pack_id.clone(),
        artifact_id: format!("artifact_import_{}", &inspection.sha256[..16]),
        parts: vec![json!({
            "part_id":part_id,
            "role":"primary_body",
            "parent_part_id":Value::Null,
            "position_mm":[0,0,0],
            "size_mm":bounds,
            "material_zone_ids":["zone_imported_model"],
            "editable_parameters":[],
            "editable_parameter_bindings":[],
            "locked":false,
            "provenance":"imported_glb"
        })],
        shape_program: json!({
            "schema_version":"ExternalGLBReference@1",
            "source_sha256":inspection.sha256,
            "editable":false,
            "reason":"Imported GLB is reference-only until rebuilt as a ShapeProgram asset."
        }),
        assembly_graph: json!({
            "schema_version":"AssemblyGraph@1",
            "graph_id":format!("mg_import_{}", &inspection.sha256[..16]),
            "source_kind":"external_glb_reference",
            "parts":[{
                "part_id":part_id,
                "role":"primary_body",
                "transform":{"position":[0,0,0],"rotation":[0,0,0],"scale":[1,1,1]},
                "connectors":[],
                "joints":[]
            }],
            "connections":[]
        }),
        material_bindings: Default::default(),
        created_at: created_at.to_string(),
    };
    version.validate()?;
    Ok(version)
}

pub fn is_external_glb_reference(version: &AgentAssetVersion) -> bool {
    version
        .shape_program
        .get("schema_version")
        .and_then(Value::as_str)
        == Some("ExternalGLBReference@1")
        && version
            .shape_program
            .get("editable")
            .and_then(Value::as_bool)
            == Some(false)
}

pub(crate) fn safe_import_file_name(value: &str) -> String {
    let normalized = value.replace('\\', "/");
    let basename = normalized.rsplit('/').next().unwrap_or_default();
    let cleaned = basename
        .replace('\0', "")
        .trim()
        .chars()
        .take(180)
        .collect::<String>();
    if cleaned.is_empty() {
        "imported-model.glb".into()
    } else {
        cleaned
    }
}

fn default_import_summary() -> String {
    "导入 GLB 参考模型".into()
}

fn require_text(field: &str, value: &str, maximum: usize) -> CoreResult<()> {
    if value.is_empty()
        || value.chars().count() > maximum
        || value.chars().any(|character| character.is_control())
    {
        return Err(CoreError::invalid_data(
            "GLB_IMPORT_REQUEST_INVALID",
            format!("{field} is empty, oversized or contains control characters."),
        ));
    }
    Ok(())
}

fn require_stable_id(field: &str, value: &str, prefix: &str) -> CoreResult<()> {
    if !value.starts_with(prefix)
        || value.len() > 160
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
    {
        return Err(CoreError::invalid_data(
            "GLB_IMPORT_RECORD_INVALID",
            format!("{field} is not a stable ForgeCAD identity."),
        ));
    }
    Ok(())
}

fn require_identifier(field: &str, value: &str) -> CoreResult<()> {
    if value.is_empty()
        || value.len() > 160
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
    {
        return Err(CoreError::invalid_data(
            "GLB_IMPORT_RECORD_INVALID",
            format!("{field} is not a stable ForgeCAD identity."),
        ));
    }
    Ok(())
}

fn valid_domain_pack_id(value: &str) -> bool {
    value.starts_with("pack_")
        && value.len() <= 160
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn validate_sha256(value: &str) -> CoreResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(CoreError::invalid_data(
            "GLB_IMPORT_HASH_INVALID",
            "Imported GLB must use a lowercase SHA-256 identity.",
        ));
    }
    Ok(())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_external_glb_inspection_reads_only_verified_facts() {
        let glb = test_glb();
        let inspection = inspect_external_glb(&glb).unwrap();
        assert_eq!(inspection.byte_size, glb.len() as u64);
        assert_eq!(inspection.triangle_count, 1);
        assert_eq!(inspection.bounds_mm, [1000.0, 1000.0, 0.0]);
        assert_eq!(inspection.mesh_count, 1);
        assert_eq!(inspection.primitive_count, 1);
        assert_eq!(inspection.material_count, 1);
        assert_eq!(inspection.node_count, 1);
    }

    #[test]
    fn strict_external_glb_rejects_external_resources_compression_and_bad_accessors() {
        let external = mutate_document(test_glb(), |document| {
            document["buffers"][0]["uri"] = json!("outside.bin");
        });
        assert_eq!(
            inspect_external_glb(&external).unwrap_err().code(),
            "GLB_IMPORT_REJECTED"
        );

        let compressed = mutate_document(test_glb(), |document| {
            document["extensionsRequired"] = json!(["KHR_draco_mesh_compression"]);
        });
        assert_eq!(
            inspect_external_glb(&compressed).unwrap_err().code(),
            "GLB_IMPORT_REJECTED"
        );

        let escaped_accessor = mutate_document(test_glb(), |document| {
            document["bufferViews"][0]["byteLength"] = json!(4);
        });
        assert_eq!(
            inspect_external_glb(&escaped_accessor).unwrap_err().code(),
            "GLB_IMPORT_REJECTED"
        );
    }

    #[test]
    fn request_hash_matches_legacy_semantics_and_filename_is_display_only() {
        let glb_base64 = BASE64_STANDARD.encode(test_glb());
        let request = ImportExternalGlbRequest {
            client_request_id: "request_a".into(),
            project_id: "prj_external".into(),
            domain_pack_id: "pack_vehicle_concept".into(),
            file_name: "../unsafe\\reference.glb".into(),
            glb_base64,
            summary: default_import_summary(),
        };
        let decoded = request.validate_and_decode().unwrap();
        assert_eq!(decoded.inspection.triangle_count, 1);
        assert_eq!(safe_import_file_name(&request.file_name), "reference.glb");
        let mut changed_transport = request.clone();
        changed_transport.client_request_id = "request_b".into();
        assert_eq!(
            request.request_hash().unwrap(),
            changed_transport.request_hash().unwrap()
        );
        changed_transport.summary = "changed".into();
        assert_ne!(
            request.request_hash().unwrap(),
            changed_transport.request_hash().unwrap()
        );
    }

    pub(crate) fn test_glb() -> Vec<u8> {
        let positions = [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let indices = [0_u16, 1, 2];
        let mut binary = Vec::new();
        for value in positions {
            binary.extend_from_slice(&value.to_le_bytes());
        }
        let index_offset = binary.len();
        for value in indices {
            binary.extend_from_slice(&value.to_le_bytes());
        }
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        let document = json!({
            "asset":{"version":"2.0"},
            "scene":0,
            "scenes":[{"nodes":[0]}],
            "nodes":[{"mesh":0}],
            "meshes":[{"primitives":[{
                "attributes":{"POSITION":0},"indices":1,"material":0,"mode":4
            }]}],
            "materials":[{}],
            "buffers":[{"byteLength":binary.len()}],
            "bufferViews":[
                {"buffer":0,"byteOffset":0,"byteLength":index_offset},
                {"buffer":0,"byteOffset":index_offset,"byteLength":6,"target":34963}
            ],
            "accessors":[
                {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]},
                {"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}
            ]
        });
        encode_glb(document, binary)
    }

    fn mutate_document(glb: Vec<u8>, mutate: impl FnOnce(&mut Value)) -> Vec<u8> {
        let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let mut document: Value =
            serde_json::from_slice(trim_glb_json_padding(&glb[20..20 + json_length])).unwrap();
        mutate(&mut document);
        let binary_offset = 20 + json_length;
        let binary_length =
            u32::from_le_bytes(glb[binary_offset..binary_offset + 4].try_into().unwrap()) as usize;
        let binary = glb[binary_offset + 8..binary_offset + 8 + binary_length].to_vec();
        encode_glb(document, binary)
    }

    fn encode_glb(document: Value, mut binary: Vec<u8>) -> Vec<u8> {
        let mut json_chunk = serde_json::to_vec(&document).unwrap();
        while json_chunk.len() % 4 != 0 {
            json_chunk.push(b' ');
        }
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        let total_length = 12 + 8 + json_chunk.len() + 8 + binary.len();
        let mut glb = Vec::with_capacity(total_length);
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&(total_length as u32).to_le_bytes());
        glb.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&json_chunk);
        glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"BIN\0");
        glb.extend_from_slice(&binary);
        glb
    }
}
