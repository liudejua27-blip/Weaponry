//! Bounded Design Surface layer contracts.
//!
//! `SurfaceLayerProgram@1` is a data-only 2D surface language, not a DOM or
//! SVG document. It admits normalized Bézier commands and reviewed material
//! tokens, but never script text, URLs, file paths or geometry operations.
//! Only its exact normal-relief subset lowers to existing A005 texture bakes.
//! The remaining layers are returned as sealed future-PBR compiler input and
//! are never represented as having already been rendered.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    semantic_sha256,
    skills::{valid_prefixed_id, valid_sha256, MATERIAL_PRESET_IDS},
    CoreError, CoreResult, SurfaceAdornmentProgram,
};

const MAX_VECTOR_PATHS: usize = 8;
const MAX_COMMANDS_PER_PATH: usize = 32;
const MAX_VECTOR_POINTS: usize = 128;
const MAX_DECAL_LAYERS: usize = 4;
const MAX_NORMAL_RELIEF_LAYERS: usize = 4;
const MAX_ROUGHNESS_MASKS: usize = 2;
const MAX_EMISSIVE_MASKS: usize = 2;
const MAX_TOTAL_PBR_LAYERS: usize = 8;
const PART_ROLES: &[&str] = &[
    "base_form",
    "turntable",
    "joint_housing",
    "link_armor",
    "cable_harness",
    "end_effector_form",
    "surface_trim",
];
const COVERAGES: &[&str] = &["full_zone", "center_band", "edge_band", "symmetric_pair"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VectorPathCommand {
    pub kind: String,
    pub points: Vec<[f32; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VectorPath {
    pub path_id: String,
    pub closed: bool,
    pub commands: Vec<VectorPathCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DecalLayer {
    pub decal_id: String,
    pub motif: String,
    pub text_token: String,
    pub color_token: String,
    pub anchor_uv: [f32; 2],
    pub scale_milli: u16,
    pub opacity_milli: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NormalReliefLayer {
    pub layer_id: String,
    pub motif: String,
    pub intensity: String,
    pub coverage: String,
    pub seed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RoughnessMask {
    pub mask_id: String,
    pub motif: String,
    pub coverage: String,
    pub intensity_milli: u16,
    pub seed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EmissiveMask {
    pub mask_id: String,
    pub motif: String,
    pub color_token: String,
    pub coverage: String,
    pub intensity_milli: u16,
    pub seed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SurfaceSymmetry {
    pub mode: String,
    pub center_uv: [f32; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UvFrame {
    pub frame_id: String,
    pub u_min: f32,
    pub u_max: f32,
    pub v_min: f32,
    pub v_max: f32,
    pub rotation_degrees: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SurfaceLayerProgram {
    pub schema_version: String,
    pub program_id: String,
    /// Compatibility mapping to the stable current AssemblyGraph part.
    pub target_part_id: String,
    /// Compatibility mapping to the stable current Material Zone.
    pub target_zone_id: String,
    pub target_part_role: String,
    pub material_zone_id: String,
    pub base_material: String,
    pub vector_paths: Vec<VectorPath>,
    pub decal_layers: Vec<DecalLayer>,
    pub normal_relief_layers: Vec<NormalReliefLayer>,
    pub roughness_masks: Vec<RoughnessMask>,
    pub emissive_masks: Vec<EmissiveMask>,
    pub symmetry: SurfaceSymmetry,
    pub uv_frame: UvFrame,
    pub quality_profile: String,
    pub execution: String,
    pub skill_id: String,
    pub skill_version: u32,
    pub skill_sha256: String,
    pub generator: String,
    pub non_functional_only: bool,
}

/// The subset no current A005 compiler is permitted to render. It remains a
/// sealed data payload with a semantic hash so a future reviewed PBR compiler
/// can consume exactly the same surface intent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RetainedSurfaceLayers {
    pub vector_paths: Vec<VectorPath>,
    pub decal_layers: Vec<DecalLayer>,
    pub roughness_masks: Vec<RoughnessMask>,
    pub emissive_masks: Vec<EmissiveMask>,
    pub symmetry: SurfaceSymmetry,
    pub uv_frame: UvFrame,
    pub quality_profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SurfaceLayerLowering {
    pub schema_version: String,
    pub source_program_sha256: String,
    pub adornments: Vec<SurfaceAdornmentProgram>,
    pub retained_layers: RetainedSurfaceLayers,
    pub retained_layers_sha256: String,
}

impl RetainedSurfaceLayers {
    pub fn validate(&self) -> CoreResult<()> {
        if self.vector_paths.len() > MAX_VECTOR_PATHS
            || self.decal_layers.len() > MAX_DECAL_LAYERS
            || self.roughness_masks.len() > MAX_ROUGHNESS_MASKS
            || self.emissive_masks.len() > MAX_EMISSIVE_MASKS
            || self.decal_layers.len() + self.roughness_masks.len() + self.emissive_masks.len()
                > MAX_TOTAL_PBR_LAYERS
            || !valid_symmetry(&self.symmetry)
            || !valid_uv_frame(&self.uv_frame)
            || !matches!(
                self.quality_profile.as_str(),
                "interactive_preview" | "production_concept"
            )
        {
            return Err(invalid());
        }

        let mut ids = BTreeSet::new();
        let mut total_points = 0usize;
        if !self.vector_paths.iter().all(|path| {
            ids.insert(&path.path_id)
                && valid_prefixed_id(&path.path_id, "path_")
                && valid_vector_path(path, &mut total_points)
        }) || total_points > MAX_VECTOR_POINTS
        {
            return Err(invalid());
        }
        if !self
            .decal_layers
            .iter()
            .all(|layer| ids.insert(&layer.decal_id) && valid_decal_layer(layer))
            || !self
                .roughness_masks
                .iter()
                .all(|mask| ids.insert(&mask.mask_id) && valid_roughness_mask(mask))
            || !self
                .emissive_masks
                .iter()
                .all(|mask| ids.insert(&mask.mask_id) && valid_emissive_mask(mask))
        {
            return Err(invalid());
        }
        Ok(())
    }
}

impl SurfaceLayerLowering {
    /// Verifies the sealed lowering independently of an authoring program.
    /// This is intentionally narrower than `SurfaceLayerProgram::validate`:
    /// it verifies exact retained facts and A005 instructions but never treats
    /// a deserialized lowering as permission to invent a new design program.
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != "SurfaceLayerLowering@1"
            || !valid_sha256(&self.source_program_sha256)
            || self.adornments.len() > MAX_NORMAL_RELIEF_LAYERS
            || !valid_sha256(&self.retained_layers_sha256)
            || semantic_sha256(&self.retained_layers)? != self.retained_layers_sha256
        {
            return Err(invalid());
        }
        self.retained_layers.validate()?;
        let mut ids = BTreeSet::new();
        let mut target: Option<(&str, &str, &str)> = None;
        for adornment in &self.adornments {
            adornment.validate()?;
            if adornment.kind != "normal_relief" || !ids.insert(&adornment.program_id) {
                return Err(invalid());
            }
            let current = (
                adornment.target_part_id.as_str(),
                adornment.target_zone_id.as_str(),
                adornment.base_material.as_str(),
            );
            if let Some(expected) = target {
                if current != expected {
                    return Err(invalid());
                }
            } else {
                target = Some(current);
            }
        }
        Ok(())
    }

    pub fn adornments(&self) -> &[SurfaceAdornmentProgram] {
        &self.adornments
    }
}

impl SurfaceLayerProgram {
    /// Validates a closed, bounded 2D surface language. The language has no
    /// free text field and all curves use normalized coordinates, so it can be
    /// displayed by an SVG/canvas editor later without treating SVG as an
    /// asset, execution input or model truth.
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != "SurfaceLayerProgram@1"
            || !valid_prefixed_id(&self.program_id, "surface_layer_")
            || !valid_prefixed_id(&self.target_part_id, "part_")
            || !valid_prefixed_id(&self.target_zone_id, "zone_")
            || !PART_ROLES.contains(&self.target_part_role.as_str())
            || !valid_prefixed_id(&self.material_zone_id, "zone_")
            || self.target_zone_id != self.material_zone_id
            || !MATERIAL_PRESET_IDS.contains(&self.base_material.as_str())
            || self.vector_paths.len() > MAX_VECTOR_PATHS
            || self.decal_layers.len() > MAX_DECAL_LAYERS
            || self.normal_relief_layers.len() > MAX_NORMAL_RELIEF_LAYERS
            || self.roughness_masks.len() > MAX_ROUGHNESS_MASKS
            || self.emissive_masks.len() > MAX_EMISSIVE_MASKS
            || total_pbr_layer_count(self) > MAX_TOTAL_PBR_LAYERS
            || self.execution != "lower_to_a005_and_retain"
            || !valid_prefixed_id(&self.skill_id, "skill_")
            || self.skill_version == 0
            || !valid_sha256(&self.skill_sha256)
            || self.generator != "surface_layer_v1"
            || !self.non_functional_only
            || !matches!(
                self.quality_profile.as_str(),
                "interactive_preview" | "production_concept"
            )
            || !valid_symmetry(&self.symmetry)
            || !valid_uv_frame(&self.uv_frame)
        {
            return Err(invalid());
        }

        if self.vector_paths.is_empty() && total_pbr_layer_count(self) == 0 {
            return Err(invalid());
        }

        let mut ids = BTreeSet::new();
        let mut total_points = 0usize;
        if !self.vector_paths.iter().all(|path| {
            ids.insert(&path.path_id)
                && valid_prefixed_id(&path.path_id, "path_")
                && valid_vector_path(path, &mut total_points)
        }) || total_points > MAX_VECTOR_POINTS
        {
            return Err(invalid());
        }
        if !self
            .decal_layers
            .iter()
            .all(|layer| ids.insert(&layer.decal_id) && valid_decal_layer(layer))
            || !self
                .normal_relief_layers
                .iter()
                .all(|layer| ids.insert(&layer.layer_id) && valid_normal_relief_layer(layer))
            || !self
                .roughness_masks
                .iter()
                .all(|mask| ids.insert(&mask.mask_id) && valid_roughness_mask(mask))
            || !self
                .emissive_masks
                .iter()
                .all(|mask| ids.insert(&mask.mask_id) && valid_emissive_mask(mask))
        {
            return Err(invalid());
        }

        Ok(())
    }

    pub fn canonical_sha256(&self) -> CoreResult<String> {
        self.validate()?;
        semantic_sha256(self)
    }

    /// Lowers only the exact A005 normal-relief subset. Vector paths, decals,
    /// roughness and emissive masks remain explicitly retained; their hash is
    /// provenance for a future reviewed PBR compiler, not evidence of a bake.
    pub fn lower(&self) -> CoreResult<SurfaceLayerLowering> {
        let source_program_sha256 = self.canonical_sha256()?;
        let adornments = self
            .normal_relief_layers
            .iter()
            .enumerate()
            .map(|(ordinal, layer)| SurfaceAdornmentProgram {
                schema_version: "SurfaceAdornmentProgram@1".into(),
                program_id: format!("adorn_{}_{}", &source_program_sha256[..40], ordinal + 1),
                target_part_id: self.target_part_id.clone(),
                target_zone_id: self.target_zone_id.clone(),
                kind: "normal_relief".into(),
                motif: layer.motif.clone(),
                intensity: layer.intensity.clone(),
                coverage: layer.coverage.clone(),
                seed: layer.seed,
                base_material: self.base_material.clone(),
                execution: "texture_bake".into(),
                skill_id: self.skill_id.clone(),
                skill_version: self.skill_version,
                skill_sha256: self.skill_sha256.clone(),
                generator: "a005_v1".into(),
                non_functional_only: true,
            })
            .collect::<Vec<_>>();
        for adornment in &adornments {
            adornment.validate()?;
        }

        let retained_layers = RetainedSurfaceLayers {
            vector_paths: self.vector_paths.clone(),
            decal_layers: self.decal_layers.clone(),
            roughness_masks: self.roughness_masks.clone(),
            emissive_masks: self.emissive_masks.clone(),
            symmetry: self.symmetry.clone(),
            uv_frame: self.uv_frame.clone(),
            quality_profile: self.quality_profile.clone(),
        };
        let retained_layers_sha256 = semantic_sha256(&retained_layers)?;
        let lowering = SurfaceLayerLowering {
            schema_version: "SurfaceLayerLowering@1".into(),
            source_program_sha256,
            adornments,
            retained_layers,
            retained_layers_sha256,
        };
        lowering.validate()?;
        Ok(lowering)
    }

    /// Compatibility helper for callers that currently accept only A005
    /// programs. It intentionally discards retained layers rather than
    /// pretending that the existing A005 bake compiled them.
    pub fn lower_to_surface_adornments(&self) -> CoreResult<Vec<SurfaceAdornmentProgram>> {
        Ok(self.lower()?.adornments)
    }
}

fn total_pbr_layer_count(program: &SurfaceLayerProgram) -> usize {
    program.decal_layers.len()
        + program.normal_relief_layers.len()
        + program.roughness_masks.len()
        + program.emissive_masks.len()
}

fn valid_vector_path(path: &VectorPath, total_points: &mut usize) -> bool {
    if !(2..=MAX_COMMANDS_PER_PATH).contains(&path.commands.len())
        || path
            .commands
            .first()
            .is_none_or(|command| command.kind != "move")
    {
        return false;
    }
    path.commands.iter().all(|command| {
        let expected_points = match command.kind.as_str() {
            "move" | "line" => 1,
            "quadratic" => 2,
            "cubic" => 3,
            _ => return false,
        };
        *total_points += command.points.len();
        command.points.len() == expected_points
            && command
                .points
                .iter()
                .all(|point| valid_normalized_uv(*point))
    })
}

fn valid_decal_layer(layer: &DecalLayer) -> bool {
    valid_prefixed_id(&layer.decal_id, "decal_")
        && matches!(
            layer.motif.as_str(),
            "chevron_mark" | "hex_badge" | "warning_stripe" | "panel_label"
        )
        && matches!(
            layer.text_token.as_str(),
            "none" | "A-01" | "SERVICE" | "CAUTION" | "01"
        )
        && matches!(
            layer.color_token.as_str(),
            "accent_blue" | "signal_red" | "graphite" | "aluminum"
        )
        && valid_normalized_uv(layer.anchor_uv)
        && (50..=500).contains(&layer.scale_milli)
        && layer.opacity_milli <= 1000
}

fn valid_normal_relief_layer(layer: &NormalReliefLayer) -> bool {
    valid_prefixed_id(&layer.layer_id, "relief_")
        && matches!(layer.motif.as_str(), "parallel_groove" | "chevron_relief")
        && matches!(
            layer.intensity.as_str(),
            "subtle" | "balanced" | "pronounced"
        )
        && COVERAGES.contains(&layer.coverage.as_str())
}

fn valid_roughness_mask(mask: &RoughnessMask) -> bool {
    valid_prefixed_id(&mask.mask_id, "rough_")
        && matches!(
            mask.motif.as_str(),
            "linear_brush" | "edge_wear" | "microgrid"
        )
        && COVERAGES.contains(&mask.coverage.as_str())
        && mask.intensity_milli <= 1000
}

fn valid_emissive_mask(mask: &EmissiveMask) -> bool {
    valid_prefixed_id(&mask.mask_id, "emissive_")
        && matches!(
            mask.motif.as_str(),
            "double_flowline" | "dot_array" | "panel_indicator"
        )
        && matches!(mask.color_token.as_str(), "accent_blue" | "signal_red")
        && COVERAGES.contains(&mask.coverage.as_str())
        && mask.intensity_milli <= 1000
}

fn valid_symmetry(value: &SurfaceSymmetry) -> bool {
    matches!(
        value.mode.as_str(),
        "none" | "mirror_u" | "mirror_v" | "radial_2" | "radial_4"
    ) && valid_normalized_uv(value.center_uv)
}

fn valid_uv_frame(value: &UvFrame) -> bool {
    valid_prefixed_id(&value.frame_id, "uvframe_")
        && valid_unit(value.u_min)
        && valid_unit(value.u_max)
        && valid_unit(value.v_min)
        && valid_unit(value.v_max)
        && value.u_min < value.u_max
        && value.v_min < value.v_max
        && value.rotation_degrees.is_finite()
        && (-180.0..=180.0).contains(&value.rotation_degrees)
}

fn valid_normalized_uv(value: [f32; 2]) -> bool {
    value.iter().all(|coordinate| valid_unit(*coordinate))
}

fn valid_unit(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn invalid() -> CoreError {
    CoreError::invalid_data(
        "SURFACE_LAYER_PROGRAM_INVALID",
        "Surface layers must remain bounded visual data that lower only to A005 texture bakes or retained PBR compiler input.",
    )
}
