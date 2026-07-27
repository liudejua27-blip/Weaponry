//! Rust-owned authoring contract for the programmatic Forge Studio MVP.
//!
//! `ForgeVisualProgram@1` does not introduce a second geometry truth. It binds
//! existing ShapeProgram, AssemblyGraph, Material Zone and surface-program
//! identities into one design-source envelope that an Agent may author through
//! typed tools. Rust validates and hashes the envelope before any geometry
//! executor sees it.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    canonical_json, normalize_persisted_shape_program, semantic_sha256, CoreError, CoreResult,
};

pub const FORGE_VISUAL_PROGRAM_SCHEMA_VERSION: &str = "ForgeVisualProgram@1";
pub const FORGE_VISUAL_PROGRAM_LOWERING_SCHEMA_VERSION: &str = "ForgeVisualProgramLowering@1";
pub const FORGE_VISUAL_PATCH_SCHEMA_VERSION: &str = "ForgeVisualPatch@1";
pub const FORGE_VISUAL_PROGRAM_REVISION_SCHEMA_VERSION: &str = "ForgeVisualProgramRevision@1";
pub const FORGE_VISUAL_PROGRAM_INSPECTION_SCHEMA_VERSION: &str = "ForgeVisualProgramInspection@1";
const MAX_VISUAL_PATCH_OPERATIONS: usize = 32;
pub const COMPILED_VISUAL_MATERIAL_IDS: &[&str] = &[
    "mat_primary",
    "mat_graphite",
    "mat_painted_steel",
    "mat_powder_coat",
    "mat_aluminum",
    "mat_signal_red",
    "mat_composite",
    "mat_abs_matte",
    "mat_carbon_composite",
    "mat_rubber",
    "mat_rubber_tire",
    "mat_dark_glass",
    "mat_clear_glass",
    "mat_emissive_blue",
    "mat_automotive_paint",
];

/// Return the exact A005 base-material identity that shares the same reviewed
/// PBR compiler slot as one authored ForgeVisualProgram material.
pub fn compiled_visual_base_material_id(material_id: &str) -> Option<&'static str> {
    match material_id {
        "mat_primary" | "mat_graphite" | "mat_painted_steel" | "mat_powder_coat" => {
            Some("mat_graphite")
        }
        "mat_aluminum" => Some("mat_aluminum"),
        "mat_signal_red" => Some("mat_signal_red"),
        "mat_composite" | "mat_abs_matte" | "mat_carbon_composite" => Some("mat_composite"),
        "mat_rubber" | "mat_rubber_tire" => Some("mat_rubber"),
        "mat_dark_glass" | "mat_clear_glass" => Some("mat_dark_glass"),
        "mat_emissive_blue" => Some("mat_emissive_blue"),
        "mat_automotive_paint" => Some("mat_automotive_paint"),
        _ => None,
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForgeVisualProgramStage {
    Draft,
    Sealed,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualDetailLevel {
    Macro,
    Meso,
    Micro,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualDetailStatus {
    Bound,
    Unresolved,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualDetailBindingKind {
    GeometryOutput,
    MaterialZone,
    SurfaceProgram,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForgeVisualExportProfile {
    InteractivePreview,
    ProductionConcept,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualDesignToken {
    pub token_id: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualPart {
    pub part_id: String,
    pub role: String,
    pub parent_part_id: Option<String>,
    pub geometry_output_ids: Vec<String>,
    pub material_zone_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualMaterialBinding {
    pub part_id: String,
    pub material_zone_id: String,
    pub material_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualSurfaceBinding {
    pub surface_program_id: String,
    pub part_id: String,
    pub material_zone_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualDetailBinding {
    pub kind: VisualDetailBindingKind,
    pub part_id: String,
    pub target_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualDetailInventoryItem {
    pub detail_id: String,
    pub level: VisualDetailLevel,
    pub description: String,
    pub critical: bool,
    pub status: VisualDetailStatus,
    pub bindings: Vec<VisualDetailBinding>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualProgram {
    pub schema_version: String,
    pub program_id: String,
    pub domain_pack_id: String,
    pub title: String,
    pub stage: ForgeVisualProgramStage,
    pub visual_only: bool,
    pub design_tokens: Vec<ForgeVisualDesignToken>,
    pub parts: Vec<ForgeVisualPart>,
    pub geometry_graph: Value,
    pub assembly_graph: Value,
    pub material_graph: Vec<ForgeVisualMaterialBinding>,
    pub surface_graph: Vec<ForgeVisualSurfaceBinding>,
    pub detail_inventory: Vec<VisualDetailInventoryItem>,
    pub export_profile: ForgeVisualExportProfile,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualProgramLowering {
    pub schema_version: String,
    pub source_program_sha256: String,
    pub shape_program: Value,
    pub assembly_graph: Value,
    pub material_bindings: Vec<ForgeVisualMaterialBinding>,
    pub surface_program_ids: Vec<String>,
    pub bound_detail_ids: Vec<String>,
}

/// One optimistic-concurrency revision of the Agent-authored design source.
/// This is an ephemeral candidate truth until PV005 promotes it through the
/// existing ChangeSet preview/confirm boundary.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualProgramRevision {
    pub schema_version: String,
    pub revision: u64,
    pub source_program_sha256: String,
    pub parent_source_program_sha256: Option<String>,
    pub program: ForgeVisualProgram,
    pub changed_domains: Vec<String>,
    pub applied_patch_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForgeVisualInspectionView {
    Summary,
    Full,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualProgramInspection {
    pub schema_version: String,
    pub revision: u64,
    pub source_program_sha256: String,
    pub parent_source_program_sha256: Option<String>,
    pub program_id: String,
    pub domain_pack_id: String,
    pub title: String,
    pub stage: ForgeVisualProgramStage,
    pub design_token_count: usize,
    pub part_count: usize,
    pub geometry_operation_count: usize,
    pub geometry_output_count: usize,
    pub material_binding_count: usize,
    pub surface_binding_count: usize,
    pub detail_count: usize,
    pub unresolved_critical_detail_ids: Vec<String>,
    pub changed_domains: Vec<String>,
    pub applied_patch_id: Option<String>,
    pub program: Option<ForgeVisualProgram>,
}

/// Typed source edits. The Agent may replace expressive subgraphs, but it may
/// not address arbitrary JSON paths or execute code. Rust validates the whole
/// resulting program transactionally before advancing the revision.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ForgeVisualPatchOperation {
    SetTitle {
        title: String,
    },
    UpsertDesignToken {
        token: ForgeVisualDesignToken,
    },
    RemoveDesignToken {
        token_id: String,
    },
    ReplaceParts {
        parts: Vec<ForgeVisualPart>,
    },
    ReplaceGeometryGraph {
        geometry_graph: Value,
    },
    /// Replaces one existing ShapeProgram operation in place. The stable
    /// operation_id is supplied twice so a patch cannot silently rename a
    /// graph node or append a new executable operation.
    UpsertGeometryOperation {
        operation_id: String,
        operation: Value,
    },
    ReplaceAssemblyGraph {
        assembly_graph: Value,
    },
    ReplaceMaterialGraph {
        material_graph: Vec<ForgeVisualMaterialBinding>,
    },
    /// Replaces one existing material binding identified by its immutable
    /// (part_id, material_zone_id) pair; it never creates a new binding.
    UpsertMaterialBinding {
        binding: ForgeVisualMaterialBinding,
    },
    ReplaceSurfaceGraph {
        surface_graph: Vec<ForgeVisualSurfaceBinding>,
    },
    /// Rebinds one existing surface program. The surface_program_id must
    /// already occur in the current Rust-owned draft.
    UpsertSurfaceBinding {
        binding: ForgeVisualSurfaceBinding,
    },
    ReplaceDetailInventory {
        detail_inventory: Vec<VisualDetailInventoryItem>,
    },
    /// Replaces one existing visual-detail row; detail_id is stable and may
    /// not be introduced by a patch.
    UpsertDetailInventoryItem {
        detail: VisualDetailInventoryItem,
    },
    SetExportProfile {
        export_profile: ForgeVisualExportProfile,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualPatch {
    pub schema_version: String,
    pub patch_id: String,
    pub expected_revision: u64,
    pub expected_source_sha256: String,
    pub preserve_geometry: bool,
    pub preserve_material_surface: bool,
    pub operations: Vec<ForgeVisualPatchOperation>,
}

fn invalid(message: impl Into<String>) -> CoreError {
    CoreError::invalid_data("FORGE_VISUAL_PROGRAM_INVALID", message)
}

fn require_id(field: &str, value: &str) -> CoreResult<()> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'));
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            "{field} must be a bounded stable identifier"
        )))
    }
}

fn require_text(field: &str, value: &str, max_chars: usize) -> CoreResult<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > max_chars {
        return Err(invalid(format!("{field} must be non-empty and bounded")));
    }
    Ok(())
}

fn object_schema<'a>(
    field: &str,
    value: &'a Value,
    expected: &str,
) -> CoreResult<&'a serde_json::Map<String, Value>> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{field} must be an object")))?;
    if object.get("schema_version").and_then(Value::as_str) != Some(expected) {
        return Err(invalid(format!(
            "{field}.schema_version must be {expected}"
        )));
    }
    Ok(object)
}

impl ForgeVisualProgram {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != FORGE_VISUAL_PROGRAM_SCHEMA_VERSION {
            return Err(invalid("schema_version must be ForgeVisualProgram@1"));
        }
        require_id("program_id", &self.program_id)?;
        require_id("domain_pack_id", &self.domain_pack_id)?;
        require_text("title", &self.title, 160)?;
        if !self.visual_only {
            return Err(invalid("visual_only must be true"));
        }
        if self.design_tokens.is_empty() || self.design_tokens.len() > 32 {
            return Err(invalid("design_tokens must contain 1 to 32 entries"));
        }
        if self.parts.is_empty() || self.parts.len() > 256 {
            return Err(invalid("parts must contain 1 to 256 entries"));
        }
        if self.detail_inventory.is_empty() || self.detail_inventory.len() > 512 {
            return Err(invalid("detail_inventory must contain 1 to 512 entries"));
        }

        let mut token_ids = BTreeSet::new();
        for token in &self.design_tokens {
            require_id("design_tokens.token_id", &token.token_id)?;
            require_text("design_tokens.value", &token.value, 120)?;
            if !token_ids.insert(token.token_id.as_str()) {
                return Err(invalid("design token identifiers must be unique"));
            }
        }

        let shape = object_schema("geometry_graph", &self.geometry_graph, "ShapeProgram@1")?;
        let _assembly = object_schema("assembly_graph", &self.assembly_graph, "AssemblyGraph@1")?;
        let operations = shape
            .get("operations")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("geometry_graph.operations must be an array"))?;
        let outputs = shape
            .get("outputs")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("geometry_graph.outputs must be an array"))?;
        if operations.is_empty() || outputs.is_empty() {
            return Err(invalid(
                "geometry_graph must contain operations and outputs",
            ));
        }
        let output_ids = outputs
            .iter()
            .map(|output| {
                let output_id = output
                    .get("output_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("every geometry output must have output_id"))?;
                require_id("geometry_graph.outputs.output_id", output_id)?;
                Ok(output_id)
            })
            .collect::<CoreResult<BTreeSet<_>>>()?;
        if output_ids.len() != outputs.len() {
            return Err(invalid("geometry output identifiers must be unique"));
        }

        let mut part_ids = BTreeSet::new();
        let mut zones_by_part: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        let mut owned_output_ids = BTreeSet::new();
        for part in &self.parts {
            require_id("parts.part_id", &part.part_id)?;
            require_id("parts.role", &part.role)?;
            if !part_ids.insert(part.part_id.as_str()) {
                return Err(invalid("part identifiers must be unique"));
            }
            if part.geometry_output_ids.is_empty() || part.material_zone_ids.is_empty() {
                return Err(invalid(
                    "every part must bind geometry outputs and material zones",
                ));
            }
            let mut part_outputs = BTreeSet::new();
            for output_id in &part.geometry_output_ids {
                require_id("parts.geometry_output_ids", output_id)?;
                if !output_ids.contains(output_id.as_str())
                    || !part_outputs.insert(output_id.as_str())
                    || !owned_output_ids.insert(output_id.as_str())
                {
                    return Err(invalid(
                        "every ShapeProgram output must be owned by exactly one part",
                    ));
                }
            }
            let zones = zones_by_part.entry(part.part_id.as_str()).or_default();
            for zone_id in &part.material_zone_ids {
                require_id("parts.material_zone_ids", zone_id)?;
                if !zones.insert(zone_id.as_str()) {
                    return Err(invalid("part material zone identifiers must be unique"));
                }
            }
        }
        if owned_output_ids != output_ids {
            return Err(invalid(
                "every ShapeProgram output must be owned by exactly one part",
            ));
        }
        for part in &self.parts {
            if let Some(parent) = &part.parent_part_id {
                if parent == &part.part_id || !part_ids.contains(parent.as_str()) {
                    return Err(invalid(
                        "parent_part_id must reference a different existing part",
                    ));
                }
            }
            let mut ancestors = BTreeSet::new();
            let mut cursor = part.parent_part_id.as_deref();
            while let Some(parent_id) = cursor {
                if !ancestors.insert(parent_id) {
                    return Err(invalid("part parent hierarchy must be acyclic"));
                }
                cursor = self
                    .parts
                    .iter()
                    .find(|candidate| candidate.part_id == parent_id)
                    .and_then(|candidate| candidate.parent_part_id.as_deref());
            }
        }

        let mut material_targets = BTreeSet::new();
        let mut zone_ids = BTreeSet::new();
        for binding in &self.material_graph {
            require_id("material_graph.part_id", &binding.part_id)?;
            require_id("material_graph.material_zone_id", &binding.material_zone_id)?;
            require_id("material_graph.material_id", &binding.material_id)?;
            if !COMPILED_VISUAL_MATERIAL_IDS.contains(&binding.material_id.as_str()) {
                return Err(invalid(
                    "material bindings must use the reviewed visual compiler catalog",
                ));
            }
            let target = (binding.part_id.as_str(), binding.material_zone_id.as_str());
            if !zones_by_part
                .get(target.0)
                .is_some_and(|zones| zones.contains(target.1))
                || !material_targets.insert(target)
            {
                return Err(invalid(
                    "material bindings must uniquely target an existing part zone",
                ));
            }
            zone_ids.insert(binding.material_zone_id.as_str());
        }
        if material_targets.len() != zones_by_part.values().map(BTreeSet::len).sum::<usize>() {
            return Err(invalid(
                "every declared part zone must have exactly one material binding",
            ));
        }

        let mut surface_ids = BTreeSet::new();
        let mut surface_targets = BTreeMap::new();
        for binding in &self.surface_graph {
            require_id("surface_graph.part_id", &binding.part_id)?;
            require_id("surface_graph.material_zone_id", &binding.material_zone_id)?;
            require_id(
                "surface_graph.surface_program_id",
                &binding.surface_program_id,
            )?;
            let target = (binding.part_id.as_str(), binding.material_zone_id.as_str());
            if !material_targets.contains(&target)
                || !surface_ids.insert(binding.surface_program_id.as_str())
            {
                return Err(invalid(
                    "surface programs must be unique and target a material-bound part zone",
                ));
            }
            surface_targets.insert(binding.surface_program_id.as_str(), target);
        }

        let output_owners = self
            .parts
            .iter()
            .flat_map(|part| {
                part.geometry_output_ids
                    .iter()
                    .map(move |output_id| (output_id.as_str(), part.part_id.as_str()))
            })
            .collect::<BTreeMap<_, _>>();

        let mut detail_ids = BTreeSet::new();
        let mut detail_levels = BTreeSet::new();
        for detail in &self.detail_inventory {
            require_id("detail_inventory.detail_id", &detail.detail_id)?;
            require_text("detail_inventory.description", &detail.description, 240)?;
            if !detail_ids.insert(detail.detail_id.as_str()) {
                return Err(invalid("detail identifiers must be unique"));
            }
            detail_levels.insert(match detail.level {
                VisualDetailLevel::Macro => "macro",
                VisualDetailLevel::Meso => "meso",
                VisualDetailLevel::Micro => "micro",
            });
            if detail.status == VisualDetailStatus::Bound && detail.bindings.is_empty() {
                return Err(invalid(
                    "bound detail must contain at least one real binding",
                ));
            }
            if self.stage == ForgeVisualProgramStage::Sealed
                && detail.status == VisualDetailStatus::Unresolved
                && detail.critical
            {
                return Err(invalid(
                    "sealed programs cannot contain unresolved critical details",
                ));
            }
            let mut unique_bindings = BTreeSet::new();
            for binding in &detail.bindings {
                require_id("detail_inventory.bindings.part_id", &binding.part_id)?;
                require_id("detail_inventory.bindings.target_id", &binding.target_id)?;
                if !part_ids.contains(binding.part_id.as_str())
                    || !unique_bindings.insert((
                        binding.part_id.as_str(),
                        format!("{:?}", binding.kind),
                        binding.target_id.as_str(),
                    ))
                {
                    return Err(invalid(
                        "detail bindings must be unique and reference an existing part",
                    ));
                }
                let exists = match binding.kind {
                    VisualDetailBindingKind::GeometryOutput => output_owners
                        .get(binding.target_id.as_str())
                        .is_some_and(|owner| *owner == binding.part_id),
                    VisualDetailBindingKind::MaterialZone => material_targets
                        .contains(&(binding.part_id.as_str(), binding.target_id.as_str())),
                    VisualDetailBindingKind::SurfaceProgram => surface_targets
                        .get(binding.target_id.as_str())
                        .is_some_and(|target| target.0 == binding.part_id),
                };
                if !exists {
                    return Err(invalid(
                        "detail binding must reference a real output owned by its declared part",
                    ));
                }
            }
        }
        if detail_levels.len() != 3 {
            return Err(invalid(
                "detail_inventory must cover macro, meso, and micro levels",
            ));
        }
        Ok(())
    }
}

impl ForgeVisualProgramRevision {
    /// Starts a new candidate workspace. A Provider cannot author a sealed
    /// program: sealing belongs to the later Rust-owned quality stage.
    pub fn author(value: &Value) -> CoreResult<Self> {
        let program: ForgeVisualProgram =
            serde_json::from_value(value.clone()).map_err(|error| {
                invalid(format!(
                    "ForgeVisualProgram@1 authoring failed closed: {error}"
                ))
            })?;
        if program.stage != ForgeVisualProgramStage::Draft {
            return Err(invalid(
                "Agent-authored ForgeVisualProgram must begin at draft stage",
            ));
        }
        program.validate()?;
        normalize_persisted_shape_program(&program.geometry_graph)?;
        let source_program_sha256 = semantic_sha256(&program)?;
        Ok(Self {
            schema_version: FORGE_VISUAL_PROGRAM_REVISION_SCHEMA_VERSION.into(),
            revision: 1,
            source_program_sha256,
            parent_source_program_sha256: None,
            program,
            changed_domains: vec![
                "design_tokens".into(),
                "parts".into(),
                "geometry".into(),
                "assembly".into(),
                "material".into(),
                "surface".into(),
                "detail_inventory".into(),
                "export_profile".into(),
            ],
            applied_patch_id: None,
        })
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != FORGE_VISUAL_PROGRAM_REVISION_SCHEMA_VERSION
            || self.revision == 0
            || !is_sha256(&self.source_program_sha256)
            || self.source_program_sha256 != semantic_sha256(&self.program)?
            || self.program.stage != ForgeVisualProgramStage::Draft
        {
            return Err(invalid(
                "ForgeVisualProgram revision identity is invalid or stale",
            ));
        }
        match (self.revision, self.parent_source_program_sha256.as_deref()) {
            (1, None) => {}
            (revision, Some(parent))
                if revision > 1 && is_sha256(parent) && parent != self.source_program_sha256 => {}
            _ => {
                return Err(invalid(
                    "ForgeVisualProgram revision parent identity is invalid",
                ))
            }
        }
        self.program.validate()?;
        normalize_persisted_shape_program(&self.program.geometry_graph)?;
        if let Some(patch_id) = self.applied_patch_id.as_deref() {
            require_id("applied_patch_id", patch_id)?;
        }
        if self.changed_domains.is_empty() || self.changed_domains.len() > 16 {
            return Err(invalid(
                "changed_domains must contain one bounded source-domain set",
            ));
        }
        let allowed = [
            "title",
            "design_tokens",
            "parts",
            "geometry",
            "assembly",
            "material",
            "surface",
            "detail_inventory",
            "export_profile",
        ];
        let mut unique = BTreeSet::new();
        if self
            .changed_domains
            .iter()
            .any(|domain| !allowed.contains(&domain.as_str()) || !unique.insert(domain.as_str()))
        {
            return Err(invalid(
                "changed_domains contains an unknown or duplicate domain",
            ));
        }
        Ok(())
    }

    pub fn inspect(
        &self,
        view: ForgeVisualInspectionView,
    ) -> CoreResult<ForgeVisualProgramInspection> {
        self.validate()?;
        let geometry_operation_count = self
            .program
            .geometry_graph
            .get("operations")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let geometry_output_count = self
            .program
            .geometry_graph
            .get("outputs")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let unresolved_critical_detail_ids = self
            .program
            .detail_inventory
            .iter()
            .filter(|detail| detail.critical && detail.status == VisualDetailStatus::Unresolved)
            .map(|detail| detail.detail_id.clone())
            .collect();
        Ok(ForgeVisualProgramInspection {
            schema_version: FORGE_VISUAL_PROGRAM_INSPECTION_SCHEMA_VERSION.into(),
            revision: self.revision,
            source_program_sha256: self.source_program_sha256.clone(),
            parent_source_program_sha256: self.parent_source_program_sha256.clone(),
            program_id: self.program.program_id.clone(),
            domain_pack_id: self.program.domain_pack_id.clone(),
            title: self.program.title.clone(),
            stage: self.program.stage.clone(),
            design_token_count: self.program.design_tokens.len(),
            part_count: self.program.parts.len(),
            geometry_operation_count,
            geometry_output_count,
            material_binding_count: self.program.material_graph.len(),
            surface_binding_count: self.program.surface_graph.len(),
            detail_count: self.program.detail_inventory.len(),
            unresolved_critical_detail_ids,
            changed_domains: self.changed_domains.clone(),
            applied_patch_id: self.applied_patch_id.clone(),
            program: (view == ForgeVisualInspectionView::Full).then(|| self.program.clone()),
        })
    }

    /// Applies one optimistic-concurrency patch to a clone and returns the
    /// next revision only after complete ForgeVisualProgram validation.
    pub fn apply_patch(&self, value: &Value) -> CoreResult<Self> {
        self.validate()?;
        let patch: ForgeVisualPatch = serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("ForgeVisualPatch@1 failed closed: {error}")))?;
        patch.validate()?;
        if patch.expected_revision != self.revision
            || patch.expected_source_sha256 != self.source_program_sha256
        {
            return Err(invalid(
                "ForgeVisualPatch expected_revision or source hash is stale",
            ));
        }

        let before_geometry = semantic_sha256(&json_value(&(
            &self.program.parts,
            &self.program.geometry_graph,
            &self.program.assembly_graph,
        ))?)?;
        let before_material_surface = semantic_sha256(&json_value(&(
            &self.program.material_graph,
            &self.program.surface_graph,
        ))?)?;
        let mut program = self.program.clone();
        let mut changed_domains = BTreeSet::new();
        for operation in patch.operations {
            match operation {
                ForgeVisualPatchOperation::SetTitle { title } => {
                    program.title = title;
                    changed_domains.insert("title".to_string());
                }
                ForgeVisualPatchOperation::UpsertDesignToken { token } => {
                    if let Some(existing) = program
                        .design_tokens
                        .iter_mut()
                        .find(|existing| existing.token_id == token.token_id)
                    {
                        *existing = token;
                    } else {
                        program.design_tokens.push(token);
                    }
                    changed_domains.insert("design_tokens".to_string());
                }
                ForgeVisualPatchOperation::RemoveDesignToken { token_id } => {
                    require_id("remove_design_token.token_id", &token_id)?;
                    let before = program.design_tokens.len();
                    program
                        .design_tokens
                        .retain(|token| token.token_id != token_id);
                    if before == program.design_tokens.len() {
                        return Err(invalid("remove_design_token target does not exist"));
                    }
                    changed_domains.insert("design_tokens".to_string());
                }
                ForgeVisualPatchOperation::ReplaceParts { parts } => {
                    program.parts = parts;
                    changed_domains.insert("parts".to_string());
                }
                ForgeVisualPatchOperation::ReplaceGeometryGraph { geometry_graph } => {
                    program.geometry_graph = geometry_graph;
                    changed_domains.insert("geometry".to_string());
                }
                ForgeVisualPatchOperation::UpsertGeometryOperation {
                    operation_id,
                    operation,
                } => {
                    replace_existing_geometry_operation(
                        &mut program.geometry_graph,
                        &operation_id,
                        operation,
                    )?;
                    changed_domains.insert("geometry".to_string());
                }
                ForgeVisualPatchOperation::ReplaceAssemblyGraph { assembly_graph } => {
                    program.assembly_graph = assembly_graph;
                    changed_domains.insert("assembly".to_string());
                }
                ForgeVisualPatchOperation::ReplaceMaterialGraph { material_graph } => {
                    program.material_graph = material_graph;
                    changed_domains.insert("material".to_string());
                }
                ForgeVisualPatchOperation::UpsertMaterialBinding { binding } => {
                    let existing = program
                        .material_graph
                        .iter_mut()
                        .find(|existing| {
                            existing.part_id == binding.part_id
                                && existing.material_zone_id == binding.material_zone_id
                        })
                        .ok_or_else(|| invalid("upsert_material_binding target does not exist"))?;
                    *existing = binding;
                    changed_domains.insert("material".to_string());
                }
                ForgeVisualPatchOperation::ReplaceSurfaceGraph { surface_graph } => {
                    program.surface_graph = surface_graph;
                    changed_domains.insert("surface".to_string());
                }
                ForgeVisualPatchOperation::UpsertSurfaceBinding { binding } => {
                    let existing = program
                        .surface_graph
                        .iter_mut()
                        .find(|existing| existing.surface_program_id == binding.surface_program_id)
                        .ok_or_else(|| invalid("upsert_surface_binding target does not exist"))?;
                    *existing = binding;
                    changed_domains.insert("surface".to_string());
                }
                ForgeVisualPatchOperation::ReplaceDetailInventory { detail_inventory } => {
                    program.detail_inventory = detail_inventory;
                    changed_domains.insert("detail_inventory".to_string());
                }
                ForgeVisualPatchOperation::UpsertDetailInventoryItem { detail } => {
                    let existing = program
                        .detail_inventory
                        .iter_mut()
                        .find(|existing| existing.detail_id == detail.detail_id)
                        .ok_or_else(|| {
                            invalid("upsert_detail_inventory_item target does not exist")
                        })?;
                    *existing = detail;
                    changed_domains.insert("detail_inventory".to_string());
                }
                ForgeVisualPatchOperation::SetExportProfile { export_profile } => {
                    program.export_profile = export_profile;
                    changed_domains.insert("export_profile".to_string());
                }
            }
        }
        program.validate()?;
        normalize_persisted_shape_program(&program.geometry_graph)?;

        let after_geometry = semantic_sha256(&json_value(&(
            &program.parts,
            &program.geometry_graph,
            &program.assembly_graph,
        ))?)?;
        let after_material_surface = semantic_sha256(&json_value(&(
            &program.material_graph,
            &program.surface_graph,
        ))?)?;
        if patch.preserve_geometry && before_geometry != after_geometry {
            return Err(invalid(
                "preserve_geometry patch changed parts, geometry, or assembly",
            ));
        }
        if patch.preserve_material_surface && before_material_surface != after_material_surface {
            return Err(invalid(
                "preserve_material_surface patch changed material or surface",
            ));
        }
        let changed_domains = [
            ("title", program.title != self.program.title),
            (
                "design_tokens",
                program.design_tokens != self.program.design_tokens,
            ),
            ("parts", program.parts != self.program.parts),
            (
                "geometry",
                program.geometry_graph != self.program.geometry_graph,
            ),
            (
                "assembly",
                program.assembly_graph != self.program.assembly_graph,
            ),
            (
                "material",
                program.material_graph != self.program.material_graph,
            ),
            (
                "surface",
                program.surface_graph != self.program.surface_graph,
            ),
            (
                "detail_inventory",
                program.detail_inventory != self.program.detail_inventory,
            ),
            (
                "export_profile",
                program.export_profile != self.program.export_profile,
            ),
        ]
        .into_iter()
        .filter_map(|(domain, changed)| {
            (changed && changed_domains.contains(domain)).then(|| domain.to_string())
        })
        .collect::<Vec<_>>();
        let source_program_sha256 = semantic_sha256(&program)?;
        if source_program_sha256 == self.source_program_sha256 || changed_domains.is_empty() {
            return Err(invalid("ForgeVisualPatch made no semantic change"));
        }
        let next = Self {
            schema_version: FORGE_VISUAL_PROGRAM_REVISION_SCHEMA_VERSION.into(),
            revision: self.revision.saturating_add(1),
            source_program_sha256,
            parent_source_program_sha256: Some(self.source_program_sha256.clone()),
            program,
            changed_domains,
            applied_patch_id: Some(patch.patch_id),
        };
        next.validate()?;
        Ok(next)
    }
}

fn replace_existing_geometry_operation(
    geometry_graph: &mut Value,
    operation_id: &str,
    operation: Value,
) -> CoreResult<()> {
    require_id("upsert_geometry_operation.operation_id", operation_id)?;
    let replacement_id = operation
        .get("operation_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("upsert_geometry_operation.operation.operation_id is required"))?;
    if replacement_id != operation_id {
        return Err(invalid(
            "upsert_geometry_operation cannot change an existing operation_id",
        ));
    }
    let operations = geometry_graph
        .get_mut("operations")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("upsert_geometry_operation requires ShapeProgram operations"))?;
    let existing = operations
        .iter_mut()
        .find(|existing| existing.get("operation_id").and_then(Value::as_str) == Some(operation_id))
        .ok_or_else(|| invalid("upsert_geometry_operation target does not exist"))?;
    *existing = operation;
    Ok(())
}

impl ForgeVisualPatch {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != FORGE_VISUAL_PATCH_SCHEMA_VERSION {
            return Err(invalid("patch schema_version must be ForgeVisualPatch@1"));
        }
        require_id("patch_id", &self.patch_id)?;
        if self.expected_revision == 0 || !is_sha256(&self.expected_source_sha256) {
            return Err(invalid(
                "patch expected revision and source hash must be valid",
            ));
        }
        if self.operations.is_empty() || self.operations.len() > MAX_VISUAL_PATCH_OPERATIONS {
            return Err(invalid("patch must contain 1 to 32 typed operations"));
        }
        Ok(())
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn json_value<T: Serialize>(value: &T) -> CoreResult<Value> {
    serde_json::to_value(value)
        .map_err(|_| invalid("ForgeVisualProgram source domain could not be serialized"))
}

pub fn lower_forge_visual_program(value: &Value) -> CoreResult<ForgeVisualProgramLowering> {
    let program: ForgeVisualProgram = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("ForgeVisualProgram@1 failed closed: {error}")))?;
    program.validate()?;
    let mut materialized_shape = normalize_persisted_shape_program(&program.geometry_graph)?;
    let output_owners = program
        .parts
        .iter()
        .flat_map(|part| {
            part.geometry_output_ids
                .iter()
                .map(move |output_id| (output_id.as_str(), part.part_id.as_str()))
        })
        .collect::<BTreeMap<_, _>>();
    let materials = program
        .material_graph
        .iter()
        .map(|binding| {
            (
                (binding.part_id.as_str(), binding.material_zone_id.as_str()),
                binding.material_id.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let output_facts = materialized_shape
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("lowered ShapeProgram outputs are missing"))?
        .iter()
        .map(|output| {
            let output_id = output
                .get("output_id")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("lowered ShapeProgram output_id is missing"))?;
            let operation_id = output
                .get("operation_id")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("lowered ShapeProgram operation_id is missing"))?;
            let part_id = output_owners
                .get(output_id)
                .copied()
                .ok_or_else(|| invalid("lowered ShapeProgram output has no owning Part"))?;
            Ok((operation_id.to_string(), part_id.to_string()))
        })
        .collect::<CoreResult<Vec<_>>>()?;
    let operations = materialized_shape
        .get_mut("operations")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("lowered ShapeProgram operations are missing"))?;
    for (operation_id, part_id) in output_facts {
        let operation = operations
            .iter_mut()
            .find(|operation| {
                operation.get("operation_id").and_then(Value::as_str) == Some(operation_id.as_str())
            })
            .ok_or_else(|| invalid("lowered ShapeProgram output operation is missing"))?;
        let Some(zone_id) = operation
            .pointer("/args/zone_id")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        let material_id = materials
            .get(&(part_id.as_str(), zone_id.as_str()))
            .copied()
            .ok_or_else(|| {
                invalid("ShapeProgram output zone has no matching Part material binding")
            })?;
        operation
            .get_mut("args")
            .and_then(Value::as_object_mut)
            .expect("normalized ShapeProgram args are objects")
            .insert("material_id".into(), Value::String(material_id.to_string()));
    }
    let normalized_shape = normalize_persisted_shape_program(&materialized_shape)?;
    let normalized_assembly: Value =
        serde_json::from_str(&canonical_json(&program.assembly_graph)?)
            .map_err(|_| invalid("assembly_graph could not be canonicalized"))?;
    let source_program_sha256 = semantic_sha256(&program)?;
    let mut surface_program_ids = program
        .surface_graph
        .iter()
        .map(|binding| binding.surface_program_id.clone())
        .collect::<Vec<_>>();
    surface_program_ids.sort();
    let mut bound_detail_ids = program
        .detail_inventory
        .iter()
        .filter(|detail| detail.status == VisualDetailStatus::Bound)
        .map(|detail| detail.detail_id.clone())
        .collect::<Vec<_>>();
    bound_detail_ids.sort();
    Ok(ForgeVisualProgramLowering {
        schema_version: FORGE_VISUAL_PROGRAM_LOWERING_SCHEMA_VERSION.into(),
        source_program_sha256,
        shape_program: normalized_shape,
        assembly_graph: normalized_assembly,
        material_bindings: program.material_graph,
        surface_program_ids,
        bound_detail_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn program(stage: &str, detail_status: &str, bindings: Value) -> Value {
        json!({
            "schema_version": "ForgeVisualProgram@1",
            "program_id": "visual_program_arm_1",
            "domain_pack_id": "pack_robotic_arm_concept",
            "title": "未来工业机械臂收藏品",
            "stage": stage,
            "visual_only": true,
            "design_tokens": [{"token_id":"surface_language","value":"graphite blue industrial"}],
            "parts": [{
                "part_id":"part_base", "role":"base", "parent_part_id":null,
                "geometry_output_ids":["output_base"], "material_zone_ids":["zone_base"]
            }],
            "geometry_graph": {
                "schema_version":"ShapeProgram@1", "program_id":"shape_arm_1",
                "operations":[{"operation_id":"op_base","op":"box","inputs":[],"args":{
                    "size":[180.0,56.0,34.0], "position":[0.0,0.0,0.0],
                    "part_role":"base", "zone_id":"zone_base", "material_id":"mat_graphite"
                }}],
                "outputs":[{"output_id":"output_base","operation_id":"op_base","kind":"mesh","part_role":"base"}]
            },
            "assembly_graph": {"schema_version":"AssemblyGraph@1","parts":[],"connections":[]},
            "material_graph": [{"part_id":"part_base","material_zone_id":"zone_base","material_id":"mat_graphite"}],
            "surface_graph": [{"surface_program_id":"surface_base_flow","part_id":"part_base","material_zone_id":"zone_base"}],
            "detail_inventory": [
                {
                    "detail_id":"detail_silhouette", "level":"macro", "description":"紧凑工业底座轮廓",
                    "critical":true, "status":"bound", "bindings":[{"kind":"geometry_output","part_id":"part_base","target_id":"output_base"}]
                },
                {
                    "detail_id":"detail_base_flow", "level":"meso", "description":"底座蓝色发光流线",
                    "critical":true, "status":detail_status, "bindings":bindings
                },
                {
                    "detail_id":"detail_coating", "level":"micro", "description":"石墨涂层粗糙度层次",
                    "critical":true, "status":"bound", "bindings":[{"kind":"material_zone","part_id":"part_base","target_id":"zone_base"}]
                }
            ],
            "export_profile":"production_concept"
        })
    }

    #[test]
    fn pv003_rejects_materials_the_restricted_pbr_compiler_cannot_build() {
        let mut unsupported = program(
            "draft",
            "bound",
            json!([{"kind":"surface_program","part_id":"part_base","target_id":"surface_base_flow"}]),
        );
        unsupported["material_graph"][0]["material_id"] = json!("mat_unreviewed_copper");
        let error = ForgeVisualProgramRevision::author(&unsupported).unwrap_err();
        assert_eq!(error.code(), "FORGE_VISUAL_PROGRAM_INVALID");
        assert!(error
            .to_string()
            .contains("reviewed visual compiler catalog"));
    }

    #[test]
    fn sealed_program_lowers_existing_truth_and_binds_detail() {
        let value = program(
            "sealed",
            "bound",
            json!([{"kind":"surface_program","part_id":"part_base","target_id":"surface_base_flow"}]),
        );
        let lowered = lower_forge_visual_program(&value).unwrap();
        assert_eq!(
            lowered.schema_version,
            FORGE_VISUAL_PROGRAM_LOWERING_SCHEMA_VERSION
        );
        assert_eq!(lowered.shape_program["schema_version"], "ShapeProgram@1");
        assert_eq!(lowered.surface_program_ids, vec!["surface_base_flow"]);
        assert_eq!(
            lowered.bound_detail_ids,
            vec!["detail_base_flow", "detail_coating", "detail_silhouette"]
        );
        assert_eq!(lowered.source_program_sha256.len(), 64);
    }

    #[test]
    fn sealed_program_rejects_unresolved_critical_detail() {
        let error =
            lower_forge_visual_program(&program("sealed", "unresolved", json!([]))).unwrap_err();
        assert_eq!(error.code(), "FORGE_VISUAL_PROGRAM_INVALID");
    }

    #[test]
    fn bound_detail_rejects_claim_without_real_output() {
        let error = lower_forge_visual_program(&program(
            "draft",
            "bound",
            json!([{"kind":"geometry_output","part_id":"part_base","target_id":"output_that_does_not_exist"}]),
        ))
        .unwrap_err();
        assert_eq!(error.code(), "FORGE_VISUAL_PROGRAM_INVALID");
    }

    #[test]
    fn unknown_fields_are_not_an_arbitrary_code_escape() {
        let mut value = program("draft", "unresolved", json!([]));
        value["javascript"] = json!("fetch('https://example.com')");
        let error = lower_forge_visual_program(&value).unwrap_err();
        assert_eq!(error.code(), "FORGE_VISUAL_PROGRAM_INVALID");
    }

    #[test]
    fn orphan_geometry_output_is_rejected() {
        let mut value = program(
            "sealed",
            "bound",
            json!([{"kind":"surface_program","part_id":"part_base","target_id":"surface_base_flow"}]),
        );
        value["geometry_graph"]["outputs"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "output_id":"output_orphan", "operation_id":"op_base", "kind":"mesh", "part_role":"trim"
            }));
        let error = lower_forge_visual_program(&value).unwrap_err();
        assert_eq!(error.code(), "FORGE_VISUAL_PROGRAM_INVALID");
    }

    #[test]
    fn cyclic_part_hierarchy_is_rejected() {
        let mut value = program(
            "sealed",
            "bound",
            json!([{"kind":"surface_program","part_id":"part_base","target_id":"surface_base_flow"}]),
        );
        value["parts"][0]["parent_part_id"] = json!("part_trim");
        value["parts"].as_array_mut().unwrap().push(json!({
            "part_id":"part_trim", "role":"trim", "parent_part_id":"part_base",
            "geometry_output_ids":["output_trim"], "material_zone_ids":["zone_trim"]
        }));
        value["geometry_graph"]["outputs"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "output_id":"output_trim", "operation_id":"op_base", "kind":"mesh", "part_role":"trim"
            }));
        value["material_graph"].as_array_mut().unwrap().push(json!({
            "part_id":"part_trim", "material_zone_id":"zone_trim", "material_id":"mat_trim"
        }));
        let error = lower_forge_visual_program(&value).unwrap_err();
        assert_eq!(error.code(), "FORGE_VISUAL_PROGRAM_INVALID");
    }

    #[test]
    fn pv003_author_and_inspect_keep_one_rust_owned_revision() {
        let revision =
            ForgeVisualProgramRevision::author(&program("draft", "unresolved", json!([]))).unwrap();
        assert_eq!(revision.revision, 1);
        assert_eq!(revision.source_program_sha256.len(), 64);
        assert_eq!(revision.program.stage, ForgeVisualProgramStage::Draft);

        let summary = revision
            .inspect(ForgeVisualInspectionView::Summary)
            .unwrap();
        assert!(summary.program.is_none());
        assert_eq!(summary.part_count, 1);
        assert_eq!(summary.geometry_operation_count, 1);
        assert_eq!(
            summary.unresolved_critical_detail_ids,
            vec!["detail_base_flow"]
        );

        let full = revision.inspect(ForgeVisualInspectionView::Full).unwrap();
        assert_eq!(full.program.unwrap(), revision.program);
    }

    #[test]
    fn pv003_patch_is_hash_bound_and_reports_semantic_domains() {
        let revision =
            ForgeVisualProgramRevision::author(&program("draft", "unresolved", json!([]))).unwrap();
        let patch = json!({
            "schema_version":"ForgeVisualPatch@1",
            "patch_id":"patch_automotive_paint_surface",
            "expected_revision":revision.revision,
            "expected_source_sha256":revision.source_program_sha256,
            "preserve_geometry":true,
            "preserve_material_surface":false,
            "operations":[
                {"op":"set_title","title":"蓝色汽车漆工业机械臂收藏品"},
                {"op":"upsert_design_token","token":{
                    "token_id":"surface_language","value":"blue automotive paint industrial"
                }},
                {"op":"replace_material_graph","material_graph":[{
                    "part_id":"part_base","material_zone_id":"zone_base","material_id":"mat_automotive_paint"
                }]}
            ]
        });
        let next = revision.apply_patch(&patch).unwrap();
        assert_eq!(next.revision, 2);
        assert_ne!(next.source_program_sha256, revision.source_program_sha256);
        assert_eq!(
            next.changed_domains,
            vec!["title", "design_tokens", "material"]
        );
        assert_eq!(
            next.parent_source_program_sha256.as_deref(),
            Some(revision.source_program_sha256.as_str())
        );
        assert_eq!(next.program.geometry_graph, revision.program.geometry_graph);
        let before_lowering =
            lower_forge_visual_program(&serde_json::to_value(&revision.program).unwrap()).unwrap();
        let after_lowering =
            lower_forge_visual_program(&serde_json::to_value(&next.program).unwrap()).unwrap();
        assert_eq!(
            before_lowering.shape_program["operations"][0]["args"]["material_id"],
            "mat_graphite"
        );
        assert_eq!(
            after_lowering.shape_program["operations"][0]["args"]["material_id"],
            "mat_automotive_paint"
        );
        assert_ne!(
            semantic_sha256(&before_lowering.shape_program).unwrap(),
            semantic_sha256(&after_lowering.shape_program).unwrap()
        );
        assert_eq!(
            next.applied_patch_id.as_deref(),
            Some("patch_automotive_paint_surface")
        );
    }

    #[test]
    fn local_patch_upserts_replace_only_existing_program_rows() {
        let mut source = program(
            "draft",
            "bound",
            json!([{"kind":"surface_program","part_id":"part_base","target_id":"surface_base_flow"}]),
        );
        source["parts"].as_array_mut().unwrap().push(json!({
            "part_id":"part_trim", "role":"trim", "parent_part_id":"part_base",
            "geometry_output_ids":["output_trim"], "material_zone_ids":["zone_trim"]
        }));
        source["geometry_graph"]["operations"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "operation_id":"op_trim", "op":"box", "inputs":[], "args":{
                    "size":[60.0,20.0,20.0], "position":[0.0,40.0,0.0],
                    "part_role":"trim", "zone_id":"zone_trim", "material_id":"mat_graphite"
                }
            }));
        source["geometry_graph"]["outputs"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "output_id":"output_trim", "operation_id":"op_trim", "kind":"mesh", "part_role":"trim"
            }));
        source["material_graph"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "part_id":"part_trim", "material_zone_id":"zone_trim", "material_id":"mat_graphite"
            }));
        let revision = ForgeVisualProgramRevision::author(&source).unwrap();
        let mut replacement_operation = revision.program.geometry_graph["operations"][0].clone();
        replacement_operation["args"]["size"][0] = json!(220.0);
        let mut replacement_detail = serde_json::to_value(
            revision
                .program
                .detail_inventory
                .iter()
                .find(|detail| detail.detail_id == "detail_base_flow")
                .unwrap(),
        )
        .unwrap();
        replacement_detail["description"] = json!("修订后的底座蓝色发光流线");
        replacement_detail["bindings"] = json!([
            {"kind":"surface_program","part_id":"part_trim","target_id":"surface_base_flow"}
        ]);
        let patch = json!({
            "schema_version":"ForgeVisualPatch@1",
            "patch_id":"patch_local_visual_rows",
            "expected_revision":revision.revision,
            "expected_source_sha256":revision.source_program_sha256,
            "preserve_geometry":false,
            "preserve_material_surface":false,
            "operations":[
                {"op":"upsert_geometry_operation", "operation_id":"op_base", "operation":replacement_operation},
                {"op":"upsert_material_binding", "binding":{
                    "part_id":"part_base", "material_zone_id":"zone_base", "material_id":"mat_automotive_paint"
                }},
                {"op":"upsert_surface_binding", "binding":{
                    "surface_program_id":"surface_base_flow", "part_id":"part_trim", "material_zone_id":"zone_trim"
                }},
                {"op":"upsert_detail_inventory_item", "detail":replacement_detail}
            ]
        });

        let next = revision.apply_patch(&patch).unwrap();
        assert_eq!(
            next.program.geometry_graph["operations"][0]["args"]["size"][0],
            220.0
        );
        assert_eq!(
            next.program.material_graph[0].material_id,
            "mat_automotive_paint"
        );
        assert_eq!(next.program.surface_graph[0].part_id, "part_trim");
        assert_eq!(
            next.program.detail_inventory[1].description,
            "修订后的底座蓝色发光流线"
        );
        assert_eq!(
            next.changed_domains,
            vec!["geometry", "material", "surface", "detail_inventory"]
        );
    }

    #[test]
    fn local_patch_upserts_reject_unknown_targets() {
        let revision = ForgeVisualProgramRevision::author(&program(
            "draft",
            "bound",
            json!([{"kind":"surface_program","part_id":"part_base","target_id":"surface_base_flow"}]),
        ))
        .unwrap();
        let operation = revision.program.geometry_graph["operations"][0].clone();
        let unknown_operations = [
            json!({"op":"upsert_geometry_operation", "operation_id":"op_unknown", "operation":{
                "operation_id":"op_unknown", "op":"box", "inputs":[], "args":{
                    "size":[180.0,56.0,34.0], "position":[0.0,0.0,0.0],
                    "part_role":"base", "zone_id":"zone_base", "material_id":"mat_graphite"
                }
            }}),
            json!({"op":"upsert_material_binding", "binding":{
                "part_id":"part_unknown", "material_zone_id":"zone_base", "material_id":"mat_graphite"
            }}),
            json!({"op":"upsert_surface_binding", "binding":{
                "surface_program_id":"surface_unknown", "part_id":"part_base", "material_zone_id":"zone_base"
            }}),
            json!({"op":"upsert_detail_inventory_item", "detail":{
                "detail_id":"detail_unknown", "level":"micro", "description":"未知", "critical":false,
                "status":"bound", "bindings":[{"kind":"geometry_output","part_id":"part_base","target_id":"output_base"}]
            }}),
        ];
        for (index, operation_patch) in unknown_operations.into_iter().enumerate() {
            let patch = json!({
                "schema_version":"ForgeVisualPatch@1",
                "patch_id":format!("patch_unknown_target_{index}"),
                "expected_revision":revision.revision,
                "expected_source_sha256":revision.source_program_sha256,
                "preserve_geometry":false,
                "preserve_material_surface":false,
                "operations":[operation_patch]
            });
            let error = revision.apply_patch(&patch).unwrap_err();
            assert_eq!(error.code(), "FORGE_VISUAL_PROGRAM_INVALID");
        }
        assert_eq!(operation["operation_id"], "op_base");
    }

    #[test]
    fn local_geometry_upsert_rejects_operation_id_drift() {
        let revision = ForgeVisualProgramRevision::author(&program(
            "draft",
            "bound",
            json!([{"kind":"surface_program","part_id":"part_base","target_id":"surface_base_flow"}]),
        ))
        .unwrap();
        let mut replacement_operation = revision.program.geometry_graph["operations"][0].clone();
        replacement_operation["operation_id"] = json!("op_renamed");
        let patch = json!({
            "schema_version":"ForgeVisualPatch@1",
            "patch_id":"patch_operation_id_drift",
            "expected_revision":revision.revision,
            "expected_source_sha256":revision.source_program_sha256,
            "preserve_geometry":false,
            "preserve_material_surface":false,
            "operations":[{
                "op":"upsert_geometry_operation", "operation_id":"op_base", "operation":replacement_operation
            }]
        });
        let error = revision.apply_patch(&patch).unwrap_err();
        assert_eq!(error.code(), "FORGE_VISUAL_PROGRAM_INVALID");
        assert!(error
            .to_string()
            .contains("cannot change an existing operation_id"));
    }

    #[test]
    fn preserve_locks_reject_local_geometry_and_material_upserts() {
        let revision = ForgeVisualProgramRevision::author(&program(
            "draft",
            "bound",
            json!([{"kind":"surface_program","part_id":"part_base","target_id":"surface_base_flow"}]),
        ))
        .unwrap();
        let mut replacement_operation = revision.program.geometry_graph["operations"][0].clone();
        replacement_operation["args"]["size"][0] = json!(220.0);
        for (patch_id, preserve_geometry, preserve_material_surface, operation) in [
            (
                "patch_locked_local_geometry",
                true,
                false,
                json!({"op":"upsert_geometry_operation", "operation_id":"op_base", "operation":replacement_operation}),
            ),
            (
                "patch_locked_local_material",
                false,
                true,
                json!({"op":"upsert_material_binding", "binding":{
                    "part_id":"part_base", "material_zone_id":"zone_base", "material_id":"mat_automotive_paint"
                }}),
            ),
        ] {
            let patch = json!({
                "schema_version":"ForgeVisualPatch@1",
                "patch_id":patch_id,
                "expected_revision":revision.revision,
                "expected_source_sha256":revision.source_program_sha256,
                "preserve_geometry":preserve_geometry,
                "preserve_material_surface":preserve_material_surface,
                "operations":[operation]
            });
            assert_eq!(
                revision.apply_patch(&patch).unwrap_err().code(),
                "FORGE_VISUAL_PROGRAM_INVALID"
            );
        }
    }

    #[test]
    fn pv003_stale_or_lock_breaking_patch_has_zero_revision_side_effects() {
        let revision =
            ForgeVisualProgramRevision::author(&program("draft", "unresolved", json!([]))).unwrap();
        let stale = json!({
            "schema_version":"ForgeVisualPatch@1",
            "patch_id":"patch_stale",
            "expected_revision":99,
            "expected_source_sha256":revision.source_program_sha256,
            "preserve_geometry":false,
            "preserve_material_surface":false,
            "operations":[{"op":"set_title","title":"不应生效"}]
        });
        assert_eq!(
            revision.apply_patch(&stale).unwrap_err().code(),
            "FORGE_VISUAL_PROGRAM_INVALID"
        );
        assert_eq!(revision.revision, 1);

        let mut changed_geometry = revision.program.geometry_graph.clone();
        changed_geometry["program_id"] = json!("shape_arm_changed");
        let locked = json!({
            "schema_version":"ForgeVisualPatch@1",
            "patch_id":"patch_break_geometry_lock",
            "expected_revision":revision.revision,
            "expected_source_sha256":revision.source_program_sha256,
            "preserve_geometry":true,
            "preserve_material_surface":false,
            "operations":[{"op":"replace_geometry_graph","geometry_graph":changed_geometry}]
        });
        assert_eq!(
            revision.apply_patch(&locked).unwrap_err().code(),
            "FORGE_VISUAL_PROGRAM_INVALID"
        );
        assert_eq!(revision.revision, 1);
    }
}
