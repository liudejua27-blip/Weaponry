use serde_json::{json, Value};

/// Read-only names reserved for the Agentic Design Runtime projection.
///
/// The enum keeps the MCP name, the Runtime method and the availability
/// decision together so a target projection cannot accidentally acquire a
/// fabricated fallback payload in the adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgenticReadTool {
    SceneObserve,
    StagePlan,
    CriticReport,
    VisualEvidenceBundle,
}

impl AgenticReadTool {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "scene_observe_get" => Self::SceneObserve,
            "design_stage_plan_get" => Self::StagePlan,
            "critic_report_get" => Self::CriticReport,
            "visual_evidence_bundle_get" => Self::VisualEvidenceBundle,
            _ => return None,
        })
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::SceneObserve => "scene_observe_get",
            Self::StagePlan => "design_stage_plan_get",
            Self::CriticReport => "critic_report_get",
            Self::VisualEvidenceBundle => "visual_evidence_bundle_get",
        }
    }

    /// Keep the MCP alias separate from the Runtime method so the adapter does
    /// not acquire any Runtime state or implementation dependency.
    pub const fn runtime_method(self) -> Option<&'static str> {
        match self {
            Self::SceneObserve => Some("agentic_scene_observe"),
            Self::StagePlan => Some("agentic_stage_plan"),
            Self::CriticReport => Some("agentic_critic_projection"),
            Self::VisualEvidenceBundle => Some("visual_evidence_get"),
        }
    }

    pub const fn available(self) -> bool {
        self.runtime_method().is_some()
    }

    pub const fn expected_runtime_method(self) -> &'static str {
        match self {
            Self::SceneObserve => "agentic_scene_observe",
            Self::StagePlan => "agentic_stage_plan",
            Self::CriticReport => "agentic_critic_projection",
            Self::VisualEvidenceBundle => "visual_evidence_get",
        }
    }

    pub const fn source_schema(self) -> Option<&'static str> {
        match self {
            Self::SceneObserve => Some("AgenticSceneObserveResult@1"),
            Self::StagePlan => Some("DesignStagePlan@1"),
            Self::CriticReport => Some("DesignCriticReport@1"),
            Self::VisualEvidenceBundle => Some("ViewerVisualEvidence@1"),
        }
    }
}

pub fn is_tool(name: &str) -> bool {
    AgenticReadTool::from_name(name).is_some()
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    AgenticReadTool::from_name(name).and_then(AgenticReadTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    let tool = AgenticReadTool::from_name(name).expect("agentic tool name was checked");
    format!(
        "CAPABILITY_UNAVAILABLE: {} requires Runtime producer {}",
        tool.name(),
        tool.expected_runtime_method()
    )
}

pub fn read_tools() -> Vec<Value> {
    [
        AgenticReadTool::SceneObserve,
        AgenticReadTool::StagePlan,
        AgenticReadTool::CriticReport,
        AgenticReadTool::VisualEvidenceBundle,
    ]
    .into_iter()
    .map(tool_definition)
    .collect()
}

fn tool_definition(tool: AgenticReadTool) -> Value {
    let (description, schema) = match tool {
        AgenticReadTool::SceneObserve => (
            "Read the canonical one-shot Runtime-owned Agentic scene observation. It returns the bound scene graph, model understanding, reference canvas, visual evidence, QualityReport projection, stage plan and critic together; use it as the primary observation call instead of repeatedly reconstructing state from fragmented reads. The projection is derived on demand, read-only, and never falls back to a fabricated scene graph or creates a checkpoint, candidate, version, or CAS object.",
            project_candidate_schema(),
        ),
        AgenticReadTool::StagePlan => (
            "Read the Runtime-owned Agentic design stage plan. Stage unlocks are fail-closed on the available evidence and this tool never advances a stage, creates a checkpoint, or invokes approval.",
            project_candidate_schema(),
        ),
        AgenticReadTool::CriticReport => (
            "Read the Runtime-owned evidence-bound critic projection. It returns DesignCriticReport@1 with bounded repair intents for inspection only; execution remains in the existing prepare and approval flow.",
            project_candidate_schema(),
        ),
        AgenticReadTool::VisualEvidenceBundle => (
            "Read the existing candidate-bound visual evidence projection. It returns ViewerVisualEvidence@1 from the Runtime and does not create a render, candidate, version, or CAS object.",
            candidate_schema(),
        ),
    };
    let mut forgecad = json!({
        "availability": if tool.available() { "available" } else { "unavailable" },
        "read_only_projection": true,
        "runtime_method": tool.expected_runtime_method(),
    });
    if let Some(source_schema) = tool.source_schema() {
        forgecad["source_schema"] = Value::String(source_schema.to_owned());
    }
    json!({
        "name": tool.name(),
        "description": description,
        "inputSchema": schema,
        "annotations": {
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        },
        "_meta": {"forgecad": forgecad}
    })
}

fn candidate_schema() -> Value {
    json!({
        "type": "object",
        "required": ["candidate_id"],
        "properties": {"candidate_id": id_property()},
        "additionalProperties": false
    })
}

fn project_candidate_schema() -> Value {
    json!({
        "type": "object",
        "required": ["project_id"],
        "properties": {
            "project_id": id_property(),
            "candidate_id": id_property()
        },
        "additionalProperties": false
    })
}

fn id_property() -> Value {
    json!({"type": "string", "minLength": 1, "maxLength": 128})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_projections_are_read_only_and_availability_is_explicit() {
        let tools = read_tools();
        assert_eq!(tools.len(), 4);
        for tool in &tools {
            assert_eq!(tool["annotations"]["readOnlyHint"], true);
            assert_eq!(tool["annotations"]["destructiveHint"], false);
            assert_eq!(tool["annotations"]["idempotentHint"], true);
            assert_eq!(tool["annotations"]["openWorldHint"], false);
            assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        }
        assert!(AgenticReadTool::from_name("scene_observe_get")
            .expect("scene tool")
            .available());
        assert!(AgenticReadTool::from_name("design_stage_plan_get")
            .expect("stage tool")
            .available());
        assert_eq!(
            AgenticReadTool::from_name("scene_observe_get")
                .expect("scene tool")
                .runtime_method(),
            Some("agentic_scene_observe")
        );
        assert_eq!(
            AgenticReadTool::from_name("design_stage_plan_get")
                .expect("stage tool")
                .runtime_method(),
            Some("agentic_stage_plan")
        );
        assert_eq!(
            AgenticReadTool::from_name("critic_report_get")
                .expect("critic tool")
                .runtime_method(),
            Some("agentic_critic_projection")
        );
        assert_eq!(
            AgenticReadTool::from_name("visual_evidence_bundle_get")
                .expect("evidence tool")
                .runtime_method(),
            Some("visual_evidence_get")
        );
        assert_eq!(
            AgenticReadTool::from_name("critic_report_get")
                .expect("critic tool")
                .source_schema(),
            Some("DesignCriticReport@1")
        );
    }
}
