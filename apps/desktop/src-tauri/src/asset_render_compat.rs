//! Rust-owned compatibility payloads for read-only concept renders.
//!
//! Geometry remains confined to the restricted executor.  This module owns
//! only the bounded API representation, PNG readback and deterministic ZIP
//! packaging; it never writes product state or invents model evidence.

use std::{collections::BTreeMap, fs, io::Cursor, path::Path, process::Command};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use forgecad_app_server::compatibility::{AllowedHttpMethod, PreparedCompatHttpRequest};
use forgecad_app_server_protocol::{
    valid_stable_id, CompatHttpResponse, ProtocolHttpBody, HTTP_COMPAT_RESPONSE_SCHEMA_VERSION,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use forgecad_core::{ForgeAssetPackage, ForgeAssetPackageFile};

const RENDERER_ID: &str = "forgecad-agent-software-raster@1";
const REQUIRED_VIEWS: [&str; 4] = ["iso", "front", "side", "top"];
const MAX_VIEW_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AssetRenderCompatRequest {
    Views {
        asset_version_id: String,
        width: u16,
        height: u16,
    },
    Package {
        asset_version_id: String,
        width: u16,
        height: u16,
        render_set_sha256: String,
    },
    AssetPackage {
        asset_version_id: String,
        width: u16,
        height: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssetRenderCompatError {
    pub status: u16,
    pub code: &'static str,
    pub message: &'static str,
    pub recoverable: bool,
}

impl AssetRenderCompatError {
    fn invalid(message: &'static str) -> Self {
        Self {
            status: 422,
            code: "RENDER_REQUEST_INVALID",
            message,
            recoverable: false,
        }
    }

    fn readback(message: &'static str) -> Self {
        Self {
            status: 409,
            code: "AGENT_RENDER_READBACK_FAILED",
            message,
            recoverable: false,
        }
    }

    fn unavailable(message: &'static str) -> Self {
        Self {
            status: 503,
            code: "ASSET_PACKAGE_ENCODER_UNAVAILABLE",
            message,
            recoverable: true,
        }
    }

    pub(crate) fn stale() -> Self {
        Self {
            status: 409,
            code: "RENDER_SET_STALE",
            message: "Concept views changed; generate them again before downloading.",
            recoverable: true,
        }
    }

    pub(crate) fn response(&self) -> CompatHttpResponse {
        json_response(
            self.status,
            json!({
                "error": {
                    "code": self.code,
                    "message": self.message,
                    "recoverable": self.recoverable,
                    "details": {}
                }
            }),
        )
    }
}

#[derive(Debug, Clone)]
struct RenderView {
    view_id: &'static str,
    png: Vec<u8>,
    sha256: String,
}

#[derive(Debug, Clone)]
pub(crate) struct SealedRenderSet {
    pub asset_version_id: String,
    pub width: u16,
    pub height: u16,
    pub render_set_sha256: String,
    pub render_set_byte_size: usize,
    pub response: Value,
    views: Vec<RenderView>,
}

pub(crate) fn parse_asset_render_request(
    request: &PreparedCompatHttpRequest,
) -> Option<Result<AssetRenderCompatRequest, AssetRenderCompatError>> {
    if request.method != AllowedHttpMethod::Get {
        return None;
    }
    let (route, query) = request.path.split_once('?').unwrap_or((&request.path, ""));
    let asset_route = route.strip_prefix("/api/v1/agent/asset-versions/")?;
    let (asset_version_id, route_kind) =
        if let Some(value) = asset_route.strip_suffix(":render-package") {
            (value, "render_package")
        } else if let Some(value) = asset_route.strip_suffix(":asset-package") {
            (value, "asset_package")
        } else if let Some(value) = asset_route.strip_suffix(":render") {
            (value, "render")
        } else {
            return None;
        };
    if !valid_stable_id(asset_version_id) || !asset_version_id.starts_with("assetver_") {
        return Some(Err(AssetRenderCompatError::invalid(
            "Asset version identity is outside the bounded contract.",
        )));
    }
    let parameters = match parse_query(query) {
        Ok(parameters) => parameters,
        Err(error) => return Some(Err(error)),
    };
    let width = match dimension(&parameters, "width", 640) {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    let height = match dimension(&parameters, "height", 640) {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    if route_kind == "render_package" {
        let Some(fingerprint) = parameters.get("render_set_sha256") else {
            return Some(Err(AssetRenderCompatError::invalid(
                "render_set_sha256 is required for a concept-view package.",
            )));
        };
        if !is_sha256(fingerprint) {
            return Some(Err(AssetRenderCompatError::invalid(
                "render_set_sha256 must be a lowercase SHA-256 value.",
            )));
        }
        Some(Ok(AssetRenderCompatRequest::Package {
            asset_version_id: asset_version_id.to_string(),
            width,
            height,
            render_set_sha256: fingerprint.clone(),
        }))
    } else if route_kind == "asset_package" {
        Some(Ok(AssetRenderCompatRequest::AssetPackage {
            asset_version_id: asset_version_id.to_string(),
            width,
            height,
        }))
    } else {
        Some(Ok(AssetRenderCompatRequest::Views {
            asset_version_id: asset_version_id.to_string(),
            width,
            height,
        }))
    }
}

pub(crate) fn seal_render_set(
    asset_version_id: &str,
    width: u16,
    height: u16,
    renderer_id: &str,
    rendered_views: &BTreeMap<String, Vec<u8>>,
    rendered_at: String,
    max_response_bytes: usize,
) -> Result<SealedRenderSet, AssetRenderCompatError> {
    if renderer_id != RENDERER_ID || rendered_views.len() != REQUIRED_VIEWS.len() {
        return Err(AssetRenderCompatError::readback(
            "Restricted rendering did not return the exact four-view contract.",
        ));
    }
    let mut views = Vec::with_capacity(REQUIRED_VIEWS.len());
    let mut response_views = Vec::with_capacity(REQUIRED_VIEWS.len());
    let mut fingerprint_views = Vec::with_capacity(REQUIRED_VIEWS.len());
    let mut render_set_byte_size = 0usize;
    for view_id in REQUIRED_VIEWS {
        let png = rendered_views.get(view_id).ok_or_else(|| {
            AssetRenderCompatError::readback("A required concept view is missing.")
        })?;
        validate_png(png, width, height)?;
        render_set_byte_size = render_set_byte_size.checked_add(png.len()).ok_or_else(|| {
            AssetRenderCompatError::readback("Concept-view byte size overflowed its bound.")
        })?;
        let sha256 = sha256_hex(png);
        response_views.push(json!({
            "schema_version": "AgentAssetRenderView@1",
            "asset_version_id": asset_version_id,
            "view_id": view_id,
            "camera_view": view_id,
            "presentation_mode": "standard",
            "background_mode": "transparent",
            "part_ids": [],
            "mime_type": "image/png",
            "width": width,
            "height": height,
            "png_base64": BASE64.encode(png),
            "sha256": sha256,
            "byte_size": png.len(),
            "readback_status": "passed"
        }));
        fingerprint_views.push(json!({
            "view_id": view_id,
            "sha256": sha256,
            "presentation_mode": "standard",
            "background_mode": "transparent",
            "part_ids": []
        }));
        views.push(RenderView {
            view_id,
            png: png.clone(),
            sha256,
        });
    }
    let exploded_unavailable_reason =
        "No stable one-to-one exploded-part render was produced by the restricted renderer.";
    let fingerprint = json!({
        "schema_version": "AgentAssetRenderSet@1",
        "asset_version_id": asset_version_id,
        "renderer_id": RENDERER_ID,
        "width": width,
        "height": height,
        "views": fingerprint_views,
        "exploded_view_available": false,
        "exploded_unavailable_reason": exploded_unavailable_reason
    });
    let render_set_sha256 = sha256_hex(canonical_json(&fingerprint).as_bytes());
    let response = json!({
        "schema_version": "AgentAssetRenderSet@1",
        "asset_version_id": asset_version_id,
        "renderer_id": RENDERER_ID,
        "width": width,
        "height": height,
        "views": response_views,
        "exploded_view_available": false,
        "exploded_unavailable_reason": exploded_unavailable_reason,
        "render_set_sha256": render_set_sha256,
        "render_set_byte_size": render_set_byte_size,
        "rendered_at": rendered_at
    });
    if canonical_json(&response).len() > max_response_bytes {
        return Err(AssetRenderCompatError {
            status: 413,
            code: "RENDER_RESPONSE_TOO_LARGE",
            message: "Concept-view response exceeds the bounded compatibility transport.",
            recoverable: true,
        });
    }
    Ok(SealedRenderSet {
        asset_version_id: asset_version_id.to_string(),
        width,
        height,
        render_set_sha256,
        render_set_byte_size,
        response,
        views,
    })
}

pub(crate) fn render_set_response(render_set: &SealedRenderSet) -> CompatHttpResponse {
    json_response(200, render_set.response.clone())
}

pub(crate) fn render_package_response(
    render_set: &SealedRenderSet,
    expected_render_set_sha256: &str,
    max_response_bytes: usize,
) -> Result<CompatHttpResponse, AssetRenderCompatError> {
    if expected_render_set_sha256 != render_set.render_set_sha256 {
        return Err(AssetRenderCompatError::stale());
    }
    let exploded_unavailable_reason = render_set
        .response
        .get("exploded_unavailable_reason")
        .cloned()
        .unwrap_or(Value::Null);
    let manifest_views = render_set
        .views
        .iter()
        .map(|view| {
            json!({
                "file_name": format!("{}.png", view.view_id),
                "asset_version_id": render_set.asset_version_id,
                "view_id": view.view_id,
                "camera_view": view.view_id,
                "presentation_mode": "standard",
                "background_mode": "transparent",
                "part_ids": [],
                "mime_type": "image/png",
                "width": render_set.width,
                "height": render_set.height,
                "sha256": view.sha256,
                "byte_size": view.png.len(),
                "readback_status": "passed"
            })
        })
        .collect::<Vec<_>>();
    let manifest = json!({
        "schema_version": "AgentAssetRenderPackage@1",
        "package_kind": "concept_view_png_bundle",
        "asset_version_id": render_set.asset_version_id,
        "renderer_id": RENDERER_ID,
        "render_set_sha256": render_set.render_set_sha256,
        "render_set_byte_size": render_set.render_set_byte_size,
        "width": render_set.width,
        "height": render_set.height,
        "views": manifest_views,
        "exploded_view_available": false,
        "exploded_unavailable_reason": exploded_unavailable_reason,
        "non_engineering_notice": "concept_views_only_not_engineering_or_manufacturing_data"
    });
    let manifest_bytes = canonical_json(&manifest).into_bytes();
    let mut entries = Vec::with_capacity(render_set.views.len() + 1);
    entries.push(("manifest.json".to_string(), manifest_bytes));
    entries.extend(
        render_set
            .views
            .iter()
            .map(|view| (format!("{}.png", view.view_id), view.png.clone())),
    );
    let archive = deterministic_stored_zip(&entries)?;
    if archive.len() > max_response_bytes {
        return Err(AssetRenderCompatError {
            status: 413,
            code: "RENDER_PACKAGE_TOO_LARGE",
            message: "Concept-view package exceeds the bounded compatibility transport.",
            recoverable: true,
        });
    }
    Ok(CompatHttpResponse {
        schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.into(),
        status: 200,
        headers: vec![
            ("Content-Type".into(), "application/zip".into()),
            ("Cache-Control".into(), "no-store".into()),
            (
                "Content-Disposition".into(),
                format!(
                    "attachment; filename=\"{}-concept-views.zip\"",
                    render_set.asset_version_id
                ),
            ),
            (
                "X-ForgeCAD-Render-Set-SHA256".into(),
                render_set.render_set_sha256.clone(),
            ),
        ],
        body: ProtocolHttpBody::Base64 {
            data: BASE64.encode(archive),
        },
    })
}

pub(crate) fn forge_asset_package_response(
    project_id: &str,
    asset_version_id: &str,
    source_glb: &[u8],
    quality_report: &Value,
    turntable_views: &BTreeMap<String, Vec<u8>>,
    max_response_bytes: usize,
) -> Result<CompatHttpResponse, AssetRenderCompatError> {
    const TURNTABLE_VIEWS: [&str; 8] = [
        "turntable_000",
        "turntable_045",
        "turntable_090",
        "turntable_135",
        "turntable_180",
        "turntable_225",
        "turntable_270",
        "turntable_315",
    ];
    if source_glb.is_empty()
        || turntable_views.len() != TURNTABLE_VIEWS.len()
        || TURNTABLE_VIEWS
            .iter()
            .any(|view_id| !turntable_views.contains_key(*view_id))
    {
        return Err(AssetRenderCompatError::readback(
            "Forge asset packaging requires one GLB and the exact eight turntable views.",
        ));
    }
    let frames = TURNTABLE_VIEWS
        .iter()
        .map(|view_id| turntable_views[*view_id].clone())
        .collect::<Vec<_>>();
    let thumbnail = webp_thumbnail(&frames[0])?;
    let turntable = encode_turntable_mp4(&frames)?;
    let source_artifact_sha256 = sha256_hex(source_glb);
    let quality_bytes = canonical_json(quality_report).into_bytes();
    let license_bytes = canonical_json(&json!({
        "schema_version":"ForgeAssetLicenseMetadata@1",
        "asset_version_id":asset_version_id,
        "license":"user_generated_visual_asset",
        "generator":"forgecad_programmatic_visual_pipeline",
        "built_in_material_sources_only":true,
        "reference_rights":"no_external_reference_embedded",
        "non_engineering_notice":"visual_concept_only_not_engineering_or_manufacturing_data"
    }))
    .into_bytes();
    let package_id = format!("package_{}", &source_artifact_sha256[..24]);
    let mut payloads = vec![
        (
            "asset.glb".to_string(),
            source_glb.to_vec(),
            "model/gltf-binary",
        ),
        ("thumbnail.webp".to_string(), thumbnail, "image/webp"),
        ("turntable.mp4".to_string(), turntable, "video/mp4"),
        (
            "quality-report.json".to_string(),
            quality_bytes,
            "application/json",
        ),
        (
            "license-metadata.json".to_string(),
            license_bytes,
            "application/json",
        ),
    ];
    let manifest_members = payloads
        .iter()
        .map(|(relative_path, bytes, media_type)| {
            json!({
                "relative_path":relative_path,
                "media_type":media_type,
                "sha256":sha256_hex(bytes),
                "byte_size":bytes.len()
            })
        })
        .collect::<Vec<_>>();
    let manifest_bytes = canonical_json(&json!({
        "schema_version":"ForgeAssetPackageManifest@1",
        "package_id":package_id,
        "project_id":project_id,
        "asset_version_id":asset_version_id,
        "source_artifact_sha256":source_artifact_sha256,
        "members":manifest_members,
        "manifest_identity":"bound_by_outer_ForgeAssetPackage@1_descriptor",
    }))
    .into_bytes();
    payloads.push((
        "manifest.json".to_string(),
        manifest_bytes,
        "application/json",
    ));
    payloads.sort_by(|left, right| left.0.cmp(&right.0));
    let descriptor = ForgeAssetPackage {
        schema_version: "ForgeAssetPackage@1".into(),
        package_id: package_id.clone(),
        project_id: project_id.to_string(),
        asset_version_id: asset_version_id.to_string(),
        source_artifact_sha256: source_artifact_sha256.clone(),
        files: payloads
            .iter()
            .map(|(relative_path, bytes, media_type)| ForgeAssetPackageFile {
                relative_path: relative_path.clone(),
                media_type: (*media_type).to_string(),
                sha256: sha256_hex(bytes),
                byte_size: bytes.len() as u64,
            })
            .collect(),
    };
    descriptor.validate().map_err(|_| {
        AssetRenderCompatError::readback("ForgeAssetPackage@1 validation failed closed.")
    })?;
    let entries = payloads
        .iter()
        .map(|(relative_path, bytes, _)| (relative_path.clone(), bytes.clone()))
        .collect::<Vec<_>>();
    let archive = deterministic_stored_zip(&entries)?;
    if archive.len() > max_response_bytes {
        return Err(AssetRenderCompatError {
            status: 413,
            code: "ASSET_PACKAGE_TOO_LARGE",
            message: "Forge asset package exceeds the bounded compatibility transport.",
            recoverable: true,
        });
    }
    let manifest_sha256 = descriptor
        .files
        .iter()
        .find(|file| file.relative_path == "manifest.json")
        .expect("validated package contains manifest")
        .sha256
        .clone();
    Ok(CompatHttpResponse {
        schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.into(),
        status: 200,
        headers: vec![
            ("Content-Type".into(), "application/zip".into()),
            ("Cache-Control".into(), "no-store".into()),
            (
                "Content-Disposition".into(),
                format!("attachment; filename=\"{asset_version_id}-forge-asset.zip\""),
            ),
            ("X-ForgeCAD-Package-ID".into(), package_id),
            (
                "X-ForgeCAD-Source-GLB-SHA256".into(),
                source_artifact_sha256,
            ),
            ("X-ForgeCAD-Manifest-SHA256".into(), manifest_sha256),
        ],
        body: ProtocolHttpBody::Base64 {
            data: BASE64.encode(archive),
        },
    })
}

fn webp_thumbnail(png: &[u8]) -> Result<Vec<u8>, AssetRenderCompatError> {
    let image =
        image::load_from_memory_with_format(png, image::ImageFormat::Png).map_err(|_| {
            AssetRenderCompatError::readback("Turntable thumbnail source is not a decodable PNG.")
        })?;
    let thumbnail = image.thumbnail(640, 640);
    let mut bytes = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut bytes, image::ImageFormat::WebP)
        .map_err(|_| {
            AssetRenderCompatError::readback("Turntable thumbnail could not be encoded as WebP.")
        })?;
    Ok(bytes.into_inner())
}

fn encode_turntable_mp4(frames: &[Vec<u8>]) -> Result<Vec<u8>, AssetRenderCompatError> {
    let ffmpeg = [
        "/opt/homebrew/bin/ffmpeg",
        "/usr/local/bin/ffmpeg",
        "/usr/bin/ffmpeg",
    ]
    .into_iter()
    .find(|path| Path::new(path).is_file())
    .ok_or_else(|| {
        AssetRenderCompatError::unavailable(
            "A code-owned FFmpeg encoder is required to create turntable.mp4.",
        )
    })?;
    let directory = tempfile::Builder::new()
        .prefix("forgecad-asset-package-")
        .tempdir()
        .map_err(|_| {
            AssetRenderCompatError::unavailable(
                "A private temporary package directory is required.",
            )
        })?;
    for (index, frame) in frames.iter().enumerate() {
        image::load_from_memory_with_format(frame, image::ImageFormat::Png).map_err(|_| {
            AssetRenderCompatError::readback("A turntable frame is not a decodable PNG.")
        })?;
        fs::write(
            directory.path().join(format!("frame-{index:03}.png")),
            frame,
        )
        .map_err(|_| {
            AssetRenderCompatError::unavailable("A turntable frame could not be staged.")
        })?;
    }
    let output = directory.path().join("turntable.mp4");
    let status = Command::new(ffmpeg)
        .current_dir(directory.path())
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-y",
            "-framerate",
            "8",
            "-start_number",
            "0",
            "-i",
            "frame-%03d.png",
            "-an",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            "-map_metadata",
            "-1",
            "-metadata",
            "creation_time=1970-01-01T00:00:00Z",
        ])
        .arg(&output)
        .status()
        .map_err(|_| {
            AssetRenderCompatError::unavailable("FFmpeg turntable encoding could not start.")
        })?;
    if !status.success() {
        return Err(AssetRenderCompatError::unavailable(
            "FFmpeg turntable encoding failed.",
        ));
    }
    let bytes = fs::read(output).map_err(|_| {
        AssetRenderCompatError::unavailable("Encoded turntable.mp4 could not be read.")
    })?;
    if bytes.len() < 32
        || bytes.get(4..8) != Some(b"ftyp")
        || !bytes.windows(4).any(|window| window == b"moov")
        || !bytes.windows(4).any(|window| window == b"mdat")
    {
        return Err(AssetRenderCompatError::readback(
            "Encoded turntable bytes failed the bounded MP4 container check.",
        ));
    }
    Ok(bytes)
}

fn parse_query(query: &str) -> Result<BTreeMap<String, String>, AssetRenderCompatError> {
    let mut result = BTreeMap::new();
    if query.is_empty() {
        return Ok(result);
    }
    for item in query.split('&') {
        let (key, value) = item.split_once('=').unwrap_or((item, ""));
        if !matches!(key, "width" | "height" | "render_set_sha256") {
            continue;
        }
        if key.is_empty()
            || value.is_empty()
            || !value.bytes().all(|byte| byte.is_ascii_alphanumeric())
            || result.insert(key.to_string(), value.to_string()).is_some()
        {
            return Err(AssetRenderCompatError::invalid(
                "Concept-view query parameters are invalid or duplicated.",
            ));
        }
    }
    Ok(result)
}

fn dimension(
    parameters: &BTreeMap<String, String>,
    key: &str,
    default: u16,
) -> Result<u16, AssetRenderCompatError> {
    let value = parameters
        .get(key)
        .map(|value| value.parse::<u16>())
        .transpose()
        .map_err(|_| AssetRenderCompatError::invalid("Render dimensions are invalid."))?
        .unwrap_or(default);
    if !(64..=2048).contains(&value) {
        return Err(AssetRenderCompatError::invalid(
            "Render dimensions must be between 64 and 2048 pixels.",
        ));
    }
    Ok(value)
}

fn validate_png(payload: &[u8], width: u16, height: u16) -> Result<(), AssetRenderCompatError> {
    if payload.len() < 33
        || payload.len() > MAX_VIEW_BYTES
        || payload.get(..8) != Some(b"\x89PNG\r\n\x1a\n")
        || payload.get(12..16) != Some(b"IHDR")
    {
        return Err(AssetRenderCompatError::readback(
            "Concept-view bytes are not a bounded PNG.",
        ));
    }
    let actual_width = u32::from_be_bytes(payload[16..20].try_into().expect("fixed PNG slice"));
    let actual_height = u32::from_be_bytes(payload[20..24].try_into().expect("fixed PNG slice"));
    if actual_width != u32::from(width)
        || actual_height != u32::from(height)
        || payload.get(24..29) != Some(&[8, 6, 0, 0, 0])
    {
        return Err(AssetRenderCompatError::readback(
            "Concept-view PNG metadata does not match the requested RGBA render.",
        ));
    }
    Ok(())
}

fn deterministic_stored_zip(
    entries: &[(String, Vec<u8>)],
) -> Result<Vec<u8>, AssetRenderCompatError> {
    if entries.is_empty() || entries.len() > u16::MAX as usize {
        return Err(AssetRenderCompatError::readback(
            "Concept-view package entry count is invalid.",
        ));
    }
    struct CentralEntry {
        name: Vec<u8>,
        crc32: u32,
        size: u32,
        offset: u32,
    }
    let mut archive = Vec::new();
    let mut central = Vec::with_capacity(entries.len());
    for (name, payload) in entries {
        let name = name.as_bytes();
        if name.is_empty()
            || name.len() > u16::MAX as usize
            || !name
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            || payload.len() > u32::MAX as usize
            || archive.len() > u32::MAX as usize
        {
            return Err(AssetRenderCompatError::readback(
                "Concept-view package entry is outside ZIP32 bounds.",
            ));
        }
        let crc32 = crc32(payload);
        let size = payload.len() as u32;
        let offset = archive.len() as u32;
        push_u32(&mut archive, 0x0403_4b50);
        push_u16(&mut archive, 20);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0); // Stored: PNG is already compressed.
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0x0021); // 1980-01-01.
        push_u32(&mut archive, crc32);
        push_u32(&mut archive, size);
        push_u32(&mut archive, size);
        push_u16(&mut archive, name.len() as u16);
        push_u16(&mut archive, 0);
        archive.extend_from_slice(name);
        archive.extend_from_slice(payload);
        central.push(CentralEntry {
            name: name.to_vec(),
            crc32,
            size,
            offset,
        });
    }
    if archive.len() > u32::MAX as usize {
        return Err(AssetRenderCompatError::readback(
            "Concept-view package exceeds ZIP32 bounds.",
        ));
    }
    let central_offset = archive.len() as u32;
    for entry in &central {
        push_u32(&mut archive, 0x0201_4b50);
        push_u16(&mut archive, 0x0314); // UNIX, version 2.0.
        push_u16(&mut archive, 20);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0x0021);
        push_u32(&mut archive, entry.crc32);
        push_u32(&mut archive, entry.size);
        push_u32(&mut archive, entry.size);
        push_u16(&mut archive, entry.name.len() as u16);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u32(&mut archive, 0o100600 << 16);
        push_u32(&mut archive, entry.offset);
        archive.extend_from_slice(&entry.name);
    }
    let central_size = archive
        .len()
        .checked_sub(central_offset as usize)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| {
            AssetRenderCompatError::readback("Concept-view central directory is invalid.")
        })?;
    push_u32(&mut archive, 0x0605_4b50);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, central.len() as u16);
    push_u16(&mut archive, central.len() as u16);
    push_u32(&mut archive, central_size);
    push_u32(&mut archive, central_offset);
    push_u16(&mut archive, 0);
    Ok(archive)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn push_u16(target: &mut Vec<u8>, value: u16) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn canonical_json(value: &Value) -> String {
    fn write(value: &Value, output: &mut String) {
        match value {
            Value::Null => output.push_str("null"),
            Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
            Value::Number(value) => output.push_str(&value.to_string()),
            Value::String(value) => output.push_str(
                &serde_json::to_string(value).expect("serializing a JSON string cannot fail"),
            ),
            Value::Array(values) => {
                output.push('[');
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        output.push(',');
                    }
                    write(value, output);
                }
                output.push(']');
            }
            Value::Object(values) => {
                output.push('{');
                let mut keys = values.keys().collect::<Vec<_>>();
                keys.sort_unstable();
                for (index, key) in keys.into_iter().enumerate() {
                    if index > 0 {
                        output.push(',');
                    }
                    output.push_str(
                        &serde_json::to_string(key)
                            .expect("serializing a JSON object key cannot fail"),
                    );
                    output.push(':');
                    write(&values[key], output);
                }
                output.push('}');
            }
        }
    }
    let mut output = String::new();
    write(value, &mut output);
    output
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn json_response(status: u16, value: Value) -> CompatHttpResponse {
    CompatHttpResponse {
        schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.into(),
        status,
        headers: vec![
            ("Content-Type".into(), "application/json".into()),
            ("Cache-Control".into(), "no-store".into()),
        ],
        body: ProtocolHttpBody::Utf8 {
            data: value.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_app_server::compatibility::LocalAgentEndpoint;
    use std::io::{Cursor, Read};

    fn request(path: &str) -> PreparedCompatHttpRequest {
        PreparedCompatHttpRequest {
            endpoint: LocalAgentEndpoint::parse("http://127.0.0.1:8000").unwrap(),
            method: AllowedHttpMethod::Get,
            path: path.into(),
            headers: Vec::new(),
            body: ProtocolHttpBody::Empty,
        }
    }

    fn rgba_png(width: u16, height: u16, suffix: u8) -> Vec<u8> {
        let mut value = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR".to_vec();
        value.extend_from_slice(&u32::from(width).to_be_bytes());
        value.extend_from_slice(&u32::from(height).to_be_bytes());
        value.extend_from_slice(&[8, 6, 0, 0, 0]);
        value.extend_from_slice(&[0, 0, 0, 0, suffix]);
        value
    }

    fn views(width: u16, height: u16) -> BTreeMap<String, Vec<u8>> {
        REQUIRED_VIEWS
            .iter()
            .enumerate()
            .map(|(index, view)| {
                (
                    (*view).to_string(),
                    rgba_png(width, height, u8::try_from(index).unwrap()),
                )
            })
            .collect()
    }

    #[test]
    fn route_parser_is_bounded_and_requires_package_fingerprint() {
        assert_eq!(
            parse_asset_render_request(&request(
                "/api/v1/agent/asset-versions/assetver_a:render?width=128&height=256"
            )),
            Some(Ok(AssetRenderCompatRequest::Views {
                asset_version_id: "assetver_a".into(),
                width: 128,
                height: 256,
            }))
        );
        let missing = parse_asset_render_request(&request(
            "/api/v1/agent/asset-versions/assetver_a:render-package?width=128",
        ))
        .unwrap()
        .unwrap_err();
        assert_eq!(missing.status, 422);
        let duplicate = parse_asset_render_request(&request(
            "/api/v1/agent/asset-versions/assetver_a:render?width=128&width=256",
        ))
        .unwrap()
        .unwrap_err();
        assert_eq!(duplicate.code, "RENDER_REQUEST_INVALID");
    }

    #[test]
    fn render_set_is_deterministic_and_rejects_png_metadata_drift() {
        let first = seal_render_set(
            "assetver_a",
            128,
            128,
            RENDERER_ID,
            &views(128, 128),
            "unix_ms_1".into(),
            1024 * 1024,
        )
        .unwrap();
        let second = seal_render_set(
            "assetver_a",
            128,
            128,
            RENDERER_ID,
            &views(128, 128),
            "unix_ms_2".into(),
            1024 * 1024,
        )
        .unwrap();
        assert_eq!(first.render_set_sha256, second.render_set_sha256);
        assert_ne!(
            first.response["rendered_at"],
            second.response["rendered_at"]
        );
        let error = seal_render_set(
            "assetver_a",
            128,
            128,
            RENDERER_ID,
            &views(64, 128),
            "unix_ms_1".into(),
            1024 * 1024,
        )
        .unwrap_err();
        assert_eq!(error.code, "AGENT_RENDER_READBACK_FAILED");
    }

    #[test]
    fn package_is_deterministic_zip32_and_stale_fingerprint_fails_closed() {
        let render_set = seal_render_set(
            "assetver_a",
            128,
            128,
            RENDERER_ID,
            &views(128, 128),
            "unix_ms_1".into(),
            1024 * 1024,
        )
        .unwrap();
        let first =
            render_package_response(&render_set, &render_set.render_set_sha256, 1024 * 1024)
                .unwrap();
        let second =
            render_package_response(&render_set, &render_set.render_set_sha256, 1024 * 1024)
                .unwrap();
        assert_eq!(first.body, second.body);
        let ProtocolHttpBody::Base64 { data } = first.body else {
            panic!("package must be binary");
        };
        let bytes = BASE64.decode(data).unwrap();
        assert!(bytes.starts_with(b"PK\x03\x04"));
        assert!(bytes.ends_with(&[0, 0]));
        assert!(bytes
            .windows("manifest.json".len())
            .any(|window| window == b"manifest.json"));
        assert_eq!(
            render_package_response(&render_set, &"0".repeat(64), 1024 * 1024)
                .unwrap_err()
                .code,
            "RENDER_SET_STALE"
        );

        // Validate the first stored local entry without relying on a ZIP crate.
        let name_length = u16::from_le_bytes(bytes[26..28].try_into().unwrap()) as usize;
        let payload_size = u32::from_le_bytes(bytes[18..22].try_into().unwrap()) as usize;
        let payload_start = 30 + name_length;
        let mut cursor = Cursor::new(&bytes[payload_start..payload_start + payload_size]);
        let mut manifest = String::new();
        cursor.read_to_string(&mut manifest).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&manifest).unwrap()["schema_version"],
            "AgentAssetRenderPackage@1"
        );
    }
}
