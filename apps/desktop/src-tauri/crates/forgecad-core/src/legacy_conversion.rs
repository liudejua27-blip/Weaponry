use crate::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};

pub const LEGACY_CONVERSION_READY: &str = "ready_for_agent_rebuild";

/// Exact read-only legacy source authorized for one future Agent rebuild.
///
/// This is deliberately a reference, not geometry or an editable Agent asset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LegacyActiveDesignSource {
    pub source: String,
    pub project_id: String,
    pub legacy_version_id: String,
    pub module_graph_id: String,
}

impl LegacyActiveDesignSource {
    pub fn new(
        project_id: impl Into<String>,
        legacy_version_id: impl Into<String>,
        module_graph_id: impl Into<String>,
    ) -> Self {
        Self {
            source: "legacy_concept_read_only".into(),
            project_id: project_id.into(),
            legacy_version_id: legacy_version_id.into(),
            module_graph_id: module_graph_id.into(),
        }
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.source != "legacy_concept_read_only"
            || self.project_id.is_empty()
            || self.legacy_version_id.is_empty()
            || self.module_graph_id.is_empty()
        {
            return Err(CoreError::invalid_data(
                "LEGACY_CONVERSION_SOURCE_INVALID",
                "Legacy conversion source must identify one read-only Project, ConceptVersion and ModuleGraph.",
            ));
        }
        Ok(())
    }
}

/// Safe S007 hand-off. Creating this response creates no AgentAssetVersion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LegacyActiveDesignConversionResponse {
    pub schema_version: String,
    pub project_id: String,
    pub source: LegacyActiveDesignSource,
    pub snapshot_revision: u64,
    pub status: String,
    pub message: String,
}

impl LegacyActiveDesignConversionResponse {
    pub fn ready(source: LegacyActiveDesignSource, snapshot_revision: u64) -> CoreResult<Self> {
        source.validate()?;
        if snapshot_revision == 0 {
            return Err(CoreError::invalid_data(
                "LEGACY_CONVERSION_REVISION_INVALID",
                "Legacy conversion authorization requires a positive Snapshot revision.",
            ));
        }
        Ok(Self {
            schema_version: "LegacyActiveDesignConversion@1".into(),
            project_id: source.project_id.clone(),
            source,
            snapshot_revision,
            status: LEGACY_CONVERSION_READY.into(),
            message: "已准备 legacy 只读设计的 Agent 重建输入；原 Concept 版本和模块图不会被修改。"
                .into(),
        })
    }

    pub fn validate(&self) -> CoreResult<()> {
        self.source.validate()?;
        if self.schema_version != "LegacyActiveDesignConversion@1"
            || self.project_id != self.source.project_id
            || self.snapshot_revision == 0
            || self.status != LEGACY_CONVERSION_READY
            || self.message.is_empty()
        {
            return Err(CoreError::invalid_data(
                "LEGACY_CONVERSION_RESPONSE_INVALID",
                "Legacy conversion response is not a complete ready-for-rebuild authorization.",
            ));
        }
        Ok(())
    }
}

/// Durable authorization readback. It contains identity only, never geometry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LegacyAgentConversionIntent {
    pub project_id: String,
    pub legacy_version_id: String,
    pub legacy_module_graph_id: String,
    pub snapshot_revision: u64,
    pub requested_at: String,
}

impl LegacyAgentConversionIntent {
    pub fn validate(&self) -> CoreResult<()> {
        if self.project_id.is_empty()
            || self.legacy_version_id.is_empty()
            || self.legacy_module_graph_id.is_empty()
            || self.snapshot_revision == 0
            || self.requested_at.is_empty()
        {
            return Err(CoreError::invalid_data(
                "LEGACY_CONVERSION_INTENT_INVALID",
                "Legacy conversion intent is incomplete.",
            ));
        }
        Ok(())
    }
}
