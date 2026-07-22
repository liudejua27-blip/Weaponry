use std::{collections::BTreeMap, fmt, str::FromStr};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{semantic_sha256, CoreError, CoreResult};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Active,
    Archived,
    SoftDeleted,
}

impl ProjectStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::SoftDeleted => "soft_deleted",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub project_id: String,
    pub profile_id: String,
    pub domain_type: String,
    pub name: String,
    pub status: ProjectStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_version_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Project {
    pub fn validate(&self) -> CoreResult<()> {
        require_id("project_id", &self.project_id)?;
        require_id("profile_id", &self.profile_id)?;
        require_text("domain_type", &self.domain_type, 128)?;
        require_text("name", &self.name, 256)?;
        require_text("created_at", &self.created_at, 128)?;
        require_text("updated_at", &self.updated_at, 128)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetVersionStatus {
    Committed,
    Superseded,
    SoftDeleted,
}

impl AssetVersionStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Committed => "committed",
            Self::Superseded => "superseded",
            Self::SoftDeleted => "soft_deleted",
        }
    }
}

impl FromStr for AssetVersionStatus {
    type Err = CoreError;

    fn from_str(value: &str) -> CoreResult<Self> {
        match value {
            "committed" => Ok(Self::Committed),
            "superseded" => Ok(Self::Superseded),
            "soft_deleted" => Ok(Self::SoftDeleted),
            _ => Err(CoreError::invalid_data(
                "ASSET_VERSION_STATUS_INVALID",
                "Agent asset version has an unsupported status.",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetStage {
    SegmentedConcept,
    EditableAsset,
}

impl AssetStage {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SegmentedConcept => "segmented_concept",
            Self::EditableAsset => "editable_asset",
        }
    }
}

impl FromStr for AssetStage {
    type Err = CoreError;

    fn from_str(value: &str) -> CoreResult<Self> {
        match value {
            "segmented_concept" => Ok(Self::SegmentedConcept),
            "editable_asset" => Ok(Self::EditableAsset),
            _ => Err(CoreError::invalid_data(
                "ASSET_STAGE_INVALID",
                "Agent asset version has an unsupported stage.",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentAssetVersion {
    pub asset_version_id: String,
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_asset_version_id: Option<String>,
    pub version_no: u64,
    pub status: AssetVersionStatus,
    pub summary: String,
    pub stage: AssetStage,
    pub plan_id: String,
    pub direction_id: String,
    pub domain_pack_id: String,
    pub artifact_id: String,
    pub parts: Vec<Value>,
    pub shape_program: Value,
    pub assembly_graph: Value,
    #[serde(default)]
    pub material_bindings: BTreeMap<String, Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStatus {
    Candidate,
    Committed,
    Discarded,
}

impl FromStr for CandidateStatus {
    type Err = CoreError;

    fn from_str(value: &str) -> CoreResult<Self> {
        match value {
            "candidate" => Ok(Self::Candidate),
            "committed" => Ok(Self::Committed),
            "discarded" => Ok(Self::Discarded),
            _ => Err(CoreError::invalid_data(
                "BLOCKOUT_CANDIDATE_STATUS_INVALID",
                "Blockout candidate has an unsupported status.",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BlockoutCandidate {
    pub artifact_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub plan_id: String,
    pub direction_id: String,
    pub domain_pack_id: String,
    pub status: CandidateStatus,
    pub candidate: Value,
    pub shape_program: Value,
    pub assembly_graph: Value,
    #[serde(default)]
    pub material_bindings: BTreeMap<String, Value>,
    pub glb_sha256: String,
    pub created_at: String,
    pub updated_at: String,
}

impl BlockoutCandidate {
    pub fn validate(&self) -> CoreResult<()> {
        require_id("artifact_id", &self.artifact_id)?;
        if let Some(project_id) = self.project_id.as_deref() {
            require_id("project_id", project_id)?;
        }
        require_id("plan_id", &self.plan_id)?;
        require_id("direction_id", &self.direction_id)?;
        require_id("domain_pack_id", &self.domain_pack_id)?;
        require_object("candidate", &self.candidate)?;
        require_object("shape_program", &self.shape_program)?;
        require_object("assembly_graph", &self.assembly_graph)?;
        require_text("created_at", &self.created_at, 128)?;
        require_text("updated_at", &self.updated_at, 128)?;
        if self.glb_sha256.len() != 64
            || !self
                .glb_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(CoreError::invalid_data(
                "BLOCKOUT_CANDIDATE_GLB_SHA_INVALID",
                "Candidate GLB must use a lowercase SHA-256 identity.",
            ));
        }
        Ok(())
    }
}

impl AgentAssetVersion {
    pub fn validate(&self) -> CoreResult<()> {
        require_id("asset_version_id", &self.asset_version_id)?;
        require_id("project_id", &self.project_id)?;
        if let Some(parent) = self.parent_asset_version_id.as_deref() {
            require_id("parent_asset_version_id", parent)?;
            if parent == self.asset_version_id {
                return Err(CoreError::invalid_data(
                    "ASSET_VERSION_PARENT_INVALID",
                    "Agent asset version cannot be its own parent.",
                ));
            }
        }
        if self.version_no == 0 {
            return Err(CoreError::invalid_data(
                "ASSET_VERSION_NUMBER_INVALID",
                "Agent asset version number must be positive.",
            ));
        }
        require_text("summary", &self.summary, 2_000)?;
        require_id("plan_id", &self.plan_id)?;
        require_id("direction_id", &self.direction_id)?;
        require_id("domain_pack_id", &self.domain_pack_id)?;
        require_id("artifact_id", &self.artifact_id)?;
        require_text("created_at", &self.created_at, 128)?;
        require_object("shape_program", &self.shape_program)?;
        require_object("assembly_graph", &self.assembly_graph)?;
        self.assembly_graph_id()?;
        let _ = self.part_zone_index()?;
        Ok(())
    }

    pub fn assembly_graph_id(&self) -> CoreResult<&str> {
        self.assembly_graph
            .get("graph_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "ACTIVE_DESIGN_GRAPH_MISSING",
                    "Agent asset version is missing a stable AssemblyGraph ID.",
                )
            })
    }

    pub fn part_zone_index(&self) -> CoreResult<BTreeMap<String, Vec<String>>> {
        let parts = self
            .assembly_graph
            .get("parts")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "ASSEMBLY_GRAPH_PARTS_INVALID",
                    "AssemblyGraph must contain a parts array.",
                )
            })?;
        let mut index = BTreeMap::new();
        for part in parts {
            let part_id = part
                .get("part_id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    CoreError::invalid_data(
                        "ASSEMBLY_GRAPH_PART_ID_INVALID",
                        "AssemblyGraph part is missing a stable part_id.",
                    )
                })?;
            require_id("part_id", part_id)?;
            let raw_zones = part
                .get("material_zone_ids")
                .or_else(|| part.get("material_zones"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let zones = raw_zones
                .iter()
                .filter_map(|zone| {
                    zone.as_str()
                        .or_else(|| zone.get("zone_id").and_then(Value::as_str))
                        .map(str::to_owned)
                })
                .collect::<Vec<_>>();
            if index.insert(part_id.to_string(), zones).is_some() {
                return Err(CoreError::invalid_data(
                    "ASSEMBLY_GRAPH_PART_DUPLICATE",
                    "AssemblyGraph contains a duplicate part_id.",
                ));
            }
        }
        Ok(index)
    }

    pub fn semantic_hash(&self) -> CoreResult<String> {
        semantic_sha256(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_zone_id: Option<String>,
}

impl Selection {
    pub fn validate(&self) -> CoreResult<()> {
        if self.material_zone_id.is_some() && self.part_id.is_none() {
            return Err(CoreError::invalid_data(
                "ACTIVE_DESIGN_SELECTION_INVALID",
                "Material Zone selection requires a selected part.",
            ));
        }
        if let Some(part_id) = self.part_id.as_deref() {
            require_id("selected_part_id", part_id)?;
        }
        if let Some(zone_id) = self.material_zone_id.as_deref() {
            require_id("selected_material_zone_id", zone_id)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActiveDesign {
    AgentAsset {
        project_id: String,
        asset_version_id: String,
        assembly_graph_id: String,
    },
    LegacyConceptReadOnly {
        project_id: String,
        legacy_version_id: String,
        module_graph_id: String,
    },
}

impl ActiveDesign {
    pub fn project_id(&self) -> &str {
        match self {
            Self::AgentAsset { project_id, .. }
            | Self::LegacyConceptReadOnly { project_id, .. } => project_id,
        }
    }

    pub fn asset_version_id(&self) -> Option<&str> {
        match self {
            Self::AgentAsset {
                asset_version_id, ..
            } => Some(asset_version_id),
            Self::LegacyConceptReadOnly { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PreviewReference {
    pub project_id: String,
    pub change_set_id: String,
    pub base_asset_version_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QualityReference {
    pub project_id: String,
    pub quality_report_id: String,
    pub asset_version_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExportReference {
    AgentAsset {
        project_id: String,
        source_version_id: String,
    },
    LegacyConceptReadOnly {
        project_id: String,
        source_version_id: String,
    },
}

impl ExportReference {
    pub fn source_version_id(&self) -> &str {
        match self {
            Self::AgentAsset {
                source_version_id, ..
            }
            | Self::LegacyConceptReadOnly {
                source_version_id, ..
            } => source_version_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RenderPreset {
    pub schema_version: String,
    pub preset_id: String,
    pub project_id: String,
    pub asset_version_id: String,
    pub camera_view: String,
    pub light_preset: String,
    pub updated_at: String,
}

impl RenderPreset {
    pub fn default_for(project_id: &str, asset_version_id: &str, updated_at: &str) -> Self {
        Self {
            schema_version: "ActiveDesignRenderPreset@1".to_string(),
            preset_id: format!("render_{asset_version_id}_iso_cad_neutral"),
            project_id: project_id.to_string(),
            asset_version_id: asset_version_id.to_string(),
            camera_view: "iso".to_string(),
            light_preset: "cad_neutral".to_string(),
            updated_at: updated_at.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PartDisplay {
    pub schema_version: String,
    pub project_id: String,
    pub asset_version_id: String,
    #[serde(default)]
    pub locked_part_ids: Vec<String>,
    #[serde(default)]
    pub hidden_part_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isolated_part_id: Option<String>,
}

impl PartDisplay {
    pub fn empty(project_id: &str, asset_version_id: &str) -> Self {
        Self {
            schema_version: "ActiveDesignPartDisplay@1".to_string(),
            project_id: project_id.to_string(),
            asset_version_id: asset_version_id.to_string(),
            locked_part_ids: Vec::new(),
            hidden_part_ids: Vec::new(),
            isolated_part_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActiveDesignSnapshot {
    pub schema_version: String,
    pub project_id: String,
    pub active_design: ActiveDesign,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_part_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_material_zone_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<PreviewReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<QualityReference>,
    pub export: ExportReference,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_preset: Option<RenderPreset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_display: Option<PartDisplay>,
    pub revision: u64,
    pub updated_at: String,
}

impl ActiveDesignSnapshot {
    pub fn selection(&self) -> Selection {
        Selection {
            part_id: self.selected_part_id.clone(),
            material_zone_id: self.selected_material_zone_id.clone(),
        }
    }

    pub fn etag(&self) -> SnapshotEtag {
        SnapshotEtag(self.revision)
    }

    pub fn semantic_hash(&self) -> CoreResult<String> {
        semantic_sha256(self)
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != "ActiveDesignSnapshot@1" || self.revision == 0 {
            return Err(CoreError::invalid_data(
                "ACTIVE_DESIGN_SNAPSHOT_INVALID",
                "Snapshot schema version and revision must be valid.",
            ));
        }
        if self.active_design.project_id() != self.project_id {
            return Err(CoreError::invalid_data(
                "ACTIVE_DESIGN_PROJECT_MISMATCH",
                "Snapshot nested project identity does not match.",
            ));
        }
        self.selection().validate()?;
        match &self.active_design {
            ActiveDesign::AgentAsset {
                asset_version_id, ..
            } => {
                if self.export.source_version_id() != asset_version_id
                    || self.preview.as_ref().is_some_and(|preview| {
                        preview.project_id != self.project_id
                            || preview.base_asset_version_id != *asset_version_id
                    })
                    || self.quality.as_ref().is_some_and(|quality| {
                        quality.project_id != self.project_id
                            || quality.asset_version_id != *asset_version_id
                    })
                {
                    return Err(CoreError::invalid_data(
                        "ACTIVE_DESIGN_REFERENCE_MISMATCH",
                        "Snapshot preview, quality and export must bind the active Agent asset.",
                    ));
                }
            }
            ActiveDesign::LegacyConceptReadOnly {
                legacy_version_id, ..
            } => {
                if self.selected_part_id.is_some()
                    || self.selected_material_zone_id.is_some()
                    || self.preview.is_some()
                    || self.quality.is_some()
                    || self.render_preset.is_some()
                    || self.part_display.is_some()
                    || self.export.source_version_id() != legacy_version_id
                {
                    return Err(CoreError::invalid_data(
                        "ACTIVE_DESIGN_LEGACY_READ_ONLY",
                        "Legacy Snapshot cannot carry Agent workflow state.",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotEtag(pub u64);

impl fmt::Display for SnapshotEtag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "W/\"active-design-{}\"", self.0)
    }
}

impl FromStr for SnapshotEtag {
    type Err = CoreError;

    fn from_str(value: &str) -> CoreResult<Self> {
        let revision = value
            .strip_prefix("W/\"active-design-")
            .and_then(|value| value.strip_suffix('\"'))
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "ACTIVE_DESIGN_ETAG_INVALID",
                    "Snapshot ETag must use W/\"active-design-{revision}\".",
                )
            })?;
        Ok(Self(revision))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeSetStatus {
    Proposed,
    Previewed,
    Confirmed,
    Rejected,
    Stale,
}

impl FromStr for ChangeSetStatus {
    type Err = CoreError;

    fn from_str(value: &str) -> CoreResult<Self> {
        match value {
            "proposed" => Ok(Self::Proposed),
            "previewed" => Ok(Self::Previewed),
            "confirmed" => Ok(Self::Confirmed),
            "rejected" => Ok(Self::Rejected),
            "stale" => Ok(Self::Stale),
            _ => Err(CoreError::invalid_data(
                "CHANGE_SET_STATUS_INVALID",
                "Agent ChangeSet has an unsupported status.",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentAssetChangeSet {
    pub change_set_id: String,
    pub project_id: String,
    pub base_asset_version_id: String,
    pub summary: String,
    pub operations: Vec<Value>,
    #[serde(default)]
    pub protected_part_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<Value>,
    pub status: ChangeSetStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resulting_asset_version_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl AgentAssetChangeSet {
    pub fn validate(&self) -> CoreResult<()> {
        require_id("change_set_id", &self.change_set_id)?;
        require_id("project_id", &self.project_id)?;
        require_id("base_asset_version_id", &self.base_asset_version_id)?;
        require_text("summary", &self.summary, 2_000)?;
        if self.operations.is_empty() {
            return Err(CoreError::invalid_data(
                "CHANGE_SET_OPERATIONS_EMPTY",
                "Agent ChangeSet must contain at least one bounded operation.",
            ));
        }
        Ok(())
    }
}

/// Immutable project-local component snapshot. This is deliberately separate
/// from the reviewed Module Asset catalog and carries no engineering or formal
/// review claim. `source_quality_status` is always recomputed when the record
/// is read; it is never persisted in `agent_components`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentComponentRecord {
    pub schema_version: String,
    pub component_id: String,
    pub project_id: String,
    pub domain_pack_id: String,
    pub role: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    pub source_asset_version_id: String,
    pub source_part_id: String,
    pub part_template: Value,
    pub shape_operation: Value,
    #[serde(default)]
    pub material_bindings: BTreeMap<String, Value>,
    pub status: String,
    pub source_quality_status: QualityStatus,
    pub created_at: String,
    pub updated_at: String,
}

impl AgentComponentRecord {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != "AgentComponent@1" {
            return Err(CoreError::invalid_data(
                "AGENT_COMPONENT_SCHEMA_INVALID",
                "Agent component must use AgentComponent@1.",
            ));
        }
        require_id("component_id", &self.component_id)?;
        require_id("project_id", &self.project_id)?;
        require_id("domain_pack_id", &self.domain_pack_id)?;
        require_text("role", &self.role, 160)?;
        require_text("display_name", &self.display_name, 120)?;
        if self.description.len() > 500 || self.description.contains('\0') {
            return Err(CoreError::invalid_data(
                "AGENT_COMPONENT_DESCRIPTION_INVALID",
                "Agent component description exceeds the bounded contract.",
            ));
        }
        require_id("source_asset_version_id", &self.source_asset_version_id)?;
        require_id("source_part_id", &self.source_part_id)?;
        require_object("part_template", &self.part_template)?;
        require_object("shape_operation", &self.shape_operation)?;
        if !matches!(self.status.as_str(), "active" | "disabled") {
            return Err(CoreError::invalid_data(
                "AGENT_COMPONENT_STATUS_INVALID",
                "Agent component status must be active or disabled.",
            ));
        }
        require_text("created_at", &self.created_at, 128)?;
        require_text("updated_at", &self.updated_at, 128)
    }
}

/// Explainable visual replacement eligibility, never an engineering fitness
/// score. The reason list is made only from persisted project facts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentComponentCompatibility {
    pub schema_version: String,
    pub component_id: String,
    pub target_asset_version_id: String,
    pub target_part_id: String,
    pub eligible: bool,
    pub source_quality_status: QualityStatus,
    pub reason_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentComponentCandidate {
    pub schema_version: String,
    pub component: AgentComponentRecord,
    pub compatibility: AgentComponentCompatibility,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentStructureSuggestion {
    pub schema_version: String,
    pub suggestion_id: String,
    pub kind: String,
    pub asset_version_id: String,
    pub part_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_part_id: Option<String>,
    pub affected_part_ids: Vec<String>,
    pub source_facts: Vec<String>,
    pub summary: String,
}

impl AgentStructureSuggestion {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != "AgentStructureSuggestion@1"
            || !matches!(self.kind.as_str(), "split_part" | "merge_parts")
        {
            return Err(CoreError::invalid_data(
                "STRUCTURE_SUGGESTION_INVALID",
                "Structure suggestion identity or kind is invalid.",
            ));
        }
        require_id("suggestion_id", &self.suggestion_id)?;
        require_id("asset_version_id", &self.asset_version_id)?;
        require_id("part_id", &self.part_id)?;
        if let Some(target) = self.target_part_id.as_deref() {
            require_id("target_part_id", target)?;
        }
        if self.affected_part_ids.is_empty() || self.affected_part_ids.len() > 4 {
            return Err(CoreError::invalid_data(
                "STRUCTURE_SUGGESTION_INVALID",
                "Structure suggestion must identify between one and four affected parts.",
            ));
        }
        require_text("summary", &self.summary, 240)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentStructureSuggestionList {
    pub schema_version: String,
    pub asset_version_id: String,
    pub suggestions: Vec<AgentStructureSuggestion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualityStatus {
    Passed,
    Warning,
    Failed,
    Unavailable,
}

impl QualityStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Warning => "warning",
            Self::Failed => "failed",
            Self::Unavailable => "unavailable",
        }
    }
}

impl FromStr for QualityStatus {
    type Err = CoreError;

    fn from_str(value: &str) -> CoreResult<Self> {
        match value {
            "passed" => Ok(Self::Passed),
            "warning" => Ok(Self::Warning),
            "failed" => Ok(Self::Failed),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err(CoreError::invalid_data(
                "QUALITY_STATUS_INVALID",
                "Quality report has an unsupported status.",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QualityReport {
    pub quality_report_id: String,
    pub project_id: String,
    pub asset_version_id: String,
    pub report: Value,
    pub status: QualityStatus,
    pub created_at: String,
}

impl QualityReport {
    pub fn validate(&self) -> CoreResult<()> {
        require_id("quality_report_id", &self.quality_report_id)?;
        require_id("project_id", &self.project_id)?;
        require_id("asset_version_id", &self.asset_version_id)?;
        require_object("quality_report", &self.report)?;
        require_text("created_at", &self.created_at, 128)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NavigationAction {
    Undo,
    Redo,
}

impl NavigationAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Undo => "undo",
            Self::Redo => "redo",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NavigationResult {
    pub version: AgentAssetVersion,
    pub snapshot: ActiveDesignSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NavigationAvailability {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_asset_version_id: Option<String>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub preview_pending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ObjectReference {
    pub reference_kind: String,
    pub owner_id: String,
    pub role: String,
}

impl ObjectReference {
    pub fn validate(&self) -> CoreResult<()> {
        match self.reference_kind.as_str() {
            "candidate" | "asset_version" | "quality" | "export" | "preview" | "texture"
            | "reference" => {}
            _ => {
                return Err(CoreError::invalid_data(
                    "OBJECT_REFERENCE_KIND_INVALID",
                    "Content object reference kind is not registered.",
                ))
            }
        }
        require_id("object_reference.owner_id", &self.owner_id)?;
        require_text("object_reference.role", &self.role, 128)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ObjectRecord {
    pub sha256: String,
    pub object_path: String,
    pub extension: String,
    pub byte_size: u64,
    pub ref_count: u64,
    pub created_at: String,
    pub updated_at: String,
}

pub const MAX_MATERIAL_TEXTURE_BYTES: usize = 4_000_000;
pub const MAX_MATERIAL_TEXTURE_DIMENSION: u32 = 4096;
pub const MAX_MATERIAL_TEXTURE_PIXELS: u64 = 16_000_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MaterialTextureRole {
    BaseColor,
    MetallicRoughness,
    Normal,
    Occlusion,
    Emissive,
    Thumbnail,
}

impl MaterialTextureRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BaseColor => "base_color",
            Self::MetallicRoughness => "metallic_roughness",
            Self::Normal => "normal",
            Self::Occlusion => "occlusion",
            Self::Emissive => "emissive",
            Self::Thumbnail => "thumbnail",
        }
    }
}

impl FromStr for MaterialTextureRole {
    type Err = CoreError;

    fn from_str(value: &str) -> CoreResult<Self> {
        match value {
            "base_color" => Ok(Self::BaseColor),
            "metallic_roughness" => Ok(Self::MetallicRoughness),
            "normal" => Ok(Self::Normal),
            "occlusion" => Ok(Self::Occlusion),
            "emissive" => Ok(Self::Emissive),
            "thumbnail" => Ok(Self::Thumbnail),
            _ => Err(CoreError::invalid_data(
                "TEXTURE_ROLE_INVALID",
                "Texture role is not part of the visual-only M103 contract.",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MaterialTextureSource {
    ForgecadBuiltin,
    UserCreated,
    ImportedReference,
}

impl MaterialTextureSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ForgecadBuiltin => "forgecad_builtin",
            Self::UserCreated => "user_created",
            Self::ImportedReference => "imported_reference",
        }
    }
}

impl FromStr for MaterialTextureSource {
    type Err = CoreError;

    fn from_str(value: &str) -> CoreResult<Self> {
        match value {
            "forgecad_builtin" => Ok(Self::ForgecadBuiltin),
            "user_created" => Ok(Self::UserCreated),
            "imported_reference" => Ok(Self::ImportedReference),
            _ => Err(CoreError::invalid_data(
                "TEXTURE_SOURCE_INVALID",
                "Texture source is not allowed by the visual-only M103 contract.",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MaterialTextureLicense {
    NotApplicable,
    SelfDeclaredOriginal,
    ThirdParty,
    Unknown,
}

impl MaterialTextureLicense {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::SelfDeclaredOriginal => "self_declared_original",
            Self::ThirdParty => "third_party",
            Self::Unknown => "unknown",
        }
    }
}

impl FromStr for MaterialTextureLicense {
    type Err = CoreError;

    fn from_str(value: &str) -> CoreResult<Self> {
        match value {
            "not_applicable" => Ok(Self::NotApplicable),
            "self_declared_original" => Ok(Self::SelfDeclaredOriginal),
            "third_party" => Ok(Self::ThirdParty),
            "unknown" => Ok(Self::Unknown),
            _ => Err(CoreError::invalid_data(
                "TEXTURE_LICENSE_INVALID",
                "Texture license is not allowed by the visual-only M103 contract.",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RegisterMaterialTextureRequest {
    pub display_name: String,
    pub texture_role: MaterialTextureRole,
    pub mime_type: String,
    pub payload_base64: String,
    pub source: MaterialTextureSource,
    pub license: MaterialTextureLicense,
    #[serde(default)]
    pub license_ref: Option<String>,
    #[serde(default)]
    pub thumbnail_asset_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedMaterialTexturePayload {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub extension: &'static str,
}

impl RegisterMaterialTextureRequest {
    pub(crate) fn validate_and_decode(&self) -> CoreResult<ValidatedMaterialTexturePayload> {
        require_text("display_name", &self.display_name, 120)?;
        validate_material_texture_provenance(
            self.source,
            self.license,
            self.license_ref.as_deref(),
        )?;
        if let Some(license_ref) = &self.license_ref {
            require_text("license_ref", license_ref, 240)?;
        }
        if let Some(texture_asset_id) = &self.thumbnail_asset_id {
            validate_material_texture_asset_id(texture_asset_id)?;
        }
        if self.payload_base64.is_empty() || self.payload_base64.len() > 5_600_000 {
            return Err(CoreError::invalid_data(
                "TEXTURE_BASE64_INVALID",
                "Texture payload must be bounded raw base64.",
            ));
        }
        if self.payload_base64.starts_with("data:") || self.payload_base64.contains("://") {
            return Err(CoreError::invalid_data(
                "TEXTURE_BASE64_INVALID",
                "Texture payload must be raw base64, not a URL or data URI.",
            ));
        }
        let bytes = BASE64_STANDARD.decode(&self.payload_base64).map_err(|_| {
            CoreError::invalid_data(
                "TEXTURE_BASE64_INVALID",
                "Texture payload is not valid base64.",
            )
        })?;
        if bytes.is_empty() {
            return Err(CoreError::invalid_data(
                "TEXTURE_EMPTY",
                "Texture payload cannot be empty.",
            ));
        }
        if bytes.len() > MAX_MATERIAL_TEXTURE_BYTES {
            return Err(CoreError::invalid_data(
                "TEXTURE_TOO_LARGE",
                "Texture object exceeds the 4 MB limit.",
            ));
        }
        let (width, height, extension) = inspect_material_texture(&bytes, &self.mime_type)?;
        if width > MAX_MATERIAL_TEXTURE_DIMENSION || height > MAX_MATERIAL_TEXTURE_DIMENSION {
            return Err(CoreError::invalid_data(
                "TEXTURE_DIMENSIONS_TOO_LARGE",
                "Texture dimensions exceed the 4096 pixel limit.",
            ));
        }
        if u64::from(width) * u64::from(height) > MAX_MATERIAL_TEXTURE_PIXELS {
            return Err(CoreError::invalid_data(
                "TEXTURE_PIXELS_TOO_MANY",
                "Texture pixel count exceeds the visual texture limit.",
            ));
        }
        Ok(ValidatedMaterialTexturePayload {
            bytes,
            width,
            height,
            extension,
        })
    }

    pub(crate) fn request_hash(&self) -> CoreResult<String> {
        semantic_sha256(&serde_json::to_value(self).map_err(|_| {
            CoreError::invalid_data(
                "TEXTURE_REQUEST_INVALID",
                "Texture registration request could not be sealed.",
            )
        })?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MaterialTextureObject {
    pub schema_version: String,
    pub texture_asset_id: String,
    pub texture_role: MaterialTextureRole,
    pub display_name: String,
    pub mime_type: String,
    pub byte_size: u64,
    pub sha256: String,
    pub object_path: String,
    pub width: u32,
    pub height: u32,
    pub source: MaterialTextureSource,
    pub license: MaterialTextureLicense,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_asset_id: Option<String>,
    pub visual_only: bool,
    pub object_exists: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl MaterialTextureObject {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != "MaterialTextureObject@1" || !self.visual_only {
            return Err(CoreError::invalid_data(
                "TEXTURE_OBJECT_INVALID",
                "Texture object must be the visual-only M103 schema.",
            ));
        }
        validate_material_texture_asset_id(&self.texture_asset_id)?;
        require_text("display_name", &self.display_name, 120)?;
        validate_material_texture_provenance(
            self.source,
            self.license,
            self.license_ref.as_deref(),
        )?;
        if let Some(license_ref) = &self.license_ref {
            require_text("license_ref", license_ref, 240)?;
        }
        if let Some(thumbnail_asset_id) = &self.thumbnail_asset_id {
            validate_material_texture_asset_id(thumbnail_asset_id)?;
        }
        let extension = match self.mime_type.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/webp" => "webp",
            _ => {
                return Err(CoreError::invalid_data(
                    "TEXTURE_MIME_UNSUPPORTED",
                    "Texture MIME type is not PNG, JPEG or WebP.",
                ))
            }
        };
        let valid_sha = self.sha256.len() == 64
            && self
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
        let expected_path = if valid_sha {
            format!(
                "objects/sha256/{}/{}/{}.{}",
                &self.sha256[..2],
                &self.sha256[2..4],
                self.sha256,
                extension
            )
        } else {
            String::new()
        };
        if !valid_sha
            || self.object_path != expected_path
            || self.byte_size == 0
            || self.byte_size > MAX_MATERIAL_TEXTURE_BYTES as u64
            || self.width == 0
            || self.height == 0
            || self.width > MAX_MATERIAL_TEXTURE_DIMENSION
            || self.height > MAX_MATERIAL_TEXTURE_DIMENSION
            || u64::from(self.width) * u64::from(self.height) > MAX_MATERIAL_TEXTURE_PIXELS
        {
            return Err(CoreError::invalid_data(
                "TEXTURE_OBJECT_INVALID",
                "Texture object identity, path, size or dimensions are invalid.",
            ));
        }
        require_text("created_at", &self.created_at, 128)?;
        require_text("updated_at", &self.updated_at, 128)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MaterialTextureSummary {
    pub texture_asset_id: String,
    pub texture_role: MaterialTextureRole,
    pub exists: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<MaterialTextureSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<MaterialTextureLicense>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialTextureQuery {
    pub texture_role: Option<MaterialTextureRole>,
    pub source: Option<MaterialTextureSource>,
    pub query: Option<String>,
    pub limit: usize,
}

impl Default for MaterialTextureQuery {
    fn default() -> Self {
        Self {
            texture_role: None,
            source: None,
            query: None,
            limit: 100,
        }
    }
}

impl MaterialTextureQuery {
    pub fn validate(&self) -> CoreResult<()> {
        if !(1..=100).contains(&self.limit) {
            return Err(CoreError::invalid_data(
                "TEXTURE_QUERY_LIMIT_INVALID",
                "Texture query limit must be between 1 and 100.",
            ));
        }
        if let Some(query) = &self.query {
            if query.chars().count() > 120 || query.chars().any(char::is_control) {
                return Err(CoreError::invalid_data(
                    "TEXTURE_QUERY_INVALID",
                    "Texture search query must be at most 120 characters.",
                ));
            }
        }
        Ok(())
    }
}

pub(crate) fn validate_material_texture_asset_id(value: &str) -> CoreResult<()> {
    let suffix = value.strip_prefix("asset_tex_").unwrap_or_default();
    if suffix.len() == 24
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(CoreError::invalid_data(
            "TEXTURE_ASSET_ID_INVALID",
            "Texture asset ID must be the stable SHA-derived M103 identifier.",
        ))
    }
}

fn validate_material_texture_provenance(
    source: MaterialTextureSource,
    license: MaterialTextureLicense,
    license_ref: Option<&str>,
) -> CoreResult<()> {
    let allowed = matches!(
        (source, license),
        (
            MaterialTextureSource::ForgecadBuiltin,
            MaterialTextureLicense::NotApplicable
        ) | (
            MaterialTextureSource::UserCreated,
            MaterialTextureLicense::SelfDeclaredOriginal
        ) | (
            MaterialTextureSource::UserCreated,
            MaterialTextureLicense::Unknown
        ) | (
            MaterialTextureSource::ImportedReference,
            MaterialTextureLicense::ThirdParty
        ) | (
            MaterialTextureSource::ImportedReference,
            MaterialTextureLicense::Unknown
        )
    );
    if !allowed || (license == MaterialTextureLicense::ThirdParty && license_ref.is_none()) {
        return Err(CoreError::invalid_data(
            "TEXTURE_PROVENANCE_INVALID",
            "Texture source, license and license reference are not an allowed visual-only combination.",
        ));
    }
    Ok(())
}

fn inspect_material_texture(bytes: &[u8], mime_type: &str) -> CoreResult<(u32, u32, &'static str)> {
    match mime_type {
        "image/png" => {
            if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR"
            {
                return Err(CoreError::invalid_data(
                    "TEXTURE_FORMAT_INVALID",
                    "PNG signature or IHDR is invalid.",
                ));
            }
            let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
            let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
            validate_nonzero_dimensions(width, height)?;
            Ok((width, height, "png"))
        }
        "image/jpeg" => {
            let (width, height) = jpeg_dimensions(bytes)?;
            Ok((width, height, "jpg"))
        }
        "image/webp" => {
            if bytes.len() < 30
                || &bytes[..4] != b"RIFF"
                || &bytes[8..12] != b"WEBP"
                || &bytes[12..16] != b"VP8X"
            {
                return Err(CoreError::invalid_data(
                    "TEXTURE_FORMAT_INVALID",
                    "Only WebP textures with a VP8X header are supported.",
                ));
            }
            let width = 1 + u32::from_le_bytes([bytes[24], bytes[25], bytes[26], 0]);
            let height = 1 + u32::from_le_bytes([bytes[27], bytes[28], bytes[29], 0]);
            validate_nonzero_dimensions(width, height)?;
            Ok((width, height, "webp"))
        }
        _ => Err(CoreError::invalid_data(
            "TEXTURE_MIME_UNSUPPORTED",
            "Texture MIME type is not PNG, JPEG or WebP.",
        )),
    }
}

fn jpeg_dimensions(bytes: &[u8]) -> CoreResult<(u32, u32)> {
    if bytes.len() < 4 || &bytes[..2] != b"\xff\xd8" {
        return Err(CoreError::invalid_data(
            "TEXTURE_FORMAT_INVALID",
            "JPEG signature is invalid.",
        ));
    }
    let mut index = 2;
    while index + 3 < bytes.len() {
        if bytes[index] != 0xff {
            index += 1;
            continue;
        }
        while index < bytes.len() && bytes[index] == 0xff {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        let marker = bytes[index];
        index += 1;
        if matches!(marker, 0xd8 | 0xd9) {
            continue;
        }
        if index + 2 > bytes.len() {
            break;
        }
        let segment_length = usize::from(u16::from_be_bytes([bytes[index], bytes[index + 1]]));
        if segment_length < 2 || index + segment_length > bytes.len() {
            break;
        }
        let is_sof = matches!(
            marker,
            0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf
        );
        if is_sof && segment_length >= 7 {
            let height = u32::from(u16::from_be_bytes([bytes[index + 3], bytes[index + 4]]));
            let width = u32::from(u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]));
            validate_nonzero_dimensions(width, height)?;
            return Ok((width, height));
        }
        index += segment_length;
    }
    Err(CoreError::invalid_data(
        "TEXTURE_DIMENSIONS_MISSING",
        "JPEG dimensions could not be read.",
    ))
}

fn validate_nonzero_dimensions(width: u32, height: u32) -> CoreResult<()> {
    if width > 0 && height > 0 {
        Ok(())
    } else {
        Err(CoreError::invalid_data(
            "TEXTURE_DIMENSIONS_INVALID",
            "Texture dimensions must be positive.",
        ))
    }
}

/// Complete, authoritative readback for the initial candidate promotion.
///
/// This is intentionally one value: callers must not infer success from a
/// candidate row, a version row or an object reference in isolation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CandidateBundleReadback {
    pub candidate: BlockoutCandidate,
    pub version: AgentAssetVersion,
    pub snapshot: ActiveDesignSnapshot,
    pub quality: QualityReport,
    pub production_glb: ObjectRecord,
    pub interactive_preview_glb: ObjectRecord,
}

/// Complete readback for an active, sealed ChangeSet preview. Preview bytes
/// remain temporary and are owned by the ChangeSet until confirmation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChangeSetPreviewBundleReadback {
    pub change_set: AgentAssetChangeSet,
    pub sealed_preview: AgentAssetVersion,
    pub snapshot: ActiveDesignSnapshot,
    pub interactive_preview_glb: ObjectRecord,
    pub interactive_readback: Value,
}

/// Complete authoritative readback after a ChangeSet preview is confirmed as
/// a new immutable asset version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChangeSetConfirmBundleReadback {
    pub change_set: AgentAssetChangeSet,
    pub version: AgentAssetVersion,
    pub snapshot: ActiveDesignSnapshot,
    pub quality: QualityReport,
    pub production_glb: ObjectRecord,
    pub interactive_preview_glb: ObjectRecord,
}

fn require_id(field: &str, value: &str) -> CoreResult<()> {
    let valid = !value.is_empty()
        && value.len() <= 256
        && value.is_ascii()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'/')
        });
    if valid {
        Ok(())
    } else {
        Err(CoreError::invalid_data(
            "STABLE_ID_INVALID",
            format!("{field} is not a bounded stable ID."),
        ))
    }
}

fn require_text(field: &str, value: &str, max: usize) -> CoreResult<()> {
    if !value.trim().is_empty()
        && value.chars().count() <= max
        && !value.chars().any(char::is_control)
    {
        Ok(())
    } else {
        Err(CoreError::invalid_data(
            "TEXT_FIELD_INVALID",
            format!("{field} must be non-empty, bounded text."),
        ))
    }
}

fn require_object(field: &str, value: &Value) -> CoreResult<()> {
    if value.is_object() {
        Ok(())
    } else {
        Err(CoreError::invalid_data(
            "JSON_OBJECT_REQUIRED",
            format!("{field} must be a JSON object."),
        ))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn etag_is_exact_and_revision_is_positive() {
        assert_eq!(SnapshotEtag(7).to_string(), "W/\"active-design-7\"");
        assert_eq!(
            "W/\"active-design-7\"".parse::<SnapshotEtag>().unwrap(),
            SnapshotEtag(7)
        );
        assert!("active-design-7".parse::<SnapshotEtag>().is_err());
        assert!("W/\"active-design-0\"".parse::<SnapshotEtag>().is_err());
    }

    #[test]
    fn assembly_part_zone_index_accepts_current_zone_shapes() {
        let version = AgentAssetVersion {
            asset_version_id: "assetver_1".into(),
            project_id: "project_1".into(),
            parent_asset_version_id: None,
            version_no: 1,
            status: AssetVersionStatus::Committed,
            summary: "initial".into(),
            stage: AssetStage::SegmentedConcept,
            plan_id: "plan_1".into(),
            direction_id: "direction_1".into(),
            domain_pack_id: "vehicle_concept_v1".into(),
            artifact_id: "artifact_1".into(),
            parts: vec![json!({"part_id": "part_body"})],
            shape_program: json!({"schema_version": "ShapeProgram@1", "operations": []}),
            assembly_graph: json!({
                "graph_id": "assembly_1",
                "parts": [
                    {"part_id": "part_body", "material_zone_ids": ["zone_body"]},
                    {"part_id": "part_glass", "material_zones": [{"zone_id": "zone_glass"}]}
                ]
            }),
            material_bindings: BTreeMap::new(),
            created_at: "2026-07-17T00:00:00Z".into(),
        };
        let index = version.part_zone_index().unwrap();
        assert_eq!(index["part_body"], vec!["zone_body"]);
        assert_eq!(index["part_glass"], vec!["zone_glass"]);
    }
}
