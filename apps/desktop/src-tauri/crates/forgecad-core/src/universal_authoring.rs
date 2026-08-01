//! Category-open, Rust-sealed authoring contracts.
//!
//! U002 deliberately separates understanding from execution. Providers may
//! describe any subject and propose a representation, while Rust owns the
//! evidence lineage, capability manifest and the final executable decision.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    lower_visual_runtime_source_v1, semantic_sha256, CoreError, CoreResult, ReferenceEvidence,
};

pub const UNIVERSAL_AUTHOR_REQUEST_SCHEMA_VERSION: &str = "UniversalAuthorRequest@1";
pub const SUBJECT_PROFILE_SCHEMA_VERSION: &str = "SubjectProfile@1";
pub const VISUAL_FEATURE_CONTRACT_SCHEMA_VERSION: &str = "VisualFeatureContract@1";
pub const REPRESENTATION_PLAN_SCHEMA_VERSION: &str = "RepresentationPlan@1";
pub const REPRESENTATION_LIMITATION_SCHEMA_VERSION: &str = "RepresentationLimitation@1";
pub const UNIVERSAL_AUTHOR_OUTCOME_SCHEMA_VERSION: &str = "UniversalAuthorOutcome@1";
pub const VISUAL_EVIDENCE_GRAPH_V2_SCHEMA_VERSION: &str = "VisualEvidenceGraph@2";
pub const REPRESENTATION_CAPABILITY_MANIFEST_SCHEMA_VERSION: &str =
    "RepresentationCapabilityManifest@1";
pub const ROBOTIC_ARM_PROCEDURAL_CAPABILITY_ID: &str = "procedural.robotic_arm_visual_v1";
/// The first category-open executable route.  It deliberately covers only
/// exterior hard-surface assets represented by the reviewed VP203 language;
/// it is not a claim that every procedural subject is currently executable.
pub const GENERIC_HARD_SURFACE_PROCEDURAL_CAPABILITY_ID: &str =
    "procedural.generic_hard_surface_v1";
/// A category-open exterior visual composition route. It uses the same Rust
/// sealed high-level geometry language as hard-surface assets, but does not
/// claim that an organic subject is a hard-surface object. The output remains
/// an appearance-first proxy until a category-specific deformable or neural
/// representation is available. It is intentionally a universal exterior
/// capability: Rust must not require the Provider to invent a
/// `visual_exterior` category tag before any non-functional visible object can
/// reach the honest proxy route.
pub const GENERIC_VISUAL_EXTERIOR_PROCEDURAL_CAPABILITY_ID: &str =
    "procedural.generic_visual_exterior_v1";
/// A local, topology-preserving 2x2x2 cage deformation for exterior
/// hard-surface shells. This is intentionally not a generic organic or
/// character capability: it may only be used when every output has a bounded
/// lattice-deform operation that Rust can re-derive and validate.
pub const LOCAL_LATTICE_DEFORMABLE_CAPABILITY_ID: &str = "deformable.local_lattice_shell_v1";
/// A bounded local mesh edit over a reviewed ShapeProgram output. It keeps
/// topology and provenance intact; it is not an imported-GLB or arbitrary
/// vertex-payload capability.
pub const LOCAL_MESH_PATCH_CAPABILITY_ID: &str = "mesh_seed.local_patch_v1";

const MAX_PARTS: usize = 256;
const MAX_FEATURES: usize = 512;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UniversalInputMode {
    Text,
    SingleImage,
    Multiview,
    ActiveAsset,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UniversalReferenceInput {
    pub evidence_id: String,
    pub evidence_sha256: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UniversalActiveAssetBinding {
    pub asset_version_id: String,
    pub snapshot_revision: u64,
    pub source_sha256: String,
    pub readback_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct UniversalSelectionScope {
    #[serde(default)]
    pub part_ids: Vec<String>,
    #[serde(default)]
    pub material_zone_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct UniversalDesignLocks {
    pub preserve_geometry: bool,
    pub preserve_material_surface: bool,
    #[serde(default)]
    pub locked_part_ids: Vec<String>,
    #[serde(default)]
    pub locked_material_zone_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UniversalAuthorRequest {
    pub schema_version: String,
    pub request_id: String,
    pub project_id: String,
    pub turn_id: String,
    pub instruction: String,
    pub input_mode: UniversalInputMode,
    #[serde(default)]
    pub reference_inputs: Vec<UniversalReferenceInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_asset: Option<UniversalActiveAssetBinding>,
    #[serde(default)]
    pub selection: UniversalSelectionScope,
    #[serde(default)]
    pub locks: UniversalDesignLocks,
    pub capability_manifest_sha256: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum VisualFeatureLevel {
    Macro,
    Meso,
    Micro,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Observed,
    Inferred,
    Hidden,
    Conflicting,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SubjectPart {
    pub part_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_part_id: Option<String>,
    pub label: String,
    pub semantic_role: String,
    #[serde(default)]
    pub traits: Vec<String>,
    pub uncertainty_bps: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SubjectFeature {
    pub feature_id: String,
    pub part_id: String,
    pub level: VisualFeatureLevel,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SubjectMaterial {
    pub material_id: String,
    pub label: String,
    #[serde(default)]
    pub part_ids: Vec<String>,
    #[serde(default)]
    pub appearance_traits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SubjectProfile {
    pub schema_version: String,
    pub profile_id: String,
    pub request_sha256: String,
    pub identity_label: String,
    /// Open text. This is descriptive metadata, never a product admission enum.
    pub category: String,
    #[serde(default)]
    pub category_tags: Vec<String>,
    pub silhouette: String,
    pub negative_space: String,
    pub pose: String,
    #[serde(default)]
    pub visible_views: Vec<String>,
    #[serde(default)]
    pub occlusions: Vec<String>,
    #[serde(default)]
    pub uncertainties: Vec<String>,
    pub parts: Vec<SubjectPart>,
    pub features: Vec<SubjectFeature>,
    #[serde(default)]
    pub materials: Vec<SubjectMaterial>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualFeatureEvidenceRegion {
    pub evidence_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region_per_mille: Option<[u16; 4]>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceChannel {
    Geometry,
    Normal,
    BaseColor,
    Roughness,
    Metallic,
    Emissive,
    Opacity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualFeatureRequirement {
    pub feature_id: String,
    pub level: VisualFeatureLevel,
    pub description: String,
    pub salience_bps: u16,
    pub evidence_status: EvidenceStatus,
    #[serde(default)]
    pub evidence_regions: Vec<VisualFeatureEvidenceRegion>,
    pub affected_part_ids: Vec<String>,
    pub channels: Vec<AppearanceChannel>,
    pub minimum_acceptance_views: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualFeatureContract {
    pub schema_version: String,
    pub contract_id: String,
    pub request_sha256: String,
    pub subject_profile_sha256: String,
    pub requirements: Vec<VisualFeatureRequirement>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepresentationKind {
    Procedural,
    Deformable,
    MeshSeed,
    Hybrid,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RepresentationCapability {
    pub capability_id: String,
    pub representation: RepresentationKind,
    pub availability: CapabilityAvailability,
    #[serde(default)]
    pub required_subject_traits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RepresentationCapabilityManifest {
    pub schema_version: String,
    pub capabilities: Vec<RepresentationCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PartRepresentationPlan {
    pub part_id: String,
    pub representation: RepresentationKind,
    pub capability_id: String,
    #[serde(default)]
    pub covered_feature_ids: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RepresentationPlan {
    pub schema_version: String,
    pub plan_id: String,
    pub request_sha256: String,
    pub subject_profile_sha256: String,
    pub visual_feature_contract_sha256: String,
    pub capability_manifest_sha256: String,
    pub parts: Vec<PartRepresentationPlan>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepresentationLimitationCode {
    NeedsMoreViews,
    RepresentationUnavailable,
    QualityLimited,
    ProviderUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RepresentationLimitation {
    pub schema_version: String,
    pub code: RepresentationLimitationCode,
    pub message: String,
    #[serde(default)]
    pub affected_part_ids: Vec<String>,
    #[serde(default)]
    pub missing_capability_ids: Vec<String>,
    #[serde(default)]
    pub suggested_views: Vec<String>,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum UniversalAuthorOutcome {
    Executable {
        schema_version: String,
        request: UniversalAuthorRequest,
        subject_profile: SubjectProfile,
        visual_feature_contract: VisualFeatureContract,
        representation_plan: RepresentationPlan,
        executable_payload: Value,
    },
    Limitation {
        schema_version: String,
        request: UniversalAuthorRequest,
        subject_profile: SubjectProfile,
        visual_feature_contract: VisualFeatureContract,
        representation_plan: RepresentationPlan,
        limitation: RepresentationLimitation,
    },
    ClarificationRequired {
        schema_version: String,
        request: UniversalAuthorRequest,
        reason: String,
        questions: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UniversalEvidenceClaim {
    pub claim_id: String,
    pub feature_id: String,
    pub status: EvidenceStatus,
    #[serde(default)]
    pub evidence_regions: Vec<VisualFeatureEvidenceRegion>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualEvidenceGraphV2 {
    pub schema_version: String,
    pub graph_id: String,
    pub universal_request_sha256: String,
    pub subject_profile_sha256: String,
    pub claims: Vec<UniversalEvidenceClaim>,
}

pub fn representation_capability_manifest() -> RepresentationCapabilityManifest {
    RepresentationCapabilityManifest {
        schema_version: REPRESENTATION_CAPABILITY_MANIFEST_SCHEMA_VERSION.into(),
        capabilities: vec![
            RepresentationCapability {
                capability_id: ROBOTIC_ARM_PROCEDURAL_CAPABILITY_ID.into(),
                representation: RepresentationKind::Procedural,
                availability: CapabilityAvailability::Available,
                required_subject_traits: vec![
                    "articulated_chain".into(),
                    "joint".into(),
                    "end_effector".into(),
                ],
            },
            RepresentationCapability {
                capability_id: GENERIC_HARD_SURFACE_PROCEDURAL_CAPABILITY_ID.into(),
                representation: RepresentationKind::Procedural,
                availability: CapabilityAvailability::Available,
                required_subject_traits: vec!["hard_surface".into()],
            },
            RepresentationCapability {
                capability_id: GENERIC_VISUAL_EXTERIOR_PROCEDURAL_CAPABILITY_ID.into(),
                representation: RepresentationKind::Procedural,
                availability: CapabilityAvailability::Available,
                // This is the category-open exterior fallback. Identity and
                // visual semantics remain in SubjectProfile; requiring a
                // Provider-authored `visual_exterior` tag here made cats,
                // plants, buildings and other valid subjects fail before the
                // Rust-reviewed proxy compiler could run.
                required_subject_traits: Vec::new(),
            },
            RepresentationCapability {
                capability_id: LOCAL_LATTICE_DEFORMABLE_CAPABILITY_ID.into(),
                representation: RepresentationKind::Deformable,
                availability: CapabilityAvailability::Available,
                required_subject_traits: vec!["hard_surface".into(), "deformable_shell".into()],
            },
            RepresentationCapability {
                capability_id: LOCAL_MESH_PATCH_CAPABILITY_ID.into(),
                representation: RepresentationKind::MeshSeed,
                availability: CapabilityAvailability::Available,
                required_subject_traits: vec!["hard_surface".into()],
            },
            unavailable("procedural.generic_v1", RepresentationKind::Procedural),
            unavailable("deformable.generic_v1", RepresentationKind::Deformable),
            unavailable("mesh_seed.generic_v1", RepresentationKind::MeshSeed),
            unavailable("hybrid.generic_v1", RepresentationKind::Hybrid),
        ],
    }
}

pub fn representation_capability_manifest_sha256() -> CoreResult<String> {
    semantic_sha256(&representation_capability_manifest())
}

fn unavailable(id: &str, representation: RepresentationKind) -> RepresentationCapability {
    RepresentationCapability {
        capability_id: id.into(),
        representation,
        availability: CapabilityAvailability::Unavailable,
        required_subject_traits: Vec::new(),
    }
}

impl UniversalAuthorRequest {
    pub fn validate_with_evidence(&self, evidence: &[ReferenceEvidence]) -> CoreResult<()> {
        if self.schema_version != UNIVERSAL_AUTHOR_REQUEST_SCHEMA_VERSION {
            return Err(invalid(
                "UNIVERSAL_REQUEST_SCHEMA_INVALID",
                "Unsupported universal author request schema.",
            ));
        }
        require_text(&self.request_id, "request_id")?;
        require_text(&self.project_id, "project_id")?;
        require_text(&self.turn_id, "turn_id")?;
        require_text(&self.instruction, "instruction")?;
        require_sha(
            &self.capability_manifest_sha256,
            "capability_manifest_sha256",
        )?;
        if self.capability_manifest_sha256 != representation_capability_manifest_sha256()? {
            return Err(invalid(
                "UNIVERSAL_CAPABILITY_MANIFEST_MISMATCH",
                "The request must bind the exact Rust-owned capability manifest.",
            ));
        }
        let sealed = evidence
            .iter()
            .map(|item| (item.evidence_id.as_str(), item))
            .collect::<BTreeMap<_, _>>();
        let mut ids = BTreeSet::new();
        for input in &self.reference_inputs {
            require_text(&input.evidence_id, "reference_inputs.evidence_id")?;
            require_sha(&input.evidence_sha256, "reference_inputs.evidence_sha256")?;
            require_text(&input.role, "reference_inputs.role")?;
            if !ids.insert(input.evidence_id.as_str()) {
                return Err(invalid(
                    "UNIVERSAL_REFERENCE_DUPLICATE",
                    "A sealed reference may appear only once.",
                ));
            }
            let item = sealed.get(input.evidence_id.as_str()).ok_or_else(|| {
                invalid(
                    "UNIVERSAL_REFERENCE_NOT_FOUND",
                    "Every reference must resolve to sealed evidence.",
                )
            })?;
            item.validate()?;
            if item.project_id != self.project_id {
                return Err(invalid(
                    "UNIVERSAL_REFERENCE_PROJECT_MISMATCH",
                    "References must belong to the request Project.",
                ));
            }
            if semantic_sha256(*item)? != input.evidence_sha256 {
                return Err(invalid(
                    "UNIVERSAL_REFERENCE_HASH_MISMATCH",
                    "Reference hash drifted from its sealed record.",
                ));
            }
        }
        let expected_mode = match (self.reference_inputs.len(), self.active_asset.is_some()) {
            (0, false) => UniversalInputMode::Text,
            (1, false) => UniversalInputMode::SingleImage,
            (2.., false) => UniversalInputMode::Multiview,
            (0, true) => UniversalInputMode::ActiveAsset,
            (_, true) => UniversalInputMode::Mixed,
        };
        if self.input_mode != expected_mode {
            return Err(invalid(
                "UNIVERSAL_INPUT_MODE_MISMATCH",
                "Input mode must be derived from sealed inputs and active state.",
            ));
        }
        let has_asset = self.active_asset.is_some();
        if !has_asset
            && (!self.selection.part_ids.is_empty()
                || !self.selection.material_zone_ids.is_empty()
                || self.locks.preserve_geometry
                || self.locks.preserve_material_surface
                || !self.locks.locked_part_ids.is_empty()
                || !self.locks.locked_material_zone_ids.is_empty())
        {
            return Err(invalid(
                "UNIVERSAL_ACTIVE_ASSET_REQUIRED",
                "Selection and locks require a sealed active asset.",
            ));
        }
        if let Some(asset) = &self.active_asset {
            require_text(&asset.asset_version_id, "active_asset.asset_version_id")?;
            require_sha(&asset.source_sha256, "active_asset.source_sha256")?;
            require_sha(&asset.readback_sha256, "active_asset.readback_sha256")?;
        }
        Ok(())
    }
}

impl SubjectProfile {
    pub fn validate_against(&self, request: &UniversalAuthorRequest) -> CoreResult<()> {
        if self.schema_version != SUBJECT_PROFILE_SCHEMA_VERSION
            || self.request_sha256 != semantic_sha256(request)?
        {
            return Err(invalid(
                "SUBJECT_PROFILE_LINEAGE_INVALID",
                "Subject profile must bind the exact universal request.",
            ));
        }
        require_text(&self.profile_id, "profile_id")?;
        require_text(&self.identity_label, "identity_label")?;
        require_text(&self.category, "category")?;
        require_text(&self.silhouette, "silhouette")?;
        require_text(&self.negative_space, "negative_space")?;
        require_text(&self.pose, "pose")?;
        if self.parts.is_empty()
            || self.parts.len() > MAX_PARTS
            || self.features.is_empty()
            || self.features.len() > MAX_FEATURES
        {
            return Err(invalid(
                "SUBJECT_PROFILE_BOUNDS_INVALID",
                "Subject profile requires bounded parts and visual features.",
            ));
        }
        let mut part_ids = BTreeSet::new();
        for part in &self.parts {
            require_text(&part.part_id, "parts.part_id")?;
            require_text(&part.label, "parts.label")?;
            require_text(&part.semantic_role, "parts.semantic_role")?;
            if part.uncertainty_bps > 10_000 || !part_ids.insert(part.part_id.as_str()) {
                return Err(invalid(
                    "SUBJECT_PART_INVALID",
                    "Part identifiers must be unique and uncertainty bounded.",
                ));
            }
        }
        for part in &self.parts {
            if let Some(parent) = part.parent_part_id.as_deref() {
                if parent == part.part_id || !part_ids.contains(parent) {
                    return Err(invalid(
                        "SUBJECT_PART_PARENT_INVALID",
                        "Part parents must reference another declared part.",
                    ));
                }
            }
            let mut ancestors = BTreeSet::new();
            let mut cursor = part.parent_part_id.as_deref();
            while let Some(parent_id) = cursor {
                if !ancestors.insert(parent_id) {
                    return Err(invalid(
                        "SUBJECT_PART_CYCLE_INVALID",
                        "Subject part tree must be acyclic.",
                    ));
                }
                cursor = self
                    .parts
                    .iter()
                    .find(|candidate| candidate.part_id == parent_id)
                    .and_then(|candidate| candidate.parent_part_id.as_deref());
            }
        }
        let mut feature_ids = BTreeSet::new();
        let mut levels = BTreeSet::new();
        for feature in &self.features {
            require_text(&feature.feature_id, "features.feature_id")?;
            require_text(&feature.description, "features.description")?;
            if !part_ids.contains(feature.part_id.as_str())
                || !feature_ids.insert(feature.feature_id.as_str())
            {
                return Err(invalid(
                    "SUBJECT_FEATURE_INVALID",
                    "Features must be unique and reference declared parts.",
                ));
            }
            levels.insert(feature.level);
        }
        if ![
            VisualFeatureLevel::Macro,
            VisualFeatureLevel::Meso,
            VisualFeatureLevel::Micro,
        ]
        .iter()
        .all(|level| levels.contains(level))
        {
            return Err(invalid(
                "SUBJECT_FEATURE_LEVELS_INCOMPLETE",
                "Subject profile must describe macro, meso and micro appearance.",
            ));
        }
        let mut material_ids = BTreeSet::new();
        for material in &self.materials {
            require_text(&material.material_id, "materials.material_id")?;
            require_text(&material.label, "materials.label")?;
            if !material_ids.insert(material.material_id.as_str())
                || material
                    .part_ids
                    .iter()
                    .any(|part_id| !part_ids.contains(part_id.as_str()))
            {
                return Err(invalid(
                    "SUBJECT_MATERIAL_INVALID",
                    "Materials must be unique and reference declared parts.",
                ));
            }
        }
        Ok(())
    }
}

impl VisualFeatureContract {
    pub fn validate_against(
        &self,
        request: &UniversalAuthorRequest,
        profile: &SubjectProfile,
    ) -> CoreResult<()> {
        profile.validate_against(request)?;
        if self.schema_version != VISUAL_FEATURE_CONTRACT_SCHEMA_VERSION
            || self.request_sha256 != semantic_sha256(request)?
            || self.subject_profile_sha256 != semantic_sha256(profile)?
        {
            return Err(invalid(
                "VISUAL_FEATURE_CONTRACT_LINEAGE_INVALID",
                "Visual feature contract lineage is invalid.",
            ));
        }
        let parts = profile
            .parts
            .iter()
            .map(|part| part.part_id.as_str())
            .collect::<BTreeSet<_>>();
        let source_features = profile
            .features
            .iter()
            .map(|feature| (feature.feature_id.as_str(), feature.level))
            .collect::<BTreeMap<_, _>>();
        let request_evidence = request
            .reference_inputs
            .iter()
            .map(|input| input.evidence_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut ids = BTreeSet::new();
        for requirement in &self.requirements {
            if requirement.salience_bps > 10_000
                || !ids.insert(requirement.feature_id.as_str())
                || source_features.get(requirement.feature_id.as_str()) != Some(&requirement.level)
                || requirement.affected_part_ids.is_empty()
                || requirement.channels.is_empty()
                || requirement.minimum_acceptance_views.is_empty()
            {
                return Err(invalid("VISUAL_FEATURE_REQUIREMENT_INVALID", "Feature requirements must be bounded, unique and linked to the SubjectProfile."));
            }
            if requirement
                .affected_part_ids
                .iter()
                .any(|id| !parts.contains(id.as_str()))
            {
                return Err(invalid(
                    "VISUAL_FEATURE_PART_INVALID",
                    "Feature requirement references an unknown part.",
                ));
            }
            for region in &requirement.evidence_regions {
                if !request_evidence.contains(region.evidence_id.as_str()) {
                    return Err(invalid(
                        "VISUAL_FEATURE_EVIDENCE_INVALID",
                        "Feature evidence must belong to the exact request.",
                    ));
                }
                if let Some([left, top, right, bottom]) = region.region_per_mille {
                    if left >= right || top >= bottom || right > 1_000 || bottom > 1_000 {
                        return Err(invalid(
                            "VISUAL_FEATURE_REGION_INVALID",
                            "Evidence region must use ordered per-mille coordinates.",
                        ));
                    }
                }
            }
            if requirement.evidence_status == EvidenceStatus::Observed
                && requirement.evidence_regions.is_empty()
            {
                return Err(invalid(
                    "VISUAL_FEATURE_OBSERVED_UNSUPPORTED",
                    "Observed features require sealed visible evidence.",
                ));
            }
        }
        if ids.len() != source_features.len() {
            return Err(invalid(
                "VISUAL_FEATURE_REQUIREMENTS_INCOMPLETE",
                "Every SubjectProfile feature requires one acceptance requirement.",
            ));
        }
        Ok(())
    }
}

impl RepresentationPlan {
    pub fn validate_against(
        &self,
        request: &UniversalAuthorRequest,
        profile: &SubjectProfile,
        contract: &VisualFeatureContract,
    ) -> CoreResult<bool> {
        contract.validate_against(request, profile)?;
        if self.schema_version != REPRESENTATION_PLAN_SCHEMA_VERSION
            || self.request_sha256 != semantic_sha256(request)?
            || self.subject_profile_sha256 != semantic_sha256(profile)?
            || self.visual_feature_contract_sha256 != semantic_sha256(contract)?
            || self.capability_manifest_sha256 != request.capability_manifest_sha256
        {
            return Err(invalid(
                "REPRESENTATION_PLAN_LINEAGE_INVALID",
                "Representation plan lineage is invalid.",
            ));
        }
        let manifest = representation_capability_manifest();
        let capabilities = manifest
            .capabilities
            .iter()
            .map(|capability| (capability.capability_id.as_str(), capability))
            .collect::<BTreeMap<_, _>>();
        let parts = profile
            .parts
            .iter()
            .map(|part| (part.part_id.as_str(), part))
            .collect::<BTreeMap<_, _>>();
        let features = contract
            .requirements
            .iter()
            .map(|feature| (feature.feature_id.as_str(), feature))
            .collect::<BTreeMap<_, _>>();
        let mut planned_parts = BTreeSet::new();
        let mut all_available = true;
        for part_plan in &self.parts {
            let part = parts.get(part_plan.part_id.as_str()).ok_or_else(|| {
                invalid(
                    "REPRESENTATION_PART_UNKNOWN",
                    "Representation plan references an unknown part.",
                )
            })?;
            if !planned_parts.insert(part_plan.part_id.as_str()) {
                return Err(invalid(
                    "REPRESENTATION_PART_PLAN_INVALID",
                    format!(
                        "Representation plan contains duplicate part_id {}.",
                        part_plan.part_id
                    ),
                ));
            }
            for feature_id in &part_plan.covered_feature_ids {
                let Some(feature) = features.get(feature_id.as_str()) else {
                    return Err(invalid(
                        "REPRESENTATION_PART_PLAN_INVALID",
                        format!(
                            "Part {} covers unknown feature {}.",
                            part_plan.part_id, feature_id
                        ),
                    ));
                };
                if !feature
                    .affected_part_ids
                    .iter()
                    .any(|part_id| part_id == &part_plan.part_id)
                {
                    return Err(invalid(
                        "REPRESENTATION_PART_PLAN_INVALID",
                        format!(
                            "Part {} covers feature {}, but that feature does not list this part in affected_part_ids.",
                            part_plan.part_id, feature_id
                        ),
                    ));
                }
            }
            let capability = capabilities
                .get(part_plan.capability_id.as_str())
                .ok_or_else(|| {
                    invalid(
                        "REPRESENTATION_CAPABILITY_UNKNOWN",
                        "Provider selected an unknown capability.",
                    )
                })?;
            if capability.representation != part_plan.representation {
                return Err(invalid(
                    "REPRESENTATION_KIND_MISMATCH",
                    "Capability and representation kind disagree.",
                ));
            }
            if capability.availability == CapabilityAvailability::Unavailable {
                all_available = false;
            } else {
                let profile_traits = profile
                    .category_tags
                    .iter()
                    .chain(part.traits.iter())
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>();
                if capability
                    .required_subject_traits
                    .iter()
                    .any(|trait_name| !profile_traits.contains(trait_name.as_str()))
                {
                    return Err(invalid(
                        "REPRESENTATION_CAPABILITY_PRECONDITION_FAILED",
                        "The subject does not satisfy the Rust-owned capability preconditions.",
                    ));
                }
            }
        }
        if planned_parts.len() != parts.len() {
            return Err(invalid(
                "REPRESENTATION_PARTS_INCOMPLETE",
                "Every subject part requires one representation plan.",
            ));
        }
        Ok(all_available)
    }
}

impl UniversalAuthorOutcome {
    pub fn validate(&self, evidence: &[ReferenceEvidence]) -> CoreResult<()> {
        match self {
            Self::Executable {
                schema_version,
                request,
                subject_profile,
                visual_feature_contract,
                representation_plan,
                executable_payload,
            } => {
                require_outcome_version(schema_version)?;
                request.validate_with_evidence(evidence)?;
                if !representation_plan.validate_against(
                    request,
                    subject_profile,
                    visual_feature_contract,
                )? {
                    return Err(invalid(
                        "UNIVERSAL_EXECUTABLE_UNAVAILABLE",
                        "Unavailable representations cannot be reported executable.",
                    ));
                }
                if executable_payload.is_null() {
                    return Err(invalid(
                        "UNIVERSAL_EXECUTABLE_PAYLOAD_INVALID",
                        "Executable universal outcomes require a non-null reviewed source payload.",
                    ));
                }
                let capability_ids = representation_plan
                    .parts
                    .iter()
                    .map(|part| part.capability_id.as_str())
                    .collect::<BTreeSet<_>>();
                let local_hybrid = capability_ids.contains(LOCAL_LATTICE_DEFORMABLE_CAPABILITY_ID)
                    && representation_plan.parts.iter().any(|part| {
                        part.representation == RepresentationKind::Procedural
                            && matches!(
                                part.capability_id.as_str(),
                                GENERIC_HARD_SURFACE_PROCEDURAL_CAPABILITY_ID
                                    | GENERIC_VISUAL_EXTERIOR_PROCEDURAL_CAPABILITY_ID
                            )
                    })
                    && representation_plan.parts.iter().any(|part| {
                        part.representation == RepresentationKind::Deformable
                            && part.capability_id == LOCAL_LATTICE_DEFORMABLE_CAPABILITY_ID
                    });
                if local_hybrid {
                    let lowering = lower_visual_runtime_source_v1(executable_payload)?;
                    let expected_domain = if capability_ids
                        .contains(GENERIC_VISUAL_EXTERIOR_PROCEDURAL_CAPABILITY_ID)
                    {
                        "generic_visual_exterior"
                    } else {
                        "generic_hard_surface"
                    };
                    if lowering.source_contract_id != "ForgeVisualGeometryProgram@2"
                        || executable_payload.get("domain").and_then(Value::as_str)
                            != Some(expected_domain)
                    {
                        return Err(invalid(
                            "UNIVERSAL_LOCAL_HYBRID_SOURCE_INVALID",
                            "Local hybrid execution requires a reviewed generic visual ForgeVisualGeometryProgram@2 source with a matching visual domain.",
                        ));
                    }
                    return Ok(());
                }
                let procedural_composition = capability_ids.len() > 1
                    && representation_plan.parts.iter().all(|part| {
                        part.representation == RepresentationKind::Procedural
                            && matches!(
                                part.capability_id.as_str(),
                                GENERIC_HARD_SURFACE_PROCEDURAL_CAPABILITY_ID
                                    | GENERIC_VISUAL_EXTERIOR_PROCEDURAL_CAPABILITY_ID
                            )
                    });
                if procedural_composition {
                    let lowering = lower_visual_runtime_source_v1(executable_payload)?;
                    let expected_domain = if capability_ids
                        .contains(GENERIC_VISUAL_EXTERIOR_PROCEDURAL_CAPABILITY_ID)
                    {
                        "generic_visual_exterior"
                    } else {
                        "generic_hard_surface"
                    };
                    if lowering.source_contract_id != "ForgeVisualGeometryProgram@2"
                        || executable_payload.get("domain").and_then(Value::as_str)
                            != Some(expected_domain)
                    {
                        return Err(invalid(
                            "UNIVERSAL_GENERIC_HARD_SURFACE_SOURCE_INVALID",
                            "Procedural capability composition requires a reviewed ForgeVisualGeometryProgram@2 with a matching visual domain.",
                        ));
                    }
                    return Ok(());
                }
                if capability_ids.len() != 1 {
                    return Err(invalid(
                        "UNIVERSAL_EXECUTABLE_CAPABILITY_MIXED",
                        "An executable candidate may mix only reviewed procedural visual capabilities or the bounded procedural/lattice hybrid.",
                    ));
                }
                match capability_ids.into_iter().next() {
                    Some(ROBOTIC_ARM_PROCEDURAL_CAPABILITY_ID) => {}
                    Some(GENERIC_HARD_SURFACE_PROCEDURAL_CAPABILITY_ID)
                    | Some(GENERIC_VISUAL_EXTERIOR_PROCEDURAL_CAPABILITY_ID) => {
                        let lowering = lower_visual_runtime_source_v1(executable_payload)?;
                        if lowering.source_contract_id != "ForgeVisualGeometryProgram@2"
                            || !matches!(
                                executable_payload.get("domain").and_then(Value::as_str),
                                Some("generic_hard_surface" | "generic_visual_exterior")
                            )
                        {
                            return Err(invalid(
                                "UNIVERSAL_GENERIC_HARD_SURFACE_SOURCE_INVALID",
                                "Generic exterior execution requires a reviewed ForgeVisualGeometryProgram@2 with a code-owned visual domain.",
                            ));
                        }
                    }
                    Some(LOCAL_LATTICE_DEFORMABLE_CAPABILITY_ID)
                    | Some(LOCAL_MESH_PATCH_CAPABILITY_ID) => {
                        let lowering = lower_visual_runtime_source_v1(executable_payload)?;
                        if lowering.source_contract_id != "ForgeVisualGeometryProgram@2"
                            || executable_payload.get("domain").and_then(Value::as_str)
                                != Some("generic_hard_surface")
                        {
                            return Err(invalid(
                                "UNIVERSAL_LOCAL_LATTICE_SOURCE_INVALID",
                                "Local lattice deformation requires a reviewed generic-hard-surface ForgeVisualGeometryProgram@2 source.",
                            ));
                        }
                    }
                    _ => {
                        return Err(invalid(
                            "UNIVERSAL_EXECUTABLE_PAYLOAD_INVALID",
                                "Only the verified robotic-arm, generic hard-surface, local lattice, local mesh patch, and their bounded local hybrid capabilities may be executable.",
                        ));
                    }
                }
            }
            Self::Limitation {
                schema_version,
                request,
                subject_profile,
                visual_feature_contract,
                representation_plan,
                limitation,
            } => {
                require_outcome_version(schema_version)?;
                request.validate_with_evidence(evidence)?;
                representation_plan.validate_against(
                    request,
                    subject_profile,
                    visual_feature_contract,
                )?;
                if limitation.schema_version != REPRESENTATION_LIMITATION_SCHEMA_VERSION
                    || limitation.message.trim().is_empty()
                {
                    return Err(invalid(
                        "REPRESENTATION_LIMITATION_INVALID",
                        "Typed limitations require the exact schema and a message.",
                    ));
                }
                let part_ids = subject_profile
                    .parts
                    .iter()
                    .map(|part| part.part_id.as_str())
                    .collect::<BTreeSet<_>>();
                if limitation
                    .affected_part_ids
                    .iter()
                    .any(|part_id| !part_ids.contains(part_id.as_str()))
                {
                    return Err(invalid(
                        "REPRESENTATION_LIMITATION_PART_INVALID",
                        "Limitation may reference only affected SubjectProfile parts.",
                    ));
                }
                let manifest = representation_capability_manifest();
                if limitation
                    .missing_capability_ids
                    .iter()
                    .any(|capability_id| {
                        !manifest.capabilities.iter().any(|capability| {
                            capability.capability_id == *capability_id
                                && capability.availability == CapabilityAvailability::Unavailable
                        })
                    })
                {
                    return Err(invalid(
                        "REPRESENTATION_LIMITATION_CAPABILITY_INVALID",
                        "Missing capabilities must be code-owned and currently unavailable.",
                    ));
                }
                if limitation.code == RepresentationLimitationCode::NeedsMoreViews
                    && limitation.suggested_views.is_empty()
                {
                    return Err(invalid(
                        "REPRESENTATION_LIMITATION_VIEWS_REQUIRED",
                        "needs_more_views requires at least one bounded view suggestion.",
                    ));
                }
            }
            Self::ClarificationRequired {
                schema_version,
                request,
                reason,
                questions,
            } => {
                require_outcome_version(schema_version)?;
                request.validate_with_evidence(evidence)?;
                if reason.trim().is_empty() || questions.is_empty() {
                    return Err(invalid(
                        "UNIVERSAL_CLARIFICATION_INVALID",
                        "Clarification is reserved for a concrete identity or target conflict.",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl VisualEvidenceGraphV2 {
    pub fn validate_against(
        &self,
        request: &UniversalAuthorRequest,
        profile: &SubjectProfile,
    ) -> CoreResult<()> {
        if self.schema_version != VISUAL_EVIDENCE_GRAPH_V2_SCHEMA_VERSION
            || self.universal_request_sha256 != semantic_sha256(request)?
            || self.subject_profile_sha256 != semantic_sha256(profile)?
        {
            return Err(invalid(
                "VISUAL_EVIDENCE_GRAPH_V2_LINEAGE_INVALID",
                "VisualEvidenceGraph@2 must bind the universal request and profile.",
            ));
        }
        let features = profile
            .features
            .iter()
            .map(|feature| feature.feature_id.as_str())
            .collect::<BTreeSet<_>>();
        let evidence = request
            .reference_inputs
            .iter()
            .map(|input| input.evidence_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut claims = BTreeSet::new();
        for claim in &self.claims {
            if !claims.insert(claim.claim_id.as_str())
                || !features.contains(claim.feature_id.as_str())
                || claim
                    .evidence_regions
                    .iter()
                    .any(|region| !evidence.contains(region.evidence_id.as_str()))
            {
                return Err(invalid("VISUAL_EVIDENCE_GRAPH_V2_CLAIM_INVALID", "Evidence claims must uniquely reference declared features and sealed request evidence."));
            }
            if claim.status == EvidenceStatus::Observed && claim.evidence_regions.is_empty() {
                return Err(invalid(
                    "VISUAL_EVIDENCE_GRAPH_V2_OBSERVED_UNSUPPORTED",
                    "Observed claims require a sealed evidence region.",
                ));
            }
        }
        Ok(())
    }
}

fn require_outcome_version(version: &str) -> CoreResult<()> {
    if version != UNIVERSAL_AUTHOR_OUTCOME_SCHEMA_VERSION {
        return Err(invalid(
            "UNIVERSAL_OUTCOME_SCHEMA_INVALID",
            "Unsupported universal outcome schema.",
        ));
    }
    Ok(())
}

fn require_sha(value: &str, field: &str) -> CoreResult<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid(
            "UNIVERSAL_SHA256_INVALID",
            format!("{field} must be a SHA-256 hex digest."),
        ));
    }
    Ok(())
}

fn require_text(value: &str, field: &str) -> CoreResult<()> {
    if value.trim().is_empty() || value.len() > 200_000 || value.chars().any(char::is_control) {
        return Err(invalid(
            "UNIVERSAL_TEXT_INVALID",
            format!("{field} is empty or unsafe."),
        ));
    }
    Ok(())
}

fn invalid(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> UniversalAuthorRequest {
        UniversalAuthorRequest {
            schema_version: UNIVERSAL_AUTHOR_REQUEST_SCHEMA_VERSION.into(),
            request_id: "uareq_test".into(),
            project_id: "project_test".into(),
            turn_id: "turn_test".into(),
            instruction: "design a cat".into(),
            input_mode: UniversalInputMode::Text,
            reference_inputs: Vec::new(),
            active_asset: None,
            selection: UniversalSelectionScope::default(),
            locks: UniversalDesignLocks::default(),
            capability_manifest_sha256: representation_capability_manifest_sha256().unwrap(),
        }
    }

    fn profile(
        request: &UniversalAuthorRequest,
        category: &str,
        traits: Vec<String>,
    ) -> SubjectProfile {
        SubjectProfile {
            schema_version: SUBJECT_PROFILE_SCHEMA_VERSION.into(),
            profile_id: "subject_test".into(),
            request_sha256: semantic_sha256(request).unwrap(),
            identity_label: category.into(),
            category: category.into(),
            category_tags: traits.clone(),
            silhouette: "readable outer contour".into(),
            negative_space: "leg gaps".into(),
            pose: "standing".into(),
            visible_views: vec!["front".into()],
            occlusions: vec!["rear hidden".into()],
            uncertainties: vec!["back surface".into()],
            parts: vec![SubjectPart {
                part_id: "part_body".into(),
                parent_part_id: None,
                label: "body".into(),
                semantic_role: "primary_mass".into(),
                traits,
                uncertainty_bps: 1000,
            }],
            features: vec![
                SubjectFeature {
                    feature_id: "feature_macro".into(),
                    part_id: "part_body".into(),
                    level: VisualFeatureLevel::Macro,
                    description: "silhouette".into(),
                },
                SubjectFeature {
                    feature_id: "feature_meso".into(),
                    part_id: "part_body".into(),
                    level: VisualFeatureLevel::Meso,
                    description: "surface regions".into(),
                },
                SubjectFeature {
                    feature_id: "feature_micro".into(),
                    part_id: "part_body".into(),
                    level: VisualFeatureLevel::Micro,
                    description: "finish".into(),
                },
            ],
            materials: vec![SubjectMaterial {
                material_id: "material_primary".into(),
                label: "primary".into(),
                part_ids: vec!["part_body".into()],
                appearance_traits: vec!["matte".into()],
            }],
        }
    }

    fn feature_contract(
        request: &UniversalAuthorRequest,
        profile: &SubjectProfile,
    ) -> VisualFeatureContract {
        VisualFeatureContract {
            schema_version: VISUAL_FEATURE_CONTRACT_SCHEMA_VERSION.into(),
            contract_id: "vfcontract_test".into(),
            request_sha256: semantic_sha256(request).unwrap(),
            subject_profile_sha256: semantic_sha256(profile).unwrap(),
            requirements: profile
                .features
                .iter()
                .map(|feature| VisualFeatureRequirement {
                    feature_id: feature.feature_id.clone(),
                    level: feature.level,
                    description: feature.description.clone(),
                    salience_bps: 8000,
                    evidence_status: EvidenceStatus::Inferred,
                    evidence_regions: Vec::new(),
                    affected_part_ids: vec!["part_body".into()],
                    channels: vec![AppearanceChannel::Geometry],
                    minimum_acceptance_views: vec!["front".into()],
                })
                .collect(),
        }
    }

    #[test]
    fn u002_open_category_is_understood_but_unavailable_capability_is_not_executable() {
        let request = request();
        let profile = profile(&request, "domestic cat", vec!["quadruped".into()]);
        let contract = feature_contract(&request, &profile);
        let plan = RepresentationPlan {
            schema_version: REPRESENTATION_PLAN_SCHEMA_VERSION.into(),
            plan_id: "repplan_cat".into(),
            request_sha256: semantic_sha256(&request).unwrap(),
            subject_profile_sha256: semantic_sha256(&profile).unwrap(),
            visual_feature_contract_sha256: semantic_sha256(&contract).unwrap(),
            capability_manifest_sha256: request.capability_manifest_sha256.clone(),
            parts: vec![PartRepresentationPlan {
                part_id: "part_body".into(),
                representation: RepresentationKind::Deformable,
                capability_id: "deformable.generic_v1".into(),
                covered_feature_ids: contract
                    .requirements
                    .iter()
                    .map(|item| item.feature_id.clone())
                    .collect(),
                rationale: "organic form".into(),
            }],
        };
        assert!(!plan
            .validate_against(&request, &profile, &contract)
            .unwrap());
    }

    #[test]
    fn u002_cat_cannot_claim_robotic_arm_capability() {
        let request = request();
        let profile = profile(&request, "domestic cat", vec!["quadruped".into()]);
        let contract = feature_contract(&request, &profile);
        let plan = RepresentationPlan {
            schema_version: REPRESENTATION_PLAN_SCHEMA_VERSION.into(),
            plan_id: "repplan_fake".into(),
            request_sha256: semantic_sha256(&request).unwrap(),
            subject_profile_sha256: semantic_sha256(&profile).unwrap(),
            visual_feature_contract_sha256: semantic_sha256(&contract).unwrap(),
            capability_manifest_sha256: request.capability_manifest_sha256.clone(),
            parts: vec![PartRepresentationPlan {
                part_id: "part_body".into(),
                representation: RepresentationKind::Procedural,
                capability_id: ROBOTIC_ARM_PROCEDURAL_CAPABILITY_ID.into(),
                covered_feature_ids: contract
                    .requirements
                    .iter()
                    .map(|item| item.feature_id.clone())
                    .collect(),
                rationale: "invalid".into(),
            }],
        };
        assert_eq!(
            plan.validate_against(&request, &profile, &contract)
                .unwrap_err()
                .code(),
            "REPRESENTATION_CAPABILITY_PRECONDITION_FAILED"
        );
    }

    #[test]
    fn u004_generic_visual_exterior_accepts_open_category_without_provider_trait() {
        let request = request();
        let profile = profile(&request, "domestic cat", vec!["quadruped".into(), "organic".into()]);
        let contract = feature_contract(&request, &profile);
        let plan = RepresentationPlan {
            schema_version: REPRESENTATION_PLAN_SCHEMA_VERSION.into(),
            plan_id: "repplan_cat_visual_exterior".into(),
            request_sha256: semantic_sha256(&request).unwrap(),
            subject_profile_sha256: semantic_sha256(&profile).unwrap(),
            visual_feature_contract_sha256: semantic_sha256(&contract).unwrap(),
            capability_manifest_sha256: request.capability_manifest_sha256.clone(),
            parts: vec![PartRepresentationPlan {
                part_id: "part_body".into(),
                representation: RepresentationKind::Procedural,
                capability_id: GENERIC_VISUAL_EXTERIOR_PROCEDURAL_CAPABILITY_ID.into(),
                covered_feature_ids: contract
                    .requirements
                    .iter()
                    .map(|item| item.feature_id.clone())
                    .collect(),
                rationale: "visible cat appearance can use the bounded exterior proxy while dedicated organic representation remains unavailable".into(),
            }],
        };

        assert!(plan.validate_against(&request, &profile, &contract).unwrap());
    }

    #[test]
    fn u004_generic_hard_surface_accepts_only_the_reviewed_geometry_source() {
        let request = request();
        let profile = profile(
            &request,
            "fictional armored drone shell",
            vec!["hard_surface".into()],
        );
        let contract = feature_contract(&request, &profile);
        let plan = RepresentationPlan {
            schema_version: REPRESENTATION_PLAN_SCHEMA_VERSION.into(),
            plan_id: "repplan_hard_surface".into(),
            request_sha256: semantic_sha256(&request).unwrap(),
            subject_profile_sha256: semantic_sha256(&profile).unwrap(),
            visual_feature_contract_sha256: semantic_sha256(&contract).unwrap(),
            capability_manifest_sha256: request.capability_manifest_sha256.clone(),
            parts: vec![PartRepresentationPlan {
                part_id: "part_body".into(),
                representation: RepresentationKind::Procedural,
                capability_id: GENERIC_HARD_SURFACE_PROCEDURAL_CAPABILITY_ID.into(),
                covered_feature_ids: contract
                    .requirements
                    .iter()
                    .map(|item| item.feature_id.clone())
                    .collect(),
                rationale: "reviewed local hard-surface compiler".into(),
            }],
        };
        let mut source: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/concept-spec/fixtures/forge-visual-geometry-v2-bracket.json"
        )))
        .unwrap();
        source["domain"] = Value::String("generic_hard_surface".into());
        let mut outcome = UniversalAuthorOutcome::Executable {
            schema_version: UNIVERSAL_AUTHOR_OUTCOME_SCHEMA_VERSION.into(),
            request,
            subject_profile: profile,
            visual_feature_contract: contract,
            representation_plan: plan,
            executable_payload: source,
        };
        outcome.validate(&[]).unwrap();
        let UniversalAuthorOutcome::Executable {
            executable_payload, ..
        } = &mut outcome
        else {
            unreachable!()
        };
        executable_payload["domain"] = Value::String("robotic_arm".into());
        assert_eq!(
            outcome.validate(&[]).unwrap_err().code(),
            "UNIVERSAL_GENERIC_HARD_SURFACE_SOURCE_INVALID"
        );
    }

    #[test]
    fn u004_distinct_procedural_visual_capabilities_can_compose_by_part() {
        let request = request();
        let mut profile = profile(
            &request,
            "fictional vehicle with an armored shell and transparent canopy",
            vec!["hard_surface".into(), "visual_exterior".into()],
        );
        profile.parts.push(SubjectPart {
            part_id: "part_canopy".into(),
            parent_part_id: Some("part_body".into()),
            label: "canopy".into(),
            semantic_role: "transparent_exterior_shell".into(),
            traits: vec!["visual_exterior".into()],
            uncertainty_bps: 2500,
        });
        profile.features.push(SubjectFeature {
            feature_id: "feature_canopy_meso".into(),
            part_id: "part_canopy".into(),
            level: VisualFeatureLevel::Meso,
            description: "透明外壳与主体的分件关系".into(),
        });
        let mut contract = feature_contract(&request, &profile);
        contract
            .requirements
            .iter_mut()
            .find(|requirement| requirement.feature_id == "feature_canopy_meso")
            .expect("canopy feature contract row")
            .affected_part_ids = vec!["part_canopy".into()];
        let plan = RepresentationPlan {
            schema_version: REPRESENTATION_PLAN_SCHEMA_VERSION.into(),
            plan_id: "repplan_vehicle_composed_procedural".into(),
            request_sha256: semantic_sha256(&request).unwrap(),
            subject_profile_sha256: semantic_sha256(&profile).unwrap(),
            visual_feature_contract_sha256: semantic_sha256(&contract).unwrap(),
            capability_manifest_sha256: request.capability_manifest_sha256.clone(),
            parts: vec![
                PartRepresentationPlan {
                    part_id: "part_body".into(),
                    representation: RepresentationKind::Procedural,
                    capability_id: GENERIC_HARD_SURFACE_PROCEDURAL_CAPABILITY_ID.into(),
                    covered_feature_ids: vec![
                        "feature_macro".into(),
                        "feature_meso".into(),
                        "feature_micro".into(),
                    ],
                    rationale: "armored primary shell".into(),
                },
                PartRepresentationPlan {
                    part_id: "part_canopy".into(),
                    representation: RepresentationKind::Procedural,
                    capability_id: GENERIC_VISUAL_EXTERIOR_PROCEDURAL_CAPABILITY_ID.into(),
                    covered_feature_ids: vec!["feature_canopy_meso".into()],
                    rationale: "distinct exterior canopy part".into(),
                },
            ],
        };
        let mut source: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/concept-spec/fixtures/forge-visual-geometry-v2-bracket.json"
        )))
        .unwrap();
        source["domain"] = Value::String("generic_visual_exterior".into());
        let outcome = UniversalAuthorOutcome::Executable {
            schema_version: UNIVERSAL_AUTHOR_OUTCOME_SCHEMA_VERSION.into(),
            request,
            subject_profile: profile,
            visual_feature_contract: contract,
            representation_plan: plan,
            executable_payload: source,
        };
        outcome.validate(&[]).unwrap();
    }
}
