//! Rust-owned sealed image to constrained camera/UV raster DTO.
//!
//! `ReferenceCameraUvRasterBake@2` is consumed by the restricted Python
//! worker, but it is never an Agent or WebView authoring surface.  This module
//! only admits an image previously sealed as `ReferenceEvidence`, verifies its
//! exact CAS digest, and binds it to an immutable candidate camera.  The DTO
//! intentionally contains no paths, URLs, executable code, or provider data.

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use image::{ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Cursor;

use crate::{
    semantic_sha256, CoreError, CoreResult, GeometryInvariantBinding,
    GeometryProjectionCameraBinding, ProjectionCameraBinding, ReferenceEvidence,
    ReferenceEvidenceKind,
};

pub const REFERENCE_CAMERA_UV_RASTER_BAKE_SCHEMA_VERSION: &str = "ReferenceCameraUvRasterBake@2";
pub const REFERENCE_CAMERA_UV_RASTER_ALGORITHM_ID: &str = "forgecad.reference_camera_uv_raster";
pub const REFERENCE_CAMERA_UV_RASTER_ALGORITHM_VERSION: &str = "1";
pub const MAX_REFERENCE_CAMERA_UV_RASTER_SOURCE_BYTES: usize = 8 * 1024 * 1024;

/// The only two worker profiles.  They deliberately mirror the sidecar's
/// fixed texture dimensions instead of accepting an arbitrary raster budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceCameraUvRasterTextureProfile {
    Preview128,
    Production1024,
}

impl ReferenceCameraUvRasterTextureProfile {
    pub fn dimensions(self) -> (u16, u16) {
        match self {
            Self::Preview128 => (128, 128),
            Self::Production1024 => (1024, 1024),
        }
    }
}

/// Exact JSON DTO accepted by the capability-gated worker.  It keeps the
/// source bytes only in the transient Rust-to-sidecar request; callers must
/// never persist or expose this value through the product API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReferenceCameraUvRasterBake {
    pub schema_version: String,
    pub projection_id: String,
    pub source_evidence_id: String,
    pub source_image_sha256: String,
    pub source_png_base64: String,
    pub camera_hypothesis_id: String,
    pub camera_provenance_sha256: String,
    pub target_material_zone_id: String,
    pub texture_width: u16,
    pub texture_height: u16,
    pub world_to_clip_row_major: [f64; 16],
}

impl ReferenceCameraUvRasterBake {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != REFERENCE_CAMERA_UV_RASTER_BAKE_SCHEMA_VERSION
            || !stable_id(&self.projection_id)
            || !stable_id(&self.source_evidence_id)
            || !stable_id(&self.camera_hypothesis_id)
            || !stable_id(&self.target_material_zone_id)
            || !sha256(&self.source_image_sha256)
            || !sha256(&self.camera_provenance_sha256)
            || !matches!(
                (self.texture_width, self.texture_height),
                (128, 128) | (1024, 1024)
            )
            || !self
                .world_to_clip_row_major
                .iter()
                .all(|value| value.is_finite())
            || self
                .world_to_clip_row_major
                .iter()
                .all(|value| value.abs() < 1e-9)
        {
            return Err(invalid(
                "REFERENCE_CAMERA_UV_RASTER_DTO_INVALID",
                "Camera UV raster DTO identity, profile, hash, or matrix is invalid.",
            ));
        }
        let bytes = BASE64_STANDARD
            .decode(&self.source_png_base64)
            .map_err(|_| {
                invalid(
                    "REFERENCE_CAMERA_UV_RASTER_SOURCE_INVALID",
                    "Camera UV raster source is not canonical base64.",
                )
            })?;
        if bytes.is_empty()
            || bytes.len() > MAX_REFERENCE_CAMERA_UV_RASTER_SOURCE_BYTES
            || hex_sha256(&bytes) != self.source_image_sha256
        {
            return Err(invalid(
                "REFERENCE_CAMERA_UV_RASTER_SOURCE_INVALID",
                "Camera UV raster source size or SHA-256 does not match the sealed DTO.",
            ));
        }
        validate_worker_png(&bytes)
    }
}

/// Create one transient worker DTO from already sealed evidence and a Rust
/// camera binding. This function is intentionally data-only: the caller still
/// has to bind the resulting DTO to an exact retained material zone and final
/// GLB before compiling it in the restricted executor.
pub fn build_reference_camera_uv_raster_bake(
    evidence: &ReferenceEvidence,
    source_png_bytes: &[u8],
    binding: &ProjectionCameraBinding,
    target_material_zone_id: &str,
    texture_profile: ReferenceCameraUvRasterTextureProfile,
) -> CoreResult<ReferenceCameraUvRasterBake> {
    binding.validate()?;
    build_reference_camera_uv_raster_bake_bound(
        evidence,
        source_png_bytes,
        &binding.candidate_glb_sha256,
        &binding.binding_sha256,
        binding.world_to_clip_row_major,
        target_material_zone_id,
        texture_profile,
    )
}

/// Two-stage variant: the camera is anchored to immutable geometry, not a
/// final GLB whose PBR bytes this very bake will change.
pub fn build_reference_camera_uv_raster_bake_from_geometry(
    evidence: &ReferenceEvidence,
    source_png_bytes: &[u8],
    geometry: &GeometryInvariantBinding,
    binding: &GeometryProjectionCameraBinding,
    target_material_zone_id: &str,
    texture_profile: ReferenceCameraUvRasterTextureProfile,
) -> CoreResult<ReferenceCameraUvRasterBake> {
    binding.validate(geometry)?;
    build_reference_camera_uv_raster_bake_bound(
        evidence,
        source_png_bytes,
        &geometry.binding_sha256,
        &binding.binding_sha256,
        binding.world_to_clip_row_major,
        target_material_zone_id,
        texture_profile,
    )
}

fn build_reference_camera_uv_raster_bake_bound(
    evidence: &ReferenceEvidence,
    source_png_bytes: &[u8],
    camera_subject_sha256: &str,
    camera_binding_sha256: &str,
    world_to_clip_row_major: [f64; 16],
    target_material_zone_id: &str,
    texture_profile: ReferenceCameraUvRasterTextureProfile,
) -> CoreResult<ReferenceCameraUvRasterBake> {
    evidence.validate()?;
    if evidence.kind != ReferenceEvidenceKind::Image || evidence.source_media_type != "image/png" {
        return Err(invalid(
            "REFERENCE_CAMERA_UV_RASTER_PNG_EVIDENCE_REQUIRED",
            "Camera UV rasterization currently requires same-project sealed PNG image evidence.",
        ));
    }
    if source_png_bytes.is_empty()
        || source_png_bytes.len() > MAX_REFERENCE_CAMERA_UV_RASTER_SOURCE_BYTES
        || hex_sha256(source_png_bytes) != evidence.source_object_sha256
    {
        return Err(invalid(
            "REFERENCE_CAMERA_UV_RASTER_EVIDENCE_DRIFT",
            "Sealed PNG bytes do not match the evidence SHA-256 or bounded worker profile.",
        ));
    }
    if !stable_id(target_material_zone_id) {
        return Err(invalid(
            "REFERENCE_CAMERA_UV_RASTER_ZONE_INVALID",
            "Camera UV rasterization requires one stable material-zone ID from Rust lowering.",
        ));
    }
    validate_worker_png(source_png_bytes)?;
    let (texture_width, texture_height) = texture_profile.dimensions();
    let projection_identity = serde_json::json!({
        "schema_version": REFERENCE_CAMERA_UV_RASTER_BAKE_SCHEMA_VERSION,
        "algorithm_id": REFERENCE_CAMERA_UV_RASTER_ALGORITHM_ID,
        "algorithm_version": REFERENCE_CAMERA_UV_RASTER_ALGORITHM_VERSION,
        "source_evidence_id": evidence.evidence_id,
        "source_image_sha256": evidence.source_object_sha256,
        "camera_subject_sha256": camera_subject_sha256,
        "camera_provenance_sha256": camera_binding_sha256,
        "target_material_zone_id": target_material_zone_id,
        "texture_width": texture_width,
        "texture_height": texture_height,
    });
    let projection_sha = semantic_sha256(&projection_identity)?;
    let dto = ReferenceCameraUvRasterBake {
        schema_version: REFERENCE_CAMERA_UV_RASTER_BAKE_SCHEMA_VERSION.into(),
        projection_id: format!("projection_{}", &projection_sha[..24]),
        source_evidence_id: evidence.evidence_id.clone(),
        source_image_sha256: evidence.source_object_sha256.clone(),
        source_png_base64: BASE64_STANDARD.encode(source_png_bytes),
        camera_hypothesis_id: format!("camera_{}", &camera_binding_sha256[..24]),
        camera_provenance_sha256: camera_binding_sha256.into(),
        target_material_zone_id: target_material_zone_id.into(),
        texture_width,
        texture_height,
        world_to_clip_row_major,
    };
    dto.validate()?;
    Ok(dto)
}

fn validate_worker_png(bytes: &[u8]) -> CoreResult<()> {
    // The sidecar only accepts non-interlaced 8-bit RGB/RGBA PNG. Validate the
    // exact IHDR profile here as well, so invalid evidence cannot advance to a
    // disposable worker process merely to fail there.
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 33
        || bytes.get(..8) != Some(PNG_SIGNATURE)
        || bytes.get(8..12) != Some(&[0, 0, 0, 13])
        || bytes.get(12..16) != Some(b"IHDR")
    {
        return Err(invalid(
            "REFERENCE_CAMERA_UV_RASTER_PNG_INVALID",
            "Camera UV raster source must be a bounded RGB or RGBA PNG.",
        ));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("IHDR width"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("IHDR height"));
    let bit_depth = bytes[24];
    let color_type = bytes[25];
    let interlace = bytes[28];
    if width == 0 || height == 0 || bit_depth != 8 || !matches!(color_type, 2 | 6) || interlace != 0
    {
        return Err(invalid(
            "REFERENCE_CAMERA_UV_RASTER_PNG_INVALID",
            "Camera UV raster source must use non-interlaced 8-bit RGB or RGBA PNG pixels.",
        ));
    }
    ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png)
        .decode()
        .map_err(|_| {
            invalid(
                "REFERENCE_CAMERA_UV_RASTER_PNG_INVALID",
                "Camera UV raster source PNG cannot be decoded.",
            )
        })?;
    Ok(())
}

fn stable_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn invalid(code: &'static str, message: &'static str) -> CoreError {
    CoreError::invalid_data(code, message)
}

#[cfg(test)]
mod tests {
    use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};

    use super::*;
    use crate::{
        derive_projection_camera_binding, ReferenceClass, ReferenceEvidenceObservations,
        ReferenceImageBrightnessBucket, ReferenceImageColorBucket, ReferenceImageEdgeDensityBucket,
        ReferenceImageForegroundConfidence, ReferenceImageSurfaceFacts,
    };

    fn png_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        PngEncoder::new(&mut bytes)
            .write_image(&[28, 90, 140, 44, 120, 180], 2, 1, ColorType::Rgb8.into())
            .unwrap();
        bytes
    }

    fn evidence(bytes: &[u8]) -> ReferenceEvidence {
        ReferenceEvidence {
            schema_version: "ReferenceEvidence@1".into(),
            evidence_id: "refevid_camera_raster_001".into(),
            project_id: "project_camera_raster_001".into(),
            kind: ReferenceEvidenceKind::Image,
            reference_class: ReferenceClass::SingleImage,
            domain_pack_id: "pack_unclassified".into(),
            source_file_name: "reference.png".into(),
            source_media_type: "image/png".into(),
            source_object_sha256: hex_sha256(bytes),
            source_imported_asset_version_id: None,
            source_statement: "user supplied authorized reference".into(),
            license_statement: "user confirms rights".into(),
            missing_views: vec!["rear".into()],
            user_notes: String::new(),
            observations: ReferenceEvidenceObservations {
                silhouette_summary: "compact hard surface form".into(),
                proportion_ranges: vec!["visible frontal profile".into()],
                material_zone_observations: vec!["painted armor".into()],
                visible_part_hypotheses: vec![],
                uncertainties: vec!["rear surface hidden".into()],
                image_surface_facts: Some(ReferenceImageSurfaceFacts {
                    width: 2,
                    height: 1,
                    aspect_ratio_milli: 2_000,
                    dominant_color_buckets: vec![ReferenceImageColorBucket::Blue],
                    brightness: ReferenceImageBrightnessBucket::Balanced,
                    edge_density: ReferenceImageEdgeDensityBucket::Low,
                    foreground_bbox_normalized: [0, 0, 1_000, 1_000],
                    contact_sheet_layout_evidence: false,
                    foreground_confidence: ReferenceImageForegroundConfidence::Medium,
                }),
            },
            created_at: "2026-07-30T00:00:00Z".into(),
            glb_inspection: None,
        }
    }

    fn binding() -> ProjectionCameraBinding {
        derive_projection_camera_binding(&"a".repeat(64), "turntable_000", [1.0, 1.5, 0.8]).unwrap()
    }

    #[test]
    fn sealed_png_and_camera_binding_build_repeatable_worker_dto() {
        let bytes = png_bytes();
        let first = build_reference_camera_uv_raster_bake(
            &evidence(&bytes),
            &bytes,
            &binding(),
            "zone_armor_primary",
            ReferenceCameraUvRasterTextureProfile::Production1024,
        )
        .unwrap();
        let second = build_reference_camera_uv_raster_bake(
            &evidence(&bytes),
            &bytes,
            &binding(),
            "zone_armor_primary",
            ReferenceCameraUvRasterTextureProfile::Production1024,
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.texture_width, 1024);
        assert_eq!(first.camera_provenance_sha256, binding().binding_sha256);
        first.validate().unwrap();
    }

    #[test]
    fn source_hash_zone_and_camera_drift_fail_closed() {
        let bytes = png_bytes();
        let mut changed = bytes.clone();
        changed[32] ^= 1;
        assert_eq!(
            build_reference_camera_uv_raster_bake(
                &evidence(&bytes),
                &changed,
                &binding(),
                "zone_armor_primary",
                ReferenceCameraUvRasterTextureProfile::Preview128,
            )
            .unwrap_err()
            .code(),
            "REFERENCE_CAMERA_UV_RASTER_EVIDENCE_DRIFT"
        );
        assert_eq!(
            build_reference_camera_uv_raster_bake(
                &evidence(&bytes),
                &bytes,
                &binding(),
                "zone has spaces",
                ReferenceCameraUvRasterTextureProfile::Preview128,
            )
            .unwrap_err()
            .code(),
            "REFERENCE_CAMERA_UV_RASTER_ZONE_INVALID"
        );
        let mut drifted_binding = binding();
        drifted_binding.world_to_clip_row_major[0] += 0.01;
        assert_eq!(
            build_reference_camera_uv_raster_bake(
                &evidence(&bytes),
                &bytes,
                &drifted_binding,
                "zone_armor_primary",
                ReferenceCameraUvRasterTextureProfile::Preview128,
            )
            .unwrap_err()
            .code(),
            "PROJECTION_CAMERA_BINDING_DRIFT"
        );
    }

    #[test]
    fn geometry_bound_camera_builds_v2_without_final_glb_lineage() {
        let bytes = png_bytes();
        let geometry = crate::derive_geometry_invariant_binding(
            &"a".repeat(64),
            &"b".repeat(64),
            42,
            [1.0, 1.5, 0.8],
        )
        .unwrap();
        let camera =
            crate::derive_geometry_projection_camera_binding(&geometry, "turntable_000").unwrap();
        let dto = build_reference_camera_uv_raster_bake_from_geometry(
            &evidence(&bytes),
            &bytes,
            &geometry,
            &camera,
            "zone_armor_primary",
            ReferenceCameraUvRasterTextureProfile::Preview128,
        )
        .unwrap();
        assert_eq!(dto.camera_provenance_sha256, camera.binding_sha256);
        dto.validate().unwrap();
        let drifted_geometry = crate::derive_geometry_invariant_binding(
            &"a".repeat(64),
            &"c".repeat(64),
            42,
            [1.0, 1.5, 0.8],
        )
        .unwrap();
        assert_eq!(
            build_reference_camera_uv_raster_bake_from_geometry(
                &evidence(&bytes),
                &bytes,
                &drifted_geometry,
                &camera,
                "zone_armor_primary",
                ReferenceCameraUvRasterTextureProfile::Preview128,
            )
            .unwrap_err()
            .code(),
            "GEOMETRY_PROJECTION_CAMERA_BINDING_DRIFT"
        );
    }
}
