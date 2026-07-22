//! R007 reference evidence and constrained rebuild-plan contracts.
//!
//! A reference is evidence, never editable geometry. Images and direct GLB
//! bytes are sealed into Rust CAS only after the user supplies source and
//! licence statements; the compatibility path may instead alias an existing,
//! same-project `external_reference_glb` import. A rebuild plan records what
//! is visibly supported and what remains unknown, then links only to the
//! normal ChangeSet preview/confirm path.

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use image::{ImageFormat, ImageReader, Limits};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

use crate::{
    builtin_surface_adornment_manifest, builtin_surface_adornment_manifest_v2, ComponentRecipeRef,
    CoreError, CoreResult, ImportedGlbInspection, ObjectRecord, RecipeRegistry, RecipeValidator,
};

pub const REFERENCE_EVIDENCE_SCHEMA_VERSION: &str = "ReferenceEvidence@1";
pub const REFERENCE_GUIDED_REBUILD_PLAN_SCHEMA_VERSION: &str = "ReferenceGuidedRebuildPlan@1";
pub const REFERENCE_SURFACE_ANALYSIS_SCHEMA_VERSION: &str = "ReferenceSurfaceAnalysis@1";
pub const REFERENCE_EVIDENCE_SOURCE_ROLE: &str = "reference_evidence_source";
pub const MAX_REFERENCE_IMAGE_BYTES: usize = 16 * 1024 * 1024;
/// The local evidence analyser intentionally remains well below the 16 MiB
/// upload cap.  It decodes only one bounded RGBA image and immediately reduces
/// it to non-reconstructable facts; it never stores a derived bitmap.
pub const MAX_REFERENCE_IMAGE_DIMENSION: u32 = 4_096;
pub const MAX_REFERENCE_IMAGE_PIXELS: u64 = 8_000_000;
pub const MAX_REFERENCE_IMAGE_DECODED_BYTES: u64 = 32 * 1024 * 1024;

const C106_ROBOTIC_ARM_DOMAIN: &str = "pack_robotic_arm_concept";
const C106_ROOT_RECIPE_IDS: &[&str] = &[
    "recipe_c106_arm_desktop_assistant",
    "recipe_c106_arm_gallery_industrial",
    "recipe_c106_arm_service_display",
];

/// Deterministically binds one R007 plan to the ordinary ChangeSet that owns
/// its preview/confirm lifecycle. Keeping the identity derivation in Core
/// avoids adding reference-only fields to the sealed C105 `replace_part`
/// operation contract.
pub fn reference_rebuild_plan_id_for_change_set(change_set_id: &str) -> CoreResult<String> {
    bounded_id("change_set_id", change_set_id, "changeset_")?;
    let hash = crate::semantic_sha256(&serde_json::json!({"identity": change_set_id}))?;
    Ok(format!("rebuildplan_{}", &hash[..24]))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceEvidenceKind {
    Image,
    Glb,
}

impl ReferenceEvidenceKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Glb => "glb",
        }
    }
}

impl std::str::FromStr for ReferenceEvidenceKind {
    type Err = CoreError;

    fn from_str(value: &str) -> CoreResult<Self> {
        match value {
            "image" => Ok(Self::Image),
            "glb" => Ok(Self::Glb),
            _ => Err(CoreError::invalid_data(
                "REFERENCE_EVIDENCE_KIND_INVALID",
                "Reference evidence kind must be image or glb.",
            )),
        }
    }
}

/// Coverage is explicit: an empty `missing_views` declaration cannot turn one
/// image into multi-view evidence. GLB is always strict readback evidence.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceClass {
    SingleImage,
    MultiViewContactSheet,
    GlbReadback,
}

impl ReferenceClass {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SingleImage => "single_image",
            Self::MultiViewContactSheet => "multi_view_contact_sheet",
            Self::GlbReadback => "glb_readback",
        }
    }
}

impl std::str::FromStr for ReferenceClass {
    type Err = CoreError;

    fn from_str(value: &str) -> CoreResult<Self> {
        match value {
            "single_image" => Ok(Self::SingleImage),
            "multi_view_contact_sheet" => Ok(Self::MultiViewContactSheet),
            "glb_readback" => Ok(Self::GlbReadback),
            _ => Err(CoreError::invalid_data(
                "REFERENCE_CLASS_INVALID",
                "Reference class must be single_image, multi_view_contact_sheet or glb_readback.",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceImageColorBucket {
    Black,
    Gray,
    White,
    Blue,
    Cyan,
    Red,
    Yellow,
    Green,
    Violet,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceImageBrightnessBucket {
    Dark,
    Balanced,
    Bright,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceImageEdgeDensityBucket {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceImageForegroundConfidence {
    Low,
    Medium,
}

/// Low-dimensional, local-only visual facts. This is deliberately not an
/// image reconstruction contract: it stores no source pixels or paths.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReferenceImageSurfaceFacts {
    pub width: u32,
    pub height: u32,
    pub aspect_ratio_milli: u32,
    pub dominant_color_buckets: Vec<ReferenceImageColorBucket>,
    pub brightness: ReferenceImageBrightnessBucket,
    pub edge_density: ReferenceImageEdgeDensityBucket,
    pub foreground_bbox_normalized: [u16; 4],
    #[serde(default)]
    pub contact_sheet_layout_evidence: bool,
    pub foreground_confidence: ReferenceImageForegroundConfidence,
}

impl ReferenceImageSurfaceFacts {
    fn validate(&self) -> CoreResult<()> {
        if self.width == 0
            || self.height == 0
            || self.width > MAX_REFERENCE_IMAGE_DIMENSION
            || self.height > MAX_REFERENCE_IMAGE_DIMENSION
            || u64::from(self.width).saturating_mul(u64::from(self.height))
                > MAX_REFERENCE_IMAGE_PIXELS
            || self.aspect_ratio_milli == 0
            || self.dominant_color_buckets.is_empty()
            || self.dominant_color_buckets.len() > 4
        {
            return Err(CoreError::invalid_data(
                "REFERENCE_IMAGE_SURFACE_FACTS_INVALID",
                "Reference image surface facts are outside the bounded local evidence contract.",
            ));
        }
        let [left, top, right, bottom] = self.foreground_bbox_normalized;
        if left >= right || top >= bottom || right > 1_000 || bottom > 1_000 {
            return Err(CoreError::invalid_data(
                "REFERENCE_IMAGE_SURFACE_FACTS_INVALID",
                "Reference image foreground bounds must be ordered normalized values.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisiblePartHypothesis {
    pub role: String,
    pub confidence: String,
    pub visible_basis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReferenceEvidenceObservations {
    pub silhouette_summary: String,
    pub proportion_ranges: Vec<String>,
    pub material_zone_observations: Vec<String>,
    pub visible_part_hypotheses: Vec<VisiblePartHypothesis>,
    pub uncertainties: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_surface_facts: Option<ReferenceImageSurfaceFacts>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReferenceEvidence {
    pub schema_version: String,
    pub evidence_id: String,
    pub project_id: String,
    pub kind: ReferenceEvidenceKind,
    pub reference_class: ReferenceClass,
    pub domain_pack_id: String,
    pub source_file_name: String,
    pub source_media_type: String,
    pub source_object_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_imported_asset_version_id: Option<String>,
    pub source_statement: String,
    pub license_statement: String,
    pub missing_views: Vec<String>,
    pub user_notes: String,
    /// Deterministically derived by Rust from the sealed source/readback and
    /// stated uncertainty boundary; never supplied as product truth by UI.
    pub observations: ReferenceEvidenceObservations,
    pub created_at: String,
    /// Present for GLB only.  It is a bounded readback fact, not a claim about
    /// hidden structure or material properties.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glb_inspection: Option<ImportedGlbInspection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CreateReferenceEvidenceRequest {
    pub schema_version: String,
    pub client_request_id: String,
    pub project_id: String,
    pub kind: ReferenceEvidenceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_class: Option<ReferenceClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    /// Required for images and for a direct GLB evidence intake. Raw base64
    /// only; this is never a URL and the bytes are sealed into the
    /// user-authorized CAS. Direct GLB evidence never creates an asset Version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_base64: Option<String>,
    /// Optional GLB compatibility path. It names an existing same-project
    /// external-reference import; callers must supply either this OR direct
    /// GLB content, never both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_asset_version_id: Option<String>,
    pub source_statement: String,
    pub license_statement: String,
    #[serde(default)]
    pub missing_views: Vec<String>,
    #[serde(default)]
    pub user_notes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_pack_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceGuidedRebuildPlanStatus {
    Draft,
    Previewed,
    Confirmed,
    Rejected,
}

impl std::str::FromStr for ReferenceGuidedRebuildPlanStatus {
    type Err = CoreError;

    fn from_str(value: &str) -> CoreResult<Self> {
        match value {
            "draft" => Ok(Self::Draft),
            "previewed" => Ok(Self::Previewed),
            "confirmed" => Ok(Self::Confirmed),
            "rejected" => Ok(Self::Rejected),
            _ => Err(CoreError::invalid_data(
                "REFERENCE_REBUILD_STATUS_INVALID",
                "Reference rebuild plan has an unsupported lifecycle status.",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReferenceGuidedRebuildPlan {
    pub schema_version: String,
    pub rebuild_plan_id: String,
    pub project_id: String,
    pub evidence_id: String,
    /// Optional by design: an authorized image may start a new C105 candidate
    /// when a project has no editable asset yet.  When populated, the normal
    /// ChangeSet path must use this exact base.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_asset_version_id: Option<String>,
    pub domain_pack_id: String,
    pub recipe_id: String,
    pub recipe_registry_sha256: String,
    pub rebuild_summary: String,
    pub intended_differences: Vec<String>,
    pub retained_evidence: Vec<String>,
    pub unresolved_uncertainties: Vec<String>,
    pub status: ReferenceGuidedRebuildPlanStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_change_set_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed_asset_version_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// R007B's bounded statement of how read-only reference evidence influences a
/// new C106 mechanical-arm concept.  It deliberately stores no source pixels,
/// source vertices, URLs, paths, dimensions, or inferred hidden structure.
/// The normal R007A plan continues to own the preview/confirm lifecycle; this
/// value is a sealed, reproducible design-surface interpretation of that plan.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceSurfaceObservationKind {
    Silhouette,
    Proportion,
    VisiblePart,
    MaterialZone,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceSurfaceFidelityCeiling {
    /// An image reference with declared missing views has only a single/limited
    /// visible-surface ceiling.
    SingleImageVisibleSurfaceOnly,
    /// A user-declared contact sheet / multi-view image without known missing
    /// views still proves only visible exterior language. It never lifts the
    /// hidden-structure, exact-dimension, material-physics or functional
    /// boundaries shared by every image reference.
    MultiViewImageVisibleSurfaceOnly,
    /// A GLB can provide only the strict readback facts copied below. It does
    /// not establish source topology, part hierarchy, hidden structure, or
    /// material physics for the new ForgeCAD asset.
    StrictGlbReadbackVisibleBoundsOnly,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceSurfaceUnresolved {
    MissingViews,
    HiddenStructure,
    ExactDimensions,
    MaterialPhysics,
    FunctionalBehavior,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceSurfaceIntentionalChange {
    NonFunctionalRecipeInterpretation,
    ReviewedRecipeComponentSubstitution,
    MaterialPresetNormalization,
    SurfaceAdornmentNormalization,
}

/// A direct projection of `ImportedGlbInspection`.  R007B repeats these
/// bounded facts only to bind a frozen analysis to its exact readback; no GLB
/// node names, vertices, images, material names, or source topology cross the
/// reference boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReferenceSurfaceGlbReadbackFacts {
    pub sha256: String,
    pub triangle_count: u64,
    pub bounds_mm: [f64; 3],
    pub mesh_count: u64,
    pub primitive_count: u64,
    pub material_count: u64,
    pub node_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReferenceSurfaceBinding {
    pub binding_id: String,
    /// The index is resolved against the evidence's existing observations;
    /// callers cannot inject a new textual observation here.
    pub observation_kind: ReferenceSurfaceObservationKind,
    pub observation_index: u32,
    /// `None` targets the root base part. Otherwise this must be an exact,
    /// required C106 root child-slot ID, which disambiguates repeated joint and
    /// link roles without inventing source hierarchy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_part_slot_id: Option<String>,
    pub target_recipe: ComponentRecipeRef,
    pub target_part_role: String,
    pub target_material_zone_id: String,
    pub target_surface_slot_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReferenceSurfaceAnalysis {
    pub schema_version: String,
    pub analysis_id: String,
    pub rebuild_plan_id: String,
    pub evidence_id: String,
    pub source_object_sha256: String,
    pub domain_pack_id: String,
    pub target_root_recipe: ComponentRecipeRef,
    pub c106_registry_sha256: String,
    /// This pins the immutable A005 v2 manifest.  v1 is intentionally unable
    /// to authorize C106 target slots retroactively.
    pub surface_skill_id: String,
    pub surface_skill_version: u32,
    pub surface_skill_sha256: String,
    pub fidelity_ceiling: ReferenceSurfaceFidelityCeiling,
    pub bindings: Vec<ReferenceSurfaceBinding>,
    pub retained_observation_kinds: Vec<ReferenceSurfaceObservationKind>,
    pub intentionally_changed: Vec<ReferenceSurfaceIntentionalChange>,
    pub unresolved: Vec<ReferenceSurfaceUnresolved>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glb_readback_facts: Option<ReferenceSurfaceGlbReadbackFacts>,
    pub created_at: String,
}

impl ReferenceEvidenceObservations {
    pub fn validate(&self) -> CoreResult<()> {
        bounded_text("silhouette_summary", &self.silhouette_summary, 1, 600)?;
        bounded_string_list("proportion_ranges", &self.proportion_ranges, 1, 12, 240)?;
        bounded_string_list(
            "material_zone_observations",
            &self.material_zone_observations,
            0,
            12,
            240,
        )?;
        bounded_string_list("uncertainties", &self.uncertainties, 1, 16, 320)?;
        if self.visible_part_hypotheses.len() > 16 {
            return Err(CoreError::invalid_data(
                "REFERENCE_EVIDENCE_OBSERVATIONS_INVALID",
                "Reference evidence has too many visible-part hypotheses.",
            ));
        }
        for hypothesis in &self.visible_part_hypotheses {
            bounded_text("visible_part_hypothesis.role", &hypothesis.role, 1, 120)?;
            if !matches!(hypothesis.confidence.as_str(), "low" | "medium" | "high") {
                return Err(CoreError::invalid_data(
                    "REFERENCE_EVIDENCE_CONFIDENCE_INVALID",
                    "Visible-part hypothesis confidence must be low, medium or high.",
                ));
            }
            bounded_text(
                "visible_part_hypothesis.visible_basis",
                &hypothesis.visible_basis,
                1,
                320,
            )?;
        }
        if let Some(facts) = &self.image_surface_facts {
            facts.validate()?;
        }
        Ok(())
    }
}

impl CreateReferenceEvidenceRequest {
    pub(crate) fn validate_shape(&self) -> CoreResult<()> {
        if self.schema_version != "ReferenceEvidenceCreateRequest@1" {
            return Err(CoreError::invalid_data(
                "REFERENCE_EVIDENCE_CREATE_SCHEMA_INVALID",
                "Reference evidence create request must use ReferenceEvidenceCreateRequest@1.",
            ));
        }
        bounded_text("client_request_id", &self.client_request_id, 1, 160)?;
        bounded_project_id(&self.project_id)?;
        bounded_text("source_statement", &self.source_statement, 1, 800)?;
        bounded_text("license_statement", &self.license_statement, 1, 800)?;
        bounded_string_list("missing_views", &self.missing_views, 0, 6, 80)?;
        bounded_text("user_notes", &self.user_notes, 0, 1_200)?;
        if let Some(domain_pack_id) = &self.domain_pack_id {
            valid_domain_pack(domain_pack_id)?;
        }
        match (self.kind, self.reference_class) {
            (ReferenceEvidenceKind::Image, Some(ReferenceClass::GlbReadback))
            | (
                ReferenceEvidenceKind::Glb,
                Some(ReferenceClass::SingleImage | ReferenceClass::MultiViewContactSheet),
            ) => {
                return Err(CoreError::invalid_data(
                    "REFERENCE_CLASS_KIND_MISMATCH",
                    "Reference class must match the evidence kind.",
                ));
            }
            _ => {}
        }
        match self.kind {
            ReferenceEvidenceKind::Image => {
                let file_name = self.file_name.as_deref().ok_or_else(|| {
                    CoreError::invalid_data(
                        "REFERENCE_IMAGE_FILE_NAME_REQUIRED",
                        "Image evidence requires a file name.",
                    )
                })?;
                safe_file_name(file_name)?;
                let media_type = self.media_type.as_deref().ok_or_else(|| {
                    CoreError::invalid_data(
                        "REFERENCE_IMAGE_MEDIA_TYPE_REQUIRED",
                        "Image evidence requires an image media type.",
                    )
                })?;
                image_extension_for_media_type(media_type)?;
                let encoded = self.content_base64.as_deref().ok_or_else(|| {
                    CoreError::invalid_data(
                        "REFERENCE_IMAGE_CONTENT_REQUIRED",
                        "Image evidence requires raw base64 content.",
                    )
                })?;
                if encoded.is_empty()
                    || encoded.len() > MAX_REFERENCE_IMAGE_BYTES.saturating_mul(2)
                    || encoded.starts_with("data:")
                    || encoded.contains("://")
                {
                    return Err(CoreError::invalid_data(
                        "REFERENCE_IMAGE_CONTENT_INVALID",
                        "Image evidence must be bounded raw base64, not a URL or data URI.",
                    ));
                }
                if self.imported_asset_version_id.is_some() {
                    return Err(CoreError::invalid_data(
                        "REFERENCE_IMAGE_IMPORT_INVALID",
                        "Image evidence cannot name an imported GLB asset.",
                    ));
                }
            }
            ReferenceEvidenceKind::Glb => {
                let imported = self.imported_asset_version_id.is_some();
                let direct = self.content_base64.is_some();
                if imported == direct {
                    return Err(CoreError::invalid_data("REFERENCE_GLB_SOURCE_REQUIRED", "GLB evidence must provide exactly one of a sealed imported asset or direct GLB bytes."));
                }
                if imported && (self.file_name.is_some() || self.media_type.is_some()) {
                    return Err(CoreError::invalid_data(
                        "REFERENCE_GLB_METADATA_FORBIDDEN",
                        "Imported GLB evidence reads its file metadata from the sealed import.",
                    ));
                }
                if direct {
                    let file_name = self.file_name.as_deref().ok_or_else(|| {
                        CoreError::invalid_data(
                            "REFERENCE_GLB_FILE_NAME_REQUIRED",
                            "Direct GLB evidence requires a file name.",
                        )
                    })?;
                    safe_file_name(file_name)?;
                    if !file_name.ends_with(".glb") {
                        return Err(CoreError::invalid_data(
                            "REFERENCE_GLB_FILE_NAME_INVALID",
                            "Direct GLB evidence file name must end in .glb.",
                        ));
                    }
                    if self.media_type.as_deref() != Some("model/gltf-binary") {
                        return Err(CoreError::invalid_data(
                            "REFERENCE_GLB_MEDIA_TYPE_INVALID",
                            "Direct GLB evidence media_type must be model/gltf-binary.",
                        ));
                    }
                    let encoded = self.content_base64.as_deref().expect("direct GLB content");
                    if encoded.is_empty()
                        || encoded.len() > 44_739_244
                        || encoded.starts_with("data:")
                        || encoded.contains("://")
                    {
                        return Err(CoreError::invalid_data("REFERENCE_GLB_CONTENT_INVALID", "Direct GLB evidence must be bounded raw base64, not a URL or data URI."));
                    }
                    if self.domain_pack_id.is_none() {
                        return Err(CoreError::invalid_data(
                            "REFERENCE_GLB_DOMAIN_REQUIRED",
                            "Direct GLB evidence must declare a visual Domain Pack.",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn decode_image(&self) -> CoreResult<Vec<u8>> {
        let encoded = self.content_base64.as_deref().ok_or_else(|| {
            CoreError::invalid_data(
                "REFERENCE_IMAGE_CONTENT_REQUIRED",
                "Image evidence requires raw base64 content.",
            )
        })?;
        let bytes = BASE64_STANDARD.decode(encoded).map_err(|_| {
            CoreError::invalid_data(
                "REFERENCE_IMAGE_CONTENT_INVALID",
                "Image evidence is not valid base64.",
            )
        })?;
        if bytes.is_empty() || bytes.len() > MAX_REFERENCE_IMAGE_BYTES {
            return Err(CoreError::invalid_data(
                "REFERENCE_IMAGE_SIZE_INVALID",
                "Image evidence bytes exceed the local reference limit.",
            ));
        }
        validate_image_magic(
            self.media_type
                .as_deref()
                .expect("validated image media type"),
            &bytes,
        )?;
        Ok(bytes)
    }

    pub(crate) fn analyze_image(&self, bytes: &[u8]) -> CoreResult<ReferenceImageSurfaceFacts> {
        analyze_reference_image_bytes(
            self.media_type
                .as_deref()
                .expect("validated image media type"),
            bytes,
        )
    }

    pub(crate) fn decode_direct_glb(&self) -> CoreResult<(Vec<u8>, ImportedGlbInspection)> {
        let encoded = self.content_base64.as_deref().ok_or_else(|| {
            CoreError::invalid_data(
                "REFERENCE_GLB_SOURCE_REQUIRED",
                "Direct GLB evidence requires raw base64 content.",
            )
        })?;
        let bytes = BASE64_STANDARD.decode(encoded).map_err(|_| {
            CoreError::invalid_data(
                "REFERENCE_GLB_CONTENT_INVALID",
                "Direct GLB evidence is not valid base64.",
            )
        })?;
        if bytes.len() > crate::MAX_IMPORTED_GLB_BYTES {
            return Err(CoreError::invalid_data(
                "REFERENCE_GLB_SIZE_INVALID",
                "Direct GLB evidence exceeds the local read-only GLB limit.",
            ));
        }
        let inspection = crate::inspect_external_glb(&bytes)?;
        Ok((bytes, inspection))
    }
}

impl ReferenceEvidence {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != REFERENCE_EVIDENCE_SCHEMA_VERSION {
            return Err(CoreError::invalid_data(
                "REFERENCE_EVIDENCE_SCHEMA_INVALID",
                "Reference evidence schema version is invalid.",
            ));
        }
        bounded_id("evidence_id", &self.evidence_id, "refevid_")?;
        bounded_project_id(&self.project_id)?;
        valid_domain_pack(&self.domain_pack_id)?;
        safe_file_name(&self.source_file_name)?;
        bounded_text("source_media_type", &self.source_media_type, 1, 80)?;
        sha256(&self.source_object_sha256)?;
        bounded_text("source_statement", &self.source_statement, 1, 800)?;
        bounded_text("license_statement", &self.license_statement, 1, 800)?;
        bounded_string_list("missing_views", &self.missing_views, 0, 6, 80)?;
        bounded_text("user_notes", &self.user_notes, 0, 1_200)?;
        bounded_text("created_at", &self.created_at, 1, 128)?;
        self.observations.validate()?;
        match self.kind {
            ReferenceEvidenceKind::Image => {
                if !matches!(
                    self.reference_class,
                    ReferenceClass::SingleImage | ReferenceClass::MultiViewContactSheet
                ) {
                    return Err(CoreError::invalid_data(
                        "REFERENCE_CLASS_KIND_MISMATCH",
                        "Image evidence must retain an image coverage class.",
                    ));
                }
                image_extension_for_media_type(&self.source_media_type)?;
                if self.source_imported_asset_version_id.is_some() || self.glb_inspection.is_some()
                {
                    return Err(CoreError::invalid_data(
                        "REFERENCE_IMAGE_RECORD_INVALID",
                        "Image evidence cannot carry GLB import/readback data.",
                    ));
                }
                let facts = self
                    .observations
                    .image_surface_facts
                    .as_ref()
                    .ok_or_else(|| {
                        CoreError::invalid_data(
                            "REFERENCE_IMAGE_SURFACE_FACTS_REQUIRED",
                            "Image evidence must retain bounded local surface facts.",
                        )
                    })?;
                if self.reference_class == ReferenceClass::MultiViewContactSheet
                    && !facts.contact_sheet_layout_evidence
                {
                    return Err(CoreError::invalid_data(
                        "REFERENCE_CLASS_LAYOUT_EVIDENCE_REQUIRED",
                        "Multi-view contact-sheet evidence requires locally observed layout evidence.",
                    ));
                }
            }
            ReferenceEvidenceKind::Glb => {
                if self.reference_class != ReferenceClass::GlbReadback
                    || self.observations.image_surface_facts.is_some()
                {
                    return Err(CoreError::invalid_data(
                        "REFERENCE_CLASS_KIND_MISMATCH",
                        "GLB evidence must retain strict GLB readback classification only.",
                    ));
                }
                if self.source_media_type != "model/gltf-binary"
                    || !self.source_file_name.ends_with(".glb")
                {
                    return Err(CoreError::invalid_data(
                        "REFERENCE_GLB_RECORD_INVALID",
                        "GLB evidence must retain sealed GLB metadata.",
                    ));
                }
                if let Some(source) = self.source_imported_asset_version_id.as_deref() {
                    bounded_id("source_imported_asset_version_id", source, "assetver_")?;
                }
                let inspection = self.glb_inspection.as_ref().ok_or_else(|| {
                    CoreError::invalid_data(
                        "REFERENCE_GLB_RECORD_INVALID",
                        "GLB evidence must retain strict import inspection.",
                    )
                })?;
                inspection.validate()?;
                if inspection.sha256 != self.source_object_sha256 {
                    return Err(CoreError::invalid_data(
                        "REFERENCE_GLB_RECORD_INVALID",
                        "GLB evidence hash must equal the sealed import inspection hash.",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl ReferenceGuidedRebuildPlan {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != REFERENCE_GUIDED_REBUILD_PLAN_SCHEMA_VERSION {
            return Err(CoreError::invalid_data(
                "REFERENCE_REBUILD_PLAN_SCHEMA_INVALID",
                "Reference rebuild plan schema version is invalid.",
            ));
        }
        bounded_id("rebuild_plan_id", &self.rebuild_plan_id, "rebuildplan_")?;
        bounded_project_id(&self.project_id)?;
        bounded_id("evidence_id", &self.evidence_id, "refevid_")?;
        if let Some(base_asset_version_id) = &self.base_asset_version_id {
            bounded_id("base_asset_version_id", base_asset_version_id, "assetver_")?;
        }
        valid_domain_pack(&self.domain_pack_id)?;
        bounded_id("recipe_id", &self.recipe_id, "recipe_")?;
        sha256(&self.recipe_registry_sha256)?;
        bounded_text("rebuild_summary", &self.rebuild_summary, 1, 800)?;
        bounded_string_list(
            "intended_differences",
            &self.intended_differences,
            1,
            16,
            320,
        )?;
        bounded_string_list("retained_evidence", &self.retained_evidence, 1, 16, 320)?;
        bounded_string_list(
            "unresolved_uncertainties",
            &self.unresolved_uncertainties,
            1,
            16,
            320,
        )?;
        bounded_text("created_at", &self.created_at, 1, 128)?;
        bounded_text("updated_at", &self.updated_at, 1, 128)?;
        match self.status {
            ReferenceGuidedRebuildPlanStatus::Draft => {
                if self.preview_change_set_id.is_some() || self.confirmed_asset_version_id.is_some()
                {
                    return Err(CoreError::invalid_data(
                        "REFERENCE_REBUILD_PLAN_STATE_INVALID",
                        "Draft rebuild plan cannot reference preview or confirmed asset.",
                    ));
                }
            }
            ReferenceGuidedRebuildPlanStatus::Previewed
            | ReferenceGuidedRebuildPlanStatus::Rejected => {
                let change = self.preview_change_set_id.as_deref().ok_or_else(|| {
                    CoreError::invalid_data(
                        "REFERENCE_REBUILD_PLAN_STATE_INVALID",
                        "Previewed/rejected rebuild plan must reference a ChangeSet.",
                    )
                })?;
                bounded_id("preview_change_set_id", change, "changeset_")?;
                if self.confirmed_asset_version_id.is_some() {
                    return Err(CoreError::invalid_data(
                        "REFERENCE_REBUILD_PLAN_STATE_INVALID",
                        "Unconfirmed rebuild plan cannot reference a confirmed asset.",
                    ));
                }
            }
            ReferenceGuidedRebuildPlanStatus::Confirmed => {
                let change = self.preview_change_set_id.as_deref().ok_or_else(|| {
                    CoreError::invalid_data(
                        "REFERENCE_REBUILD_PLAN_STATE_INVALID",
                        "Confirmed rebuild plan must retain its ChangeSet.",
                    )
                })?;
                let asset = self.confirmed_asset_version_id.as_deref().ok_or_else(|| {
                    CoreError::invalid_data(
                        "REFERENCE_REBUILD_PLAN_STATE_INVALID",
                        "Confirmed rebuild plan must retain its new asset.",
                    )
                })?;
                bounded_id("preview_change_set_id", change, "changeset_")?;
                bounded_id("confirmed_asset_version_id", asset, "assetver_")?;
            }
        }
        Ok(())
    }
}

impl ReferenceSurfaceGlbReadbackFacts {
    pub fn from_inspection(inspection: &ImportedGlbInspection) -> Self {
        Self {
            sha256: inspection.sha256.clone(),
            triangle_count: inspection.triangle_count,
            bounds_mm: inspection.bounds_mm,
            mesh_count: inspection.mesh_count,
            primitive_count: inspection.primitive_count,
            material_count: inspection.material_count,
            node_count: inspection.node_count,
        }
    }

    fn validate_against(&self, inspection: &ImportedGlbInspection) -> CoreResult<()> {
        sha256(&self.sha256)?;
        if !self
            .bounds_mm
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
            || self.triangle_count == 0
            || self.mesh_count == 0
            || self.primitive_count == 0
            || self.sha256 != inspection.sha256
            || self.triangle_count != inspection.triangle_count
            || self.bounds_mm != inspection.bounds_mm
            || self.mesh_count != inspection.mesh_count
            || self.primitive_count != inspection.primitive_count
            || self.material_count != inspection.material_count
            || self.node_count != inspection.node_count
        {
            return Err(CoreError::invalid_data(
                "REFERENCE_SURFACE_GLB_READBACK_MISMATCH",
                "Reference surface analysis must retain only the exact strict GLB readback facts.",
            ));
        }
        Ok(())
    }
}

impl ReferenceSurfaceAnalysis {
    /// Validates the sealed R007B interpretation against an existing R007A
    /// evidence/plan pair and the immutable C106 + A005 v2 catalogs.  This is
    /// intentionally pure: it does not read a file, call a Provider, mutate a
    /// Snapshot, compile geometry, or advance a Version.
    pub fn validate_for_plan(
        &self,
        evidence: &ReferenceEvidence,
        plan: &ReferenceGuidedRebuildPlan,
    ) -> CoreResult<()> {
        evidence.validate()?;
        plan.validate()?;
        if self.schema_version != REFERENCE_SURFACE_ANALYSIS_SCHEMA_VERSION {
            return Err(CoreError::invalid_data(
                "REFERENCE_SURFACE_SCHEMA_INVALID",
                "Reference surface analysis must use ReferenceSurfaceAnalysis@1.",
            ));
        }
        bounded_id("analysis_id", &self.analysis_id, "refsrfanalysis_")?;
        bounded_id("rebuild_plan_id", &self.rebuild_plan_id, "rebuildplan_")?;
        bounded_id("evidence_id", &self.evidence_id, "refevid_")?;
        valid_domain_pack(&self.domain_pack_id)?;
        sha256(&self.source_object_sha256)?;
        bounded_text("created_at", &self.created_at, 1, 128)?;
        if self.rebuild_plan_id != plan.rebuild_plan_id
            || self.evidence_id != evidence.evidence_id
            || self.source_object_sha256 != evidence.source_object_sha256
            || plan.project_id != evidence.project_id
            || self.domain_pack_id != C106_ROBOTIC_ARM_DOMAIN
            || plan.domain_pack_id != C106_ROBOTIC_ARM_DOMAIN
            || evidence.domain_pack_id != C106_ROBOTIC_ARM_DOMAIN
        {
            return Err(CoreError::invalid_data(
                "REFERENCE_SURFACE_CONTEXT_MISMATCH",
                "Reference surface analysis must bind one same-domain C106 plan and sealed evidence object.",
            ));
        }

        let registry = RecipeRegistry::from_embedded_c106_robotic_arm()?;
        if self.c106_registry_sha256 != registry.registry_sha256()
            || plan.recipe_registry_sha256 != registry.registry_sha256()
            || self.target_root_recipe.recipe_id != plan.recipe_id
            || !C106_ROOT_RECIPE_IDS.contains(&self.target_root_recipe.recipe_id.as_str())
        {
            return Err(CoreError::invalid_data(
                "REFERENCE_SURFACE_C106_PROVENANCE_INVALID",
                "Reference surface analysis must pin the exact reviewed C106 root and registry.",
            ));
        }
        validate_recipe_ref(&registry, &self.target_root_recipe)?;

        let current_skill = builtin_surface_adornment_manifest_v2();
        let legacy_skill = builtin_surface_adornment_manifest();
        let current_skill_sha256 = current_skill.canonical_sha256()?;
        let legacy_skill_sha256 = legacy_skill.canonical_sha256()?;
        if self.surface_skill_id != current_skill.skill_id
            || self.surface_skill_version != current_skill.version
            || self.surface_skill_sha256 != current_skill_sha256
            || self.surface_skill_sha256 == legacy_skill_sha256
        {
            return Err(CoreError::invalid_data(
                "REFERENCE_SURFACE_A005_V2_REQUIRED",
                "C106 reference surfaces require the immutable A005 v2 manifest; legacy v1 is denied.",
            ));
        }

        validate_unique_enum_list(
            "REFERENCE_SURFACE_INTENTIONAL_CHANGE_INVALID",
            &self.intentionally_changed,
            1,
            4,
        )?;
        validate_unique_enum_list(
            "REFERENCE_SURFACE_UNRESOLVED_INVALID",
            &self.unresolved,
            1,
            5,
        )?;
        let mut binding_ids = std::collections::BTreeSet::new();
        if self.bindings.len() < 3 || self.bindings.len() > 16 {
            return Err(CoreError::invalid_data(
                "REFERENCE_SURFACE_BINDINGS_INVALID",
                "Reference surface analysis must contain a bounded set of explicit evidence bindings.",
            ));
        }
        let mut bound_kinds = std::collections::BTreeSet::new();
        for binding in &self.bindings {
            bounded_id("binding_id", &binding.binding_id, "refsrfbind_")?;
            if !binding_ids.insert(&binding.binding_id) {
                return Err(CoreError::invalid_data(
                    "REFERENCE_SURFACE_BINDINGS_INVALID",
                    "Reference surface analysis binding IDs must be unique.",
                ));
            }
            validate_observation_reference(evidence, binding)?;
            validate_c106_surface_target(&registry, &self.target_root_recipe.recipe_id, binding)?;
            bound_kinds.insert(binding.observation_kind);
        }
        validate_retained_observation_kinds(&self.retained_observation_kinds, &bound_kinds)?;

        if !bound_kinds.contains(&ReferenceSurfaceObservationKind::Silhouette)
            || !bound_kinds.contains(&ReferenceSurfaceObservationKind::Proportion)
            || !bound_kinds.contains(&ReferenceSurfaceObservationKind::MaterialZone)
        {
            return Err(CoreError::invalid_data(
                "REFERENCE_SURFACE_EVIDENCE_COVERAGE_INVALID",
                "Reference surface analysis must map visible silhouette, relative proportion and material-zone observations.",
            ));
        }

        match evidence.kind {
            ReferenceEvidenceKind::Image => {
                let expected_ceiling = if evidence.missing_views.is_empty() {
                    ReferenceSurfaceFidelityCeiling::MultiViewImageVisibleSurfaceOnly
                } else {
                    ReferenceSurfaceFidelityCeiling::SingleImageVisibleSurfaceOnly
                };
                if self.fidelity_ceiling != expected_ceiling
                    || self.glb_readback_facts.is_some()
                    || !self
                        .unresolved
                        .contains(&ReferenceSurfaceUnresolved::HiddenStructure)
                    || !self
                        .unresolved
                        .contains(&ReferenceSurfaceUnresolved::ExactDimensions)
                    || !self
                        .unresolved
                        .contains(&ReferenceSurfaceUnresolved::MaterialPhysics)
                    || (!evidence.missing_views.is_empty()
                        && !self
                            .unresolved
                            .contains(&ReferenceSurfaceUnresolved::MissingViews))
                    || (evidence.missing_views.is_empty()
                        && self
                            .unresolved
                            .contains(&ReferenceSurfaceUnresolved::MissingViews))
                    || (!evidence.observations.visible_part_hypotheses.is_empty()
                        && !bound_kinds.contains(&ReferenceSurfaceObservationKind::VisiblePart))
                {
                    return Err(CoreError::invalid_data(
                        "REFERENCE_SURFACE_IMAGE_CEILING_INVALID",
                        "Image evidence must retain the single-view visible-surface ceiling and its unresolved boundaries.",
                    ));
                }
            }
            ReferenceEvidenceKind::Glb => {
                let inspection = evidence.glb_inspection.as_ref().ok_or_else(|| {
                    CoreError::invalid_data(
                        "REFERENCE_SURFACE_GLB_READBACK_REQUIRED",
                        "GLB surface analysis requires the sealed strict readback inspection.",
                    )
                })?;
                validate_strict_glb_surface_observations(evidence, inspection)?;
                let facts = self.glb_readback_facts.as_ref().ok_or_else(|| {
                    CoreError::invalid_data(
                        "REFERENCE_SURFACE_GLB_READBACK_REQUIRED",
                        "GLB surface analysis requires exact copied readback facts.",
                    )
                })?;
                if self.fidelity_ceiling
                    != ReferenceSurfaceFidelityCeiling::StrictGlbReadbackVisibleBoundsOnly
                    || bound_kinds.contains(&ReferenceSurfaceObservationKind::VisiblePart)
                    || !self
                        .unresolved
                        .contains(&ReferenceSurfaceUnresolved::HiddenStructure)
                    || !self
                        .unresolved
                        .contains(&ReferenceSurfaceUnresolved::ExactDimensions)
                    || !self
                        .unresolved
                        .contains(&ReferenceSurfaceUnresolved::MaterialPhysics)
                {
                    return Err(CoreError::invalid_data(
                        "REFERENCE_SURFACE_GLB_CEILING_INVALID",
                        "GLB evidence may bind only strict readback-visible facts and must preserve all hidden-structure boundaries.",
                    ));
                }
                facts.validate_against(inspection)?;
            }
        }
        Ok(())
    }
}

/// Public functional entry point for callers that prefer an explicit policy
/// function rather than a method. It is equivalent to
/// `ReferenceSurfaceAnalysis::validate_for_plan`.
pub fn validate_reference_surface_analysis_for_plan(
    analysis: &ReferenceSurfaceAnalysis,
    evidence: &ReferenceEvidence,
    plan: &ReferenceGuidedRebuildPlan,
) -> CoreResult<()> {
    analysis.validate_for_plan(evidence, plan)
}

fn validate_recipe_ref(
    registry: &RecipeRegistry,
    reference: &ComponentRecipeRef,
) -> CoreResult<()> {
    if reference.schema_version != "ComponentRecipeRef@1" {
        return Err(CoreError::invalid_data(
            "REFERENCE_SURFACE_RECIPE_REF_INVALID",
            "Reference surface analysis must use ComponentRecipeRef@1.",
        ));
    }
    let recipe = registry.recipe(&reference.recipe_id).ok_or_else(|| {
        CoreError::invalid_data(
            "REFERENCE_SURFACE_RECIPE_REF_INVALID",
            "Reference surface analysis names a Recipe outside the C106 registry.",
        )
    })?;
    if reference.version != recipe.version
        || reference.recipe_sha256 != RecipeValidator::recipe_sha256(recipe)?
    {
        return Err(CoreError::invalid_data(
            "REFERENCE_SURFACE_RECIPE_REF_INVALID",
            "Reference surface analysis Recipe identity does not match the reviewed C106 catalog.",
        ));
    }
    Ok(())
}

fn validate_c106_surface_target(
    registry: &RecipeRegistry,
    root_recipe_id: &str,
    binding: &ReferenceSurfaceBinding,
) -> CoreResult<()> {
    let root = registry.recipe(root_recipe_id).ok_or_else(|| {
        CoreError::invalid_data(
            "REFERENCE_SURFACE_TARGET_INVALID",
            "Reference surface analysis root is absent from C106.",
        )
    })?;
    let expected = match binding.target_part_slot_id.as_deref() {
        None => (root, root.component_role.as_str()),
        Some(slot_id) => {
            bounded_id("target_part_slot_id", slot_id, "slot_")?;
            let slot = root
                .child_slots
                .iter()
                .find(|slot| slot.slot_id == slot_id && slot.required)
                .ok_or_else(|| {
                    CoreError::invalid_data(
                        "REFERENCE_SURFACE_TARGET_INVALID",
                        "Reference surface target must be a required C106 root child slot.",
                    )
                })?;
            let recipe = registry.recipe(&slot.child_recipe_id).ok_or_else(|| {
                CoreError::invalid_data(
                    "REFERENCE_SURFACE_TARGET_INVALID",
                    "Reference surface target child Recipe is absent from C106.",
                )
            })?;
            if !slot
                .accepted_roles
                .iter()
                .any(|role| role == &recipe.component_role)
            {
                return Err(CoreError::invalid_data(
                    "REFERENCE_SURFACE_TARGET_INVALID",
                    "C106 target child slot does not retain its reviewed component role.",
                ));
            }
            (recipe, recipe.component_role.as_str())
        }
    };
    validate_recipe_ref(registry, &binding.target_recipe)?;
    if binding.target_recipe.recipe_id != expected.0.recipe_id
        || binding.target_part_role != expected.1
        || !expected
            .0
            .material_zones
            .iter()
            .any(|zone| zone["zone_id"].as_str() == Some(binding.target_material_zone_id.as_str()))
        || !expected.0.surface_adornment_slots.iter().any(|slot| {
            slot.slot_id == binding.target_surface_slot_id
                && slot.zone_id == binding.target_material_zone_id
        })
    {
        return Err(CoreError::invalid_data(
            "REFERENCE_SURFACE_TARGET_INVALID",
            "Reference surface mapping must target an exact C106 Recipe, semantic part, Material Zone and A005 slot.",
        ));
    }
    let manifest = builtin_surface_adornment_manifest_v2();
    if !manifest
        .recipe_ids
        .iter()
        .any(|recipe_id| recipe_id == &binding.target_recipe.recipe_id)
    {
        return Err(CoreError::invalid_data(
            "REFERENCE_SURFACE_A005_V2_REQUIRED",
            "Reference surface target is not authorized by immutable A005 v2.",
        ));
    }
    Ok(())
}

fn validate_observation_reference(
    evidence: &ReferenceEvidence,
    binding: &ReferenceSurfaceBinding,
) -> CoreResult<()> {
    let index = binding.observation_index as usize;
    let exists = match binding.observation_kind {
        ReferenceSurfaceObservationKind::Silhouette => index == 0,
        ReferenceSurfaceObservationKind::Proportion => {
            index < evidence.observations.proportion_ranges.len()
        }
        ReferenceSurfaceObservationKind::VisiblePart => {
            let Some(hypothesis) = evidence.observations.visible_part_hypotheses.get(index) else {
                return Err(CoreError::invalid_data(
                    "REFERENCE_SURFACE_OBSERVATION_INVALID",
                    "Reference surface binding does not point to an existing visible-part observation.",
                ));
            };
            visible_part_role_matches_target(&hypothesis.role, &binding.target_part_role)
        }
        ReferenceSurfaceObservationKind::MaterialZone => {
            index < evidence.observations.material_zone_observations.len()
        }
    };
    if !exists {
        return Err(CoreError::invalid_data(
            "REFERENCE_SURFACE_OBSERVATION_INVALID",
            "Reference surface binding does not point to a compatible sealed evidence observation.",
        ));
    }
    Ok(())
}

fn visible_part_role_matches_target(observed_role: &str, target_part_role: &str) -> bool {
    matches!(
        (observed_role, target_part_role),
        ("base_form", "base_form")
            | ("joint_housing", "joint_housing")
            | ("upper_link_form", "link_armor")
            | ("visual_cable", "cable_harness")
            | ("end_effector_form", "end_effector_form")
    )
}

fn validate_retained_observation_kinds(
    retained: &[ReferenceSurfaceObservationKind],
    bound: &std::collections::BTreeSet<ReferenceSurfaceObservationKind>,
) -> CoreResult<()> {
    if retained.is_empty() || retained.len() > 4 {
        return Err(CoreError::invalid_data(
            "REFERENCE_SURFACE_RETAINED_INVALID",
            "Reference surface analysis must explicitly record bounded retained evidence kinds.",
        ));
    }
    let retained_set = retained
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    if retained_set.len() != retained.len() || retained_set != *bound {
        return Err(CoreError::invalid_data(
            "REFERENCE_SURFACE_RETAINED_INVALID",
            "Retained evidence kinds must exactly match the explicit binding kinds.",
        ));
    }
    Ok(())
}

fn validate_unique_enum_list<T: Ord>(
    code: &'static str,
    values: &[T],
    min: usize,
    max: usize,
) -> CoreResult<()> {
    if values.len() < min
        || values.len() > max
        || values
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != values.len()
    {
        return Err(CoreError::invalid_data(code, "Reference surface analysis enum list is empty, duplicated or outside its bounded contract."));
    }
    Ok(())
}

fn validate_strict_glb_surface_observations(
    evidence: &ReferenceEvidence,
    inspection: &ImportedGlbInspection,
) -> CoreResult<()> {
    let [x, y, z] = inspection.bounds_mm;
    let expected_silhouette = format!(
        "基于用户授权的只读 GLB 外观证据：{} 个网格、{} 个 primitive；仅用于可见轮廓与比例，不恢复隐藏结构。",
        inspection.mesh_count, inspection.primitive_count
    );
    let expected_proportion = format!(
        "已读取包围范围 {:.1} × {:.1} × {:.1} mm；仅作为相对比例区间，不是制造尺寸。",
        x, y, z
    );
    let expected_material = format!(
        "GLB 读取到 {} 个可见材质槽；材质名称和物理属性不作为事实恢复。",
        inspection.material_count
    );
    if evidence.observations.silhouette_summary != expected_silhouette
        || evidence.observations.proportion_ranges != [expected_proportion]
        || evidence.observations.material_zone_observations != [expected_material]
    {
        return Err(CoreError::invalid_data(
            "REFERENCE_SURFACE_GLB_OBSERVATIONS_UNVERIFIED",
            "GLB reference surface analysis accepts only repository-derived strict readback observations.",
        ));
    }
    Ok(())
}

pub(crate) fn image_extension_for_media_type(media_type: &str) -> CoreResult<&'static str> {
    match media_type {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/webp" => Ok("webp"),
        _ => Err(CoreError::invalid_data(
            "REFERENCE_IMAGE_MEDIA_TYPE_INVALID",
            "Reference image media type must be image/png, image/jpeg or image/webp.",
        )),
    }
}

fn validate_image_magic(media_type: &str, bytes: &[u8]) -> CoreResult<()> {
    let valid = match media_type {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.len() >= 3 && bytes[..3] == [0xff, 0xd8, 0xff],
        "image/webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(CoreError::invalid_data(
            "REFERENCE_IMAGE_MAGIC_INVALID",
            "Reference image bytes do not match the declared PNG, JPEG or WebP media type.",
        ))
    }
}

/// Decodes only an explicitly declared, magic-checked PNG/JPEG/WebP into a
/// bounded in-memory image and reduces it to coarse visual facts.  The caller
/// must seal the original separately; this function never writes files or
/// returns pixels.  Container metadata/profiles are rejected rather than
/// interpreted, which keeps colour-management and metadata parsing outside
/// the local evidence contract.
pub fn analyze_reference_image_bytes(
    media_type: &str,
    bytes: &[u8],
) -> CoreResult<ReferenceImageSurfaceFacts> {
    if bytes.is_empty() || bytes.len() > MAX_REFERENCE_IMAGE_BYTES {
        return Err(CoreError::invalid_data(
            "REFERENCE_IMAGE_SIZE_INVALID",
            "Reference image bytes exceed the local reference limit.",
        ));
    }
    validate_image_magic(media_type, bytes)?;
    let (header_width, header_height) = inspect_image_container(media_type, bytes)?;
    validate_image_dimensions(header_width, header_height)?;

    let format = match media_type {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/webp" => ImageFormat::WebP,
        _ => {
            return Err(CoreError::invalid_data(
                "REFERENCE_IMAGE_MEDIA_TYPE_INVALID",
                "Reference image media type must be image/png, image/jpeg or image/webp.",
            ));
        }
    };
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_REFERENCE_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_REFERENCE_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_REFERENCE_IMAGE_DECODED_BYTES);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let (decoded_width, decoded_height) = reader.into_dimensions().map_err(|_| {
        CoreError::invalid_data(
            "REFERENCE_IMAGE_DECODE_FAILED",
            "Reference image dimensions could not be decoded within local limits.",
        )
    })?;
    if (decoded_width, decoded_height) != (header_width, header_height) {
        return Err(CoreError::invalid_data(
            "REFERENCE_IMAGE_DIMENSION_MISMATCH",
            "Reference image header and decoder dimensions disagree.",
        ));
    }

    // Re-create the reader after its header pass. JPEG/WebP decoders do not
    // offer a strict allocation limit, so the independently parsed dimensions
    // and this checked RGBA byte calculation are the fail-closed allocation
    // boundary for all three permitted formats.
    let decoded_bytes = u64::from(header_width)
        .checked_mul(u64::from(header_height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| {
            CoreError::invalid_data(
                "REFERENCE_IMAGE_DIMENSIONS_INVALID",
                "Reference image dimensions overflow the local decode budget.",
            )
        })?;
    if decoded_bytes > MAX_REFERENCE_IMAGE_DECODED_BYTES {
        return Err(CoreError::invalid_data(
            "REFERENCE_IMAGE_DECODE_BUDGET_EXCEEDED",
            "Reference image decoded RGBA bytes exceed the local analysis budget.",
        ));
    }
    let mut decode_reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut decode_limits = Limits::default();
    decode_limits.max_image_width = Some(MAX_REFERENCE_IMAGE_DIMENSION);
    decode_limits.max_image_height = Some(MAX_REFERENCE_IMAGE_DIMENSION);
    decode_limits.max_alloc = Some(MAX_REFERENCE_IMAGE_DECODED_BYTES);
    decode_reader.limits(decode_limits);
    let rgba = decode_reader
        .decode()
        .map_err(|_| {
            CoreError::invalid_data(
                "REFERENCE_IMAGE_DECODE_FAILED",
                "Reference image could not be decoded within local limits.",
            )
        })?
        .to_rgba8();
    if rgba.width() != header_width
        || rgba.height() != header_height
        || rgba.as_raw().len() as u64 != decoded_bytes
    {
        return Err(CoreError::invalid_data(
            "REFERENCE_IMAGE_DECODE_MISMATCH",
            "Reference image decode did not produce the declared bounded RGBA surface.",
        ));
    }
    summarize_rgba_surface(header_width, header_height, rgba.as_raw())
}

fn validate_image_dimensions(width: u32, height: u32) -> CoreResult<()> {
    if width == 0
        || height == 0
        || width > MAX_REFERENCE_IMAGE_DIMENSION
        || height > MAX_REFERENCE_IMAGE_DIMENSION
        || u64::from(width).saturating_mul(u64::from(height)) > MAX_REFERENCE_IMAGE_PIXELS
    {
        return Err(CoreError::invalid_data(
            "REFERENCE_IMAGE_DIMENSIONS_INVALID",
            "Reference image dimensions exceed the local pixel or side-length limit.",
        ));
    }
    Ok(())
}

fn inspect_image_container(media_type: &str, bytes: &[u8]) -> CoreResult<(u32, u32)> {
    match media_type {
        "image/png" => inspect_png_container(bytes),
        "image/jpeg" => inspect_jpeg_container(bytes),
        "image/webp" => inspect_webp_container(bytes),
        _ => Err(CoreError::invalid_data(
            "REFERENCE_IMAGE_MEDIA_TYPE_INVALID",
            "Reference image media type must be image/png, image/jpeg or image/webp.",
        )),
    }
}

fn inspect_png_container(bytes: &[u8]) -> CoreResult<(u32, u32)> {
    if bytes.len() < 33 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(CoreError::invalid_data(
            "REFERENCE_IMAGE_CONTAINER_INVALID",
            "PNG reference image has an invalid or truncated container.",
        ));
    }
    let mut offset = 8usize;
    let mut dimensions = None;
    while offset < bytes.len() {
        let length = read_be_u32(bytes, offset)? as usize;
        let kind_start = offset.checked_add(4).ok_or_else(image_container_error)?;
        let data_start = kind_start
            .checked_add(4)
            .ok_or_else(image_container_error)?;
        let data_end = data_start
            .checked_add(length)
            .ok_or_else(image_container_error)?;
        let next = data_end.checked_add(4).ok_or_else(image_container_error)?;
        if next > bytes.len() {
            return Err(image_container_error());
        }
        let kind = &bytes[kind_start..data_start];
        if matches!(kind, b"iCCP" | b"eXIf" | b"tEXt" | b"zTXt" | b"iTXt") {
            return Err(CoreError::invalid_data(
                "REFERENCE_IMAGE_METADATA_FORBIDDEN",
                "Reference image metadata or ICC profiles are not accepted by local analysis.",
            ));
        }
        if dimensions.is_none() {
            if kind != b"IHDR" || length != 13 {
                return Err(image_container_error());
            }
            dimensions = Some((
                read_be_u32(bytes, data_start)?,
                read_be_u32(bytes, data_start + 4)?,
            ));
        }
        if kind == b"IEND" {
            if next != bytes.len() {
                return Err(image_container_error());
            }
            return dimensions.ok_or_else(image_container_error);
        }
        offset = next;
    }
    Err(image_container_error())
}

fn inspect_jpeg_container(bytes: &[u8]) -> CoreResult<(u32, u32)> {
    if bytes.len() < 4 || bytes[..2] != [0xff, 0xd8] {
        return Err(image_container_error());
    }
    let mut offset = 2usize;
    while offset < bytes.len() {
        while offset < bytes.len() && bytes[offset] == 0xff {
            offset += 1;
        }
        if offset >= bytes.len() {
            return Err(image_container_error());
        }
        let marker = bytes[offset];
        offset += 1;
        if marker == 0xd9 {
            break;
        }
        if marker == 0x00 || marker == 0xd8 || (0xd0..=0xd7).contains(&marker) || marker == 0x01 {
            continue;
        }
        if offset
            .checked_add(2)
            .filter(|end| *end <= bytes.len())
            .is_none()
        {
            return Err(image_container_error());
        }
        let length = usize::from(u16::from_be_bytes([bytes[offset], bytes[offset + 1]]));
        if length < 2 {
            return Err(image_container_error());
        }
        let data_start = offset + 2;
        let data_end = offset
            .checked_add(length)
            .ok_or_else(image_container_error)?;
        if data_end > bytes.len() {
            return Err(image_container_error());
        }
        // APP0/JFIF remains allowed as a required common container header;
        // all user metadata/profile/comment segments are rejected closed.
        if (0xe1..=0xef).contains(&marker) || marker == 0xfe {
            return Err(CoreError::invalid_data(
                "REFERENCE_IMAGE_METADATA_FORBIDDEN",
                "Reference image metadata or ICC profiles are not accepted by local analysis.",
            ));
        }
        if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            if data_end < data_start + 6 {
                return Err(image_container_error());
            }
            let height = u32::from(u16::from_be_bytes([
                bytes[data_start + 1],
                bytes[data_start + 2],
            ]));
            let width = u32::from(u16::from_be_bytes([
                bytes[data_start + 3],
                bytes[data_start + 4],
            ]));
            return Ok((width, height));
        }
        offset = data_end;
    }
    Err(image_container_error())
}

fn inspect_webp_container(bytes: &[u8]) -> CoreResult<(u32, u32)> {
    if bytes.len() < 20 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return Err(image_container_error());
    }
    let declared = read_le_u32(bytes, 4)? as usize;
    if declared.checked_add(8) != Some(bytes.len()) {
        return Err(image_container_error());
    }
    let mut offset = 12usize;
    let mut dimensions = None;
    while offset < bytes.len() {
        let chunk_id_end = offset.checked_add(4).ok_or_else(image_container_error)?;
        let size_offset = chunk_id_end;
        let data_start = size_offset
            .checked_add(4)
            .ok_or_else(image_container_error)?;
        if data_start > bytes.len() {
            return Err(image_container_error());
        }
        let size = read_le_u32(bytes, size_offset)? as usize;
        let data_end = data_start
            .checked_add(size)
            .ok_or_else(image_container_error)?;
        let next = data_end
            .checked_add(size & 1)
            .ok_or_else(image_container_error)?;
        if next > bytes.len() {
            return Err(image_container_error());
        }
        let kind = &bytes[offset..chunk_id_end];
        if matches!(kind, b"ICCP" | b"EXIF" | b"XMP ") {
            return Err(CoreError::invalid_data(
                "REFERENCE_IMAGE_METADATA_FORBIDDEN",
                "Reference image metadata or ICC profiles are not accepted by local analysis.",
            ));
        }
        match kind {
            b"VP8X" => {
                if size != 10 || data_start + 10 > bytes.len() {
                    return Err(image_container_error());
                }
                let flags = bytes[data_start];
                if flags & 0b0010_1100 != 0 {
                    return Err(CoreError::invalid_data(
                        "REFERENCE_IMAGE_METADATA_FORBIDDEN",
                        "Reference image metadata, ICC profiles or animation are not accepted by local analysis.",
                    ));
                }
                if flags & 0b0000_0010 != 0 {
                    return Err(CoreError::invalid_data(
                        "REFERENCE_IMAGE_ANIMATION_FORBIDDEN",
                        "Animated WebP is not accepted as a single visible-surface reference.",
                    ));
                }
                dimensions = Some((
                    read_le_u24(bytes, data_start + 4)? + 1,
                    read_le_u24(bytes, data_start + 7)? + 1,
                ));
            }
            b"VP8 " => {
                if size < 10 || bytes.get(data_start + 3..data_start + 6) != Some(b"\x9d\x01\x2a") {
                    return Err(image_container_error());
                }
                dimensions = Some((
                    u32::from(
                        u16::from_le_bytes([bytes[data_start + 6], bytes[data_start + 7]]) & 0x3fff,
                    ),
                    u32::from(
                        u16::from_le_bytes([bytes[data_start + 8], bytes[data_start + 9]]) & 0x3fff,
                    ),
                ));
            }
            b"VP8L" => {
                if size < 5 || bytes[data_start] != 0x2f {
                    return Err(image_container_error());
                }
                let packed = read_le_u32(bytes, data_start + 1)?;
                dimensions = Some(((packed & 0x3fff) + 1, ((packed >> 14) & 0x3fff) + 1));
            }
            _ => {}
        }
        offset = next;
    }
    dimensions.ok_or_else(image_container_error)
}

fn summarize_rgba_surface(
    width: u32,
    height: u32,
    pixels: &[u8],
) -> CoreResult<ReferenceImageSurfaceFacts> {
    let expected = usize::try_from(u64::from(width) * u64::from(height) * 4)
        .map_err(|_| image_container_error())?;
    if pixels.len() != expected {
        return Err(image_container_error());
    }
    let corner_points = [
        (0, 0),
        (width.saturating_sub(1), 0),
        (0, height.saturating_sub(1)),
        (width.saturating_sub(1), height.saturating_sub(1)),
    ];
    let mut background = [0u64; 3];
    let mut corners = 0u64;
    for (x, y) in corner_points {
        let index = ((y * width + x) * 4) as usize;
        let alpha = u64::from(pixels[index + 3]);
        if alpha > 0 {
            for channel in 0..3 {
                background[channel] += u64::from(pixels[index + channel]) * alpha;
            }
            corners += alpha;
        }
    }
    let background = if corners > 0 {
        [
            (background[0] / corners) as i32,
            (background[1] / corners) as i32,
            (background[2] / corners) as i32,
        ]
    } else {
        [0, 0, 0]
    };
    let mut buckets = std::collections::BTreeMap::<ReferenceImageColorBucket, u64>::new();
    let mut brightness_total = 0u64;
    let mut visible_pixels = 0u64;
    let mut foreground_count = 0u64;
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut foreground_columns = vec![false; width as usize];
    let mut foreground_rows = vec![false; height as usize];
    let mut edge_count = 0u64;
    let mut edge_tests = 0u64;
    for y in 0..height {
        for x in 0..width {
            let index = ((y * width + x) * 4) as usize;
            let rgba = &pixels[index..index + 4];
            let alpha = u64::from(rgba[3]);
            if alpha == 0 {
                continue;
            }
            let pixel_luma = luma(rgba[0], rgba[1], rgba[2]);
            brightness_total += u64::from(pixel_luma) * alpha;
            visible_pixels += alpha;
            *buckets
                .entry(color_bucket(rgba[0], rgba[1], rgba[2]))
                .or_default() += alpha;
            let distance = (i32::from(rgba[0]) - background[0]).unsigned_abs()
                + (i32::from(rgba[1]) - background[1]).unsigned_abs()
                + (i32::from(rgba[2]) - background[2]).unsigned_abs();
            if alpha >= 48 && distance >= 84 {
                foreground_count += 1;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                foreground_columns[x as usize] = true;
                foreground_rows[y as usize] = true;
            }
            for (next_x, next_y) in [(x.saturating_add(1), y), (x, y.saturating_add(1))] {
                if next_x >= width || next_y >= height {
                    continue;
                }
                let neighbor = ((next_y * width + next_x) * 4) as usize;
                if pixels[neighbor + 3] < 48 {
                    continue;
                }
                edge_tests += 1;
                let neighbor_luma =
                    luma(pixels[neighbor], pixels[neighbor + 1], pixels[neighbor + 2]);
                if pixel_luma.abs_diff(neighbor_luma) >= 36 {
                    edge_count += 1;
                }
            }
        }
    }
    if visible_pixels == 0 || buckets.is_empty() {
        return Err(CoreError::invalid_data(
            "REFERENCE_IMAGE_TRANSPARENT_INVALID",
            "Reference image has no visible pixels for local surface analysis.",
        ));
    }
    let total_pixels = u64::from(width) * u64::from(height);
    let minimum_foreground = (total_pixels / 200).max(1);
    let (left, top, right, bottom, confidence) = if foreground_count >= minimum_foreground {
        let coverage_milli = foreground_count.saturating_mul(1_000) / total_pixels;
        (
            normalize_edge(min_x, width, false),
            normalize_edge(min_y, height, false),
            normalize_edge(max_x.saturating_add(1), width, true),
            normalize_edge(max_y.saturating_add(1), height, true),
            if (10..=900).contains(&coverage_milli) {
                ReferenceImageForegroundConfidence::Medium
            } else {
                ReferenceImageForegroundConfidence::Low
            },
        )
    } else {
        (0, 0, 1_000, 1_000, ReferenceImageForegroundConfidence::Low)
    };
    let mut ordered_buckets = buckets.into_iter().collect::<Vec<_>>();
    ordered_buckets.sort_by(|(left_bucket, left_weight), (right_bucket, right_weight)| {
        right_weight
            .cmp(left_weight)
            .then_with(|| left_bucket.cmp(right_bucket))
    });
    let dominant_color_buckets = ordered_buckets
        .into_iter()
        .take(4)
        .map(|(bucket, _)| bucket)
        .collect();
    let average_luma = brightness_total / visible_pixels;
    let brightness = if average_luma < 85 {
        ReferenceImageBrightnessBucket::Dark
    } else if average_luma < 170 {
        ReferenceImageBrightnessBucket::Balanced
    } else {
        ReferenceImageBrightnessBucket::Bright
    };
    let edge_density = if edge_tests == 0 {
        ReferenceImageEdgeDensityBucket::Low
    } else {
        match edge_count.saturating_mul(1_000) / edge_tests {
            0..=44 => ReferenceImageEdgeDensityBucket::Low,
            45..=139 => ReferenceImageEdgeDensityBucket::Medium,
            _ => ReferenceImageEdgeDensityBucket::High,
        }
    };
    let facts = ReferenceImageSurfaceFacts {
        width,
        height,
        aspect_ratio_milli: (u64::from(width) * 1_000 / u64::from(height)) as u32,
        dominant_color_buckets,
        brightness,
        edge_density,
        foreground_bbox_normalized: [left, top, right, bottom],
        contact_sheet_layout_evidence: foreground_count >= minimum_foreground
            && (has_full_projection_gap(&foreground_columns)
                || has_full_projection_gap(&foreground_rows)),
        foreground_confidence: confidence,
    };
    facts.validate()?;
    Ok(facts)
}

/// A contact sheet needs an entire divider row/column with visible content on
/// both sides. This intentionally rejects ambiguous grids and isolated arm
/// components rather than treating an empty `missing_views` declaration as
/// multi-view proof.
fn has_full_projection_gap(present: &[bool]) -> bool {
    if present.len() < 24 {
        return false;
    }
    let minimum_gap = (present.len() / 30).max(2);
    let mut start = 0usize;
    while start < present.len() {
        if present[start] {
            start += 1;
            continue;
        }
        let end = present[start..]
            .iter()
            .position(|value| *value)
            .map(|relative| start + relative)
            .unwrap_or(present.len());
        if end.saturating_sub(start) >= minimum_gap
            && present[..start].iter().any(|value| *value)
            && present[end..].iter().any(|value| *value)
        {
            return true;
        }
        start = end.saturating_add(1);
    }
    false
}

fn luma(red: u8, green: u8, blue: u8) -> u8 {
    ((u16::from(red) * 54 + u16::from(green) * 183 + u16::from(blue) * 19) / 256) as u8
}

fn color_bucket(red: u8, green: u8, blue: u8) -> ReferenceImageColorBucket {
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    let chroma = maximum - minimum;
    let luminance = luma(red, green, blue);
    if luminance < 45 {
        ReferenceImageColorBucket::Black
    } else if chroma < 30 {
        if luminance > 210 {
            ReferenceImageColorBucket::White
        } else {
            ReferenceImageColorBucket::Gray
        }
    } else if blue > red.saturating_add(20) && blue > green.saturating_add(8) {
        if green.saturating_add(18) > red {
            ReferenceImageColorBucket::Cyan
        } else if red.saturating_add(18) > green {
            ReferenceImageColorBucket::Violet
        } else {
            ReferenceImageColorBucket::Blue
        }
    } else if red > green.saturating_add(24) && red > blue.saturating_add(24) {
        if green > blue.saturating_add(18) {
            ReferenceImageColorBucket::Yellow
        } else {
            ReferenceImageColorBucket::Red
        }
    } else if green > red.saturating_add(16) && green > blue.saturating_add(12) {
        ReferenceImageColorBucket::Green
    } else {
        ReferenceImageColorBucket::Gray
    }
}

fn normalize_edge(value: u32, dimension: u32, upper: bool) -> u16 {
    let scaled = u64::from(value).saturating_mul(1_000) / u64::from(dimension.max(1));
    let rounded = if upper { scaled.max(1) } else { scaled };
    rounded.min(1_000) as u16
}

fn read_be_u32(bytes: &[u8], offset: usize) -> CoreResult<u32> {
    bytes
        .get(offset..offset.saturating_add(4))
        .and_then(|slice| slice.try_into().ok())
        .map(u32::from_be_bytes)
        .ok_or_else(image_container_error)
}

fn read_le_u32(bytes: &[u8], offset: usize) -> CoreResult<u32> {
    bytes
        .get(offset..offset.saturating_add(4))
        .and_then(|slice| slice.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(image_container_error)
}

fn read_le_u24(bytes: &[u8], offset: usize) -> CoreResult<u32> {
    let slice = bytes
        .get(offset..offset.saturating_add(3))
        .ok_or_else(image_container_error)?;
    Ok(u32::from(slice[0]) | (u32::from(slice[1]) << 8) | (u32::from(slice[2]) << 16))
}

fn image_container_error() -> CoreError {
    CoreError::invalid_data(
        "REFERENCE_IMAGE_CONTAINER_INVALID",
        "Reference image container is malformed, truncated or unsupported for local analysis.",
    )
}

pub(crate) fn valid_domain_pack(value: &str) -> CoreResult<()> {
    if matches!(
        value,
        "pack_future_weapon_prop"
            | "pack_vehicle_concept"
            | "pack_aircraft_concept"
            | "pack_robotic_arm_concept"
    ) {
        Ok(())
    } else {
        Err(CoreError::invalid_data(
            "DOMAIN_PACK_INVALID",
            "Reference evidence must use a registered visual Domain Pack.",
        ))
    }
}

pub(crate) fn bounded_id(field: &str, value: &str, prefix: &str) -> CoreResult<()> {
    if value.len() < prefix.len() + 1
        || value.len() > 160
        || !value.starts_with(prefix)
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
    {
        return Err(CoreError::invalid_data(
            "REFERENCE_EVIDENCE_ID_INVALID",
            format!("{field} must use the stable {prefix} identity namespace."),
        ));
    }
    Ok(())
}

fn bounded_project_id(value: &str) -> CoreResult<()> {
    if value.starts_with("prj_") {
        bounded_id("project_id", value, "prj_")
    } else {
        // Retain the historical fixture namespace while production uses prj_.
        bounded_id("project_id", value, "project_")
    }
}

fn bounded_text(field: &str, value: &str, min: usize, max: usize) -> CoreResult<()> {
    if value.len() < min || value.len() > max || value.contains('\0') {
        return Err(CoreError::invalid_data(
            "REFERENCE_EVIDENCE_TEXT_INVALID",
            format!("{field} is outside the bounded text contract."),
        ));
    }
    Ok(())
}

fn bounded_string_list(
    field: &str,
    values: &[String],
    min: usize,
    max: usize,
    item_max: usize,
) -> CoreResult<()> {
    if values.len() < min || values.len() > max {
        return Err(CoreError::invalid_data(
            "REFERENCE_EVIDENCE_LIST_INVALID",
            format!("{field} is outside the bounded list contract."),
        ));
    }
    let mut distinct = std::collections::BTreeSet::new();
    for value in values {
        bounded_text(field, value, 1, item_max)?;
        if !distinct.insert(value) {
            return Err(CoreError::invalid_data(
                "REFERENCE_EVIDENCE_LIST_INVALID",
                format!("{field} contains a duplicate value."),
            ));
        }
    }
    Ok(())
}

fn safe_file_name(value: &str) -> CoreResult<()> {
    bounded_text("source_file_name", value, 1, 180)?;
    if value.contains(['/', '\\']) || value == "." || value == ".." {
        return Err(CoreError::invalid_data(
            "REFERENCE_EVIDENCE_FILE_NAME_INVALID",
            "Reference source file name must not contain a path.",
        ));
    }
    Ok(())
}

fn sha256(value: &str) -> CoreResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(CoreError::invalid_data(
            "REFERENCE_EVIDENCE_SHA256_INVALID",
            "Reference evidence hash must be lowercase SHA-256.",
        ));
    }
    Ok(())
}

/// Source data returned by repository construction, so callers cannot invent
/// a CAS hash or GLB inspection record.
pub(crate) struct ResolvedReferenceSource {
    pub file_name: String,
    pub media_type: String,
    pub object: ObjectRecord,
    pub imported_asset_version_id: Option<String>,
    pub glb_inspection: Option<ImportedGlbInspection>,
    pub image_surface_facts: Option<ReferenceImageSurfaceFacts>,
}
