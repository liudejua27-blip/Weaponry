//! Rust-derived universal asset source and appearance-lineage contracts.
//!
//! U003 does not add a new geometry executor. It seals the already validated
//! U002 request/profile/feature/representation contracts to the current
//! procedural source, then optionally to one exact compiled GLB/readback/view
//! set. Bounded local lattice and per-part hard-surface hybrid branches enter
//! this same envelope through U004; mesh-seed remains unavailable rather than
//! creating another asset truth.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    compiled_visual_base_material_id, lower_visual_runtime_source_v1, semantic_sha256,
    AppearanceChannel, CoreError, CoreResult, DecalLayer, EmissiveMask, EvidenceStatus,
    ForgeVisualProgramRevision, NormalReliefLayer, RepresentationKind, RepresentationPlan,
    ReferenceAppearanceBinding, ReferenceEvidence, ReferenceEvidenceKind,
    ReferenceImageBrightnessBucket,
    ReferenceImageColorBucket, ReferenceImageEdgeDensityBucket, ReferenceImageSurfaceFacts,
    RoughnessMask, SubjectProfile, SurfaceLayerProgram, SurfaceSymmetry,
    UniversalAuthorRequest,
    UvFrame, VectorPath, VectorPathCommand, VisualDetailLevel, VisualFeatureContract,
    GENERIC_HARD_SURFACE_PROCEDURAL_CAPABILITY_ID,
    GENERIC_VISUAL_EXTERIOR_PROCEDURAL_CAPABILITY_ID,
    LOCAL_LATTICE_DEFORMABLE_CAPABILITY_ID,
    LOCAL_MESH_PATCH_CAPABILITY_ID,
};

pub const REFERENCE_CAMERA_HYPOTHESIS_SCHEMA_VERSION: &str = "ReferenceCameraHypothesis@1";
pub const APPEARANCE_EVIDENCE_BUNDLE_SCHEMA_VERSION: &str = "AppearanceEvidenceBundle@1";
pub const REFERENCE_APPEARANCE_PROJECTION_RECEIPT_SCHEMA_VERSION: &str =
    "ReferenceAppearanceProjectionReceipt@1";
pub const VISUAL_DETAIL_CLAIM_V2_SCHEMA_VERSION: &str = "VisualDetailClaim@2";
pub const MATERIAL_ZONE_APPEARANCE_SCHEMA_VERSION: &str = "MaterialZoneAppearance@1";
pub const UNIVERSAL_ASSET_SOURCE_SCHEMA_VERSION: &str = "UniversalAssetSource@1";
pub const UNIVERSAL_ASSET_SOURCE_V2_SCHEMA_VERSION: &str = "UniversalAssetSource@2";
pub const GENERIC_HARD_SURFACE_APPEARANCE_COMPILATION_SCHEMA_VERSION: &str =
    "GenericHardSurfaceAppearanceCompilation@2";
pub const REFERENCE_SURFACE_APPEARANCE_BINDING_SCHEMA_VERSION: &str =
    "ReferenceSurfaceAppearanceBinding@1";

const MAX_TEXTURE_EDGE: u16 = 2048;
const MAX_COMPONENTS: usize = 256;
const MAX_DETAIL_CLAIMS: usize = 512;
const MAX_MATERIAL_ZONES: usize = 128;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceProjectionType {
    Perspective,
    Orthographic,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CameraParameterSource {
    Metadata,
    LandmarkFit,
    SilhouetteFit,
    DefaultHypothesis,
    Unresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReferenceCameraHypothesis {
    pub schema_version: String,
    pub hypothesis_id: String,
    pub evidence_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_id: Option<String>,
    pub projection_type: ReferenceProjectionType,
    pub parameter_source: CameraParameterSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical_fov_millidegrees: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reprojection_error_bps: Option<u16>,
    #[serde(default)]
    pub landmark_feature_ids: Vec<String>,
    pub confidence_bps: u16,
    #[serde(default)]
    pub unresolved_fields: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceEvidenceArtifactKind {
    Mask,
    Region,
    DelightedColorHint,
    NormalHint,
    RoughnessHint,
    MetallicHint,
    UnobservedTexelMask,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppearanceEvidenceArtifact {
    pub artifact_id: String,
    pub evidence_id: String,
    pub kind: AppearanceEvidenceArtifactKind,
    pub content_sha256: String,
    pub algorithm_id: String,
    pub algorithm_version: String,
    /// Derived image evidence never becomes material truth by naming alone.
    pub evidence_only: bool,
}

/// Rust-owned proof that a sealed reference image was rasterized into the
/// exact PBR GLB produced by one UAS@2 compilation.  This is deliberately a
/// receipt rather than a texture/material source: image bytes remain in the
/// sealed evidence store and the GLB remains the only asset truth.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReferenceAppearanceProjectionReceipt {
    pub schema_version: String,
    pub source_request_sha256: String,
    pub source_program_sha256: String,
    pub final_glb_sha256: String,
    pub compile_readback_sha256: String,
    pub worker_receipt_sha256: String,
    pub worker_schema_version: String,
    pub algorithm_id: String,
    pub algorithm_version: String,
    pub projection_id: String,
    pub projection_sha256: String,
    pub source_evidence_id: String,
    pub source_image_sha256: String,
    pub camera_hypothesis_id: String,
    pub camera_provenance_sha256: String,
    pub target_material_zone_id: String,
    pub base_color_texture_id: String,
    pub base_color_sha256: String,
    pub base_color_byte_size: u64,
    pub unobserved_texel_mask_id: String,
    pub unobserved_texel_mask_sha256: String,
    pub unobserved_texel_mask_byte_size: u64,
    pub observed_texel_count: u64,
    pub unobserved_texel_count: u64,
    pub fusion_count: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raster_triangle_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world_to_clip_sha256: Option<String>,
    #[serde(default)]
    pub source_evidence_ids: Vec<String>,
    #[serde(default)]
    pub source_image_sha256s: Vec<String>,
    #[serde(default)]
    pub camera_hypothesis_ids: Vec<String>,
    #[serde(default)]
    pub camera_provenance_sha256s: Vec<String>,
    #[serde(default)]
    pub world_to_clip_sha256s: Vec<String>,
}

impl ReferenceAppearanceProjectionReceipt {
    pub fn validate(&self) -> CoreResult<()> {
        let valid_id = |value: &str| {
            !value.is_empty()
                && value.len() <= 200
                && value
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        };
        if self.schema_version != REFERENCE_APPEARANCE_PROJECTION_RECEIPT_SCHEMA_VERSION
            || !is_sha256(&self.source_request_sha256)
            || !is_sha256(&self.source_program_sha256)
            || !is_sha256(&self.final_glb_sha256)
            || !is_sha256(&self.compile_readback_sha256)
            || !is_sha256(&self.worker_receipt_sha256)
            || !is_sha256(&self.projection_sha256)
            || !is_sha256(&self.source_image_sha256)
            || !is_sha256(&self.camera_provenance_sha256)
            || !is_sha256(&self.base_color_sha256)
            || !is_sha256(&self.unobserved_texel_mask_sha256)
            || !valid_id(&self.projection_id)
            || !valid_id(&self.source_evidence_id)
            || !valid_id(&self.camera_hypothesis_id)
            || !valid_id(&self.target_material_zone_id)
            || !valid_id(&self.base_color_texture_id)
            || !valid_id(&self.unobserved_texel_mask_id)
            || self.worker_schema_version != "ReferenceCameraUvRasterBakeReceipt@2"
                && self.worker_schema_version != "ReferenceCameraUvRasterFusionReceipt@3"
            || self.algorithm_id != "forgecad.reference_camera_uv_raster"
            || self.algorithm_version != "1"
            || self.base_color_byte_size == 0
            || self.unobserved_texel_mask_byte_size == 0
            || self.observed_texel_count == 0
            || self.unobserved_texel_count == 0
            || self.fusion_count == 0
            || self.fusion_count > 2
            || self.observed_texel_count.saturating_add(self.unobserved_texel_count) == 0
        {
            return Err(invalid(
                "REFERENCE_APPEARANCE_PROJECTION_RECEIPT_INVALID",
                "Reference appearance projection receipt identity or bounded raster facts are invalid.",
            ));
        }
        let hashes = |values: &[String]| values.iter().all(|value| is_sha256(value));
        let ids = |values: &[String]| values.iter().all(|value| valid_id(value));
        if self.worker_schema_version == "ReferenceCameraUvRasterBakeReceipt@2" {
            if self.fusion_count != 1
                || self.raster_triangle_count.is_none()
                || self.world_to_clip_sha256.is_none()
                || !self.source_evidence_ids.is_empty()
                || !self.source_image_sha256s.is_empty()
                || !self.camera_hypothesis_ids.is_empty()
                || !self.camera_provenance_sha256s.is_empty()
                || !self.world_to_clip_sha256s.is_empty()
            {
                return Err(invalid(
                    "REFERENCE_APPEARANCE_PROJECTION_RECEIPT_INVALID",
                    "A single-view projection receipt must contain one camera raster fact and no fusion arrays.",
                ));
            }
        } else if self.fusion_count != 2
            || self.raster_triangle_count.is_none()
            || self.world_to_clip_sha256.is_some()
            || self.source_evidence_ids.len() != 2
            || self.source_image_sha256s.len() != 2
            || self.camera_hypothesis_ids.len() != 2
            || self.camera_provenance_sha256s.len() != 2
            || self.world_to_clip_sha256s.len() != 2
            || self.source_evidence_ids[0] != self.source_evidence_id
            || self.source_image_sha256s[0] != self.source_image_sha256
            || self.camera_hypothesis_ids[0] != self.camera_hypothesis_id
            || self.camera_provenance_sha256s[0] != self.camera_provenance_sha256
            || self.source_evidence_ids[0] == self.source_evidence_ids[1]
            || !ids(&self.source_evidence_ids)
            || !ids(&self.camera_hypothesis_ids)
            || !hashes(&self.source_image_sha256s)
            || !hashes(&self.camera_provenance_sha256s)
            || !hashes(&self.world_to_clip_sha256s)
        {
            return Err(invalid(
                "REFERENCE_APPEARANCE_PROJECTION_RECEIPT_INVALID",
                "A fused projection receipt must contain two distinct, hash-bound camera raster inputs.",
            ));
        }
        if self.raster_triangle_count == Some(0)
            || self.world_to_clip_sha256.as_ref().is_some_and(|value| !is_sha256(value))
        {
            return Err(invalid(
                "REFERENCE_APPEARANCE_PROJECTION_RECEIPT_INVALID",
                "Reference projection raster facts are outside the bounded hash/count contract.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppearanceEvidenceReference {
    pub evidence_id: String,
    pub evidence_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppearanceEvidenceBundle {
    pub schema_version: String,
    pub bundle_id: String,
    pub request_sha256: String,
    #[serde(default)]
    pub references: Vec<AppearanceEvidenceReference>,
    #[serde(default)]
    pub camera_hypotheses: Vec<ReferenceCameraHypothesis>,
    #[serde(default)]
    pub derived_artifacts: Vec<AppearanceEvidenceArtifact>,
    #[serde(default)]
    pub projection_receipts: Vec<ReferenceAppearanceProjectionReceipt>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UniversalDetailBindingKind {
    ProceduralProgram,
    GeometryOutput,
    MaterialZone,
    SurfaceProgram,
    ProjectionLayer,
    Unresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UniversalDetailBinding {
    pub kind: UniversalDetailBindingKind,
    pub source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualDetailClaimV2 {
    pub schema_version: String,
    pub claim_id: String,
    pub feature_id: String,
    pub level: crate::VisualFeatureLevel,
    pub evidence_status: EvidenceStatus,
    pub salience_bps: u16,
    pub affected_part_ids: Vec<String>,
    pub channels: Vec<AppearanceChannel>,
    pub silhouette_impact: bool,
    pub bindings: Vec<UniversalDetailBinding>,
    pub minimum_acceptance_views: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PbrTextureChannel {
    BaseColor,
    Metallic,
    Roughness,
    Normal,
    Occlusion,
    Emissive,
    Opacity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppearanceProjectionLayer {
    pub layer_id: String,
    pub evidence_artifact_id: String,
    pub camera_hypothesis_id: String,
    pub channels: Vec<PbrTextureChannel>,
    pub unobserved_texel_mask_artifact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MaterialZoneAppearance {
    pub schema_version: String,
    pub appearance_id: String,
    pub material_zone_id: String,
    pub source_part_id: String,
    pub base_material_id: String,
    pub finish: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coating: Option<String>,
    pub transmission_bps: u16,
    pub uncertainty_bps: u16,
    pub texture_width: u16,
    pub texture_height: u16,
    pub channels: Vec<PbrTextureChannel>,
    #[serde(default)]
    pub projection_layers: Vec<AppearanceProjectionLayer>,
}

/// One exact visible PBR zone compiled from the sealed universal source.
/// The worker can only consume this Rust-sealed lowering; providers cannot
/// independently add texture programs or choose material-zone bindings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GenericHardSurfaceAppearanceZone {
    pub target_subject_part_id: String,
    pub target_material_zone_id: String,
    pub base_material_id: String,
    pub surface_layer_program: SurfaceLayerProgram,
    pub surface_layer_program_sha256: String,
}

/// Rust-derived, low-dimensional appearance evidence from an exact sealed
/// image. It is intentionally not a texture or free RGB payload. The bounded
/// tokens are only fallback compiler hints: explicit observed/inferred
/// material traits remain higher priority, while the evidence/hash keeps the
/// reference-conditioned decision reproducible and auditable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReferenceSurfaceAppearanceBinding {
    pub schema_version: String,
    pub evidence_id: String,
    pub evidence_sha256: String,
    pub facts: ReferenceImageSurfaceFacts,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_color_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_finish_token: Option<String>,
    pub roughness_motif: String,
    pub binding_sha256: String,
}

/// A Rust-derived, bounded multi-zone appearance plan for the first
/// category-open executable representation. It retains at most eight
/// independently sealed exterior zones so shells, frames, trims and emissive
/// details can reach the GLB as distinct five-channel PBR material inputs.
/// This is still not texture projection or neural reconstruction: every zone
/// is bound to a reviewed local catalog material and a real procedural output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GenericHardSurfaceAppearanceCompilation {
    pub schema_version: String,
    pub compiler_id: String,
    pub source_program_sha256: String,
    pub zones: Vec<GenericHardSurfaceAppearanceZone>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_surface_bindings: Vec<ReferenceSurfaceAppearanceBinding>,
    pub compilation_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UniversalComponentSource {
    pub component_source_id: String,
    pub subject_part_id: String,
    pub representation: RepresentationKind,
    pub capability_id: String,
    pub source_program_id: String,
    pub source_program_sha256: String,
    pub source_program_part_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UniversalAssetSourceState {
    Planned,
    Compiled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UniversalCompiledArtifactBinding {
    pub source_program_sha256: String,
    pub shape_program_sha256: String,
    pub glb_sha256: String,
    pub readback_sha256: String,
    pub compile_readback_sha256: String,
    pub artifact_profile_id: String,
    pub renderer_id: String,
    pub view_sha256: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UniversalAssetSource {
    pub schema_version: String,
    pub source_id: String,
    pub state: UniversalAssetSourceState,
    pub request: UniversalAuthorRequest,
    pub request_sha256: String,
    pub subject_profile: SubjectProfile,
    pub subject_profile_sha256: String,
    pub visual_feature_contract: VisualFeatureContract,
    pub visual_feature_contract_sha256: String,
    pub representation_plan: RepresentationPlan,
    pub representation_plan_sha256: String,
    pub capability_manifest_sha256: String,
    pub procedural_source: ForgeVisualProgramRevision,
    pub component_sources: Vec<UniversalComponentSource>,
    pub detail_claims: Vec<VisualDetailClaimV2>,
    pub material_zones: Vec<MaterialZoneAppearance>,
    pub appearance_evidence: AppearanceEvidenceBundle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compiled_artifact: Option<UniversalCompiledArtifactBinding>,
}

/// The first executable mesh-seed slice is a local patch over a reviewed
/// ShapeProgram output. It does not carry external bytes or arbitrary vertex
/// data; the Worker replays the typed patch operation against the sealed
/// procedural source and proves the resulting GLB with its normal readback.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UniversalLocalMeshPatchSourceV2 {
    pub source_contract_id: String,
    pub procedural_source: UniversalProceduralSourceV2,
    pub patches: Vec<UniversalLocalMeshPatchBindingV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UniversalLocalMeshPatchBindingV2 {
    pub patch_id: String,
    pub subject_part_id: String,
    pub source_operation_id: String,
    pub patch_operation_id: String,
    pub patch_center: [f64; 3],
    pub patch_radius: f64,
    pub patch_offset: [f64; 3],
}

/// U004's source union.  The envelope deliberately contains data, source
/// hashes and compiler receipts rather than executable code.  Only the
/// procedural, local-lattice and the bounded local hard-surface hybrid
/// branches are executable in this slice; all remaining branches stay
/// reserved contracts so they cannot silently degrade into the arm program.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UniversalRepresentationSourceV2 {
    Procedural(UniversalProceduralSourceV2),
    Deformable(UniversalLocalLatticeDeformSourceV2),
    LocalMeshPatch(UniversalLocalMeshPatchSourceV2),
    Hybrid(UniversalLocalHardSurfaceHybridSourceV2),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UniversalProceduralSourceV2 {
    pub source_contract_id: String,
    pub compiler_profile_id: String,
    pub source_program: Value,
    pub source_program_id: String,
    pub source_program_sha256: String,
    pub shape_program_sha256: String,
    pub shape_program: Value,
    pub part_bindings: Vec<UniversalProceduralPartBindingV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UniversalProceduralPartBindingV2 {
    pub subject_part_id: String,
    pub output_id: String,
    pub terminal_operation_id: String,
    pub material_zone_id: String,
    pub material_id: String,
}

/// Rust-derived source contract for the first local deformable representation.
/// The nested runtime source remains the same bounded ShapeProgram truth used
/// by the geometry worker; the additional bindings prove that every output is
/// actually backed by a 2x2x2 `lattice_deform`, rather than merely labelling a
/// procedural model as deformable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UniversalLocalLatticeDeformSourceV2 {
    pub source_contract_id: String,
    pub procedural_source: UniversalProceduralSourceV2,
    pub deformations: Vec<UniversalLatticeDeformationBindingV2>,
}

/// The first executable hybrid representation remains deliberately local and
/// bounded: selected hard-surface parts retain their exact procedural
/// terminals while other declared parts end in a verified 2x2x2 lattice
/// deformation.  It is not a mesh-seed merge, boolean import, or free-form
/// mesh editing capability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UniversalLocalHardSurfaceHybridSourceV2 {
    pub source_contract_id: String,
    pub procedural_source: UniversalProceduralSourceV2,
    pub deformations: Vec<UniversalLatticeDeformationBindingV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UniversalLatticeDeformationBindingV2 {
    pub deformation_id: String,
    pub subject_part_id: String,
    pub source_operation_id: String,
    pub deformation_operation_id: String,
    pub corner_offsets: [[f64; 3]; 8],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UniversalAssetSourceV2 {
    pub schema_version: String,
    pub source_id: String,
    pub state: UniversalAssetSourceState,
    pub request: UniversalAuthorRequest,
    pub request_sha256: String,
    pub subject_profile: SubjectProfile,
    pub subject_profile_sha256: String,
    pub visual_feature_contract: VisualFeatureContract,
    pub visual_feature_contract_sha256: String,
    pub representation_plan: RepresentationPlan,
    pub representation_plan_sha256: String,
    pub capability_manifest_sha256: String,
    pub representation_source: UniversalRepresentationSourceV2,
    pub component_sources: Vec<UniversalComponentSource>,
    pub detail_claims: Vec<VisualDetailClaimV2>,
    pub material_zones: Vec<MaterialZoneAppearance>,
    pub appearance_compilation: GenericHardSurfaceAppearanceCompilation,
    pub appearance_evidence: AppearanceEvidenceBundle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_asset_profile: Option<crate::GameAssetProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compiled_artifact: Option<UniversalCompiledArtifactBinding>,
    /// A Rust-derived receipt for the exportable game-delivery GLB. The
    /// compiled artifact remains the exact LOD0/source GLB used for visual
    /// comparison; this receipt proves that the preview/export GLB was
    /// deterministically derived from it with LODs, collision proxies,
    /// sockets and measured PBR density.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_asset_delivery: Option<crate::GameAssetDeliveryReadback>,
}

impl UniversalRepresentationSourceV2 {
    fn runtime_procedural(&self) -> CoreResult<&UniversalProceduralSourceV2> {
        match self {
            Self::Procedural(source) => Ok(source),
            Self::Deformable(source) => Ok(&source.procedural_source),
            Self::Hybrid(source) => Ok(&source.procedural_source),
            Self::LocalMeshPatch(source) => Ok(&source.procedural_source),
        }
    }

    fn validate(&self) -> CoreResult<()> {
        match self {
            Self::Procedural(source) => source.validate(),
            Self::Deformable(source) => source.validate(),
            Self::Hybrid(source) => source.validate(),
            Self::LocalMeshPatch(source) => source.validate(),
        }
    }

    fn representation_kind(&self) -> RepresentationKind {
        match self {
            Self::Procedural(_) => RepresentationKind::Procedural,
            Self::Deformable(_) => RepresentationKind::Deformable,
            Self::LocalMeshPatch(_) => RepresentationKind::MeshSeed,
            Self::Hybrid(_) => RepresentationKind::Hybrid,
        }
    }
}

impl UniversalProceduralSourceV2 {
    fn validate(&self) -> CoreResult<()> {
        let lowering = lower_visual_runtime_source_v1(&self.source_program)?;
        let source_program_id = self
            .source_program
            .get("program_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                invalid(
                    "UNIVERSAL_V2_SOURCE_PROGRAM_ID_INVALID",
                    "source program requires a bounded program_id",
                )
            })?;
        if self.source_contract_id != lowering.source_contract_id
            || self.compiler_profile_id != lowering.compiler_profile_id
            || self.source_program_id != source_program_id
            || self.source_program_sha256 != lowering.source_program_sha256
            || self.shape_program_sha256 != lowering.shape_program_sha256
            || self.shape_program != lowering.shape_program
        {
            return Err(invalid(
                "UNIVERSAL_V2_PROCEDURAL_LINEAGE_INVALID",
                "procedural source must exactly reproduce its reviewed runtime lowering",
            ));
        }
        let operations = self
            .shape_program
            .get("operations")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                invalid(
                    "UNIVERSAL_V2_SHAPE_PROGRAM_INVALID",
                    "lowered ShapeProgram operations are missing",
                )
            })?;
        let operation_args = operations
            .iter()
            .filter_map(|operation| {
                Some((
                    operation.get("operation_id")?.as_str()?,
                    operation.get("args")?.as_object()?,
                ))
            })
            .collect::<BTreeMap<_, _>>();
        let outputs = self
            .shape_program
            .get("outputs")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                invalid(
                    "UNIVERSAL_V2_SHAPE_PROGRAM_INVALID",
                    "lowered ShapeProgram outputs are missing",
                )
            })?;
        let mut output_ids = BTreeSet::new();
        let mut covered_operations = BTreeSet::new();
        let mut subject_parts = BTreeSet::new();
        for binding in &self.part_bindings {
            let output = outputs.iter().find(|output| {
                output.get("output_id").and_then(Value::as_str) == Some(binding.output_id.as_str())
            });
            let Some(output) = output else {
                return Err(invalid(
                    "UNIVERSAL_V2_PART_BINDING_INVALID",
                    "part binding references no lowered output",
                ));
            };
            let terminal_operation_id = output.get("operation_id").and_then(Value::as_str);
            let args = operation_args.get(binding.terminal_operation_id.as_str());
            if !output_ids.insert(binding.output_id.as_str())
                || !covered_operations.insert(binding.terminal_operation_id.as_str())
                || !subject_parts.insert(binding.subject_part_id.as_str())
                || terminal_operation_id != Some(binding.terminal_operation_id.as_str())
                || args
                    .and_then(|args| args.get("zone_id"))
                    .and_then(Value::as_str)
                    != Some(binding.material_zone_id.as_str())
                || args
                    .and_then(|args| args.get("material_id"))
                    .and_then(Value::as_str)
                    != Some(binding.material_id.as_str())
            {
                return Err(invalid(
                    "UNIVERSAL_V2_PART_BINDING_INVALID",
                    "part binding must uniquely bind a real output, operation and material zone",
                ));
            }
        }
        if output_ids.len() != outputs.len() || self.part_bindings.is_empty() {
            return Err(invalid(
                "UNIVERSAL_V2_PART_BINDING_INCOMPLETE",
                "every lowered procedural output requires exactly one subject-part binding",
            ));
        }
        Ok(())
    }
}

impl UniversalLocalMeshPatchSourceV2 {
    fn validate(&self) -> CoreResult<()> {
        if self.source_contract_id != "ForgeLocalMeshPatchSource@1" {
            return Err(invalid(
                "UNIVERSAL_LOCAL_MESH_PATCH_SOURCE_CONTRACT_INVALID",
                "local mesh patch source must use ForgeLocalMeshPatchSource@1",
            ));
        }
        self.procedural_source.validate()?;
        let operations = self
            .procedural_source
            .shape_program
            .get("operations")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                invalid(
                    "UNIVERSAL_LOCAL_MESH_PATCH_SOURCE_INVALID",
                    "local mesh patch source requires lowered ShapeProgram operations",
                )
            })?
            .iter()
            .filter_map(|operation| {
                Some((
                    operation.get("operation_id")?.as_str()?,
                    (
                        operation.get("op")?.as_str()?,
                        operation.get("inputs")?.as_array()?,
                        operation.get("args")?.as_object()?,
                    ),
                ))
            })
            .collect::<BTreeMap<_, _>>();
        let expected_parts = self
            .procedural_source
            .part_bindings
            .iter()
            .map(|binding| binding.subject_part_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut ids = BTreeSet::new();
        let mut operation_ids = BTreeSet::new();
        let mut part_ids = BTreeSet::new();
        for patch in &self.patches {
            let Some((op, inputs, args)) = operations.get(patch.patch_operation_id.as_str())
                .map(|(op, inputs, args)| (*op, *inputs, *args)) else {
                return Err(invalid(
                    "UNIVERSAL_LOCAL_MESH_PATCH_OPERATION_INVALID",
                    "local mesh patch binding references no lowered operation",
                ));
            };
            let source_input = inputs.first().and_then(Value::as_str);
            if !ids.insert(patch.patch_id.as_str())
                || !operation_ids.insert(patch.patch_operation_id.as_str())
                || !part_ids.insert(patch.subject_part_id.as_str())
                || op != "local_mesh_patch"
                || inputs.len() != 1
                || source_input != Some(patch.source_operation_id.as_str())
                || args.get("patch_center") != Some(&serde_json::json!(patch.patch_center))
                || args.get("patch_radius") != Some(&serde_json::json!(patch.patch_radius))
                || args.get("patch_offset") != Some(&serde_json::json!(patch.patch_offset))
                || patch.patch_center.iter().any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
                || !patch.patch_radius.is_finite()
                || !(0.05..=0.4).contains(&patch.patch_radius)
                || patch.patch_offset.iter().any(|value| !value.is_finite() || value.abs() > 0.2)
                || !patch.patch_offset.iter().any(|value| value.abs() > 1e-9)
            {
                return Err(invalid(
                    "UNIVERSAL_LOCAL_MESH_PATCH_BINDING_INVALID",
                    "each local mesh patch binding must exactly reproduce one bounded lowered operation",
                ));
            }
        }
        if self.patches.is_empty() || part_ids != expected_parts {
            return Err(invalid(
                "UNIVERSAL_LOCAL_MESH_PATCH_BINDING_INCOMPLETE",
                "every local mesh-seed subject part requires exactly one patch binding",
            ));
        }
        for part in &self.procedural_source.part_bindings {
            let patch = self
                .patches
                .iter()
                .find(|item| item.subject_part_id == part.subject_part_id)
                .ok_or_else(|| {
                    invalid(
                        "UNIVERSAL_LOCAL_MESH_PATCH_BINDING_INCOMPLETE",
                        "every procedural output requires a local mesh patch binding",
                    )
                })?;
            if part.terminal_operation_id != patch.patch_operation_id {
                return Err(invalid(
                    "UNIVERSAL_LOCAL_MESH_PATCH_TERMINAL_INVALID",
                    "local mesh patch operation must remain the exact output terminal",
                ));
            }
        }
        Ok(())
    }
}

impl UniversalLocalLatticeDeformSourceV2 {
    fn validate(&self) -> CoreResult<()> {
        if self.source_contract_id != "ForgeLocalLatticeDeformSource@1" {
            return Err(invalid(
                "UNIVERSAL_LATTICE_SOURCE_CONTRACT_INVALID",
                "local lattice deformation must use ForgeLocalLatticeDeformSource@1",
            ));
        }
        self.procedural_source.validate()?;
        let expected_parts = self
            .procedural_source
            .part_bindings
            .iter()
            .map(|binding| binding.subject_part_id.as_str())
            .collect::<BTreeSet<_>>();
        validate_lattice_deformations(
            &self.procedural_source,
            &self.deformations,
            &expected_parts,
            "UNIVERSAL_LATTICE_BINDING_INCOMPLETE",
            "every local-deformable subject part requires exactly one lattice binding",
        )
    }
}

impl UniversalLocalHardSurfaceHybridSourceV2 {
    fn validate(&self) -> CoreResult<()> {
        if self.source_contract_id != "ForgeLocalHardSurfaceHybridSource@1" {
            return Err(invalid(
                "UNIVERSAL_HYBRID_SOURCE_CONTRACT_INVALID",
                "local hard-surface hybrid must use ForgeLocalHardSurfaceHybridSource@1",
            ));
        }
        self.procedural_source.validate()?;
        let expected_lattice_parts = self
            .deformations
            .iter()
            .map(|binding| binding.subject_part_id.as_str())
            .collect::<BTreeSet<_>>();
        let all_parts = self
            .procedural_source
            .part_bindings
            .iter()
            .map(|binding| binding.subject_part_id.as_str())
            .collect::<BTreeSet<_>>();
        if expected_lattice_parts.is_empty() || expected_lattice_parts.len() == all_parts.len() {
            return Err(invalid(
                "UNIVERSAL_HYBRID_COMPOSITION_INVALID",
                "local hard-surface hybrid requires both procedural and lattice-deformed parts",
            ));
        }
        validate_lattice_deformations(
            &self.procedural_source,
            &self.deformations,
            &expected_lattice_parts,
            "UNIVERSAL_HYBRID_LATTICE_BINDING_INVALID",
            "hybrid lattice bindings must cover exactly the declared deformable parts",
        )
    }
}

fn validate_lattice_deformations(
    procedural_source: &UniversalProceduralSourceV2,
    deformations: &[UniversalLatticeDeformationBindingV2],
    expected_lattice_parts: &BTreeSet<&str>,
    incomplete_code: &'static str,
    incomplete_message: &'static str,
) -> CoreResult<()> {
    let operations = procedural_source
        .shape_program
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid(
                "UNIVERSAL_LATTICE_SOURCE_INVALID",
                "local lattice source requires lowered ShapeProgram operations",
            )
        })?
        .iter()
        .filter_map(|operation| {
            Some((
                operation.get("operation_id")?.as_str()?,
                (
                    operation.get("op")?.as_str()?,
                    operation.get("inputs")?.as_array()?,
                    operation.get("args")?.as_object()?,
                ),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    let mut bound_parts = BTreeSet::new();
    let mut deformation_ids = BTreeSet::new();
    let mut deformation_operations = BTreeSet::new();
    for binding in deformations {
        let Some((op, inputs, args)) = operations
            .get(binding.deformation_operation_id.as_str())
            .map(|(op, inputs, args)| (*op, *inputs, *args))
        else {
            return Err(invalid(
                "UNIVERSAL_LATTICE_OPERATION_INVALID",
                "local lattice binding references no lowered operation",
            ));
        };
        let source_input = inputs.first().and_then(Value::as_str);
        let offsets = args.get("corner_offsets");
        if !deformation_ids.insert(binding.deformation_id.as_str())
            || !bound_parts.insert(binding.subject_part_id.as_str())
            || !deformation_operations.insert(binding.deformation_operation_id.as_str())
            || op != "lattice_deform"
            || inputs.len() != 1
            || source_input != Some(binding.source_operation_id.as_str())
            || offsets != Some(&serde_json::json!(binding.corner_offsets))
            || binding
                .corner_offsets
                .iter()
                .flatten()
                .any(|value| !value.is_finite() || value.abs() > 0.25)
            || !binding
                .corner_offsets
                .iter()
                .flatten()
                .any(|value| value.abs() > 1e-9)
        {
            return Err(invalid(
                "UNIVERSAL_LATTICE_BINDING_INVALID",
                "each local lattice binding must exactly reproduce one bounded lowered lattice operation",
            ));
        }
    }
    if deformations.is_empty() || bound_parts != *expected_lattice_parts {
        return Err(invalid(incomplete_code, incomplete_message));
    }
    for part in &procedural_source.part_bindings {
        let terminal_operation = operations
            .get(part.terminal_operation_id.as_str())
            .map(|(op, _, _)| *op)
            .ok_or_else(|| {
                invalid(
                    "UNIVERSAL_LATTICE_OPERATION_INVALID",
                    "part terminal operation is missing from lowered ShapeProgram",
                )
            })?;
        if expected_lattice_parts.contains(part.subject_part_id.as_str()) {
            let binding = deformations
                .iter()
                .find(|item| item.subject_part_id == part.subject_part_id)
                .expect("exact lattice coverage was checked above");
            if part.terminal_operation_id != binding.deformation_operation_id {
                return Err(invalid(
                    "UNIVERSAL_LATTICE_TERMINAL_INVALID",
                    "the local lattice operation must remain the exact output terminal",
                ));
            }
        } else if terminal_operation == "lattice_deform" {
            return Err(invalid(
                "UNIVERSAL_HYBRID_PROCEDURAL_TERMINAL_INVALID",
                "a hybrid procedural part cannot silently terminate in a lattice deformation",
            ));
        }
    }
    Ok(())
}

impl UniversalAssetSource {
    pub fn from_procedural(
        request: &UniversalAuthorRequest,
        profile: &SubjectProfile,
        feature_contract: &VisualFeatureContract,
        representation_plan: &RepresentationPlan,
        revision: &ForgeVisualProgramRevision,
    ) -> CoreResult<Self> {
        representation_plan.validate_against(request, profile, feature_contract)?;
        revision.validate()?;
        let program_part_ids = revision
            .program
            .parts
            .iter()
            .map(|part| part.part_id.clone())
            .collect::<Vec<_>>();
        if program_part_ids.is_empty() {
            return Err(invalid(
                "UNIVERSAL_SOURCE_PROGRAM_PARTS_EMPTY",
                "Procedural source must expose stable program Part identities.",
            ));
        }
        let component_sources = representation_plan
            .parts
            .iter()
            .map(|part| UniversalComponentSource {
                component_source_id: format!("component_source_{}", part.part_id),
                subject_part_id: part.part_id.clone(),
                representation: part.representation,
                capability_id: part.capability_id.clone(),
                source_program_id: revision.program.program_id.clone(),
                source_program_sha256: revision.source_program_sha256.clone(),
                source_program_part_ids: program_part_ids.clone(),
            })
            .collect::<Vec<_>>();

        let material_zone_ids = revision
            .program
            .material_graph
            .iter()
            .map(|binding| material_zone_key(&binding.part_id, &binding.material_zone_id))
            .collect::<BTreeSet<_>>();
        let detail_claims =
            feature_contract
                .requirements
                .iter()
                .map(|requirement| {
                    let detail_ids = revision
                        .program
                        .detail_inventory
                        .iter()
                        .filter(|detail| visual_levels_match(detail.level, requirement.level))
                        .map(|detail| detail.detail_id.clone())
                        .collect::<Vec<_>>();
                    if detail_ids.is_empty() {
                        return Err(invalid(
                        "UNIVERSAL_DETAIL_SOURCE_UNRESOLVED",
                        "Every visual feature level must bind to a real procedural detail source.",
                    ));
                    }
                    let mut bindings = vec![UniversalDetailBinding {
                        kind: UniversalDetailBindingKind::ProceduralProgram,
                        source_id: revision.program.program_id.clone(),
                    }];
                    bindings.extend(detail_ids.into_iter().map(|source_id| {
                        UniversalDetailBinding {
                            kind: UniversalDetailBindingKind::GeometryOutput,
                            source_id,
                        }
                    }));
                    if requirement
                        .channels
                        .iter()
                        .any(|channel| !matches!(channel, AppearanceChannel::Geometry))
                    {
                        bindings.extend(material_zone_ids.iter().cloned().map(|source_id| {
                            UniversalDetailBinding {
                                kind: UniversalDetailBindingKind::MaterialZone,
                                source_id,
                            }
                        }));
                    }
                    Ok(VisualDetailClaimV2 {
                        schema_version: VISUAL_DETAIL_CLAIM_V2_SCHEMA_VERSION.into(),
                        claim_id: format!("detail_claim_{}", requirement.feature_id),
                        feature_id: requirement.feature_id.clone(),
                        level: requirement.level,
                        evidence_status: requirement.evidence_status,
                        salience_bps: requirement.salience_bps,
                        affected_part_ids: requirement.affected_part_ids.clone(),
                        channels: requirement.channels.clone(),
                        silhouette_impact: requirement.level == crate::VisualFeatureLevel::Macro
                            && requirement
                                .channels
                                .iter()
                                .any(|channel| *channel == AppearanceChannel::Geometry),
                        bindings,
                        minimum_acceptance_views: requirement.minimum_acceptance_views.clone(),
                    })
                })
                .collect::<CoreResult<Vec<_>>>()?;

        let material_zones = revision
            .program
            .material_graph
            .iter()
            .map(|binding| MaterialZoneAppearance {
                schema_version: MATERIAL_ZONE_APPEARANCE_SCHEMA_VERSION.into(),
                appearance_id: material_zone_key(&binding.part_id, &binding.material_zone_id),
                material_zone_id: binding.material_zone_id.clone(),
                source_part_id: binding.part_id.clone(),
                base_material_id: binding.material_id.clone(),
                finish: "reviewed_catalog_pbr".into(),
                coating: None,
                transmission_bps: 0,
                uncertainty_bps: if request.reference_inputs.is_empty() {
                    5_000
                } else {
                    7_500
                },
                texture_width: 1024,
                texture_height: 1024,
                channels: vec![
                    PbrTextureChannel::BaseColor,
                    PbrTextureChannel::Metallic,
                    PbrTextureChannel::Roughness,
                    PbrTextureChannel::Normal,
                    PbrTextureChannel::Occlusion,
                    PbrTextureChannel::Emissive,
                ],
                projection_layers: Vec::new(),
            })
            .collect::<Vec<_>>();

        let request_sha256 = semantic_sha256(request)?;
        let appearance_evidence = AppearanceEvidenceBundle {
            schema_version: APPEARANCE_EVIDENCE_BUNDLE_SCHEMA_VERSION.into(),
            bundle_id: format!("appearance_evidence_{}", &request_sha256[..24]),
            request_sha256: request_sha256.clone(),
            references: request
                .reference_inputs
                .iter()
                .map(|reference| AppearanceEvidenceReference {
                    evidence_id: reference.evidence_id.clone(),
                    evidence_sha256: reference.evidence_sha256.clone(),
                })
                .collect(),
            camera_hypotheses: request
                .reference_inputs
                .iter()
                .map(|reference| ReferenceCameraHypothesis {
                    schema_version: REFERENCE_CAMERA_HYPOTHESIS_SCHEMA_VERSION.into(),
                    hypothesis_id: format!("camera_hypothesis_{}", reference.evidence_id),
                    evidence_id: reference.evidence_id.clone(),
                    view_id: reference.view_hint.clone(),
                    projection_type: ReferenceProjectionType::Unknown,
                    parameter_source: CameraParameterSource::Unresolved,
                    vertical_fov_millidegrees: None,
                    reprojection_error_bps: None,
                    landmark_feature_ids: Vec::new(),
                    confidence_bps: 0,
                    unresolved_fields: vec![
                        "projection_type".into(),
                        "extrinsics".into(),
                        "intrinsics".into(),
                    ],
                })
                .collect(),
            derived_artifacts: Vec::new(),
            projection_receipts: Vec::new(),
        };
        let source = Self {
            schema_version: UNIVERSAL_ASSET_SOURCE_SCHEMA_VERSION.into(),
            source_id: format!("universal_source_{}", &request_sha256[..24]),
            state: UniversalAssetSourceState::Planned,
            request: request.clone(),
            request_sha256,
            subject_profile: profile.clone(),
            subject_profile_sha256: semantic_sha256(profile)?,
            visual_feature_contract: feature_contract.clone(),
            visual_feature_contract_sha256: semantic_sha256(feature_contract)?,
            representation_plan: representation_plan.clone(),
            representation_plan_sha256: semantic_sha256(representation_plan)?,
            capability_manifest_sha256: request.capability_manifest_sha256.clone(),
            procedural_source: revision.clone(),
            component_sources,
            detail_claims,
            material_zones,
            appearance_evidence,
            compiled_artifact: None,
        };
        source.validate()?;
        Ok(source)
    }

    pub fn with_compiled_artifact(
        mut self,
        binding: UniversalCompiledArtifactBinding,
    ) -> CoreResult<Self> {
        self.state = UniversalAssetSourceState::Compiled;
        self.compiled_artifact = Some(binding);
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != UNIVERSAL_ASSET_SOURCE_SCHEMA_VERSION
            || self.request_sha256 != semantic_sha256(&self.request)?
            || self.subject_profile_sha256 != semantic_sha256(&self.subject_profile)?
            || self.visual_feature_contract_sha256
                != semantic_sha256(&self.visual_feature_contract)?
            || self.representation_plan_sha256 != semantic_sha256(&self.representation_plan)?
            || self.capability_manifest_sha256 != self.request.capability_manifest_sha256
        {
            return Err(invalid(
                "UNIVERSAL_ASSET_SOURCE_LINEAGE_INVALID",
                "Universal asset source must bind the exact request/profile/feature/plan lineage.",
            ));
        }
        self.representation_plan.validate_against(
            &self.request,
            &self.subject_profile,
            &self.visual_feature_contract,
        )?;
        self.procedural_source.validate()?;
        if self.component_sources.is_empty()
            || self.component_sources.len() > MAX_COMPONENTS
            || self.detail_claims.is_empty()
            || self.detail_claims.len() > MAX_DETAIL_CLAIMS
            || self.material_zones.len() > MAX_MATERIAL_ZONES
        {
            return Err(invalid(
                "UNIVERSAL_ASSET_SOURCE_BOUNDS_INVALID",
                "Universal source component/detail/material collections are outside bounds.",
            ));
        }
        validate_appearance_evidence(&self.appearance_evidence, &self.request)?;
        validate_components(self)?;
        validate_detail_claims(self)?;
        validate_material_zones(self)?;
        match (self.state, self.compiled_artifact.as_ref()) {
            (UniversalAssetSourceState::Planned, None) => {}
            (UniversalAssetSourceState::Compiled, Some(binding)) => {
                validate_compiled_binding(binding, &self.procedural_source)?;
            }
            _ => return Err(invalid(
                "UNIVERSAL_ASSET_SOURCE_STATE_INVALID",
                "Compiled artifact lineage must be complete or absent according to source state.",
            )),
        }
        Ok(())
    }
}

impl UniversalAssetSourceV2 {
    /// Returns the exact reviewed local program underlying an executable UAS@2
    /// representation. Mesh-seed remains unavailable; Hybrid is only the
    /// bounded mix of this local procedural and lattice source, never an
    /// arbitrary mesh merge.
    pub fn runtime_procedural(&self) -> CoreResult<&UniversalProceduralSourceV2> {
        self.representation_source.runtime_procedural()
    }

    /// Recompiles only the Rust-owned appearance layer from exact sealed
    /// image observations. Geometry, SubjectProfile, feature contract and
    /// representation choice remain unchanged. The method is used after the
    /// app-server has validated the evidence lineage; it is intentionally
    /// unavailable to provider payloads.
    pub fn with_reference_surface_facts(
        mut self,
        evidence: &[ReferenceEvidence],
    ) -> CoreResult<Self> {
        let bindings = derive_reference_surface_appearance_bindings(&self, evidence)?;
        let scoped_bindings = crate::derive_reference_appearance_bindings(&self, evidence)?;
        let procedural = self.runtime_procedural()?.clone();
        self.appearance_compilation = compile_generic_hard_surface_appearance(
            &self.subject_profile,
            &self.visual_feature_contract,
            &procedural,
            &self.material_zones,
            &bindings,
            &scoped_bindings,
        )?;
        self.validate()?;
        Ok(self)
    }

    /// Rust constructs this source only after independently lowering the
    /// supplied bounded program.  The Provider cannot submit UAS@2 directly.
    pub fn from_runtime_procedural(
        request: &UniversalAuthorRequest,
        profile: &SubjectProfile,
        feature_contract: &VisualFeatureContract,
        representation_plan: &RepresentationPlan,
        source_program: Value,
    ) -> CoreResult<Self> {
        Self::from_runtime_with_representation(
            request,
            profile,
            feature_contract,
            representation_plan,
            source_program,
            RepresentationKind::Procedural,
        )
    }

    /// Creates the first executable local-deformable UAS branch.  It reuses
    /// the reviewed runtime lowering but refuses a program whose outputs are
    /// not terminal bounded lattice deformations.
    pub fn from_runtime_local_lattice(
        request: &UniversalAuthorRequest,
        profile: &SubjectProfile,
        feature_contract: &VisualFeatureContract,
        representation_plan: &RepresentationPlan,
        source_program: Value,
    ) -> CoreResult<Self> {
        Self::from_runtime_with_representation(
            request,
            profile,
            feature_contract,
            representation_plan,
            source_program,
            RepresentationKind::Deformable,
        )
    }

    /// Creates the first per-part executable source mix.  The plan, not the
    /// Provider, determines which parts retain a procedural terminal and
    /// which must terminate in a reviewed local 2x2x2 lattice deformation.
    pub fn from_runtime_local_hybrid(
        request: &UniversalAuthorRequest,
        profile: &SubjectProfile,
        feature_contract: &VisualFeatureContract,
        representation_plan: &RepresentationPlan,
        source_program: Value,
    ) -> CoreResult<Self> {
        Self::from_runtime_with_representation(
            request,
            profile,
            feature_contract,
            representation_plan,
            source_program,
            RepresentationKind::Hybrid,
        )
    }

    /// Creates the first executable local mesh-seed source. Every declared
    /// part must terminate in one bounded `local_mesh_patch` operation over
    /// the reviewed local procedural output.
    pub fn from_runtime_local_mesh_patch(
        request: &UniversalAuthorRequest,
        profile: &SubjectProfile,
        feature_contract: &VisualFeatureContract,
        representation_plan: &RepresentationPlan,
        source_program: Value,
    ) -> CoreResult<Self> {
        Self::from_runtime_with_representation(
            request,
            profile,
            feature_contract,
            representation_plan,
            source_program,
            RepresentationKind::MeshSeed,
        )
    }

    fn from_runtime_with_representation(
        request: &UniversalAuthorRequest,
        profile: &SubjectProfile,
        feature_contract: &VisualFeatureContract,
        representation_plan: &RepresentationPlan,
        source_program: Value,
        representation: RepresentationKind,
    ) -> CoreResult<Self> {
        representation_plan.validate_against(request, profile, feature_contract)?;
        validate_runtime_representation_plan(representation_plan, representation)?;
        let source_program =
            normalize_generic_hard_surface_material_bases(profile, source_program)?;
        let lowering = lower_visual_runtime_source_v1(&source_program)?;
        let source_program_id = source_program
            .get("program_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                invalid(
                    "UNIVERSAL_V2_SOURCE_PROGRAM_ID_INVALID",
                    "runtime source requires program_id",
                )
            })?
            .to_string();
        let outputs = lowering
            .shape_program
            .get("outputs")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                invalid(
                    "UNIVERSAL_V2_SHAPE_PROGRAM_INVALID",
                    "runtime lowering has no outputs",
                )
            })?;
        if outputs.len() != profile.parts.len() || outputs.is_empty() {
            return Err(invalid(
                "UNIVERSAL_V2_PART_BINDING_CARDINALITY",
                "runtime procedural output count must exactly match SubjectProfile parts",
            ));
        }
        let operations = lowering
            .shape_program
            .get("operations")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                invalid(
                    "UNIVERSAL_V2_SHAPE_PROGRAM_INVALID",
                    "runtime lowering has no operations",
                )
            })?;
        let operation_args = operations
            .iter()
            .filter_map(|operation| {
                Some((
                    operation.get("operation_id")?.as_str()?,
                    operation.get("args")?.as_object()?,
                ))
            })
            .collect::<BTreeMap<_, _>>();
        let part_bindings = profile
            .parts
            .iter()
            .zip(outputs)
            .map(|(part, output)| {
                let output_id =
                    output
                        .get("output_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            invalid(
                                "UNIVERSAL_V2_SHAPE_PROGRAM_INVALID",
                                "runtime output has no output_id",
                            )
                        })?;
                let terminal_operation_id = output
                    .get("operation_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        invalid(
                            "UNIVERSAL_V2_SHAPE_PROGRAM_INVALID",
                            "runtime output has no operation_id",
                        )
                    })?;
                let args = operation_args.get(terminal_operation_id).ok_or_else(|| {
                    invalid(
                        "UNIVERSAL_V2_SHAPE_PROGRAM_INVALID",
                        "runtime output operation is missing",
                    )
                })?;
                let material_zone_id =
                    args.get("zone_id").and_then(Value::as_str).ok_or_else(|| {
                        invalid(
                            "UNIVERSAL_V2_MATERIAL_ZONE_REQUIRED",
                            "runtime output requires a material zone",
                        )
                    })?;
                let material_id =
                    args.get("material_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            invalid(
                                "UNIVERSAL_V2_MATERIAL_ZONE_REQUIRED",
                                "runtime output requires a material",
                            )
                        })?;
                Ok(UniversalProceduralPartBindingV2 {
                    subject_part_id: part.part_id.clone(),
                    output_id: output_id.to_string(),
                    terminal_operation_id: terminal_operation_id.to_string(),
                    material_zone_id: material_zone_id.to_string(),
                    material_id: material_id.to_string(),
                })
            })
            .collect::<CoreResult<Vec<_>>>()?;
        let procedural_source = UniversalProceduralSourceV2 {
            source_contract_id: lowering.source_contract_id,
            compiler_profile_id: lowering.compiler_profile_id,
            source_program,
            source_program_id,
            source_program_sha256: lowering.source_program_sha256,
            shape_program_sha256: lowering.shape_program_sha256,
            shape_program: lowering.shape_program,
            part_bindings,
        };
        let request_sha256 = semantic_sha256(request)?;
        let component_sources = representation_plan
            .parts
            .iter()
            .map(|part| UniversalComponentSource {
                component_source_id: format!("component_source_{}", part.part_id),
                subject_part_id: part.part_id.clone(),
                representation: part.representation,
                capability_id: part.capability_id.clone(),
                source_program_id: procedural_source.source_program_id.clone(),
                source_program_sha256: procedural_source.source_program_sha256.clone(),
                source_program_part_ids: procedural_source
                    .part_bindings
                    .iter()
                    .filter(|binding| binding.subject_part_id == part.part_id)
                    .map(|binding| binding.output_id.clone())
                    .collect(),
            })
            .collect::<Vec<_>>();
        let detail_claims = feature_contract
            .requirements
            .iter()
            .map(|requirement| {
                let bindings = procedural_source
                    .part_bindings
                    .iter()
                    .filter(|binding| {
                        requirement
                            .affected_part_ids
                            .contains(&binding.subject_part_id)
                    })
                    .collect::<Vec<_>>();
                if bindings.is_empty() {
                    return Err(invalid(
                        "UNIVERSAL_V2_DETAIL_PART_UNBOUND",
                        "every visual requirement must affect a procedurally bound part",
                    ));
                }
                let mut source_bindings = vec![UniversalDetailBinding {
                    kind: UniversalDetailBindingKind::ProceduralProgram,
                    source_id: procedural_source.source_program_id.clone(),
                }];
                source_bindings.extend(bindings.iter().map(|binding| UniversalDetailBinding {
                    kind: UniversalDetailBindingKind::GeometryOutput,
                    source_id: binding.terminal_operation_id.clone(),
                }));
                if requirement
                    .channels
                    .iter()
                    .any(|channel| *channel != AppearanceChannel::Geometry)
                {
                    source_bindings.extend(bindings.iter().map(|binding| UniversalDetailBinding {
                        kind: UniversalDetailBindingKind::MaterialZone,
                        source_id: material_zone_key(
                            &binding.subject_part_id,
                            &binding.material_zone_id,
                        ),
                    }));
                }
                Ok(VisualDetailClaimV2 {
                    schema_version: VISUAL_DETAIL_CLAIM_V2_SCHEMA_VERSION.into(),
                    claim_id: format!("detail_claim_{}", requirement.feature_id),
                    feature_id: requirement.feature_id.clone(),
                    level: requirement.level,
                    evidence_status: requirement.evidence_status,
                    salience_bps: requirement.salience_bps,
                    affected_part_ids: requirement.affected_part_ids.clone(),
                    channels: requirement.channels.clone(),
                    silhouette_impact: requirement.level == crate::VisualFeatureLevel::Macro
                        && requirement.channels.contains(&AppearanceChannel::Geometry),
                    bindings: source_bindings,
                    minimum_acceptance_views: requirement.minimum_acceptance_views.clone(),
                })
            })
            .collect::<CoreResult<Vec<_>>>()?;
        let material_zones = procedural_source
            .part_bindings
            .iter()
            .map(|binding| MaterialZoneAppearance {
                schema_version: MATERIAL_ZONE_APPEARANCE_SCHEMA_VERSION.into(),
                appearance_id: material_zone_key(
                    &binding.subject_part_id,
                    &binding.material_zone_id,
                ),
                material_zone_id: binding.material_zone_id.clone(),
                source_part_id: binding.subject_part_id.clone(),
                base_material_id: binding.material_id.clone(),
                finish: "reviewed_catalog_pbr".into(),
                coating: None,
                transmission_bps: 0,
                uncertainty_bps: if request.reference_inputs.is_empty() {
                    5_000
                } else {
                    7_500
                },
                texture_width: 1024,
                texture_height: 1024,
                channels: vec![
                    PbrTextureChannel::BaseColor,
                    PbrTextureChannel::Metallic,
                    PbrTextureChannel::Roughness,
                    PbrTextureChannel::Normal,
                    PbrTextureChannel::Occlusion,
                    PbrTextureChannel::Emissive,
                ],
                projection_layers: Vec::new(),
            })
            .collect::<Vec<_>>();
        let appearance_compilation = compile_generic_hard_surface_appearance(
            profile,
            feature_contract,
            &procedural_source,
            &material_zones,
            &[],
            &[],
        )?;
        let representation_source = match representation {
            RepresentationKind::Procedural => {
                UniversalRepresentationSourceV2::Procedural(procedural_source.clone())
            }
            RepresentationKind::Deformable => UniversalRepresentationSourceV2::Deformable(
                build_local_lattice_deform_source(procedural_source.clone())?,
            ),
            RepresentationKind::Hybrid => UniversalRepresentationSourceV2::Hybrid(
                build_local_hard_surface_hybrid_source(
                    procedural_source.clone(),
                    representation_plan,
                )?,
            ),
            RepresentationKind::MeshSeed => {
                UniversalRepresentationSourceV2::LocalMeshPatch(
                    build_local_mesh_patch_source(procedural_source.clone())?,
                )
            }
        };
        let source = Self {
            schema_version: UNIVERSAL_ASSET_SOURCE_V2_SCHEMA_VERSION.into(),
            source_id: format!("universal_source_{}", &request_sha256[..24]),
            state: UniversalAssetSourceState::Planned,
            request: request.clone(),
            request_sha256: request_sha256.clone(),
            subject_profile: profile.clone(),
            subject_profile_sha256: semantic_sha256(profile)?,
            visual_feature_contract: feature_contract.clone(),
            visual_feature_contract_sha256: semantic_sha256(feature_contract)?,
            representation_plan: representation_plan.clone(),
            representation_plan_sha256: semantic_sha256(representation_plan)?,
            capability_manifest_sha256: request.capability_manifest_sha256.clone(),
            representation_source,
            component_sources,
            detail_claims,
            material_zones,
            appearance_compilation,
            appearance_evidence: default_appearance_evidence(request, &request_sha256),
            game_asset_profile: None,
            compiled_artifact: None,
            game_asset_delivery: None,
        };
        source.validate()?;
        Ok(source)
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != UNIVERSAL_ASSET_SOURCE_V2_SCHEMA_VERSION
            || self.request_sha256 != semantic_sha256(&self.request)?
            || self.subject_profile_sha256 != semantic_sha256(&self.subject_profile)?
            || self.visual_feature_contract_sha256
                != semantic_sha256(&self.visual_feature_contract)?
            || self.representation_plan_sha256 != semantic_sha256(&self.representation_plan)?
            || self.capability_manifest_sha256 != self.request.capability_manifest_sha256
        {
            return Err(invalid(
                "UNIVERSAL_V2_LINEAGE_INVALID",
                "UAS@2 must bind exact request/profile/feature/plan lineage",
            ));
        }
        self.representation_plan.validate_against(
            &self.request,
            &self.subject_profile,
            &self.visual_feature_contract,
        )?;
        self.representation_source.validate()?;
        if let UniversalRepresentationSourceV2::Hybrid(source) = &self.representation_source {
            validate_hybrid_representation_plan(source, &self.representation_plan)?;
        }
        let procedural = self.representation_source.runtime_procedural()?;
        if self.component_sources.is_empty()
            || self.component_sources.len() > MAX_COMPONENTS
            || self.detail_claims.is_empty()
            || self.detail_claims.len() > MAX_DETAIL_CLAIMS
            || self.material_zones.len() > MAX_MATERIAL_ZONES
        {
            return Err(invalid(
                "UNIVERSAL_V2_BOUNDS_INVALID",
                "UAS@2 collections are outside reviewed bounds",
            ));
        }
        validate_appearance_evidence(&self.appearance_evidence, &self.request)?;
        validate_v2_components(self, procedural)?;
        validate_v2_detail_claims(self, procedural)?;
        validate_v2_material_zones(self, procedural)?;
        if let Some(game_asset_profile) = &self.game_asset_profile {
            game_asset_profile.validate()?;
            let part_ids = self
                .subject_profile
                .parts
                .iter()
                .map(|part| part.part_id.as_str())
                .collect::<BTreeSet<_>>();
            if game_asset_profile
                .collision_proxy_part_ids
                .iter()
                .any(|part_id| !part_ids.contains(part_id.as_str()))
                || game_asset_profile
                    .sockets
                    .iter()
                    .any(|socket| !part_ids.contains(socket.part_id.as_str()))
            {
                return Err(invalid("GAME_ASSET_PROFILE_PART_INVALID", "Game asset collision and socket bindings must target declared SubjectProfile parts."));
            }
            if let Some(delivery) = &self.game_asset_delivery {
                validate_game_asset_delivery_receipt(self, game_asset_profile, delivery, procedural)?;
            }
        } else if self.game_asset_delivery.is_some() {
            return Err(invalid(
                "GAME_ASSET_DELIVERY_PROFILE_REQUIRED",
                "A game delivery receipt requires its sealed game asset profile.",
            ));
        }
        self.appearance_compilation
            .validate_against(self, procedural)?;
        match (self.state, self.compiled_artifact.as_ref(), self.game_asset_delivery.as_ref()) {
            (UniversalAssetSourceState::Planned, None, None) => Ok(()),
            (UniversalAssetSourceState::Compiled, Some(binding), _) => {
                validate_v2_compiled_binding(binding, procedural)
            }
            _ => Err(invalid(
                "UNIVERSAL_V2_STATE_INVALID",
                "UAS@2 compiled state must carry one exact artifact binding",
            )),
        }
    }

    pub fn with_compiled_artifact(
        mut self,
        binding: UniversalCompiledArtifactBinding,
    ) -> CoreResult<Self> {
        self.state = UniversalAssetSourceState::Compiled;
        self.compiled_artifact = Some(binding);
        self.game_asset_delivery = None;
        self.validate()?;
        Ok(self)
    }

    /// Reopens an already compiled source for one Rust-owned appearance
    /// recompile.  This is intentionally narrower than a general edit: the
    /// request, subject, feature, representation and component truth remain
    /// identical, while stale compiled/projection receipts are removed before
    /// a newly sealed GLB is attached.
    pub fn reopen_for_appearance_recompile(mut self) -> CoreResult<Self> {
        if self.state != UniversalAssetSourceState::Compiled
            || self.compiled_artifact.is_none()
        {
            return Err(invalid(
                "UNIVERSAL_V2_RECOMPILE_STATE_INVALID",
                "Appearance recompilation requires one previously compiled UAS@2 source.",
            ));
        }
        self.state = UniversalAssetSourceState::Planned;
        self.compiled_artifact = None;
        self.game_asset_delivery = None;
        self.appearance_evidence.projection_receipts.clear();
        self.validate()?;
        Ok(self)
    }

    /// Attaches only the bounded Rust/worker receipt for reference pixels that
    /// were actually present in the final compiled GLB. The receipt carries
    /// no image bytes and cannot be supplied by a Provider.
    pub fn with_reference_appearance_projection_receipts(
        mut self,
        receipts: Vec<ReferenceAppearanceProjectionReceipt>,
    ) -> CoreResult<Self> {
        if receipts.is_empty() {
            return Ok(self);
        }
        if self.state != UniversalAssetSourceState::Planned {
            return Err(invalid(
                "REFERENCE_APPEARANCE_PROJECTION_STATE_INVALID",
                "Reference appearance receipts must be attached before the compiled UAS artifact is sealed.",
            ));
        }
        self.appearance_evidence.projection_receipts = receipts;
        self.validate()?;
        Ok(self)
    }

    /// Promotes an already compiled UAS@2 source to its game-delivery
    /// derivative. This deliberately accepts only a receipt that still binds
    /// the exact source GLB and sealed profile; callers must retain the
    /// delivery bytes separately until preview/confirmation consumes them.
    pub fn with_game_asset_delivery(
        mut self,
        delivery: crate::GameAssetDeliveryReadback,
    ) -> CoreResult<Self> {
        if self.state != UniversalAssetSourceState::Compiled || self.compiled_artifact.is_none() {
            return Err(invalid(
                "GAME_ASSET_DELIVERY_STATE_INVALID",
                "Game delivery may only be attached after the exact UAS@2 source GLB is compiled.",
            ));
        }
        if self.game_asset_profile.is_none() {
            return Err(invalid(
                "GAME_ASSET_DELIVERY_PROFILE_REQUIRED",
                "Game delivery requires a sealed game asset profile.",
            ));
        }
        self.game_asset_delivery = Some(delivery);
        self.validate()?;
        Ok(self)
    }

    /// Attaches a Rust-derived game delivery profile before preview/confirmation.
    pub fn with_game_asset_profile(mut self, profile: crate::GameAssetProfile) -> CoreResult<Self> {
        if self.state != UniversalAssetSourceState::Planned {
            return Err(invalid(
                "GAME_ASSET_PROFILE_STATE_INVALID",
                "Game delivery requirements must be sealed before compilation.",
            ));
        }
        self.game_asset_profile = Some(profile);
        self.validate()?;
        Ok(self)
    }

    /// Attaches bounded, Rust-derived discrete-view fits after the exact
    /// candidate GLB has been captured by the workbench renderer. This does
    /// not add a projection layer or claim that pixels have been rasterized
    /// into UV space; it only replaces matching unresolved camera records.
    pub fn with_fitted_camera_hypotheses(
        mut self,
        fitted: Vec<ReferenceCameraHypothesis>,
    ) -> CoreResult<Self> {
        if fitted.is_empty() {
            return Ok(self);
        }
        let expected = self
            .appearance_evidence
            .references
            .iter()
            .map(|reference| reference.evidence_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut by_evidence = BTreeMap::new();
        for camera in fitted {
            if camera.schema_version != REFERENCE_CAMERA_HYPOTHESIS_SCHEMA_VERSION
                || camera.parameter_source != CameraParameterSource::SilhouetteFit
                || camera.projection_type == ReferenceProjectionType::Unknown
                || camera.vertical_fov_millidegrees.is_none()
                || camera.reprojection_error_bps.is_none()
                || camera.confidence_bps == 0
                || !camera.unresolved_fields.is_empty()
                || !expected.contains(camera.evidence_id.as_str())
                || by_evidence
                    .insert(camera.evidence_id.clone(), camera)
                    .is_some()
            {
                return Err(invalid(
                    "REFERENCE_CAMERA_FIT_INVALID",
                    "A fitted camera must replace one sealed unresolved reference camera with bounded silhouette evidence.",
                ));
            }
        }
        for existing in &mut self.appearance_evidence.camera_hypotheses {
            let Some(camera) = by_evidence.remove(existing.evidence_id.as_str()) else {
                continue;
            };
            if existing.parameter_source == CameraParameterSource::SilhouetteFit
                && existing == &camera
            {
                continue;
            }
            if existing.parameter_source != CameraParameterSource::Unresolved {
                return Err(invalid(
                    "REFERENCE_CAMERA_FIT_REPLACEMENT_INVALID",
                    "A silhouette fit may replace only the initial unresolved camera hypothesis.",
                ));
            }
            *existing = camera;
        }
        if !by_evidence.is_empty() {
            return Err(invalid(
                "REFERENCE_CAMERA_FIT_REPLACEMENT_INVALID",
                "A fitted camera did not resolve to a retained appearance-evidence record.",
            ));
        }
        self.validate()?;
        Ok(self)
    }
}

fn validate_runtime_representation_plan(
    representation_plan: &RepresentationPlan,
    representation: RepresentationKind,
) -> CoreResult<()> {
    if representation_plan.parts.is_empty() {
        return Err(invalid(
            "UNIVERSAL_V2_REPRESENTATION_PLAN_INVALID",
            "runtime source requires at least one reviewed part plan",
        ));
    }
    let valid = match representation {
        RepresentationKind::Procedural => representation_plan.parts.iter().all(|part| {
            part.representation == RepresentationKind::Procedural
                && matches!(
                    part.capability_id.as_str(),
                    GENERIC_HARD_SURFACE_PROCEDURAL_CAPABILITY_ID
                        | GENERIC_VISUAL_EXTERIOR_PROCEDURAL_CAPABILITY_ID
                )
        }),
        RepresentationKind::Deformable => representation_plan.parts.iter().all(|part| {
            part.representation == RepresentationKind::Deformable
                && part.capability_id == LOCAL_LATTICE_DEFORMABLE_CAPABILITY_ID
        }),
        RepresentationKind::Hybrid => {
            let mut procedural_parts = 0usize;
            let mut lattice_parts = 0usize;
            for part in &representation_plan.parts {
                match (part.representation, part.capability_id.as_str()) {
                    (
                        RepresentationKind::Procedural,
                        GENERIC_HARD_SURFACE_PROCEDURAL_CAPABILITY_ID
                        | GENERIC_VISUAL_EXTERIOR_PROCEDURAL_CAPABILITY_ID,
                    ) => procedural_parts += 1,
                    (RepresentationKind::Deformable, LOCAL_LATTICE_DEFORMABLE_CAPABILITY_ID) => lattice_parts += 1,
                    _ => return Err(invalid(
                        "UNIVERSAL_LOCAL_HYBRID_PLAN_INVALID",
                        "local hybrid may contain only reviewed procedural visual-exterior/hard-surface and lattice-deformable part capabilities",
                    )),
                }
            }
            procedural_parts > 0 && lattice_parts > 0
        }
        RepresentationKind::MeshSeed => {
            representation_plan.parts.iter().all(|part| {
                part.representation == RepresentationKind::MeshSeed
                    && part.capability_id == LOCAL_MESH_PATCH_CAPABILITY_ID
            })
        }
    };
    if !valid {
        return Err(invalid(
            "UNIVERSAL_V2_REPRESENTATION_PLAN_INVALID",
            "runtime source representation must exactly match its reviewed part capabilities",
        ));
    }
    Ok(())
}

fn build_local_lattice_deform_source(
    procedural_source: UniversalProceduralSourceV2,
) -> CoreResult<UniversalLocalLatticeDeformSourceV2> {
    let expected_parts = procedural_source
        .part_bindings
        .iter()
        .map(|part| part.subject_part_id.as_str())
        .collect::<BTreeSet<_>>();
    let deformations = build_lattice_deformation_bindings(&procedural_source, &expected_parts)?;
    let source = UniversalLocalLatticeDeformSourceV2 {
        source_contract_id: "ForgeLocalLatticeDeformSource@1".into(),
        procedural_source,
        deformations,
    };
    source.validate()?;
    Ok(source)
}

fn build_local_hard_surface_hybrid_source(
    procedural_source: UniversalProceduralSourceV2,
    representation_plan: &RepresentationPlan,
) -> CoreResult<UniversalLocalHardSurfaceHybridSourceV2> {
    let lattice_part_ids = representation_plan
        .parts
        .iter()
        .filter(|part| part.representation == RepresentationKind::Deformable)
        .map(|part| part.part_id.as_str())
        .collect::<BTreeSet<_>>();
    let deformations = build_lattice_deformation_bindings(&procedural_source, &lattice_part_ids)?;
    let source = UniversalLocalHardSurfaceHybridSourceV2 {
        source_contract_id: "ForgeLocalHardSurfaceHybridSource@1".into(),
        procedural_source,
        deformations,
    };
    source.validate()?;
    validate_hybrid_representation_plan(&source, representation_plan)?;
    Ok(source)
}

fn build_local_mesh_patch_source(
    procedural_source: UniversalProceduralSourceV2,
) -> CoreResult<UniversalLocalMeshPatchSourceV2> {
    let expected_parts = procedural_source
        .part_bindings
        .iter()
        .map(|part| part.subject_part_id.as_str())
        .collect::<BTreeSet<_>>();
    let operations = procedural_source
        .shape_program
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid(
                "UNIVERSAL_LOCAL_MESH_PATCH_SOURCE_INVALID",
                "local mesh patch source requires lowered ShapeProgram operations",
            )
        })?;
    let operations = operations
        .iter()
        .filter_map(|operation| {
            Some((
                operation.get("operation_id")?.as_str()?,
                (
                    operation.get("op")?.as_str()?,
                    operation.get("inputs")?.as_array()?,
                    operation.get("args")?.as_object()?,
                ),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    let mut patches = Vec::with_capacity(expected_parts.len());
    for part in &procedural_source.part_bindings {
        let (op, inputs, args) = operations
            .get(part.terminal_operation_id.as_str())
            .map(|(op, inputs, args)| (*op, *inputs, *args))
            .ok_or_else(|| {
                invalid(
                    "UNIVERSAL_LOCAL_MESH_PATCH_OPERATION_INVALID",
                    "local mesh patch output terminal is missing from lowered ShapeProgram",
                )
            })?;
        let source_operation_id = inputs
            .first()
            .and_then(Value::as_str)
            .filter(|_| op == "local_mesh_patch" && inputs.len() == 1)
            .ok_or_else(|| {
                invalid(
                    "UNIVERSAL_LOCAL_MESH_PATCH_TERMINAL_INVALID",
                    "every mesh-seed output must terminate in one local_mesh_patch operation",
                )
            })?;
        let patch_center = serde_json::from_value::<[f64; 3]>(
            args.get("patch_center").cloned().ok_or_else(|| {
                invalid(
                    "UNIVERSAL_LOCAL_MESH_PATCH_OPERATION_INVALID",
                    "local mesh patch output is missing its normalized patch center",
                )
            })?,
        )
        .map_err(|_| {
            invalid(
                "UNIVERSAL_LOCAL_MESH_PATCH_OPERATION_INVALID",
                "local mesh patch center must be a numeric triplet",
            )
        })?;
        let patch_radius = args
            .get("patch_radius")
            .and_then(Value::as_f64)
            .ok_or_else(|| {
                invalid(
                    "UNIVERSAL_LOCAL_MESH_PATCH_OPERATION_INVALID",
                    "local mesh patch output is missing its radius",
                )
            })?;
        let patch_offset = serde_json::from_value::<[f64; 3]>(
            args.get("patch_offset").cloned().ok_or_else(|| {
                invalid(
                    "UNIVERSAL_LOCAL_MESH_PATCH_OPERATION_INVALID",
                    "local mesh patch output is missing its offset",
                )
            })?,
        )
        .map_err(|_| {
            invalid(
                "UNIVERSAL_LOCAL_MESH_PATCH_OPERATION_INVALID",
                "local mesh patch offset must be a numeric triplet",
            )
        })?;
        patches.push(UniversalLocalMeshPatchBindingV2 {
            patch_id: format!("local_mesh_patch_{}", part.subject_part_id),
            subject_part_id: part.subject_part_id.clone(),
            source_operation_id: source_operation_id.to_string(),
            patch_operation_id: part.terminal_operation_id.clone(),
            patch_center,
            patch_radius,
            patch_offset,
        });
    }
    let source = UniversalLocalMeshPatchSourceV2 {
        source_contract_id: "ForgeLocalMeshPatchSource@1".into(),
        procedural_source,
        patches,
    };
    source.validate()?;
    Ok(source)
}

fn build_lattice_deformation_bindings(
    procedural_source: &UniversalProceduralSourceV2,
    expected_part_ids: &BTreeSet<&str>,
) -> CoreResult<Vec<UniversalLatticeDeformationBindingV2>> {
    let operations = procedural_source
        .shape_program
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid(
                "UNIVERSAL_LATTICE_SOURCE_INVALID",
                "local lattice source requires lowered ShapeProgram operations",
            )
        })?;
    let operations = operations
        .iter()
        .filter_map(|operation| {
            Some((
                operation.get("operation_id")?.as_str()?,
                (
                    operation.get("op")?.as_str()?,
                    operation.get("inputs")?.as_array()?,
                    operation.get("args")?.as_object()?,
                ),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    let mut deformations = Vec::with_capacity(expected_part_ids.len());
    for part in procedural_source
        .part_bindings
        .iter()
        .filter(|part| expected_part_ids.contains(part.subject_part_id.as_str()))
    {
        let Some((op, inputs, args)) = operations
            .get(part.terminal_operation_id.as_str())
            .map(|(op, inputs, args)| (*op, *inputs, *args))
        else {
            return Err(invalid(
                "UNIVERSAL_LATTICE_OPERATION_INVALID",
                "local lattice output terminal is missing from lowered ShapeProgram",
            ));
        };
        let source_operation_id = inputs
            .first()
            .and_then(Value::as_str)
            .filter(|_| op == "lattice_deform" && inputs.len() == 1)
            .ok_or_else(|| {
                invalid(
                    "UNIVERSAL_LATTICE_TERMINAL_INVALID",
                    "every selected local-deformable output must terminate in one lattice_deform operation",
                )
            })?;
        let corner_offsets = serde_json::from_value::<[[f64; 3]; 8]>(
            args.get("corner_offsets").cloned().ok_or_else(|| {
                invalid(
                    "UNIVERSAL_LATTICE_OPERATION_INVALID",
                    "local lattice output is missing its corner offsets",
                )
            })?,
        )
        .map_err(|_| {
            invalid(
                "UNIVERSAL_LATTICE_OPERATION_INVALID",
                "local lattice offsets must be eight numeric corner triplets",
            )
        })?;
        deformations.push(UniversalLatticeDeformationBindingV2 {
            deformation_id: format!("lattice_deform_{}", part.subject_part_id),
            subject_part_id: part.subject_part_id.clone(),
            source_operation_id: source_operation_id.to_string(),
            deformation_operation_id: part.terminal_operation_id.clone(),
            corner_offsets,
        });
    }
    if deformations.len() != expected_part_ids.len() {
        return Err(invalid(
            "UNIVERSAL_LATTICE_BINDING_INCOMPLETE",
            "every selected local-deformable part requires one lattice terminal binding",
        ));
    }
    Ok(deformations)
}

fn validate_hybrid_representation_plan(
    source: &UniversalLocalHardSurfaceHybridSourceV2,
    representation_plan: &RepresentationPlan,
) -> CoreResult<()> {
    validate_runtime_representation_plan(representation_plan, RepresentationKind::Hybrid)?;
    let planned_lattice_parts = representation_plan
        .parts
        .iter()
        .filter(|part| part.representation == RepresentationKind::Deformable)
        .map(|part| part.part_id.as_str())
        .collect::<BTreeSet<_>>();
    let actual_lattice_parts = source
        .deformations
        .iter()
        .map(|binding| binding.subject_part_id.as_str())
        .collect::<BTreeSet<_>>();
    if planned_lattice_parts != actual_lattice_parts {
        return Err(invalid(
            "UNIVERSAL_LOCAL_HYBRID_PLAN_LINEAGE_INVALID",
            "hybrid lattice bindings must exactly match the sealed deformable part plan",
        ));
    }
    Ok(())
}

/// Converts only reviewed visual words from the sealed SubjectProfile into an
/// existing local PBR catalog base. Geometry and material identities remain
/// authored by the bounded program. If two visible parts try to reuse one
/// authored material while asking for incompatible observed appearance, the
/// source is rejected rather than silently picking a template material.
fn normalize_generic_hard_surface_material_bases(
    profile: &SubjectProfile,
    mut source_program: Value,
) -> CoreResult<Value> {
    let outputs = source_program
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid(
                "UNIVERSAL_V2_MATERIAL_SOURCE_INVALID",
                "runtime program has no outputs",
            )
        })?;
    let nodes = source_program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid(
                "UNIVERSAL_V2_MATERIAL_SOURCE_INVALID",
                "runtime program has no nodes",
            )
        })?;
    if outputs.len() != profile.parts.len() {
        return Err(invalid(
            "UNIVERSAL_V2_MATERIAL_SOURCE_INVALID",
            "runtime program outputs must remain one-to-one with SubjectProfile parts",
        ));
    }
    let mut desired_by_authored_material = BTreeMap::<String, String>::new();
    for (part, output) in profile.parts.iter().zip(outputs) {
        let Some(output_node_id) = output.get("node_id").and_then(Value::as_str) else {
            return Err(invalid(
                "UNIVERSAL_V2_MATERIAL_SOURCE_INVALID",
                "runtime program output requires one node identity",
            ));
        };
        let Some(authored_material_id) = nodes.iter().find_map(|node| {
            (node.get("node_id").and_then(Value::as_str) == Some(output_node_id)
                && node.get("kind").and_then(Value::as_str) == Some("material_zone"))
            .then(|| node.get("material_id").and_then(Value::as_str))
            .flatten()
        }) else {
            return Err(invalid(
                "UNIVERSAL_V2_MATERIAL_SOURCE_INVALID",
                "every category-open output must terminate in one material_zone node",
            ));
        };
        let Some(desired_base) = profile_material_base_for_part(profile, &part.part_id) else {
            continue;
        };
        if let Some(existing) = desired_by_authored_material
            .insert(authored_material_id.to_string(), desired_base.to_string())
        {
            if existing != desired_base {
                return Err(invalid(
                    "UNIVERSAL_V2_MATERIAL_SOURCE_CONFLICT",
                    "one authored material cannot satisfy incompatible visible subject materials; author separate bounded zones",
                ));
            }
        }
    }
    let materials = source_program
        .get_mut("materials")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            invalid(
                "UNIVERSAL_V2_MATERIAL_SOURCE_INVALID",
                "runtime program has no materials",
            )
        })?;
    let mut applied = BTreeSet::new();
    for material in materials {
        let Some(material_id) = material
            .get("material_id")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            return Err(invalid(
                "UNIVERSAL_V2_MATERIAL_SOURCE_INVALID",
                "runtime material requires material_id",
            ));
        };
        if let Some(base_material_id) = desired_by_authored_material.get(&material_id) {
            material["base_material_id"] = Value::String(base_material_id.clone());
            applied.insert(material_id);
        }
    }
    if applied.len() != desired_by_authored_material.len() {
        return Err(invalid(
            "UNIVERSAL_V2_MATERIAL_SOURCE_INVALID",
            "a material-zone output references no declared authored material",
        ));
    }
    Ok(source_program)
}

fn profile_material_base_for_part(profile: &SubjectProfile, part_id: &str) -> Option<&'static str> {
    let material = profile
        .materials
        .iter()
        .find(|material| material.part_ids.iter().any(|id| id == part_id))?;
    let mut visual_words = material.label.to_lowercase();
    for trait_name in &material.appearance_traits {
        visual_words.push(' ');
        visual_words.push_str(&trait_name.to_lowercase());
    }
    if visual_words.contains("red") || visual_words.contains('红') {
        Some("mat_signal_red")
    } else if (visual_words.contains("blue") || visual_words.contains('蓝'))
        && (visual_words.contains("emissive")
            || visual_words.contains("luminous")
            || visual_words.contains('发'))
    {
        Some("mat_emissive_blue")
    } else if visual_words.contains("glass") || visual_words.contains('玻') {
        Some("mat_dark_glass")
    } else if visual_words.contains("rubber") || visual_words.contains('橡') {
        Some("mat_rubber")
    } else if visual_words.contains("carbon")
        || visual_words.contains("composite")
        || visual_words.contains('碳')
        || visual_words.contains('复')
    {
        Some("mat_composite")
    } else if visual_words.contains("paint")
        || visual_words.contains("coated")
        || visual_words.contains('漆')
    {
        Some("mat_automotive_paint")
    } else if visual_words.contains("aluminum")
        || visual_words.contains("silver")
        || visual_words.contains("metal")
        || visual_words.contains("brushed")
        || visual_words.contains('铝')
        || visual_words.contains('银')
        || visual_words.contains('金')
    {
        Some("mat_aluminum")
    } else {
        None
    }
}

impl GenericHardSurfaceAppearanceCompilation {
    fn validate_against(
        &self,
        source: &UniversalAssetSourceV2,
        procedural: &UniversalProceduralSourceV2,
    ) -> CoreResult<()> {
        validate_reference_surface_bindings(&self.reference_surface_bindings, source)?;
        if self.schema_version != GENERIC_HARD_SURFACE_APPEARANCE_COMPILATION_SCHEMA_VERSION
            || self.compiler_id != "forgecad.generic_hard_surface_appearance.v2"
            || self.source_program_sha256 != procedural.source_program_sha256
            || self.zones.is_empty()
            || self.zones.len() > 8
            || self.compilation_sha256 != generic_hard_surface_appearance_compilation_sha256(self)?
        {
            return Err(invalid(
                "UNIVERSAL_V2_APPEARANCE_COMPILATION_INVALID",
                "generic hard-surface appearance compilation must bind the exact procedural source and sealed PBR zones",
            ));
        }
        let mut target_zones = BTreeSet::new();
        for zone in &self.zones {
            if !target_zones.insert((&zone.target_subject_part_id, &zone.target_material_zone_id))
                || zone.surface_layer_program_sha256
                    != zone.surface_layer_program.canonical_sha256()?
                || zone.surface_layer_program.target_part_id != zone.target_subject_part_id
                || zone.surface_layer_program.target_zone_id != zone.target_material_zone_id
                || zone.surface_layer_program.material_zone_id != zone.target_material_zone_id
                || zone.surface_layer_program.base_material != zone.base_material_id
                || zone.surface_layer_program.quality_profile != "production_concept"
            {
                return Err(invalid(
                    "UNIVERSAL_V2_APPEARANCE_COMPILATION_INVALID",
                    "each appearance compilation zone must be unique and exactly sealed",
                ));
            }
            let binding = procedural
                .part_bindings
                .iter()
                .find(|binding| {
                    binding.subject_part_id == zone.target_subject_part_id
                        && binding.material_zone_id == zone.target_material_zone_id
                })
                .ok_or_else(|| {
                    invalid(
                        "UNIVERSAL_V2_APPEARANCE_TARGET_INVALID",
                        "appearance compilation must target real UAS@2 procedural material zones",
                    )
                })?;
            let expected_base =
                compiled_visual_base_material_id(&binding.material_id).ok_or_else(|| {
                    invalid(
                        "UNIVERSAL_V2_APPEARANCE_MATERIAL_INVALID",
                        "appearance compilation target has no reviewed PBR base material",
                    )
                })?;
            if zone.base_material_id != expected_base
                || !source.material_zones.iter().any(|material_zone| {
                    material_zone.source_part_id == zone.target_subject_part_id
                        && material_zone.material_zone_id == zone.target_material_zone_id
                        && material_zone.base_material_id == binding.material_id
                })
            {
                return Err(invalid(
                    "UNIVERSAL_V2_APPEARANCE_MATERIAL_INVALID",
                    "appearance compilation must preserve reviewed geometry-zone material bindings",
                ));
            }
        }
        Ok(())
    }
}

fn generic_hard_surface_appearance_compilation_sha256(
    compilation: &GenericHardSurfaceAppearanceCompilation,
) -> CoreResult<String> {
    semantic_sha256(&serde_json::json!({
        "schema_version": compilation.schema_version,
        "compiler_id": compilation.compiler_id,
        "source_program_sha256": compilation.source_program_sha256,
        "zones": compilation.zones,
        "reference_surface_bindings": compilation.reference_surface_bindings,
    }))
}

fn compile_generic_hard_surface_appearance(
    profile: &SubjectProfile,
    feature_contract: &VisualFeatureContract,
    procedural: &UniversalProceduralSourceV2,
    material_zones: &[MaterialZoneAppearance],
    reference_surface_bindings: &[ReferenceSurfaceAppearanceBinding],
    reference_appearance_bindings: &[ReferenceAppearanceBinding],
) -> CoreResult<GenericHardSurfaceAppearanceCompilation> {
    let mut candidates = procedural
        .part_bindings
        .iter()
        .filter_map(|binding| {
            let part = profile
                .parts
                .iter()
                .find(|part| part.part_id == binding.subject_part_id)?;
            material_zones.iter().find(|zone| {
                zone.source_part_id == binding.subject_part_id
                    && zone.material_zone_id == binding.material_zone_id
            })?;
            // Hidden and conflicting claims may remain in the sealed contract
            // for uncertainty/readback, but they must never choose visible
            // geometry or PBR motifs. Inferred claims are allowed because the
            // authoring contract explicitly distinguishes them from hidden
            // evidence and keeps their uncertainty in the profile.
            let feature_requirements = feature_contract
                .requirements
                .iter()
                .filter(|requirement| {
                    matches!(
                        requirement.evidence_status,
                        EvidenceStatus::Observed | EvidenceStatus::Inferred
                    ) && requirement
                        .affected_part_ids
                        .contains(&binding.subject_part_id)
                })
                .collect::<Vec<_>>();
            let score = feature_requirements
                .iter()
                .filter(|requirement| {
                    requirement.channels.iter().any(|channel| {
                        matches!(
                            channel,
                            AppearanceChannel::Normal
                                | AppearanceChannel::Roughness
                                | AppearanceChannel::Metallic
                                | AppearanceChannel::BaseColor
                                | AppearanceChannel::Emissive
                        )
                    })
                })
                .map(|requirement| u32::from(requirement.salience_bps))
                .sum::<u32>();
            let traits = profile
                .materials
                .iter()
                .filter(|material| material.part_ids.contains(&binding.subject_part_id))
                .flat_map(|material| material.appearance_traits.iter())
                .map(|trait_name| trait_name.to_lowercase())
                .collect::<Vec<_>>();
            let feature_text = feature_requirements
                .iter()
                .map(|requirement| requirement.description.to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");
            let feature_salience_bps = feature_requirements
                .iter()
                .map(|requirement| requirement.salience_bps)
                .max()
                .unwrap_or(0);
            let feature_has_emissive_channel = feature_requirements.iter().any(|requirement| {
                requirement
                    .channels
                    .contains(&AppearanceChannel::Emissive)
            });
            let feature_has_base_color_channel = feature_requirements.iter().any(|requirement| {
                requirement
                    .channels
                    .contains(&AppearanceChannel::BaseColor)
            });
            let feature_has_vector_channel = feature_requirements.iter().any(|requirement| {
                requirement.channels.iter().any(|channel| {
                    matches!(
                        channel,
                        AppearanceChannel::BaseColor
                            | AppearanceChannel::Normal
                            | AppearanceChannel::Roughness
                            | AppearanceChannel::Metallic
                    )
                })
            });
            Some((
                score,
                traits.len(),
                binding.subject_part_id.clone(),
                binding.material_zone_id.clone(),
                binding.material_id.clone(),
                part.semantic_role.clone(),
                traits,
                feature_text,
                feature_salience_bps,
                feature_has_emissive_channel,
                feature_has_base_color_channel,
                feature_has_vector_channel,
            ))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.cmp(&right.3))
    });
    if candidates.is_empty() {
        return Err(invalid(
            "UNIVERSAL_V2_APPEARANCE_TARGET_MISSING",
            "generic hard-surface appearance compilation requires bound procedural material zones",
        ));
    }
    let source_program_sha256 = procedural.source_program_sha256.clone();
    let mut zones = Vec::new();
    for (
        index,
        (
            _,
            _,
            part_id,
            zone_id,
            material_id,
            part_role,
            traits,
            feature_text,
            feature_salience_bps,
            feature_has_emissive_channel,
            feature_has_base_color_channel,
            feature_has_vector_channel,
        ),
    ) in
        candidates.into_iter().take(8).enumerate()
    {
        let base_material_id = compiled_visual_base_material_id(&material_id)
            .ok_or_else(|| {
                invalid(
                    "UNIVERSAL_V2_APPEARANCE_MATERIAL_INVALID",
                    "target material has no reviewed PBR base",
                )
        })?
        .to_string();
        let seed_material = format!("{}:{}:{}", source_program_sha256, part_id, zone_id);
        // The retained Python PBR compiler shares the ShapeProgram signed
        // 31-bit seed envelope. Keep room for the deterministic +1/+2 seeds
        // used by roughness/emissive maps so a valid Rust-owned UAS source
        // cannot be rejected only after crossing the local worker boundary.
        let seed = u32::from_str_radix(&semantic_sha256(&seed_material)?[..8], 16)
            .unwrap_or(0)
            & 0x7fff_fffd;
        let suffix = format!("{}_{}", &source_program_sha256[..16], index);
        let traits_text = traits.join(" ");
        let visual_text = format!("{traits_text} {feature_text}");
        // Whole-image surface facts are eligible only when the same sealed
        // image is explicitly attached to an observed appearance feature
        // affecting this exact Part/Zone. A shared SubjectProfile Part is not
        // enough to let a dominant palette leak into sibling material zones.
        let reference_hint = reference_appearance_bindings
            .iter()
            .filter(|binding| {
                binding.target_subject_part_id == part_id
                    && binding.target_material_zone_id == zone_id
            })
            .filter_map(|binding| {
                reference_surface_bindings
                    .iter()
                    .find(|surface| surface.evidence_id == binding.evidence_id)
            })
            .next();
        let reference_fallback_allowed = reference_surface_fallback_allowed(
            &part_role,
            &base_material_id,
            &visual_text,
        );
        let reference_base_color_token = reference_fallback_allowed
            .then(|| reference_hint.and_then(|binding| binding.base_color_token.as_deref()))
            .flatten();
        let base_color_token =
            feature_base_color_token(&visual_text).or(reference_base_color_token);
        let reference_surface_finish_token = reference_fallback_allowed
            .then(|| reference_hint.and_then(|binding| binding.surface_finish_token.as_deref()))
            .flatten();
        let surface_finish_token = feature_surface_finish_token(&visual_text, &base_material_id)
            .or(reference_surface_finish_token);
        let is_emissive = (traits_text.contains("emissive")
            || traits_text.contains("luminous")
            || traits_text.contains("发光")
            || (feature_has_emissive_channel
                && contains_any(
                    &visual_text,
                    &["emissive", "luminous", "glow", "light", "发光", "光带", "能量"],
                )))
            || base_material_id == "mat_emissive_blue";
        let is_brushed = traits_text.contains("brushed")
            || traits_text.contains("metallic")
            || traits_text.contains("metal")
            || traits_text.contains("silver")
            || traits_text.contains("铝")
            || traits_text.contains("金属")
            || base_material_id == "mat_aluminum";
        // The compiler deliberately consumes only the sealed visual feature
        // description and material traits.  It does not classify the object
        // into a domain or select a whole-object template.  This gives a
        // single author request a different, reproducible PBR surface grammar
        // for each part while keeping the motif vocabulary Rust/Worker-owned.
        let angular_relief = contains_any(
            &visual_text,
            &[
                "chevron",
                "angled",
                "angular",
                "triangular",
                "斜",
                "人字",
                "三角",
            ],
        );
        let normal_motif = if angular_relief {
            "chevron_relief"
        } else {
            "parallel_groove"
        };
        let normal_coverage = if contains_any(
            &visual_text,
            &["edge", "rim", "border", "边缘", "边框", "轮廓"],
        ) {
            "edge_band"
        } else if contains_any(
            &visual_text,
            &["center", "core", "chest", "center_band", "中心", "核心"],
        ) {
            "center_band"
        } else {
            "full_zone"
        };
        let normal_intensity = if feature_salience_bps >= 9_000 {
            "pronounced"
        } else if feature_salience_bps <= 5_000 {
            "subtle"
        } else {
            "balanced"
        };
        let roughness_motif = if contains_any(
            &visual_text,
            &[
                "hex",
                "honeycomb",
                "microgrid",
                "grid",
                "蜂窝",
                "六边形",
                "网格",
            ],
        ) {
            "microgrid"
        } else if contains_any(
            &visual_text,
            &["wear", "scratch", "scuff", "weather", "磨损", "划痕", "做旧"],
        ) || contains_any(
            &visual_text,
            &["edge", "rim", "border", "边缘", "边框"],
        ) {
            "edge_wear"
        } else if is_brushed {
            "linear_brush"
        } else {
            if reference_fallback_allowed {
                reference_hint
                    .map(|binding| binding.roughness_motif.as_str())
                    .unwrap_or("edge_wear")
            } else {
                "edge_wear"
            }
        };
        let roughness_coverage = if contains_any(
            &visual_text,
            &["edge", "rim", "border", "边缘", "边框"],
        ) {
            "edge_band"
        } else if contains_any(
            &visual_text,
            &["center", "core", "center_band", "中心", "核心"],
        ) {
            "center_band"
        } else {
            "full_zone"
        };
        let emissive_motif = if contains_any(
            &visual_text,
            &["indicator", "sensor", "status", "指示", "传感", "状态"],
        ) {
            "panel_indicator"
        } else if contains_any(&visual_text, &["dot", "array", "点阵", "阵列"]) {
            "dot_array"
        } else {
            "double_flowline"
        };
        let emissive_color = if contains_any(&visual_text, &["red", "signal", "红", "警示"]) {
            "signal_red"
        } else {
            "accent_blue"
        };
        let decal_layers = feature_driven_decal(
            &feature_text,
            feature_has_base_color_channel,
            feature_salience_bps,
            seed,
            &format!("decal_uas_{suffix}"),
        );
        let vector_paths = feature_driven_vector_paths(
            &feature_text,
            feature_has_vector_channel,
            seed,
            &format!("path_uas_{suffix}"),
        );
        let symmetry_mode = if contains_any(
            &feature_text,
            &["symmetric", "paired", "mirror", "对称", "双侧", "左右"],
        ) {
            "mirror_u"
        } else if contains_any(&feature_text, &["radial", "ring", "环形", "径向"]) {
            "radial_4"
        } else {
            "none"
        };
        let compiler_manifest_sha256 = semantic_sha256(&serde_json::json!({
            "schema_version": GENERIC_HARD_SURFACE_APPEARANCE_COMPILATION_SCHEMA_VERSION,
            "compiler_id": "forgecad.generic_hard_surface_appearance.v2",
            "base_material_id": &base_material_id,
            "reference_surface_binding_sha256": reference_hint.map(|binding| &binding.binding_sha256),
            "reference_appearance_binding_sha256": reference_appearance_bindings
                .iter()
                .find(|binding| {
                    binding.target_subject_part_id == part_id
                        && binding.target_material_zone_id == zone_id
                })
                .map(|binding| &binding.binding_sha256),
        }))?;
        let program = SurfaceLayerProgram {
            schema_version: "SurfaceLayerProgram@1".into(),
            program_id: format!("surface_layer_uas_{suffix}"),
            target_part_id: part_id.clone(),
            target_zone_id: zone_id.clone(),
            target_part_role: generic_hard_surface_layer_role(&part_role).into(),
            material_zone_id: zone_id.clone(),
            base_material: base_material_id.clone(),
            base_color_token: base_color_token.map(str::to_string),
            surface_finish_token: surface_finish_token.map(str::to_string),
            vector_paths,
            decal_layers,
            normal_relief_layers: vec![NormalReliefLayer {
                layer_id: format!("relief_uas_{suffix}"),
                motif: normal_motif.into(),
                intensity: normal_intensity.into(),
                coverage: normal_coverage.into(),
                seed,
            }],
            roughness_masks: vec![RoughnessMask {
                mask_id: format!("rough_uas_{suffix}"),
                motif: roughness_motif.into(),
                coverage: roughness_coverage.into(),
                intensity_milli: if feature_salience_bps >= 9_000 {
                    620
                } else if is_brushed {
                    560
                } else {
                    420
                },
                seed: seed.wrapping_add(1),
            }],
            emissive_masks: if is_emissive {
                vec![EmissiveMask {
                    mask_id: format!("emissive_uas_{suffix}"),
                    motif: emissive_motif.into(),
                    color_token: emissive_color.into(),
                    coverage: if contains_any(
                        &visual_text,
                        &["edge", "rim", "border", "边缘", "边框"],
                    ) {
                        "edge_band".into()
                    } else {
                        "center_band".into()
                    },
                    intensity_milli: 620,
                    seed: seed.wrapping_add(2),
                }]
            } else {
                Vec::new()
            },
            symmetry: SurfaceSymmetry {
                mode: symmetry_mode.into(),
                center_uv: [0.5, 0.5],
            },
            uv_frame: UvFrame {
                frame_id: format!("uvframe_uas_{suffix}"),
                u_min: 0.0,
                u_max: 1.0,
                v_min: 0.0,
                v_max: 1.0,
                rotation_degrees: 0.0,
            },
            quality_profile: "production_concept".into(),
            execution: "lower_to_a005_and_retain".into(),
            skill_id: "skill_generic_hard_surface_appearance".into(),
            skill_version: 1,
            skill_sha256: compiler_manifest_sha256,
            generator: "surface_layer_v1".into(),
            non_functional_only: true,
        };
        zones.push(GenericHardSurfaceAppearanceZone {
            target_subject_part_id: part_id,
            target_material_zone_id: zone_id,
            base_material_id,
            surface_layer_program_sha256: program.canonical_sha256()?,
            surface_layer_program: program,
        });
    }
    let mut compilation = GenericHardSurfaceAppearanceCompilation {
        schema_version: GENERIC_HARD_SURFACE_APPEARANCE_COMPILATION_SCHEMA_VERSION.into(),
        compiler_id: "forgecad.generic_hard_surface_appearance.v2".into(),
        source_program_sha256,
        zones,
        reference_surface_bindings: reference_surface_bindings.to_vec(),
        compilation_sha256: String::new(),
    };
    compilation.compilation_sha256 =
        generic_hard_surface_appearance_compilation_sha256(&compilation)?;
    Ok(compilation)
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

/// Reference surface facts describe the visible object as a whole, so they
/// cannot safely recolor every material zone.  Apply them only to reviewed
/// exterior roles with a compatible base material; explicit feature/material
/// semantics are resolved before this fallback is consulted.  Special zones
/// such as glass, rubber, emissive trims and signal colors must keep their
/// own catalog identity even when the reference has a dominant palette.
fn reference_surface_fallback_allowed(
    semantic_role: &str,
    base_material_id: &str,
    visual_text: &str,
) -> bool {
    let normalized_role = semantic_role.to_lowercase();
    let role = generic_hard_surface_layer_role(&normalized_role);
    let exterior_role = matches!(
        role,
        "primary_shell"
            | "secondary_shell"
            | "armor_panel"
            | "exterior_panel"
            | "decorative_panel"
            | "enclosure"
            | "body_shell"
            | "surface_trim"
    );
    let special_material_semantics = contains_any(
        visual_text,
        &[
            "rubber",
            "grip",
            "橡胶",
            "握把",
            "glass",
            "transparent",
            "玻璃",
            "透明",
            "emissive",
            "luminous",
            "glow",
            "发光",
            "光带",
            "signal",
            "warning",
            "警示",
            "红色",
            "red coating",
        ],
    );
    exterior_role
        && !special_material_semantics
        && matches!(
            base_material_id,
            "mat_graphite" | "mat_aluminum" | "mat_composite" | "mat_automotive_paint"
        )
}

/// Select only a bounded appearance tint from sealed material/feature text.
/// This is deliberately not a free RGB channel: the token is Rust-owned,
/// evidence-conditioned and compiled into the retained PBR bake.
fn feature_base_color_token(value: &str) -> Option<&'static str> {
    if contains_any(value, &["silver", "aluminum", "aluminium", "银", "白银", "铝"]) {
        Some("silver")
    } else if contains_any(value, &["ceramic", "porcelain", "white ceramic", "陶瓷", "白瓷"]) {
        Some("white_ceramic")
    } else if contains_any(
        value,
        &["gunmetal", "gun metal", "gray", "grey", "灰", "枪灰"],
    ) {
        Some("gunmetal")
    } else if contains_any(value, &["copper", "bronze", "铜", "青铜"]) {
        Some("copper")
    } else if contains_any(value, &["signal red", "red coating", "红色", "红涂层", "警示红"]) {
        Some("signal_red")
    } else if contains_any(
        value,
        &[
            "foliage", "leaf", "leaves", "greenery", "green", "叶", "叶片", "绿叶", "绿色",
        ],
    ) {
        Some("foliage_green")
    } else if contains_any(value, &["skin", "flesh", "肤色", "皮肤", "肉色"]) {
        Some("skin_warm")
    } else if contains_any(value, &["fur", "毛发", "毛皮", "绒毛", "毛茸茸"]) {
        Some("fur_warm")
    } else if contains_any(value, &["fabric", "cloth", "textile", "布料", "织物", "服装"]) {
        Some("fabric_blue")
    } else if contains_any(value, &["bark", "trunk", "tree", "树皮", "树干", "树"]) {
        Some("bark_brown")
    } else if contains_any(value, &["wood", "wooden", "木头", "木材"]) {
        Some("wood_warm")
    } else if contains_any(value, &["stone", "rock", "石头", "岩石"]) {
        Some("stone_gray")
    } else if contains_any(value, &["concrete", "cement", "混凝土", "水泥"]) {
        Some("concrete_gray")
    } else if contains_any(value, &["clay", "terracotta", "陶土", "赤陶"]) {
        Some("clay_terracotta")
    } else if contains_any(
        value,
        &["graphite", "black", "dark", "石墨", "黑", "深灰", "深色"],
    ) {
        Some("graphite")
    } else {
        None
    }
}

/// Select a reviewed material response independently from base color. This
/// keeps a silver ceramic shell, a polished metal ring and a graphite rubber
/// grip visually distinct without exposing arbitrary metallic/roughness
/// numbers to the Provider or worker.
fn feature_surface_finish_token(value: &str, base_material_id: &str) -> Option<&'static str> {
    if base_material_id == "mat_dark_glass"
        || contains_any(value, &["glass", "transparent", "玻璃", "透明"])
    {
        Some("dark_glass")
    } else if base_material_id == "mat_emissive_blue"
        || contains_any(value, &["emissive", "luminous", "glow", "发光", "光带"])
    {
        Some("emissive_trim")
    } else if base_material_id == "mat_rubber"
        || contains_any(value, &["rubber", "grip", "橡胶", "握把"])
    {
        Some("rubberized")
    } else if contains_any(value, &["bark", "trunk", "树皮", "树干"]) {
        Some("bark_ridged")
    } else if contains_any(value, &["leaf", "leaves", "foliage", "greenery", "叶", "叶片", "绿叶"]) {
        Some("leaf_waxy")
    } else if contains_any(value, &["wood", "wooden", "grain", "木头", "木材", "木纹"]) {
        Some("wood_grain")
    } else if contains_any(value, &["fabric", "cloth", "textile", "布料", "织物", "服装"]) {
        Some("fabric_weave")
    } else if contains_any(value, &["fur", "毛发", "毛皮", "绒毛", "毛茸茸"]) {
        Some("fur_soft")
    } else if contains_any(value, &["skin", "flesh", "肤色", "皮肤", "肉色"]) {
        Some("skin_matte")
    } else if contains_any(value, &["stone", "rock", "石头", "岩石"]) {
        Some("stone_rough")
    } else if contains_any(value, &["concrete", "cement", "混凝土", "水泥"]) {
        Some("concrete_rough")
    } else if contains_any(value, &["clay", "terracotta", "陶土", "赤陶"]) {
        Some("clay_matte")
    } else if contains_any(value, &["ceramic", "porcelain", "陶瓷", "白瓷"])
    {
        Some("ceramic_coat")
    } else if contains_any(value, &["polished", "mirror", "chrome", "抛光", "镜面", "铬"])
    {
        Some("polished_metal")
    } else if base_material_id == "mat_automotive_paint"
        || contains_any(value, &["paint", "coated", "coating", "漆", "涂层", "喷涂"])
    {
        Some("glossy_coat")
    } else if base_material_id == "mat_composite"
        || contains_any(value, &["matte", "rough", "composite", "哑光", "复合"])
    {
        Some("matte_coat")
    } else if base_material_id == "mat_aluminum"
        || contains_any(value, &["brushed", "aluminum", "aluminium", "silver", "metal", "拉丝", "铝", "银", "金属"])
    {
        Some("brushed_metal")
    } else {
        None
    }
}

fn derive_reference_surface_appearance_bindings(
    source: &UniversalAssetSourceV2,
    evidence: &[ReferenceEvidence],
) -> CoreResult<Vec<ReferenceSurfaceAppearanceBinding>> {
    let expected = source
        .request
        .reference_inputs
        .iter()
        .map(|reference| (reference.evidence_id.as_str(), reference.evidence_sha256.as_str()))
        .collect::<BTreeMap<_, _>>();
    let evidence_by_id = evidence
        .iter()
        .map(|item| (item.evidence_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    if evidence_by_id.len() != evidence.len() {
        return Err(invalid(
            "REFERENCE_SURFACE_APPEARANCE_EVIDENCE_DUPLICATE",
            "sealed reference evidence IDs must be unique before appearance compilation",
        ));
    }

    let mut bindings = Vec::new();
    for (evidence_id, expected_sha256) in expected {
        let sealed = evidence_by_id.get(evidence_id).ok_or_else(|| {
            invalid(
                "REFERENCE_SURFACE_APPEARANCE_EVIDENCE_MISSING",
                "an image reference must have its exact sealed evidence before appearance compilation",
            )
        })?;
        sealed.validate()?;
        if sealed.project_id != source.request.project_id
            || semantic_sha256(*sealed)? != expected_sha256
        {
            return Err(invalid(
                "REFERENCE_SURFACE_APPEARANCE_EVIDENCE_INVALID",
                "appearance facts require exact same-project sealed evidence lineage",
            ));
        }
        if sealed.kind != ReferenceEvidenceKind::Image {
            continue;
        }
        let facts = sealed
            .observations
            .image_surface_facts
            .clone()
            .ok_or_else(|| {
                invalid(
                    "REFERENCE_SURFACE_APPEARANCE_FACTS_MISSING",
                    "image evidence has no Rust-derived surface facts",
                )
            })?;
        let base_color_token = reference_surface_base_color_token(&facts);
        let surface_finish_token = reference_surface_finish_hint(&facts, base_color_token);
        let roughness_motif = reference_surface_roughness_motif(&facts).to_string();
        let mut binding = ReferenceSurfaceAppearanceBinding {
            schema_version: REFERENCE_SURFACE_APPEARANCE_BINDING_SCHEMA_VERSION.into(),
            evidence_id: evidence_id.to_string(),
            evidence_sha256: expected_sha256.to_string(),
            facts,
            base_color_token: base_color_token.map(str::to_string),
            surface_finish_token: surface_finish_token.map(str::to_string),
            roughness_motif,
            binding_sha256: String::new(),
        };
        binding.binding_sha256 = reference_surface_appearance_binding_sha256(&binding)?;
        bindings.push(binding);
    }
    bindings.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    Ok(bindings)
}

fn validate_reference_surface_bindings(
    bindings: &[ReferenceSurfaceAppearanceBinding],
    source: &UniversalAssetSourceV2,
) -> CoreResult<()> {
    if bindings.len() > source.request.reference_inputs.len()
        || bindings.len() > 8
    {
        return Err(invalid(
            "REFERENCE_SURFACE_APPEARANCE_BOUNDS_INVALID",
            "reference surface appearance bindings exceed the sealed request bound",
        ));
    }
    let expected = source
        .request
        .reference_inputs
        .iter()
        .map(|reference| (reference.evidence_id.as_str(), reference.evidence_sha256.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut ids = BTreeSet::new();
    for binding in bindings {
        let valid_base_color = binding.base_color_token.as_deref().is_none_or(|token| {
            matches!(
                token,
                "silver" | "white_ceramic" | "gunmetal" | "graphite" | "copper"
                    | "signal_red" | "bark_brown" | "wood_warm" | "foliage_green"
                    | "skin_warm" | "fur_warm" | "fabric_blue" | "stone_gray"
                    | "concrete_gray" | "clay_terracotta"
            )
        });
        let valid_surface_finish = binding
            .surface_finish_token
            .as_deref()
            .is_none_or(|token| {
                matches!(
                    token,
                    "brushed_metal"
                        | "polished_metal"
                        | "ceramic_coat"
                        | "glossy_coat"
                        | "matte_coat"
                        | "rubberized"
                        | "dark_glass"
                        | "emissive_trim"
                        | "wood_grain"
                        | "bark_ridged"
                        | "leaf_waxy"
                        | "fabric_weave"
                        | "fur_soft"
                        | "skin_matte"
                        | "stone_rough"
                        | "concrete_rough"
                        | "clay_matte"
                )
            });
        if binding.schema_version != REFERENCE_SURFACE_APPEARANCE_BINDING_SCHEMA_VERSION
            || !ids.insert(binding.evidence_id.as_str())
            || !is_sha256(&binding.evidence_sha256)
            || expected.get(binding.evidence_id.as_str()).copied()
                != Some(binding.evidence_sha256.as_str())
            || binding.binding_sha256 != reference_surface_appearance_binding_sha256(binding)?
            || !matches!(
                binding.roughness_motif.as_str(),
                "linear_brush" | "edge_wear" | "microgrid"
            )
            || !valid_base_color
            || !valid_surface_finish
        {
            return Err(invalid(
                "REFERENCE_SURFACE_APPEARANCE_BINDING_INVALID",
                "reference surface appearance binding is unbounded, detached or hash-drifted",
            ));
        }
        binding.facts.validate()?;
    }
    Ok(())
}

fn reference_surface_appearance_binding_sha256(
    binding: &ReferenceSurfaceAppearanceBinding,
) -> CoreResult<String> {
    semantic_sha256(&serde_json::json!({
        "schema_version": binding.schema_version,
        "evidence_id": binding.evidence_id,
        "evidence_sha256": binding.evidence_sha256,
        "facts": binding.facts,
        "base_color_token": binding.base_color_token,
        "surface_finish_token": binding.surface_finish_token,
        "roughness_motif": binding.roughness_motif,
    }))
}

fn reference_surface_color_buckets(
    facts: &ReferenceImageSurfaceFacts,
) -> &[ReferenceImageColorBucket] {
    if facts.foreground_dominant_color_buckets.is_empty() {
        &facts.dominant_color_buckets
    } else {
        &facts.foreground_dominant_color_buckets
    }
}

fn reference_surface_base_color_token(
    facts: &ReferenceImageSurfaceFacts,
) -> Option<&'static str> {
    match reference_surface_color_buckets(facts).first().copied() {
        Some(ReferenceImageColorBucket::White) => Some("silver"),
        Some(ReferenceImageColorBucket::Gray) => Some("gunmetal"),
        Some(ReferenceImageColorBucket::Black) => Some("graphite"),
        Some(ReferenceImageColorBucket::Red) => Some("signal_red"),
        _ => None,
    }
}

fn reference_surface_finish_hint(
    facts: &ReferenceImageSurfaceFacts,
    base_color_token: Option<&str>,
) -> Option<&'static str> {
    match (base_color_token, facts.brightness, facts.edge_density) {
        (Some("silver"), ReferenceImageBrightnessBucket::Bright, ReferenceImageEdgeDensityBucket::Low) => {
            Some("polished_metal")
        }
        (Some("silver" | "gunmetal"), _, ReferenceImageEdgeDensityBucket::High) => {
            Some("brushed_metal")
        }
        (Some("graphite"), ReferenceImageBrightnessBucket::Dark, _) => Some("matte_coat"),
        _ => None,
    }
}

fn reference_surface_roughness_motif(facts: &ReferenceImageSurfaceFacts) -> &'static str {
    match facts.edge_density {
        ReferenceImageEdgeDensityBucket::High => "microgrid",
        ReferenceImageEdgeDensityBucket::Medium => "edge_wear",
        ReferenceImageEdgeDensityBucket::Low => "linear_brush",
    }
}

/// Lower only observed/inferred line-like evidence into the bounded retained
/// vector layer. The path is a 2D PBR marking; it is never a geometry edge,
/// CAD sketch, free-form SVG, or provider-authored curve. Keeping the path
/// deterministic gives the Worker a real visible seam/trim response while
/// preserving the single Rust-owned source of truth.
fn feature_driven_vector_paths(
    feature_text: &str,
    has_vector_channel: bool,
    seed: u32,
    path_id: &str,
) -> Vec<VectorPath> {
    if !has_vector_channel
        || !contains_any(
            feature_text,
            &[
                "seam",
                "panel",
                "groove",
                "slot",
                "trim",
                "contour",
                "line",
                "strip",
                "edge",
                "rim",
                "接缝",
                "面板",
                "凹槽",
                "槽",
                "饰条",
                "轮廓",
                "边缘",
            ],
        )
    {
        return Vec::new();
    }

    let path = if contains_any(
        feature_text,
        &["edge", "rim", "border", "轮廓", "边缘", "边框"],
    ) {
        VectorPath {
            path_id: path_id.into(),
            closed: true,
            commands: vec![
                VectorPathCommand {
                    kind: "move".into(),
                    points: vec![[0.16, 0.18]],
                },
                VectorPathCommand {
                    kind: "line".into(),
                    points: vec![[0.84, 0.18]],
                },
                VectorPathCommand {
                    kind: "line".into(),
                    points: vec![[0.84, 0.82]],
                },
                VectorPathCommand {
                    kind: "line".into(),
                    points: vec![[0.16, 0.82]],
                },
            ],
        }
    } else {
        let offset = if seed & 1 == 0 { 0.0 } else { 0.04 };
        VectorPath {
            path_id: path_id.into(),
            closed: false,
            commands: vec![
                VectorPathCommand {
                    kind: "move".into(),
                    points: vec![[0.18, 0.28 + offset]],
                },
                VectorPathCommand {
                    kind: "line".into(),
                    points: vec![[0.42, 0.44 + offset]],
                },
                VectorPathCommand {
                    kind: "quadratic".into(),
                    points: vec![[0.58, 0.56 + offset], [0.82, 0.72 + offset]],
                },
            ],
        }
    };
    vec![path]
}

fn feature_driven_decal(
    feature_text: &str,
    has_base_color_channel: bool,
    salience_bps: u16,
    seed: u32,
    decal_id: &str,
) -> Vec<DecalLayer> {
    if !has_base_color_channel {
        return Vec::new();
    }
    let motif = if contains_any(
        feature_text,
        &["warning", "hazard", "caution", "stripe", "警示", "警告", "条纹"],
    ) {
        "warning_stripe"
    } else if contains_any(feature_text, &["hex", "badge", "六边形", "徽章"]) {
        "hex_badge"
    } else if contains_any(feature_text, &["chevron", "arrow", "人字", "箭头"]) {
        "chevron_mark"
    } else if contains_any(
        feature_text,
        &[
            "decal", "logo", "label", "mark", "marking", "serial", "insignia", "贴花",
            "标识", "标记", "铭牌", "编号",
        ],
    ) {
        "panel_label"
    } else {
        return Vec::new();
    };
    let color_token = if contains_any(
        feature_text,
        &["warning", "hazard", "caution", "red", "signal", "警示", "警告", "红"],
    ) {
        "signal_red"
    } else if contains_any(feature_text, &["blue", "energy", "luminous", "蓝", "能量"]) {
        "accent_blue"
    } else if contains_any(feature_text, &["silver", "aluminum", "metal", "银", "铝", "金属"]) {
        "aluminum"
    } else {
        "graphite"
    };
    let text_token = if motif == "warning_stripe" {
        "CAUTION"
    } else if contains_any(feature_text, &["serial", "编号"]) {
        "A-01"
    } else if motif == "panel_label" {
        "SERVICE"
    } else {
        "none"
    };
    let anchor_uv = if contains_any(feature_text, &["edge", "rim", "border", "边缘", "边框"]) {
        [0.16, 0.5]
    } else if contains_any(feature_text, &["center", "core", "中心", "核心"]) {
        [0.5, 0.5]
    } else if seed & 1 == 0 {
        [0.5, 0.34]
    } else {
        [0.5, 0.66]
    };
    let scale_milli = if salience_bps >= 9_000 {
        240
    } else if salience_bps <= 5_000 {
        120
    } else {
        180
    };
    let opacity_milli = if salience_bps >= 9_000 { 860 } else { 700 };
    vec![DecalLayer {
        decal_id: decal_id.into(),
        motif: motif.into(),
        text_token: text_token.into(),
        color_token: color_token.into(),
        anchor_uv,
        scale_milli,
        opacity_milli,
    }]
}

fn generic_hard_surface_layer_role(semantic_role: &str) -> &str {
    match semantic_role {
        "primary_shell" | "secondary_shell" | "armor_panel" | "mechanical_core"
        | "sensor_housing" | "structural_frame" | "exterior_panel" | "decorative_panel"
        | "accent_trim" | "enclosure" | "body_shell" | "surface_trim" => semantic_role,
        role if role.contains("trim") || role.contains("accent") => "accent_trim",
        role if role.contains("armor") || role.contains("shell") => "primary_shell",
        role if role.contains("sensor") => "sensor_housing",
        role if role.contains("frame") || role.contains("structure") => "structural_frame",
        _ => "exterior_panel",
    }
}

fn default_appearance_evidence(
    request: &UniversalAuthorRequest,
    request_sha256: &str,
) -> AppearanceEvidenceBundle {
    AppearanceEvidenceBundle {
        schema_version: APPEARANCE_EVIDENCE_BUNDLE_SCHEMA_VERSION.into(),
        bundle_id: format!("appearance_evidence_{}", &request_sha256[..24]),
        request_sha256: request_sha256.into(),
        references: request
            .reference_inputs
            .iter()
            .map(|reference| AppearanceEvidenceReference {
                evidence_id: reference.evidence_id.clone(),
                evidence_sha256: reference.evidence_sha256.clone(),
            })
            .collect(),
        camera_hypotheses: request
            .reference_inputs
            .iter()
            .map(|reference| ReferenceCameraHypothesis {
                schema_version: REFERENCE_CAMERA_HYPOTHESIS_SCHEMA_VERSION.into(),
                hypothesis_id: format!("camera_hypothesis_{}", reference.evidence_id),
                evidence_id: reference.evidence_id.clone(),
                view_id: reference.view_hint.clone(),
                projection_type: ReferenceProjectionType::Unknown,
                parameter_source: CameraParameterSource::Unresolved,
                vertical_fov_millidegrees: None,
                reprojection_error_bps: None,
                landmark_feature_ids: Vec::new(),
                confidence_bps: 0,
                unresolved_fields: vec![
                    "projection_type".into(),
                    "extrinsics".into(),
                    "intrinsics".into(),
                ],
            })
            .collect(),
            derived_artifacts: Vec::new(),
            projection_receipts: Vec::new(),
        }
}

fn validate_v2_components(
    source: &UniversalAssetSourceV2,
    procedural: &UniversalProceduralSourceV2,
) -> CoreResult<()> {
    let profile_parts = source
        .subject_profile
        .parts
        .iter()
        .map(|part| part.part_id.as_str())
        .collect::<BTreeSet<_>>();
    let plans = source
        .representation_plan
        .parts
        .iter()
        .map(|part| (part.part_id.as_str(), part))
        .collect::<BTreeMap<_, _>>();
    let bindings = procedural
        .part_bindings
        .iter()
        .map(|binding| (binding.subject_part_id.as_str(), binding))
        .collect::<BTreeMap<_, _>>();
    let mut ids = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for component in &source.component_sources {
        let Some(plan) = plans.get(component.subject_part_id.as_str()) else {
            return Err(invalid(
                "UNIVERSAL_V2_COMPONENT_PLAN_INVALID",
                "component references no planned part",
            ));
        };
        let Some(binding) = bindings.get(component.subject_part_id.as_str()) else {
            return Err(invalid(
                "UNIVERSAL_V2_COMPONENT_BINDING_INVALID",
                "component references no procedural output binding",
            ));
        };
        if !profile_parts.contains(component.subject_part_id.as_str())
            || !ids.insert(component.component_source_id.as_str())
            || !covered.insert(component.subject_part_id.as_str())
            || (source.representation_source.representation_kind() != RepresentationKind::Hybrid
                && component.representation != source.representation_source.representation_kind())
            || component.representation != plan.representation
            || component.capability_id != plan.capability_id
            || component.source_program_id != procedural.source_program_id
            || component.source_program_sha256 != procedural.source_program_sha256
            || component.source_program_part_ids != vec![binding.output_id.clone()]
        {
            return Err(invalid(
                "UNIVERSAL_V2_COMPONENT_INVALID",
                "component must exactly bind its planned procedural output",
            ));
        }
    }
    if covered != profile_parts {
        return Err(invalid(
            "UNIVERSAL_V2_COMPONENT_INCOMPLETE",
            "every SubjectProfile part requires one UAS@2 component",
        ));
    }
    Ok(())
}

fn validate_v2_detail_claims(
    source: &UniversalAssetSourceV2,
    procedural: &UniversalProceduralSourceV2,
) -> CoreResult<()> {
    let requirements = source
        .visual_feature_contract
        .requirements
        .iter()
        .map(|item| (item.feature_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let operation_ids = procedural
        .part_bindings
        .iter()
        .map(|binding| binding.terminal_operation_id.as_str())
        .collect::<BTreeSet<_>>();
    let zone_ids = procedural
        .part_bindings
        .iter()
        .map(|binding| material_zone_key(&binding.subject_part_id, &binding.material_zone_id))
        .collect::<BTreeSet<_>>();
    let mut claims = BTreeSet::new();
    let mut features = BTreeSet::new();
    for claim in &source.detail_claims {
        let Some(requirement) = requirements.get(claim.feature_id.as_str()) else {
            return Err(invalid(
                "UNIVERSAL_V2_DETAIL_FEATURE_UNKNOWN",
                "detail claim references no visual feature",
            ));
        };
        if claim.schema_version != VISUAL_DETAIL_CLAIM_V2_SCHEMA_VERSION
            || !claims.insert(claim.claim_id.as_str())
            || !features.insert(claim.feature_id.as_str())
            || claim.level != requirement.level
            || claim.evidence_status != requirement.evidence_status
            || claim.salience_bps != requirement.salience_bps
            || claim.affected_part_ids != requirement.affected_part_ids
            || claim.channels != requirement.channels
            || claim.minimum_acceptance_views != requirement.minimum_acceptance_views
            || claim.bindings.is_empty()
        {
            return Err(invalid(
                "UNIVERSAL_V2_DETAIL_INVALID",
                "detail claim must exactly preserve its visual contract",
            ));
        }
        for binding in &claim.bindings {
            let valid = match binding.kind {
                UniversalDetailBindingKind::ProceduralProgram => {
                    binding.source_id == procedural.source_program_id
                }
                UniversalDetailBindingKind::GeometryOutput => {
                    operation_ids.contains(binding.source_id.as_str())
                }
                UniversalDetailBindingKind::MaterialZone => {
                    zone_ids.contains(binding.source_id.as_str())
                }
                UniversalDetailBindingKind::SurfaceProgram
                | UniversalDetailBindingKind::ProjectionLayer
                | UniversalDetailBindingKind::Unresolved => false,
            };
            if !valid {
                return Err(invalid(
                    "UNIVERSAL_V2_DETAIL_BINDING_INVALID",
                    "detail binding is not a real UAS@2 source",
                ));
            }
        }
    }
    if features.len() != requirements.len() {
        return Err(invalid(
            "UNIVERSAL_V2_DETAIL_INCOMPLETE",
            "every visual feature requires one UAS@2 detail claim",
        ));
    }
    Ok(())
}

fn validate_v2_material_zones(
    source: &UniversalAssetSourceV2,
    procedural: &UniversalProceduralSourceV2,
) -> CoreResult<()> {
    let expected = procedural
        .part_bindings
        .iter()
        .map(|binding| {
            (
                material_zone_key(&binding.subject_part_id, &binding.material_zone_id),
                (
                    binding.subject_part_id.as_str(),
                    binding.material_zone_id.as_str(),
                    binding.material_id.as_str(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let actual = source
        .material_zones
        .iter()
        .map(|zone| {
            (
                zone.appearance_id.clone(),
                (
                    zone.source_part_id.as_str(),
                    zone.material_zone_id.as_str(),
                    zone.base_material_id.as_str(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if source.material_zones.iter().any(|zone| {
        zone.schema_version != MATERIAL_ZONE_APPEARANCE_SCHEMA_VERSION
            || zone.channels.is_empty()
            || zone.texture_width == 0
            || zone.texture_height == 0
            || zone.texture_width > MAX_TEXTURE_EDGE
            || zone.texture_height > MAX_TEXTURE_EDGE
    }) || actual.len() != source.material_zones.len()
        || actual != expected
    {
        return Err(invalid(
            "UNIVERSAL_V2_MATERIAL_ZONE_INVALID",
            "UAS@2 material zones must exactly bind procedural part outputs",
        ));
    }
    let artifacts = source
        .appearance_evidence
        .derived_artifacts
        .iter()
        .map(|artifact| (artifact.artifact_id.as_str(), artifact))
        .collect::<BTreeMap<_, _>>();
    let cameras = source
        .appearance_evidence
        .camera_hypotheses
        .iter()
        .map(|camera| (camera.hypothesis_id.as_str(), camera))
        .collect::<BTreeMap<_, _>>();
    let mut layers = BTreeSet::<String>::new();
    for zone in &source.material_zones {
        if zone.channels.iter().collect::<BTreeSet<_>>().len() != zone.channels.len() {
            return Err(invalid(
                "UNIVERSAL_V2_MATERIAL_ZONE_INVALID",
                "UAS@2 material zone PBR channels must be unique.",
            ));
        }
        for layer in &zone.projection_layers {
            validate_projection_layer(layer, zone, &artifacts, &cameras, &mut layers)?;
        }
    }
    Ok(())
}

fn validate_v2_compiled_binding(
    binding: &UniversalCompiledArtifactBinding,
    procedural: &UniversalProceduralSourceV2,
) -> CoreResult<()> {
    if binding.source_program_sha256 != procedural.source_program_sha256
        || binding.shape_program_sha256 != procedural.shape_program_sha256
        || !is_sha256(&binding.glb_sha256)
        || !is_sha256(&binding.readback_sha256)
        || !is_sha256(&binding.compile_readback_sha256)
        || binding.artifact_profile_id.trim().is_empty()
        || binding.renderer_id.trim().is_empty()
        || binding.view_sha256.is_empty()
        || binding.view_sha256.len() > 16
        || binding.view_sha256.values().any(|hash| !is_sha256(hash))
    {
        return Err(invalid(
            "UNIVERSAL_V2_COMPILED_BINDING_INVALID",
            "compiled artifact must bind the exact UAS@2 procedural source",
        ));
    }
    Ok(())
}

fn validate_appearance_evidence(
    bundle: &AppearanceEvidenceBundle,
    request: &UniversalAuthorRequest,
) -> CoreResult<()> {
    if bundle.schema_version != APPEARANCE_EVIDENCE_BUNDLE_SCHEMA_VERSION
        || bundle.request_sha256 != semantic_sha256(request)?
    {
        return Err(invalid(
            "APPEARANCE_EVIDENCE_LINEAGE_INVALID",
            "Appearance evidence must bind the exact universal request.",
        ));
    }
    let expected = request
        .reference_inputs
        .iter()
        .map(|item| (item.evidence_id.as_str(), item.evidence_sha256.as_str()))
        .collect::<BTreeMap<_, _>>();
    let actual = bundle
        .references
        .iter()
        .map(|item| (item.evidence_id.as_str(), item.evidence_sha256.as_str()))
        .collect::<BTreeMap<_, _>>();
    if expected != actual || actual.len() != bundle.references.len() {
        return Err(invalid(
            "APPEARANCE_EVIDENCE_REFERENCE_INVALID",
            "Appearance evidence references must exactly match the sealed request.",
        ));
    }
    let mut camera_ids = BTreeSet::new();
    for camera in &bundle.camera_hypotheses {
        if camera.schema_version != REFERENCE_CAMERA_HYPOTHESIS_SCHEMA_VERSION
            || !expected.contains_key(camera.evidence_id.as_str())
            || camera.confidence_bps > 10_000
            || camera
                .reprojection_error_bps
                .is_some_and(|value| value > 10_000)
            || !camera_ids.insert(camera.hypothesis_id.as_str())
        {
            return Err(invalid(
                "REFERENCE_CAMERA_HYPOTHESIS_INVALID",
                "Reference camera hypothesis is unbounded, duplicated or detached from evidence.",
            ));
        }
        if camera.parameter_source == CameraParameterSource::Unresolved
            && (camera.projection_type != ReferenceProjectionType::Unknown
                || camera.vertical_fov_millidegrees.is_some()
                || camera.reprojection_error_bps.is_some()
                || camera.confidence_bps != 0
                || camera.unresolved_fields.is_empty())
        {
            return Err(invalid(
                "REFERENCE_CAMERA_FALSE_SOLVE",
                "An unresolved camera cannot claim solved projection parameters or confidence.",
            ));
        }
        if camera.parameter_source != CameraParameterSource::Unresolved
            && camera.confidence_bps == 0
        {
            return Err(invalid(
                "REFERENCE_CAMERA_EVIDENCE_REQUIRED",
                "A fitted camera requires bounded positive confidence.",
            ));
        }
    }
    let mut artifact_ids = BTreeSet::new();
    for artifact in &bundle.derived_artifacts {
        if !artifact.evidence_only
            || !expected.contains_key(artifact.evidence_id.as_str())
            || !is_sha256(&artifact.content_sha256)
            || artifact.algorithm_id.trim().is_empty()
            || artifact.algorithm_version.trim().is_empty()
            || !artifact_ids.insert(artifact.artifact_id.as_str())
        {
            return Err(invalid(
                "APPEARANCE_DERIVED_EVIDENCE_INVALID",
                "Derived appearance data must remain hash-bound evidence with algorithm provenance.",
            ));
        }
    }
    if bundle.projection_receipts.len() > MAX_MATERIAL_ZONES {
        return Err(invalid(
            "REFERENCE_APPEARANCE_PROJECTION_RECEIPT_INVALID",
            "Reference appearance projection receipts exceed the bounded material-zone budget.",
        ));
    }
    let mut projection_ids = BTreeSet::new();
    for receipt in &bundle.projection_receipts {
        receipt.validate()?;
        if receipt.source_request_sha256 != bundle.request_sha256
            || !expected.contains_key(receipt.source_evidence_id.as_str())
            || !projection_ids.insert(receipt.projection_id.as_str())
        {
            return Err(invalid(
                "REFERENCE_APPEARANCE_PROJECTION_LINEAGE_INVALID",
                "Reference appearance projection receipts must bind the exact request and sealed evidence.",
            ));
        }
    }
    Ok(())
}

fn validate_components(source: &UniversalAssetSource) -> CoreResult<()> {
    let profile_parts = source
        .subject_profile
        .parts
        .iter()
        .map(|part| part.part_id.as_str())
        .collect::<BTreeSet<_>>();
    let program_parts = source
        .procedural_source
        .program
        .parts
        .iter()
        .map(|part| part.part_id.as_str())
        .collect::<BTreeSet<_>>();
    let plan = source
        .representation_plan
        .parts
        .iter()
        .map(|part| (part.part_id.as_str(), part))
        .collect::<BTreeMap<_, _>>();
    let mut ids = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for component in &source.component_sources {
        let Some(part_plan) = plan.get(component.subject_part_id.as_str()) else {
            return Err(invalid(
                "UNIVERSAL_COMPONENT_PLAN_INVALID",
                "Component source references no representation plan Part.",
            ));
        };
        if !profile_parts.contains(component.subject_part_id.as_str())
            || !ids.insert(component.component_source_id.as_str())
            || !covered.insert(component.subject_part_id.as_str())
            || component.representation != part_plan.representation
            || component.capability_id != part_plan.capability_id
            || component.source_program_id != source.procedural_source.program.program_id
            || component.source_program_sha256 != source.procedural_source.source_program_sha256
            || component.source_program_part_ids.is_empty()
            || component
                .source_program_part_ids
                .iter()
                .any(|part_id| !program_parts.contains(part_id.as_str()))
        {
            return Err(invalid(
                "UNIVERSAL_COMPONENT_SOURCE_INVALID",
                "Component sources must bind one planned Subject Part to real procedural Part IDs.",
            ));
        }
    }
    if covered != profile_parts {
        return Err(invalid(
            "UNIVERSAL_COMPONENT_SOURCE_INCOMPLETE",
            "Every SubjectProfile Part requires one universal component source.",
        ));
    }
    Ok(())
}

fn validate_detail_claims(source: &UniversalAssetSource) -> CoreResult<()> {
    let requirements = source
        .visual_feature_contract
        .requirements
        .iter()
        .map(|requirement| (requirement.feature_id.as_str(), requirement))
        .collect::<BTreeMap<_, _>>();
    let program_details = source
        .procedural_source
        .program
        .detail_inventory
        .iter()
        .map(|detail| detail.detail_id.as_str())
        .collect::<BTreeSet<_>>();
    let program_zones = source
        .procedural_source
        .program
        .material_graph
        .iter()
        .map(|binding| material_zone_key(&binding.part_id, &binding.material_zone_id))
        .collect::<BTreeSet<_>>();
    let mut claims = BTreeSet::new();
    let mut features = BTreeSet::new();
    for claim in &source.detail_claims {
        let Some(requirement) = requirements.get(claim.feature_id.as_str()) else {
            return Err(invalid(
                "UNIVERSAL_DETAIL_FEATURE_UNKNOWN",
                "Visual detail claim references no feature requirement.",
            ));
        };
        if claim.schema_version != VISUAL_DETAIL_CLAIM_V2_SCHEMA_VERSION
            || !claims.insert(claim.claim_id.as_str())
            || !features.insert(claim.feature_id.as_str())
            || claim.level != requirement.level
            || claim.evidence_status != requirement.evidence_status
            || claim.salience_bps != requirement.salience_bps
            || claim.affected_part_ids != requirement.affected_part_ids
            || claim.channels != requirement.channels
            || claim.minimum_acceptance_views != requirement.minimum_acceptance_views
            || claim.bindings.is_empty()
        {
            return Err(invalid(
                "UNIVERSAL_DETAIL_CLAIM_INVALID",
                "Visual detail claim must reproduce and bind one exact feature requirement.",
            ));
        }
        for binding in &claim.bindings {
            let valid = match binding.kind {
                UniversalDetailBindingKind::ProceduralProgram => {
                    binding.source_id == source.procedural_source.program.program_id
                }
                UniversalDetailBindingKind::GeometryOutput => {
                    program_details.contains(binding.source_id.as_str())
                }
                UniversalDetailBindingKind::MaterialZone => {
                    program_zones.contains(binding.source_id.as_str())
                }
                UniversalDetailBindingKind::SurfaceProgram => source
                    .procedural_source
                    .program
                    .surface_graph
                    .iter()
                    .any(|item| item.surface_program_id == binding.source_id),
                UniversalDetailBindingKind::ProjectionLayer => source
                    .material_zones
                    .iter()
                    .flat_map(|zone| zone.projection_layers.iter())
                    .any(|layer| layer.layer_id == binding.source_id),
                UniversalDetailBindingKind::Unresolved => false,
            };
            if !valid {
                return Err(invalid(
                    "UNIVERSAL_DETAIL_BINDING_INVALID",
                    "Visual detail binding does not resolve to the sealed design source.",
                ));
            }
        }
    }
    if features.len() != requirements.len() {
        return Err(invalid(
            "UNIVERSAL_DETAIL_CLAIMS_INCOMPLETE",
            "Every visual feature requirement requires exactly one source-bound detail claim.",
        ));
    }
    Ok(())
}

fn validate_material_zones(source: &UniversalAssetSource) -> CoreResult<()> {
    let expected = source
        .procedural_source
        .program
        .material_graph
        .iter()
        .map(|binding| {
            (
                material_zone_key(&binding.part_id, &binding.material_zone_id),
                (
                    binding.part_id.as_str(),
                    binding.material_zone_id.as_str(),
                    binding.material_id.as_str(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let artifacts = source
        .appearance_evidence
        .derived_artifacts
        .iter()
        .map(|artifact| (artifact.artifact_id.as_str(), artifact))
        .collect::<BTreeMap<_, _>>();
    let cameras = source
        .appearance_evidence
        .camera_hypotheses
        .iter()
        .map(|camera| (camera.hypothesis_id.as_str(), camera))
        .collect::<BTreeMap<_, _>>();
    let mut zones = BTreeSet::new();
    let mut layers = BTreeSet::new();
    for zone in &source.material_zones {
        if zone.schema_version != MATERIAL_ZONE_APPEARANCE_SCHEMA_VERSION
            || expected.get(zone.appearance_id.as_str())
                != Some(&(
                    zone.source_part_id.as_str(),
                    zone.material_zone_id.as_str(),
                    zone.base_material_id.as_str(),
                ))
            || !zones.insert(zone.appearance_id.as_str())
            || zone.transmission_bps > 10_000
            || zone.uncertainty_bps > 10_000
            || zone.texture_width == 0
            || zone.texture_height == 0
            || zone.texture_width > MAX_TEXTURE_EDGE
            || zone.texture_height > MAX_TEXTURE_EDGE
            || zone.channels.is_empty()
        {
            return Err(invalid(
                "MATERIAL_ZONE_APPEARANCE_INVALID",
                "Material appearance must bind one real source zone within PBR budgets.",
            ));
        }
        let channel_set = zone.channels.iter().collect::<BTreeSet<_>>();
        if channel_set.len() != zone.channels.len() {
            return Err(invalid(
                "MATERIAL_ZONE_CHANNEL_DUPLICATE",
                "Material appearance channels must be unique.",
            ));
        }
        for layer in &zone.projection_layers {
            validate_projection_layer(layer, zone, &artifacts, &cameras, &mut layers)?;
        }
    }
    if zones.len() != expected.len() {
        return Err(invalid(
            "MATERIAL_ZONE_APPEARANCE_INCOMPLETE",
            "Every procedural Material Zone requires one appearance contract.",
        ));
    }
    let known_zone_ids = source
        .material_zones
        .iter()
        .map(|zone| zone.material_zone_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut receipt_zones = BTreeSet::new();
    for receipt in &source.appearance_evidence.projection_receipts {
        if !known_zone_ids.contains(receipt.target_material_zone_id.as_str())
            || !receipt_zones.insert(receipt.target_material_zone_id.as_str())
        {
            return Err(invalid(
                "REFERENCE_APPEARANCE_PROJECTION_ZONE_INVALID",
                "Reference appearance projection receipt must target one unique retained Material Zone.",
            ));
        }
    }
    Ok(())
}

/// A projection layer carries observed appearance evidence only when all of
/// its inputs belong to the same sealed view and the camera has actually been
/// solved.  Keeping these checks in Core prevents an author/provider from
/// calling a guessed front projection "observed" or from painting a texture
/// onto a part with an unrelated reference image.
fn validate_projection_layer(
    layer: &AppearanceProjectionLayer,
    zone: &MaterialZoneAppearance,
    artifacts: &BTreeMap<&str, &AppearanceEvidenceArtifact>,
    cameras: &BTreeMap<&str, &ReferenceCameraHypothesis>,
    layer_ids: &mut BTreeSet<String>,
) -> CoreResult<()> {
    let Some(evidence) = artifacts.get(layer.evidence_artifact_id.as_str()) else {
        return Err(invalid(
            "APPEARANCE_PROJECTION_LAYER_INVALID",
            "Projection layers require a sealed derived appearance artifact.",
        ));
    };
    let Some(unobserved_mask) = artifacts.get(layer.unobserved_texel_mask_artifact_id.as_str())
    else {
        return Err(invalid(
            "APPEARANCE_PROJECTION_LAYER_INVALID",
            "Projection layers require a sealed unobserved-texel mask artifact.",
        ));
    };
    let Some(camera) = cameras.get(layer.camera_hypothesis_id.as_str()) else {
        return Err(invalid(
            "APPEARANCE_PROJECTION_LAYER_INVALID",
            "Projection layers require a sealed fitted reference camera.",
        ));
    };
    let zone_channels = zone.channels.iter().collect::<BTreeSet<_>>();
    let layer_channels = layer.channels.iter().collect::<BTreeSet<_>>();
    let evidence_channels_match = match evidence.kind {
        AppearanceEvidenceArtifactKind::DelightedColorHint
        | AppearanceEvidenceArtifactKind::Region => layer.channels.iter().all(|channel| {
            matches!(
                channel,
                PbrTextureChannel::BaseColor | PbrTextureChannel::Opacity
            )
        }),
        AppearanceEvidenceArtifactKind::NormalHint => layer
            .channels
            .iter()
            .all(|channel| *channel == PbrTextureChannel::Normal),
        AppearanceEvidenceArtifactKind::RoughnessHint => layer
            .channels
            .iter()
            .all(|channel| *channel == PbrTextureChannel::Roughness),
        AppearanceEvidenceArtifactKind::MetallicHint => layer
            .channels
            .iter()
            .all(|channel| *channel == PbrTextureChannel::Metallic),
        AppearanceEvidenceArtifactKind::Mask
        | AppearanceEvidenceArtifactKind::UnobservedTexelMask => false,
    };
    if !layer_ids.insert(layer.layer_id.clone())
        || layer.layer_id.trim().is_empty()
        || layer.channels.is_empty()
        || layer_channels.len() != layer.channels.len()
        || !layer_channels.is_subset(&zone_channels)
        || layer.evidence_artifact_id == layer.unobserved_texel_mask_artifact_id
        || unobserved_mask.kind != AppearanceEvidenceArtifactKind::UnobservedTexelMask
        || evidence.evidence_id != unobserved_mask.evidence_id
        || evidence.evidence_id != camera.evidence_id
        || camera.parameter_source == CameraParameterSource::Unresolved
        || camera.projection_type == ReferenceProjectionType::Unknown
        || camera.confidence_bps == 0
        || !camera.unresolved_fields.is_empty()
        || !evidence_channels_match
    {
        return Err(invalid(
            "APPEARANCE_PROJECTION_LAYER_INVALID",
            "Projection layers require channel-compatible evidence, a fully fitted same-view camera and an exact unobserved-texel mask.",
        ));
    }
    Ok(())
}

fn validate_compiled_binding(
    binding: &UniversalCompiledArtifactBinding,
    revision: &ForgeVisualProgramRevision,
) -> CoreResult<()> {
    if binding.source_program_sha256 != revision.source_program_sha256
        || !is_sha256(&binding.shape_program_sha256)
        || !is_sha256(&binding.glb_sha256)
        || !is_sha256(&binding.readback_sha256)
        || !is_sha256(&binding.compile_readback_sha256)
        || binding.artifact_profile_id.trim().is_empty()
        || binding.renderer_id.trim().is_empty()
        || binding.view_sha256.is_empty()
        || binding
            .view_sha256
            .iter()
            .any(|(view_id, sha)| view_id.trim().is_empty() || !is_sha256(sha))
    {
        return Err(invalid(
            "UNIVERSAL_COMPILED_ARTIFACT_INVALID",
            "Compiled universal source must bind one exact program/ShapeProgram/GLB/readback/view set.",
        ));
    }
    Ok(())
}

fn visual_levels_match(source: VisualDetailLevel, target: crate::VisualFeatureLevel) -> bool {
    matches!(
        (source, target),
        (VisualDetailLevel::Macro, crate::VisualFeatureLevel::Macro)
            | (VisualDetailLevel::Meso, crate::VisualFeatureLevel::Meso)
            | (VisualDetailLevel::Micro, crate::VisualFeatureLevel::Micro)
    )
}

fn material_zone_key(part_id: &str, material_zone_id: &str) -> String {
    format!("{part_id}:{material_zone_id}")
}

fn validate_game_asset_delivery_receipt(
    source: &UniversalAssetSourceV2,
    profile: &crate::GameAssetProfile,
    delivery: &crate::GameAssetDeliveryReadback,
    procedural: &UniversalProceduralSourceV2,
) -> CoreResult<()> {
    let compiled = source.compiled_artifact.as_ref().ok_or_else(|| {
        invalid(
            "GAME_ASSET_DELIVERY_STATE_INVALID",
            "A game delivery receipt requires one compiled UAS@2 source artifact.",
        )
    })?;
    let expected_profile_sha256 = crate::semantic_sha256(profile)?;
    let terminal_by_part = procedural
        .part_bindings
        .iter()
        .map(|binding| {
            (
                binding.subject_part_id.as_str(),
                binding.terminal_operation_id.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let requested_parts = profile
        .collision_proxy_part_ids
        .iter()
        .chain(profile.sockets.iter().map(|socket| &socket.part_id))
        .collect::<BTreeSet<_>>();
    let expected_bindings = crate::GameAssetDeliveryBindings {
        schema_version: crate::GAME_ASSET_DELIVERY_BINDINGS_SCHEMA_VERSION.into(),
        source_id: source.source_id.clone(),
        source_request_sha256: source.request_sha256.clone(),
        game_asset_profile_sha256: expected_profile_sha256.clone(),
        parts: requested_parts
            .iter()
            .map(|part_id| {
                let terminal_operation_id = terminal_by_part.get(part_id.as_str()).ok_or_else(|| {
                    invalid(
                        "GAME_ASSET_DELIVERY_PART_BINDING_INVALID",
                        "A game delivery profile part has no exact executable terminal binding.",
                    )
                })?;
                Ok(crate::GameAssetDeliveryPartBinding {
                    subject_part_id: (*part_id).clone(),
                    terminal_operation_id: (*terminal_operation_id).to_string(),
                })
            })
            .collect::<CoreResult<Vec<_>>>()?,
    };
    let expected_bindings_sha256 = crate::semantic_sha256(&expected_bindings)?;
    let expected_collision_parts = profile
        .collision_proxy_part_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let actual_collision_parts = delivery
        .collision_proxies
        .iter()
        .map(|proxy| proxy.subject_part_id.as_str())
        .collect::<BTreeSet<_>>();
    let expected_socket_ids = profile
        .sockets
        .iter()
        .map(|socket| socket.socket_id.as_str())
        .collect::<BTreeSet<_>>();
    let actual_socket_ids = delivery
        .sockets
        .iter()
        .map(|socket| socket.socket_id.as_str())
        .collect::<BTreeSet<_>>();
    let collision_bindings_match = delivery.collision_proxies.iter().all(|proxy| {
        terminal_by_part.get(proxy.subject_part_id.as_str())
            == Some(&proxy.terminal_operation_id.as_str())
            && proxy.mesh_index > 0
            && proxy
                .bounds_meters
                .iter()
                .flatten()
                .all(|value| value.is_finite())
    });
    let socket_profile_by_id = profile
        .sockets
        .iter()
        .map(|socket| (socket.socket_id.as_str(), socket))
        .collect::<BTreeMap<_, _>>();
    let socket_bindings_match = delivery.sockets.iter().all(|socket| {
        socket_profile_by_id
            .get(socket.socket_id.as_str())
            .is_some_and(|profile_socket| {
                socket.subject_part_id == profile_socket.part_id
                    && terminal_by_part.get(socket.subject_part_id.as_str())
                        == Some(&socket.terminal_operation_id.as_str())
                    && socket.node_index > 0
                    && socket.pivot_meters.iter().all(|value| value.is_finite())
                    && socket.forward.iter().all(|value| value.is_finite())
            })
    });
    let mut failures = Vec::new();
    macro_rules! require_delivery {
        ($condition:expr, $label:literal) => {
            if !$condition {
                failures.push($label);
            }
        };
    }
    require_delivery!(
        delivery.schema_version == crate::GAME_ASSET_DELIVERY_RECEIPT_SCHEMA_VERSION,
        "receipt_schema"
    );
    require_delivery!(delivery.source_glb_sha256 == compiled.glb_sha256, "source_hash");
    require_delivery!(is_sha256(&delivery.delivery_glb_sha256), "delivery_hash");
    require_delivery!(
        delivery.delivery_glb_sha256 != delivery.source_glb_sha256,
        "delivery_distinct"
    );
    require_delivery!(
        delivery.game_asset_profile_sha256 == expected_profile_sha256,
        "profile_hash"
    );
    require_delivery!(
        delivery.bindings_sha256 == expected_bindings_sha256,
        "bindings_hash"
    );
    require_delivery!(
        delivery.lod.schema_version == crate::GAME_ASSET_LOD_RECEIPT_SCHEMA_VERSION,
        "lod_schema"
    );
    require_delivery!(delivery.lod.source_glb_sha256 == compiled.glb_sha256, "lod_source_hash");
    require_delivery!(
        is_sha256(&delivery.lod.delivery_glb_sha256),
        "lod_delivery_hash"
    );
    // The LOD receipt hashes the standalone source+LOD GLB. The enclosing
    // delivery receipt hashes that GLB after collision/socket nodes and its
    // own receipt are added, so these are intentionally distinct artifacts.
    require_delivery!(
        delivery.lod.delivery_glb_sha256 != compiled.glb_sha256,
        "lod_delivery_distinct"
    );
    require_delivery!(
        delivery.lod.game_asset_profile_sha256 == expected_profile_sha256,
        "lod_profile_hash"
    );
    require_delivery!(
        delivery.lod.game_asset_profile_id == profile.profile_id,
        "lod_profile_id"
    );
    require_delivery!(delivery.lod.lods[0].level == 0, "lod0_level");
    require_delivery!(delivery.lod.lods[0].triangle_count > 0, "lod0_triangles");
    require_delivery!(delivery.lod.lods[1].level == 1, "lod1_level");
    require_delivery!(delivery.lod.lods[2].level == 2, "lod2_level");
    require_delivery!(
        delivery.lod.lods[1].triangle_count < delivery.lod.lods[0].triangle_count,
        "lod1_not_reduced"
    );
    require_delivery!(
        delivery.lod.lods[2].triangle_count < delivery.lod.lods[1].triangle_count,
        "lod2_not_reduced"
    );
    require_delivery!(
        actual_collision_parts == expected_collision_parts,
        "collision_parts"
    );
    require_delivery!(
        delivery.collision_proxies.len() == expected_collision_parts.len(),
        "collision_count"
    );
    require_delivery!(collision_bindings_match, "collision_bindings");
    require_delivery!(actual_socket_ids == expected_socket_ids, "socket_ids");
    require_delivery!(
        delivery.sockets.len() == expected_socket_ids.len(),
        "socket_count"
    );
    require_delivery!(socket_bindings_match, "socket_bindings");
    require_delivery!(
        delivery.texel_density.target_texel_density_pixels_per_meter
            == profile.target_texel_density_pixels_per_meter,
        "texel_target"
    );
    require_delivery!(delivery.texel_density.target_met, "texel_target_not_met");
    require_delivery!(
        delivery
            .texel_density
            .effective_texel_density_pixels_per_meter
            .is_finite(),
        "texel_effective_density"
    );
    require_delivery!(
        !delivery.texel_density.material_zones.is_empty(),
        "texel_zones"
    );
    if !failures.is_empty() {
        return Err(CoreError::invalid_data(
            "GAME_ASSET_DELIVERY_READBACK_INVALID",
            &format!(
                "Game delivery receipt must prove exact source, LOD, collision, socket and measured texture-density lineage (failed: {}).",
                failures.join(",")
            ),
        ));
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn invalid(code: &'static str, message: &'static str) -> CoreError {
    CoreError::invalid_data(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        representation_capability_manifest_sha256, reviewed_c111_draft_visual_program,
        ForgeVisualProgramRevision, ForgeVisualProgramStage, PartRepresentationPlan,
        ReferenceClass, ReferenceEvidence, ReferenceEvidenceKind, ReferenceEvidenceObservations,
        ReferenceImageBrightnessBucket, ReferenceImageColorBucket, ReferenceImageEdgeDensityBucket,
        ReferenceImageForegroundConfidence, ReferenceImageSurfaceFacts, SubjectFeature,
        SubjectMaterial, SubjectPart, UniversalReferenceInput, VisualFeatureEvidenceRegion,
        VisualFeatureLevel, VisualFeatureRequirement,
        GENERIC_HARD_SURFACE_PROCEDURAL_CAPABILITY_ID, LOCAL_LATTICE_DEFORMABLE_CAPABILITY_ID,
        LOCAL_MESH_PATCH_CAPABILITY_ID,
        ROBOTIC_ARM_PROCEDURAL_CAPABILITY_ID,
    };
    use serde_json::json;

    fn fixture() -> UniversalAssetSource {
        let request = UniversalAuthorRequest {
            schema_version: "UniversalAuthorRequest@1".into(),
            request_id: "uareq_u003_arm".into(),
            project_id: "project_u003".into(),
            turn_id: "turn_u003".into(),
            instruction: "生成银白色机械臂".into(),
            input_mode: crate::UniversalInputMode::Text,
            reference_inputs: Vec::new(),
            active_asset: None,
            selection: Default::default(),
            locks: Default::default(),
            capability_manifest_sha256: representation_capability_manifest_sha256().unwrap(),
        };
        let request_sha256 = semantic_sha256(&request).unwrap();
        let profile = SubjectProfile {
            schema_version: "SubjectProfile@1".into(),
            profile_id: "subject_u003_arm".into(),
            request_sha256: request_sha256.clone(),
            identity_label: "银白色机械臂".into(),
            category: "articulated robotic arm".into(),
            category_tags: vec!["mechanical".into()],
            silhouette: "串联关节长行程轮廓".into(),
            negative_space: "连杆和关节间开放空间".into(),
            pose: "extended".into(),
            visible_views: Vec::new(),
            occlusions: Vec::new(),
            uncertainties: Vec::new(),
            parts: vec![SubjectPart {
                part_id: "part_subject_arm".into(),
                parent_part_id: None,
                label: "机械臂总成".into(),
                semantic_role: "primary_articulated_assembly".into(),
                traits: vec![
                    "articulated_chain".into(),
                    "joint".into(),
                    "end_effector".into(),
                ],
                uncertainty_bps: 500,
            }],
            features: vec![
                feature("feature_u003_macro", VisualFeatureLevel::Macro),
                feature("feature_u003_meso", VisualFeatureLevel::Meso),
                feature("feature_u003_micro", VisualFeatureLevel::Micro),
            ],
            materials: vec![SubjectMaterial {
                material_id: "material_u003_silver".into(),
                label: "银白装甲".into(),
                part_ids: vec!["part_subject_arm".into()],
                appearance_traits: vec!["metallic".into()],
            }],
        };
        let profile_sha256 = semantic_sha256(&profile).unwrap();
        let requirements = vec![
            requirement(
                "feature_u003_macro",
                VisualFeatureLevel::Macro,
                vec![AppearanceChannel::Geometry],
            ),
            requirement(
                "feature_u003_meso",
                VisualFeatureLevel::Meso,
                vec![AppearanceChannel::Geometry, AppearanceChannel::Normal],
            ),
            requirement(
                "feature_u003_micro",
                VisualFeatureLevel::Micro,
                vec![AppearanceChannel::BaseColor, AppearanceChannel::Roughness],
            ),
        ];
        let contract = VisualFeatureContract {
            schema_version: "VisualFeatureContract@1".into(),
            contract_id: "vfcontract_u003".into(),
            request_sha256: request_sha256.clone(),
            subject_profile_sha256: profile_sha256.clone(),
            requirements,
        };
        let plan = RepresentationPlan {
            schema_version: "RepresentationPlan@1".into(),
            plan_id: "repplan_u003".into(),
            request_sha256,
            subject_profile_sha256: profile_sha256,
            visual_feature_contract_sha256: semantic_sha256(&contract).unwrap(),
            capability_manifest_sha256: request.capability_manifest_sha256.clone(),
            parts: vec![PartRepresentationPlan {
                part_id: "part_subject_arm".into(),
                representation: RepresentationKind::Procedural,
                capability_id: ROBOTIC_ARM_PROCEDURAL_CAPABILITY_ID.into(),
                covered_feature_ids: vec![
                    "feature_u003_macro".into(),
                    "feature_u003_meso".into(),
                    "feature_u003_micro".into(),
                ],
                rationale: "当前受检机械臂程序化能力".into(),
            }],
        };
        let mut program = reviewed_c111_draft_visual_program().unwrap();
        program.stage = ForgeVisualProgramStage::Draft;
        let revision =
            ForgeVisualProgramRevision::author(&serde_json::to_value(program).unwrap()).unwrap();
        UniversalAssetSource::from_procedural(&request, &profile, &contract, &plan, &revision)
            .unwrap()
    }

    fn feature(id: &str, level: VisualFeatureLevel) -> SubjectFeature {
        SubjectFeature {
            feature_id: id.into(),
            part_id: "part_subject_arm".into(),
            level,
            description: format!("{id} appearance"),
        }
    }

    fn requirement(
        id: &str,
        level: VisualFeatureLevel,
        channels: Vec<AppearanceChannel>,
    ) -> VisualFeatureRequirement {
        VisualFeatureRequirement {
            feature_id: id.into(),
            level,
            description: format!("{id} requirement"),
            salience_bps: 8_000,
            evidence_status: EvidenceStatus::Inferred,
            evidence_regions: Vec::<VisualFeatureEvidenceRegion>::new(),
            affected_part_ids: vec!["part_subject_arm".into()],
            channels,
            minimum_acceptance_views: vec!["iso".into()],
        }
    }

    fn u004_reference_evidence() -> ReferenceEvidence {
        ReferenceEvidence {
            schema_version: "ReferenceEvidence@1".into(),
            evidence_id: "refevid_u004_image".into(),
            project_id: "project_u004".into(),
            kind: ReferenceEvidenceKind::Image,
            reference_class: ReferenceClass::SingleImage,
            domain_pack_id: "pack_unclassified".into(),
            source_file_name: "u004-reference.png".into(),
            source_media_type: "image/png".into(),
            source_object_sha256: "a".repeat(64),
            source_imported_asset_version_id: None,
            source_statement: "user supplied authorized reference".into(),
            license_statement: "user confirms rights".into(),
            missing_views: vec!["rear".into()],
            user_notes: String::new(),
            observations: ReferenceEvidenceObservations {
                silhouette_summary: "hard surface support bracket".into(),
                proportion_ranges: vec!["visible frontal proportions".into()],
                material_zone_observations: vec!["silver metallic trim".into()],
                visible_part_hypotheses: Vec::new(),
                uncertainties: vec!["rear hidden".into()],
                image_surface_facts: Some(ReferenceImageSurfaceFacts {
                    width: 2,
                    height: 1,
                    aspect_ratio_milli: 2_000,
                    dominant_color_buckets: vec![ReferenceImageColorBucket::Gray],
                    foreground_dominant_color_buckets: Vec::new(),
                    brightness: ReferenceImageBrightnessBucket::Balanced,
                    edge_density: ReferenceImageEdgeDensityBucket::Medium,
                    foreground_bbox_normalized: [0, 0, 1_000, 1_000],
                    contact_sheet_layout_evidence: false,
                    foreground_confidence: ReferenceImageForegroundConfidence::Medium,
                }),
            },
            created_at: "2026-07-30T00:00:00Z".into(),
            glb_inspection: None,
        }
    }

    #[test]
    fn u003_procedural_source_binds_contracts_components_details_and_materials() {
        let source = fixture();
        source.validate().unwrap();
        assert_eq!(source.state, UniversalAssetSourceState::Planned);
        assert_eq!(source.component_sources.len(), 1);
        assert_eq!(source.detail_claims.len(), 3);
        assert!(!source.material_zones.is_empty());
        assert!(source
            .material_zones
            .iter()
            .all(|zone| zone.projection_layers.is_empty()));
        assert!(source.appearance_evidence.derived_artifacts.is_empty());
    }

    #[test]
    fn u004_v2_runtime_source_relowers_and_rejects_part_binding_drift() {
        let evidence = u004_reference_evidence();
        let request = UniversalAuthorRequest {
            schema_version: "UniversalAuthorRequest@1".into(),
            request_id: "uareq_u004_v2".into(),
            project_id: "project_u004".into(),
            turn_id: "turn_u004".into(),
            instruction: "生成模块化机械支架".into(),
            input_mode: crate::UniversalInputMode::SingleImage,
            reference_inputs: vec![UniversalReferenceInput {
                evidence_id: "refevid_u004_image".into(),
                evidence_sha256: semantic_sha256(&evidence).unwrap(),
                role: "primary_reference".into(),
                view_hint: Some("front".into()),
            }],
            active_asset: None,
            selection: Default::default(),
            locks: Default::default(),
            capability_manifest_sha256: representation_capability_manifest_sha256().unwrap(),
        };
        let request_sha256 = semantic_sha256(&request).unwrap();
        let parts = ["part_subject_base", "part_subject_trim"]
            .into_iter()
            .map(|part_id| SubjectPart {
                part_id: part_id.into(),
                parent_part_id: None,
                label: part_id.into(),
                semantic_role: "mechanical_assembly".into(),
                traits: vec![
                    "articulated_chain".into(),
                    "joint".into(),
                    "end_effector".into(),
                ],
                uncertainty_bps: 500,
            })
            .collect::<Vec<_>>();
        let profile = SubjectProfile {
            schema_version: "SubjectProfile@1".into(),
            profile_id: "subject_u004_v2".into(),
            request_sha256: request_sha256.clone(),
            identity_label: "模块化机械支架".into(),
            category: "hard surface assembly".into(),
            category_tags: vec![
                "mechanical".into(),
                "hard_surface".into(),
                "deformable_shell".into(),
            ],
            silhouette: "对称支架与轮廓条".into(),
            negative_space: "中央开槽".into(),
            pose: "static".into(),
            visible_views: Vec::new(),
            occlusions: Vec::new(),
            uncertainties: Vec::new(),
            parts,
            features: vec![
                SubjectFeature {
                    feature_id: "feature_u004_macro".into(),
                    part_id: "part_subject_base".into(),
                    level: VisualFeatureLevel::Macro,
                    description: "主体轮廓".into(),
                },
                SubjectFeature {
                    feature_id: "feature_u004_meso".into(),
                    part_id: "part_subject_trim".into(),
                    level: VisualFeatureLevel::Meso,
                    description: "轮廓装饰".into(),
                },
                SubjectFeature {
                    feature_id: "feature_u004_micro".into(),
                    part_id: "part_subject_trim".into(),
                    level: VisualFeatureLevel::Micro,
                    description: "表面细节".into(),
                },
            ],
            materials: vec![SubjectMaterial {
                material_id: "material_u004".into(),
                label: "金属".into(),
                part_ids: vec!["part_subject_base".into(), "part_subject_trim".into()],
                appearance_traits: vec!["metallic".into(), "brushed".into()],
            }],
        };
        let profile_sha256 = semantic_sha256(&profile).unwrap();
        let mut requirements = vec![
            requirement(
                "feature_u004_macro",
                VisualFeatureLevel::Macro,
                vec![AppearanceChannel::Geometry],
            ),
            requirement(
                "feature_u004_meso",
                VisualFeatureLevel::Meso,
                vec![AppearanceChannel::Geometry, AppearanceChannel::Normal],
            ),
            requirement(
                "feature_u004_micro",
                VisualFeatureLevel::Micro,
                vec![AppearanceChannel::BaseColor, AppearanceChannel::Roughness],
            ),
        ];
        requirements[0].affected_part_ids = vec!["part_subject_base".into()];
        requirements[1].affected_part_ids = vec!["part_subject_trim".into()];
        requirements[2].affected_part_ids = vec!["part_subject_trim".into()];
        requirements[1].description = "angled chevron armor relief".into();
        requirements[2].description = "silver hexagonal microgrid with edge wear and panel seam".into();
        for requirement in requirements.iter_mut().skip(1) {
            requirement.evidence_status = EvidenceStatus::Observed;
            requirement.evidence_regions = vec![VisualFeatureEvidenceRegion {
                evidence_id: "refevid_u004_image".into(),
                view_id: Some("front".into()),
                region_per_mille: Some([120, 160, 820, 870]),
            }];
        }
        let contract = VisualFeatureContract {
            schema_version: "VisualFeatureContract@1".into(),
            contract_id: "vfcontract_u004_v2".into(),
            request_sha256: request_sha256.clone(),
            subject_profile_sha256: profile_sha256.clone(),
            requirements,
        };
        let plan = RepresentationPlan {
            schema_version: "RepresentationPlan@1".into(),
            plan_id: "repplan_u004_v2".into(),
            request_sha256,
            subject_profile_sha256: profile_sha256,
            visual_feature_contract_sha256: semantic_sha256(&contract).unwrap(),
            capability_manifest_sha256: request.capability_manifest_sha256.clone(),
            parts: ["part_subject_base", "part_subject_trim"]
                .into_iter()
                .map(|part_id| PartRepresentationPlan {
                    part_id: part_id.into(),
                    representation: RepresentationKind::Procedural,
                    capability_id: GENERIC_HARD_SURFACE_PROCEDURAL_CAPABILITY_ID.into(),
                    covered_feature_ids: if part_id == "part_subject_base" {
                        vec!["feature_u004_macro".into()]
                    } else {
                        vec!["feature_u004_meso".into(), "feature_u004_micro".into()]
                    },
                    rationale: "contract test only".into(),
                })
                .collect(),
        };
        let program: Value = serde_json::from_str(include_str!("../../../../../../packages/concept-spec/fixtures/forge-visual-geometry-v2-bracket.json")).unwrap();
        let source = UniversalAssetSourceV2::from_runtime_procedural(
            &request,
            &profile,
            &contract,
            &plan,
            program.clone(),
        )
        .unwrap();
        source.validate().unwrap();
        let runtime = source.runtime_procedural().unwrap();
        let projection_receipt = ReferenceAppearanceProjectionReceipt {
            schema_version: REFERENCE_APPEARANCE_PROJECTION_RECEIPT_SCHEMA_VERSION.into(),
            source_request_sha256: source.request_sha256.clone(),
            source_program_sha256: runtime.source_program_sha256.clone(),
            final_glb_sha256: "1".repeat(64),
            compile_readback_sha256: "2".repeat(64),
            worker_receipt_sha256: "3".repeat(64),
            worker_schema_version: "ReferenceCameraUvRasterBakeReceipt@2".into(),
            algorithm_id: "forgecad.reference_camera_uv_raster".into(),
            algorithm_version: "1".into(),
            projection_id: "projection_u004_core".into(),
            projection_sha256: "4".repeat(64),
            source_evidence_id: "refevid_u004_image".into(),
            source_image_sha256: "5".repeat(64),
            camera_hypothesis_id: "camera_u004_core".into(),
            camera_provenance_sha256: "6".repeat(64),
            target_material_zone_id: source.material_zones[0].material_zone_id.clone(),
            base_color_texture_id: "vtex_reference_u004".into(),
            base_color_sha256: "7".repeat(64),
            base_color_byte_size: 1,
            unobserved_texel_mask_id: "vtexmask_reference_u004".into(),
            unobserved_texel_mask_sha256: "8".repeat(64),
            unobserved_texel_mask_byte_size: 1,
            observed_texel_count: 1,
            unobserved_texel_count: 1,
            fusion_count: 1,
            raster_triangle_count: Some(1),
            world_to_clip_sha256: Some("9".repeat(64)),
            source_evidence_ids: Vec::new(),
            source_image_sha256s: Vec::new(),
            camera_hypothesis_ids: Vec::new(),
            camera_provenance_sha256s: Vec::new(),
            world_to_clip_sha256s: Vec::new(),
        };
        projection_receipt.validate().unwrap();
        let projected = source
            .clone()
            .with_reference_appearance_projection_receipts(vec![projection_receipt.clone()])
            .unwrap();
        projected.validate().unwrap();
        let mut drifted_projection = projected;
        drifted_projection
            .appearance_evidence
            .projection_receipts[0]
            .source_request_sha256 = "a".repeat(64);
        assert_eq!(
            drifted_projection.validate().unwrap_err().code(),
            "REFERENCE_APPEARANCE_PROJECTION_LINEAGE_INVALID"
        );

        let mut hybrid_plan = plan.clone();
        let trim_plan = hybrid_plan
            .parts
            .iter_mut()
            .find(|part| part.part_id == "part_subject_trim")
            .unwrap();
        trim_plan.representation = RepresentationKind::Deformable;
        trim_plan.capability_id = LOCAL_LATTICE_DEFORMABLE_CAPABILITY_ID.into();
        let mut hybrid_program = program.clone();
        hybrid_program["domain"] = Value::String("generic_hard_surface".into());
        let hybrid_nodes = hybrid_program["nodes"].as_array_mut().unwrap();
        let trim_part_index = hybrid_nodes
            .iter()
            .position(|node| node["node_id"] == "node_trim_part")
            .unwrap();
        hybrid_nodes.insert(trim_part_index, json!({
            "kind":"lattice_deform",
            "node_id":"node_trim_lattice",
            "input_node_id":"node_profile_trim",
            "corner_offsets":[[0.0,0.0,0.0],[0.04,0.0,0.0],[0.0,0.03,0.0],[0.04,0.03,0.0],[0.0,0.0,-0.05],[0.04,0.0,-0.05],[0.0,0.03,-0.05],[0.04,0.03,-0.05]]
        }));
        hybrid_nodes
            .iter_mut()
            .find(|node| node["node_id"] == "node_trim_part")
            .unwrap()["input_node_id"] = Value::String("node_trim_lattice".into());
        let hybrid_source = UniversalAssetSourceV2::from_runtime_local_hybrid(
            &request,
            &profile,
            &contract,
            &hybrid_plan,
            hybrid_program,
        )
        .unwrap();
        hybrid_source.validate().unwrap();
        assert!(matches!(
            hybrid_source.representation_source,
            UniversalRepresentationSourceV2::Hybrid(_)
        ));
        let mut drifted_hybrid = hybrid_source.clone();
        let UniversalRepresentationSourceV2::Hybrid(hybrid) =
            &mut drifted_hybrid.representation_source
        else {
            unreachable!();
        };
        hybrid.deformations[0].subject_part_id = "part_subject_base".into();
        assert_eq!(
            drifted_hybrid.validate().unwrap_err().code(),
            "UNIVERSAL_LATTICE_TERMINAL_INVALID"
        );

        let mut mesh_patch_plan = plan.clone();
        for part in &mut mesh_patch_plan.parts {
            part.representation = RepresentationKind::MeshSeed;
            part.capability_id = LOCAL_MESH_PATCH_CAPABILITY_ID.into();
        }
        let mut mesh_patch_program = program.clone();
        mesh_patch_program["domain"] = Value::String("generic_hard_surface".into());
        let mesh_patch_nodes = mesh_patch_program["nodes"].as_array_mut().unwrap();
        let bracket_part_index = mesh_patch_nodes
            .iter()
            .position(|node| node["node_id"] == "node_bracket_part")
            .unwrap();
        mesh_patch_nodes.insert(bracket_part_index, json!({
            "kind":"local_mesh_patch",
            "node_id":"node_bracket_patch",
            "input_node_id":"node_symmetric",
            "patch_center":[0.0,0.0,0.0],
            "patch_radius":0.2,
            "patch_offset":[0.1,0.0,0.0]
        }));
        let trim_part_index = mesh_patch_nodes
            .iter()
            .position(|node| node["node_id"] == "node_trim_part")
            .unwrap();
        mesh_patch_nodes.insert(trim_part_index, json!({
            "kind":"local_mesh_patch",
            "node_id":"node_trim_patch",
            "input_node_id":"node_profile_trim",
            "patch_center":[0.0,0.0,0.0],
            "patch_radius":0.2,
            "patch_offset":[0.0,0.1,0.0]
        }));
        mesh_patch_nodes
            .iter_mut()
            .find(|node| node["node_id"] == "node_bracket_part")
            .unwrap()["input_node_id"] = Value::String("node_bracket_patch".into());
        mesh_patch_nodes
            .iter_mut()
            .find(|node| node["node_id"] == "node_trim_part")
            .unwrap()["input_node_id"] = Value::String("node_trim_patch".into());
        let mesh_patch_source = UniversalAssetSourceV2::from_runtime_local_mesh_patch(
            &request,
            &profile,
            &contract,
            &mesh_patch_plan,
            mesh_patch_program,
        )
        .unwrap();
        mesh_patch_source.validate().unwrap();
        assert!(matches!(
            mesh_patch_source.representation_source,
            UniversalRepresentationSourceV2::LocalMeshPatch(_)
        ));
        assert_eq!(
            mesh_patch_source
                .representation_source
                .runtime_procedural()
                .unwrap()
                .shape_program["operations"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|operation| operation["op"] == "local_mesh_patch")
                .count(),
            2
        );

        let mut lattice_plan = plan.clone();
        for part in &mut lattice_plan.parts {
            part.representation = RepresentationKind::Deformable;
            part.capability_id = LOCAL_LATTICE_DEFORMABLE_CAPABILITY_ID.into();
        }
        let mut lattice_program = program.clone();
        lattice_program["domain"] = Value::String("generic_hard_surface".into());
        let nodes = lattice_program["nodes"].as_array_mut().unwrap();
        let bracket_part_index = nodes
            .iter()
            .position(|node| node["node_id"] == "node_bracket_part")
            .unwrap();
        nodes.insert(bracket_part_index, json!({
            "kind":"lattice_deform",
            "node_id":"node_bracket_lattice",
            "input_node_id":"node_symmetric",
            "corner_offsets":[[0.0,0.0,0.0],[0.08,0.0,0.0],[0.0,0.04,0.0],[0.08,0.04,0.0],[0.0,0.0,-0.10],[0.08,0.0,-0.10],[0.0,0.04,-0.10],[0.08,0.04,-0.10]]
        }));
        let trim_part_index = nodes
            .iter()
            .position(|node| node["node_id"] == "node_trim_part")
            .unwrap();
        nodes.insert(trim_part_index, json!({
            "kind":"lattice_deform",
            "node_id":"node_trim_lattice",
            "input_node_id":"node_profile_trim",
            "corner_offsets":[[0.0,0.0,0.0],[0.04,0.0,0.0],[0.0,0.03,0.0],[0.04,0.03,0.0],[0.0,0.0,-0.05],[0.04,0.0,-0.05],[0.0,0.03,-0.05],[0.04,0.03,-0.05]]
        }));
        nodes
            .iter_mut()
            .find(|node| node["node_id"] == "node_bracket_part")
            .unwrap()["input_node_id"] = Value::String("node_bracket_lattice".into());
        nodes
            .iter_mut()
            .find(|node| node["node_id"] == "node_trim_part")
            .unwrap()["input_node_id"] = Value::String("node_trim_lattice".into());
        let lattice_source = UniversalAssetSourceV2::from_runtime_local_lattice(
            &request,
            &profile,
            &contract,
            &lattice_plan,
            lattice_program,
        )
        .unwrap();
        lattice_source.validate().unwrap();
        assert!(matches!(
            lattice_source.representation_source,
            UniversalRepresentationSourceV2::Deformable(_)
        ));
        let mut drifted_lattice = lattice_source.clone();
        let UniversalRepresentationSourceV2::Deformable(deformable) =
            &mut drifted_lattice.representation_source
        else {
            unreachable!();
        };
        deformable.deformations[0].corner_offsets[0][0] = 0.12;
        assert_eq!(
            drifted_lattice.validate().unwrap_err().code(),
            "UNIVERSAL_LATTICE_BINDING_INVALID"
        );
        let game_ready = source
            .clone()
            .with_game_asset_profile(crate::GameAssetProfile {
                schema_version: crate::GAME_ASSET_PROFILE_SCHEMA_VERSION.into(),
                profile_id: "generic_hard_surface_game_delivery".into(),
                lod_triangle_budgets: [90_000, 36_000, 8_000],
                collision_proxy_part_ids: vec!["part_subject_base".into()],
                sockets: vec![crate::GameAssetSocket {
                    socket_id: "socket_vfx_trim".into(),
                    part_id: "part_subject_trim".into(),
                    pivot_meters: [0.0, 0.0, 0.0],
                    forward: [0.0, 0.0, 1.0],
                }],
                target_texel_density_pixels_per_meter: 1024,
            })
            .unwrap();
        assert!(game_ready.game_asset_profile.is_some());
        let invalid_game_part = source
            .clone()
            .with_game_asset_profile(crate::GameAssetProfile {
                schema_version: crate::GAME_ASSET_PROFILE_SCHEMA_VERSION.into(),
                profile_id: "generic_hard_surface_game_delivery".into(),
                lod_triangle_budgets: [90_000, 36_000, 8_000],
                collision_proxy_part_ids: vec!["part_not_in_subject".into()],
                sockets: Vec::new(),
                target_texel_density_pixels_per_meter: 1024,
        })
        .unwrap_err();
        assert_eq!(invalid_game_part.code(), "GAME_ASSET_PROFILE_PART_INVALID");
        let runtime = game_ready.runtime_procedural().unwrap();
        let game_source_compiled = game_ready
            .clone()
            .with_compiled_artifact(UniversalCompiledArtifactBinding {
                source_program_sha256: runtime.source_program_sha256.clone(),
                shape_program_sha256: runtime.shape_program_sha256.clone(),
                glb_sha256: "3".repeat(64),
                readback_sha256: "4".repeat(64),
                compile_readback_sha256: "5".repeat(64),
                artifact_profile_id: "production_concept".into(),
                renderer_id: "forgecad-workbench-pbr@1".into(),
                view_sha256: BTreeMap::from([("turntable_000".into(), "6".repeat(64))]),
            })
            .unwrap();
        assert!(game_source_compiled.game_asset_delivery.is_none());
        let mut post_compile = source.clone();
        post_compile.state = UniversalAssetSourceState::Compiled;
        assert_eq!(
            post_compile
                .with_game_asset_profile(crate::GameAssetProfile {
                    schema_version: crate::GAME_ASSET_PROFILE_SCHEMA_VERSION.into(),
                    profile_id: "late_game_delivery".into(),
                    lod_triangle_budgets: [90_000, 36_000, 8_000],
                    collision_proxy_part_ids: vec!["part_subject_base".into()],
                    sockets: Vec::new(),
                    target_texel_density_pixels_per_meter: 1024,
                })
                .unwrap_err()
                .code(),
            "GAME_ASSET_PROFILE_STATE_INVALID"
        );
        let bindings =
            crate::derive_reference_appearance_bindings(&source, &[evidence.clone()]).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].source_view_id, "turntable_000");
        assert_eq!(
            bindings[0].feature_ids,
            vec!["feature_u004_meso", "feature_u004_micro"]
        );
        assert_eq!(bindings[0].target_subject_part_id, "part_subject_trim");
        bindings[0]
            .validate_against(&source, &[evidence.clone()])
            .unwrap();
        let reference_bound = source
            .clone()
            .with_reference_surface_facts(&[evidence.clone()])
            .unwrap();
        reference_bound.validate().unwrap();
        assert_eq!(
            reference_bound
                .appearance_compilation
                .reference_surface_bindings
                .len(),
            1
        );
        assert_eq!(
            reference_bound
                .appearance_compilation
                .reference_surface_bindings[0]
                .evidence_id,
            "refevid_u004_image"
        );
        let base_zone = reference_bound
            .appearance_compilation
            .zones
            .iter()
            .find(|zone| zone.target_subject_part_id == "part_subject_base")
            .expect("the fixture retains a sibling base zone");
        assert_eq!(
            base_zone.surface_layer_program.base_color_token, None,
            "a whole-image color fact must not leak into a Part with no observed appearance region"
        );
        let trim_zone = reference_bound
            .appearance_compilation
            .zones
            .iter()
            .find(|zone| zone.target_subject_part_id == "part_subject_trim")
            .expect("the observed trim zone is retained");
        assert_eq!(
            trim_zone.surface_layer_program.base_color_token.as_deref(),
            Some("silver"),
            "the explicitly observed trim zone keeps its feature/material color semantics"
        );
        assert_ne!(
            reference_bound.appearance_compilation.compilation_sha256,
            source.appearance_compilation.compilation_sha256,
            "reference-conditioned appearance must change the sealed compilation hash"
        );
        let mut drifted_reference = reference_bound.clone();
        drifted_reference
            .appearance_compilation
            .reference_surface_bindings[0]
            .facts
            .brightness = ReferenceImageBrightnessBucket::Bright;
        assert_eq!(
            drifted_reference.validate().unwrap_err().code(),
            "REFERENCE_SURFACE_APPEARANCE_BINDING_INVALID"
        );
        let mut cross_project = evidence;
        cross_project.project_id = "project_other".into();
        assert_eq!(
            crate::derive_reference_appearance_bindings(&source, &[cross_project])
                .unwrap_err()
                .code(),
            "REFERENCE_APPEARANCE_BINDING_EVIDENCE_INVALID"
        );
        assert_eq!(
            source.representation_source.representation_kind(),
            RepresentationKind::Procedural
        );
        assert_eq!(
            source.appearance_compilation.compiler_id,
            "forgecad.generic_hard_surface_appearance.v2"
        );
        assert_eq!(source.appearance_compilation.zones.len(), 2);
        assert_eq!(
            source
                .appearance_compilation
                .zones[0]
                .surface_layer_program
                .target_part_role,
            "exterior_panel",
            "open SubjectProfile roles must be mapped only inside the bounded visual layer vocabulary"
        );
        assert_eq!(
            source
                .appearance_compilation
                .zones[0]
                .surface_layer_program
                .base_color_token
                .as_deref(),
            Some("silver"),
            "sealed silver material evidence should select the bounded silver PBR tint"
        );
        assert_eq!(
            source
                .appearance_compilation
                .zones[0]
                .surface_layer_program
                .surface_finish_token
                .as_deref(),
            Some("brushed_metal"),
            "sealed metallic/brushed evidence should select a bounded metallic finish"
        );
        assert_eq!(
            source.appearance_compilation.zones[0]
                .surface_layer_program
                .normal_relief_layers
                .len(),
            1
        );
        assert_eq!(
            source.appearance_compilation.zones[0]
                .surface_layer_program
                .normal_relief_layers[0]
                .motif,
            "chevron_relief",
            "meso feature language should choose a bounded angular relief motif"
        );
        assert_eq!(
            source.appearance_compilation.zones[0]
                .surface_layer_program
                .roughness_masks[0]
                .motif,
            "microgrid",
            "micro feature language should choose a bounded microgrid roughness motif"
        );
        assert_eq!(
            source.appearance_compilation.zones[0]
                .surface_layer_program
                .roughness_masks
                .len(),
            1
        );
        assert_eq!(
            source.appearance_compilation.zones[0]
                .surface_layer_program
                .decal_layers
                .len(),
            1,
            "visible base-color feature language should compile one bounded decal layer"
        );
        assert_eq!(
            source.appearance_compilation.zones[0]
                .surface_layer_program
                .decal_layers[0]
                .motif,
            "hex_badge",
            "visible hexagonal feature language should choose a reviewed decal motif"
        );
        assert_eq!(
            source.appearance_compilation.zones[0]
                .surface_layer_program
                .vector_paths
                .len(),
            1,
            "visible panel seam evidence should compile one bounded retained vector path"
        );
        assert_eq!(
            source.appearance_compilation.zones[0]
                .surface_layer_program
                .vector_paths[0]
                .commands[0]
                .kind,
            "move"
        );
        let mut hidden_contract = contract.clone();
        hidden_contract.requirements[2].evidence_status = EvidenceStatus::Hidden;
        hidden_contract.requirements[2].evidence_regions.clear();
        let mut hidden_plan = plan.clone();
        hidden_plan.visual_feature_contract_sha256 = semantic_sha256(&hidden_contract).unwrap();
        let hidden_source = UniversalAssetSourceV2::from_runtime_procedural(
            &request,
            &profile,
            &hidden_contract,
            &hidden_plan,
            program.clone(),
        )
        .unwrap();
        hidden_source.validate().unwrap();
        assert!(
            hidden_source
                .appearance_compilation
                .zones
                .iter()
                .all(|zone| {
                    zone.surface_layer_program.decal_layers.is_empty()
                        && zone.surface_layer_program.vector_paths.is_empty()
                }),
            "hidden visual evidence must not create visible retained surface marks"
        );
        let UniversalRepresentationSourceV2::Procedural(procedural) = &source.representation_source
        else {
            unreachable!()
        };
        assert!(
            procedural
                .source_program
                .pointer("/materials/0/base_material_id")
                .is_some_and(|value| value == "mat_aluminum"),
            "visible metallic profile material must compile to the reviewed aluminum PBR base"
        );
        let mut tampered = source.clone();
        tampered.appearance_compilation.zones[0].base_material_id =
            if tampered.appearance_compilation.zones[0].base_material_id == "mat_aluminum" {
                "mat_signal_red".into()
            } else {
                "mat_aluminum".into()
            };
        assert_eq!(
            tampered.validate().unwrap_err().code(),
            "UNIVERSAL_V2_APPEARANCE_COMPILATION_INVALID"
        );
        let mut tampered = source;
        let UniversalRepresentationSourceV2::Procedural(procedural) =
            &mut tampered.representation_source
        else {
            unreachable!()
        };
        procedural.part_bindings[0].material_zone_id = "zone_not_real".into();
        assert_eq!(
            tampered.validate().unwrap_err().code(),
            "UNIVERSAL_V2_PART_BINDING_INVALID"
        );
    }

    #[test]
    fn u004_reference_surface_fallback_is_scoped_to_compatible_exterior_zones() {
        assert!(reference_surface_fallback_allowed(
            "primary_shell",
            "mat_graphite",
            ""
        ));
        assert!(reference_surface_fallback_allowed(
            "armor_panel",
            "mat_aluminum",
            "metallic"
        ));
        assert!(!reference_surface_fallback_allowed(
            "structural_frame",
            "mat_graphite",
            ""
        ));
        assert!(!reference_surface_fallback_allowed(
            "accent_trim",
            "mat_graphite",
            ""
        ));
        assert!(!reference_surface_fallback_allowed(
            "primary_shell",
            "mat_rubber",
            ""
        ));
        assert!(!reference_surface_fallback_allowed(
            "primary_shell",
            "mat_graphite",
            "emissive status strip"
        ));
        assert_eq!(feature_base_color_token("black graphite"), Some("graphite"));
        assert_eq!(feature_base_color_token("gray metal"), Some("gunmetal"));
        assert_eq!(feature_base_color_token("green foliage and leaves"), Some("foliage_green"));
        assert_eq!(feature_base_color_token("warm skin and fabric"), Some("skin_warm"));
        assert_eq!(feature_base_color_token("bark and wood grain"), Some("bark_brown"));
        assert_eq!(feature_surface_finish_token("leaf veins", "mat_composite"), Some("leaf_waxy"));
        assert_eq!(feature_surface_finish_token("woven fabric", "mat_composite"), Some("fabric_weave"));
        assert_eq!(feature_surface_finish_token("rough concrete", "mat_composite"), Some("concrete_rough"));
    }

    #[test]
    fn u003_compiled_binding_is_exact_and_hash_only() {
        let source = fixture();
        let compiled = source
            .clone()
            .with_compiled_artifact(UniversalCompiledArtifactBinding {
                source_program_sha256: source.procedural_source.source_program_sha256.clone(),
                shape_program_sha256: "1".repeat(64),
                glb_sha256: "2".repeat(64),
                readback_sha256: "3".repeat(64),
                compile_readback_sha256: "4".repeat(64),
                artifact_profile_id: "production_concept".into(),
                renderer_id: "software_renderer_v1".into(),
                view_sha256: BTreeMap::from([("iso".into(), "5".repeat(64))]),
            })
            .unwrap();
        assert_eq!(compiled.state, UniversalAssetSourceState::Compiled);
        compiled.validate().unwrap();
        let mut stale = compiled;
        stale.compiled_artifact.as_mut().unwrap().glb_sha256 = "x".repeat(64);
        assert_eq!(
            stale.validate().unwrap_err().code(),
            "UNIVERSAL_COMPILED_ARTIFACT_INVALID"
        );
    }

    #[test]
    fn u003_rejects_provider_like_projection_without_camera_and_mask_evidence() {
        let mut source = fixture();
        source.material_zones[0]
            .projection_layers
            .push(AppearanceProjectionLayer {
                layer_id: "projection_unsealed".into(),
                evidence_artifact_id: "artifact_missing".into(),
                camera_hypothesis_id: "camera_missing".into(),
                channels: vec![PbrTextureChannel::BaseColor],
                unobserved_texel_mask_artifact_id: "mask_missing".into(),
            });
        assert_eq!(
            source.validate().unwrap_err().code(),
            "APPEARANCE_PROJECTION_LAYER_INVALID"
        );
    }

    #[test]
    fn u004_projection_layers_require_same_view_fitted_camera_and_channel_evidence() {
        let zone = MaterialZoneAppearance {
            schema_version: MATERIAL_ZONE_APPEARANCE_SCHEMA_VERSION.into(),
            appearance_id: "appearance_projection_test".into(),
            material_zone_id: "zone_projection_test".into(),
            source_part_id: "part_projection_test".into(),
            base_material_id: "mat_aluminum".into(),
            finish: "reviewed_catalog_pbr".into(),
            coating: None,
            transmission_bps: 0,
            uncertainty_bps: 0,
            texture_width: 1024,
            texture_height: 1024,
            channels: vec![PbrTextureChannel::BaseColor, PbrTextureChannel::Roughness],
            projection_layers: Vec::new(),
        };
        let color = AppearanceEvidenceArtifact {
            artifact_id: "artifact_color".into(),
            evidence_id: "evidence_front".into(),
            kind: AppearanceEvidenceArtifactKind::DelightedColorHint,
            content_sha256: "1".repeat(64),
            algorithm_id: "forgecad.local_projection_prep".into(),
            algorithm_version: "1".into(),
            evidence_only: true,
        };
        let mask = AppearanceEvidenceArtifact {
            artifact_id: "artifact_mask".into(),
            evidence_id: "evidence_front".into(),
            kind: AppearanceEvidenceArtifactKind::UnobservedTexelMask,
            content_sha256: "2".repeat(64),
            algorithm_id: "forgecad.local_projection_mask".into(),
            algorithm_version: "1".into(),
            evidence_only: true,
        };
        let camera = ReferenceCameraHypothesis {
            schema_version: REFERENCE_CAMERA_HYPOTHESIS_SCHEMA_VERSION.into(),
            hypothesis_id: "camera_front".into(),
            evidence_id: "evidence_front".into(),
            view_id: Some("front".into()),
            projection_type: ReferenceProjectionType::Perspective,
            parameter_source: CameraParameterSource::LandmarkFit,
            vertical_fov_millidegrees: Some(45_000),
            reprojection_error_bps: Some(300),
            landmark_feature_ids: vec!["feature_projection_test".into()],
            confidence_bps: 8_000,
            unresolved_fields: Vec::new(),
        };
        let layer = AppearanceProjectionLayer {
            layer_id: "projection_front_base_color".into(),
            evidence_artifact_id: color.artifact_id.clone(),
            camera_hypothesis_id: camera.hypothesis_id.clone(),
            channels: vec![PbrTextureChannel::BaseColor],
            unobserved_texel_mask_artifact_id: mask.artifact_id.clone(),
        };
        let artifacts = BTreeMap::from([
            (color.artifact_id.as_str(), &color),
            (mask.artifact_id.as_str(), &mask),
        ]);
        let cameras = BTreeMap::from([(camera.hypothesis_id.as_str(), &camera)]);
        validate_projection_layer(&layer, &zone, &artifacts, &cameras, &mut BTreeSet::new())
            .unwrap();

        let mut unresolved = camera;
        unresolved.parameter_source = CameraParameterSource::Unresolved;
        unresolved.projection_type = ReferenceProjectionType::Unknown;
        unresolved.vertical_fov_millidegrees = None;
        unresolved.reprojection_error_bps = None;
        unresolved.confidence_bps = 0;
        unresolved.unresolved_fields = vec!["extrinsics".into()];
        let cameras = BTreeMap::from([(unresolved.hypothesis_id.as_str(), &unresolved)]);
        assert_eq!(
            validate_projection_layer(&layer, &zone, &artifacts, &cameras, &mut BTreeSet::new())
                .unwrap_err()
                .code(),
            "APPEARANCE_PROJECTION_LAYER_INVALID"
        );

        let wrong_channel = AppearanceProjectionLayer {
            channels: vec![PbrTextureChannel::Roughness],
            ..layer
        };
        assert_eq!(
            validate_projection_layer(
                &wrong_channel,
                &zone,
                &artifacts,
                &BTreeMap::from([("camera_front", &unresolved)]),
                &mut BTreeSet::new()
            )
            .unwrap_err()
            .code(),
            "APPEARANCE_PROJECTION_LAYER_INVALID"
        );
    }

    #[test]
    fn u003_rejects_cross_contract_hash_drift() {
        let mut source = fixture();
        source.subject_profile_sha256 = "0".repeat(64);
        assert_eq!(
            source.validate().unwrap_err().code(),
            "UNIVERSAL_ASSET_SOURCE_LINEAGE_INVALID"
        );
    }

    #[test]
    fn u004_unavailable_representation_is_not_an_asset_source() {
        let unavailable = json!({
            "kind": "unavailable",
            "source_contract_id": "ForgeUnavailableRepresentation@1",
            "reason": "representation_unavailable"
        });
        assert!(
            serde_json::from_value::<UniversalRepresentationSourceV2>(unavailable).is_err(),
            "unavailable capability outcomes must remain typed limitations and never deserialize as UAS@2"
        );
    }
}
