use crate::optimization_tools::intent_property;
use serde_json::{json, Map, Value};

const DESIGN_STAGES: [&str; 6] = [
    "reference-canvas",
    "primary-form",
    "secondary-structure",
    "tertiary-detail",
    "uv-pbr",
    "final-review",
];

const BOUNDED_ACTION_KINDS: [&str; 16] = [
    "reference-import",
    "coverage-annotation",
    "mark-unknown",
    "primary-blockout",
    "primary-form-adjustment",
    "secondary-structure",
    "tertiary-detail",
    "material-zone",
    "final-review",
    "request-reference",
    "bounded-repair",
    "checkpoint",
    "rollback",
    "human-review",
    "next-stage",
    "uv-pbr",
];

const ACTION_FIELDS: [&str; 8] = [
    "action_id",
    "action_kind",
    "scope_kind",
    "target_id",
    "operator_id",
    "parameter_changes",
    "bounded",
    "description",
];

const REQUIRED_ACTION_FIELDS: [&str; 7] = [
    "action_id",
    "action_kind",
    "scope_kind",
    "target_id",
    "operator_id",
    "parameter_changes",
    "bounded",
];

const OPERATOR_IDS: [&str; 17] = [
    "forgecad.geometry.primitive@2",
    "forgecad.geometry.profile-extrude@1",
    "forgecad.geometry.profile-loft@1",
    "forgecad.geometry.profile-loft@2",
    "forgecad.geometry.subd-cage@1",
    "forgecad.geometry.surface-patch@1",
    "forgecad.geometry.surface-shell@1",
    "forgecad.geometry.revolve@1",
    "forgecad.geometry.tube-sweep@1",
    "forgecad.geometry.transform@2",
    "forgecad.geometry.mirror@1",
    "forgecad.geometry.array@1",
    "forgecad.geometry.panel@1",
    "forgecad.geometry.vent-array@1",
    "forgecad.geometry.joint-stack@1",
    "forgecad.geometry.boolean@1",
    "forgecad.geometry.part-output@1",
];

const READ_FIELDS: [&str; 4] = ["project_id", "session_id", "candidate_id", "run_id"];

const WRITE_FIELDS: [&str; 17] = [
    "project_id",
    "session_id",
    "candidate_id",
    "run_id",
    "action",
    "input_sha256",
    "requested_stage",
    "approved",
    "approval_receipt_id",
    "approval_summary",
    "approval_expires_at",
    "approval_session_id",
    "idempotency_key",
    "observation_sha256",
    "proposal",
    "optimization_intent",
    "view_spec",
];

const REPAIR_INTENT_RUN_FIELDS: [&str; 19] = [
    "project_id",
    "session_id",
    "candidate_id",
    "run_id",
    "intent_sha256",
    "intent_object_sha256",
    "observation_sha256",
    "action",
    "proposal",
    "requested_stage",
    "input_sha256",
    "approved",
    "approval_receipt_id",
    "approval_summary",
    "approval_expires_at",
    "approval_session_id",
    "idempotency_key",
    "source_evidence_sha256",
    "reference_sha256",
];

const OPTIMIZATION_PROPOSAL_FIELDS: [&str; 13] = [
    "project_id",
    "session_id",
    "candidate_id",
    "run_id",
    "job_id",
    "view_spec",
    "input_sha256",
    "approved",
    "approval_receipt_id",
    "approval_summary",
    "approval_expires_at",
    "approval_session_id",
    "idempotency_key",
];

const REPAIR_APPLY_FIELDS: [&str; 21] = [
    "project_id",
    "session_id",
    "candidate_id",
    "proposal_candidate_id",
    "run_id",
    "source_candidate_state_sha256",
    "intent_sha256",
    "intent_object_sha256",
    "proposal_candidate_state_sha256",
    "prepared_object_id",
    "prepared_object_sha256",
    "quality_report_id",
    "cross_view_evidence_sha256",
    "base_version_id",
    "input_sha256",
    "approved",
    "approval_receipt_id",
    "approval_summary",
    "approval_expires_at",
    "approval_session_id",
    "idempotency_key",
];

const REPAIR_APPLY_CONFIRM_FIELDS: [&str; 13] = [
    "project_id",
    "session_id",
    "candidate_id",
    "proposal_candidate_id",
    "run_id",
    "apply_intent_object_sha256",
    "apply_intent_canonical_sha256",
    "approved",
    "approval_receipt_id",
    "approval_summary",
    "approval_expires_at",
    "approval_session_id",
    "idempotency_key",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgenticActionTool {
    DesignActionRunGet,
    DesignActionRunPrepare,
    DesignActionOptimizationProposalPrepare,
    RepairIntentRunPrepare,
    RepairApplyPrepare,
    RepairApplyConfirm,
}

pub type AgenticTool = AgenticActionTool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Read,
    Write,
}

pub type NameCategory = ToolKind;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Binding {
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub candidate_id: Option<String>,
    pub run_id: Option<String>,
}

impl AgenticActionTool {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "design_action_run_get" => Self::DesignActionRunGet,
            "design_action_run_prepare" => Self::DesignActionRunPrepare,
            "design_action_optimization_proposal_prepare" => {
                Self::DesignActionOptimizationProposalPrepare
            }
            "repair_intent_run_prepare" => Self::RepairIntentRunPrepare,
            "repair_apply_prepare" => Self::RepairApplyPrepare,
            "repair_apply_confirm" => Self::RepairApplyConfirm,
            _ => return None,
        })
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::DesignActionRunGet => "design_action_run_get",
            Self::DesignActionRunPrepare => "design_action_run_prepare",
            Self::DesignActionOptimizationProposalPrepare => {
                "design_action_optimization_proposal_prepare"
            }
            Self::RepairIntentRunPrepare => "repair_intent_run_prepare",
            Self::RepairApplyPrepare => "repair_apply_prepare",
            Self::RepairApplyConfirm => "repair_apply_confirm",
        }
    }

    pub const fn kind(self) -> ToolKind {
        match self {
            Self::DesignActionRunGet => ToolKind::Read,
            Self::DesignActionRunPrepare
            | Self::DesignActionOptimizationProposalPrepare
            | Self::RepairIntentRunPrepare
            | Self::RepairApplyPrepare
            | Self::RepairApplyConfirm => ToolKind::Write,
        }
    }

    pub const fn is_write(self) -> bool {
        matches!(
            self,
            Self::DesignActionRunPrepare
                | Self::DesignActionOptimizationProposalPrepare
                | Self::RepairIntentRunPrepare
                | Self::RepairApplyPrepare
                | Self::RepairApplyConfirm
        )
    }

    pub const fn read_only(self) -> bool {
        !self.is_write()
    }

    pub const fn requires_approval(self) -> bool {
        self.is_write()
    }

    pub const fn destructive(self) -> bool {
        false
    }

    pub const fn idempotent(self) -> bool {
        true
    }

    pub const fn runtime_method(self) -> &'static str {
        self.name()
    }

    pub const fn implemented(self) -> bool {
        true
    }
}

impl ToolKind {
    pub const fn is_read(self) -> bool {
        matches!(self, Self::Read)
    }

    pub const fn is_write(self) -> bool {
        matches!(self, Self::Write)
    }
}

impl Binding {
    pub fn is_bound(&self) -> bool {
        self.session_id.is_some()
            && self.project_id.is_some()
            && self.candidate_id.is_some()
            && self.run_id.is_some()
    }

    pub fn has_scope(&self) -> bool {
        self.session_id.is_some()
            || self.project_id.is_some()
            || self.candidate_id.is_some()
            || self.run_id.is_some()
    }
}

pub fn is_tool(name: &str) -> bool {
    AgenticActionTool::from_name(name).is_some()
}

pub fn is_read_tool(name: &str) -> bool {
    AgenticActionTool::from_name(name).is_some_and(|tool| tool.kind().is_read())
}

pub fn is_write_tool(name: &str) -> bool {
    AgenticActionTool::from_name(name).is_some_and(AgenticActionTool::is_write)
}

pub fn classify_name(name: &str) -> Option<ToolKind> {
    AgenticActionTool::from_name(name).map(AgenticActionTool::kind)
}

pub fn name_category(name: &str) -> Option<NameCategory> {
    classify_name(name)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    AgenticActionTool::from_name(name).map(AgenticActionTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    let tool = AgenticActionTool::from_name(name).expect("action tool name was checked");
    format!(
        "AGENTIC_ACTION_RUNTIME_METHOD_UNAVAILABLE: {} requires Runtime method {}",
        tool.name(),
        tool.runtime_method()
    )
}

pub fn read_tool_names() -> Vec<String> {
    [AgenticActionTool::DesignActionRunGet]
        .into_iter()
        .map(|tool| tool.name().to_owned())
        .collect()
}

pub fn write_tool_names() -> Vec<String> {
    [
        AgenticActionTool::DesignActionRunPrepare,
        AgenticActionTool::DesignActionOptimizationProposalPrepare,
        AgenticActionTool::RepairIntentRunPrepare,
        AgenticActionTool::RepairApplyPrepare,
        AgenticActionTool::RepairApplyConfirm,
    ]
    .into_iter()
    .map(|tool| tool.name().to_owned())
    .collect()
}

pub fn all_tool_names() -> Vec<String> {
    read_tool_names()
        .into_iter()
        .chain(write_tool_names())
        .collect()
}

pub fn read_tools() -> Vec<Value> {
    [AgenticActionTool::DesignActionRunGet]
        .into_iter()
        .map(tool_definition)
        .collect()
}

pub fn write_tools() -> Vec<Value> {
    [
        AgenticActionTool::DesignActionRunPrepare,
        AgenticActionTool::DesignActionOptimizationProposalPrepare,
        AgenticActionTool::RepairIntentRunPrepare,
        AgenticActionTool::RepairApplyPrepare,
        AgenticActionTool::RepairApplyConfirm,
    ]
    .into_iter()
    .map(tool_definition)
    .collect()
}

pub fn all_tools() -> Vec<Value> {
    read_tools().into_iter().chain(write_tools()).collect()
}

pub fn tool_definition_by_name(name: &str) -> Option<Value> {
    AgenticActionTool::from_name(name).map(tool_definition)
}

pub fn input_schema(name: &str) -> Option<Value> {
    AgenticActionTool::from_name(name).map(input_schema_for)
}

pub fn bounded_action_kinds() -> &'static [&'static str] {
    &BOUNDED_ACTION_KINDS
}

pub fn design_stages() -> &'static [&'static str] {
    &DESIGN_STAGES
}

pub fn operator_ids() -> &'static [&'static str] {
    &OPERATOR_IDS
}

fn tool_definition(tool: AgenticActionTool) -> Value {
    let description = match tool {
        AgenticActionTool::DesignActionRunGet => {
            "Read one exact-bound, Runtime-owned DesignActionRun receipt after its bounded execution loop."
        }
        AgenticActionTool::DesignActionRunPrepare => {
            "Execute one bounded, Runtime-owned DesignActionRun over the bound candidate's typed geometry and visual evidence. A single-Part geometry action may provide a bound ReferenceViewSpec and typed parameter_changes; Runtime then materializes one constrained RuntimeParameterPatch and prepares a separate reviewable candidate. An explicit RepairIntent remains supported. Approval is required; it never confirms, exports, or mutates a candidate version."
        }
        AgenticActionTool::DesignActionOptimizationProposalPrepare => {
            "Read one completed ActionRun-bound CADFit OptimizationJob, recompile its strict-improvement GeometryProgram into a separate reviewable candidate, and bind an explicit ReferenceViewSpec comparison. It never changes the source candidate, parent ActionRun, version history, Repair or confirmation state."
        }
        AgenticActionTool::RepairIntentRunPrepare => {
            "Execute one immutable, CAS-bound RepairIntent through the Runtime-owned bounded compile, readback, render and compare loop. The intent and observation hashes are revalidated before a separate review candidate is prepared; source candidate, version history and confirmation remain unchanged."
        }
        AgenticActionTool::RepairApplyPrepare => {
            "Prepare a CAS-backed, replayable Repair application intent by revalidating the source candidate, RepairIntent, proposal candidate and visual evidence. It never mutates the active snapshot/version; final candidate confirmation or cross-view promotion remains a separate user-approved transaction."
        }
        AgenticActionTool::RepairApplyConfirm => {
            "Consume one exact-bound, single-view RepairApplyIntent after fresh user approval, revalidate its source/run/proposal/artifact/visual lineage, and create one immutable version. Multi-view intents fail closed to cross_view_promotion_confirm."
        }
    };

    json!({
        "name": tool.name(),
        "description": description,
        "inputSchema": input_schema_for(tool),
        "annotations": {
            "readOnlyHint": tool.read_only(),
            "destructiveHint": tool.destructive(),
            "idempotentHint": tool.idempotent(),
            "openWorldHint": false,
            "writeIntent": tool.is_write(),
            "approvalRequired": tool.requires_approval()
        },
            "_meta": {"forgecad": {
            "availability": if tool.implemented() { "available" } else { "unavailable" },
            "runtime_method": tool.runtime_method(),
            "requiresConfirmation": tool.requires_approval(),
            "transaction": "ADR-0026",
            "definition_only": !tool.implemented()
        }}
    })
}

fn input_schema_for(tool: AgenticActionTool) -> Value {
    match tool {
        AgenticActionTool::DesignActionRunGet => read_schema(),
        AgenticActionTool::DesignActionRunPrepare => write_schema(),
        AgenticActionTool::DesignActionOptimizationProposalPrepare => {
            optimization_proposal_schema()
        }
        AgenticActionTool::RepairIntentRunPrepare => repair_intent_run_schema(),
        AgenticActionTool::RepairApplyPrepare => repair_apply_schema(),
        AgenticActionTool::RepairApplyConfirm => repair_apply_confirm_schema(),
    }
}

fn read_schema() -> Value {
    object_schema(
        READ_FIELDS.to_vec(),
        Map::from_iter([
            ("project_id".to_owned(), id_property()),
            ("session_id".to_owned(), id_property()),
            ("candidate_id".to_owned(), id_property()),
            ("run_id".to_owned(), id_property()),
        ]),
    )
}

fn write_schema() -> Value {
    object_schema(
        vec![
            "project_id",
            "session_id",
            "candidate_id",
            "run_id",
            "action",
            "input_sha256",
            "requested_stage",
            "approved",
            "approval_receipt_id",
            "approval_summary",
            "approval_expires_at",
            "approval_session_id",
            "observation_sha256",
            "idempotency_key",
        ],
        Map::from_iter([
            ("project_id".to_owned(), id_property()),
            ("session_id".to_owned(), id_property()),
            ("candidate_id".to_owned(), id_property()),
            ("run_id".to_owned(), id_property()),
            ("action".to_owned(), bounded_action_schema()),
            ("input_sha256".to_owned(), sha256_property()),
            ("requested_stage".to_owned(), stage_property()),
            ("approved".to_owned(), json!({"const": true})),
            ("approval_receipt_id".to_owned(), id_property()),
            ("approval_summary".to_owned(), safe_text_property(512)),
            ("approval_expires_at".to_owned(), safe_text_property(64)),
            ("approval_session_id".to_owned(), id_property()),
            ("idempotency_key".to_owned(), id_property()),
            ("observation_sha256".to_owned(), sha256_property()),
            ("proposal".to_owned(), repair_proposal_property()),
            ("optimization_intent".to_owned(), intent_property()),
            ("view_spec".to_owned(), json!({"type":"object"})),
        ]),
    )
}

fn repair_intent_run_schema() -> Value {
    object_schema(
        REPAIR_INTENT_RUN_FIELDS.to_vec(),
        Map::from_iter([
            ("project_id".to_owned(), id_property()),
            ("session_id".to_owned(), id_property()),
            ("candidate_id".to_owned(), id_property()),
            ("run_id".to_owned(), id_property()),
            ("intent_sha256".to_owned(), sha256_property()),
            ("intent_object_sha256".to_owned(), sha256_property()),
            ("observation_sha256".to_owned(), sha256_property()),
            ("source_evidence_sha256".to_owned(), sha256_property()),
            ("reference_sha256".to_owned(), sha256_property()),
            ("action".to_owned(), bounded_action_schema()),
            (
                "proposal".to_owned(),
                repair_proposal_without_intent_property(),
            ),
            ("requested_stage".to_owned(), stage_property()),
            ("input_sha256".to_owned(), sha256_property()),
            ("approved".to_owned(), json!({"const": true})),
            ("approval_receipt_id".to_owned(), id_property()),
            ("approval_summary".to_owned(), safe_text_property(512)),
            ("approval_expires_at".to_owned(), safe_text_property(64)),
            ("approval_session_id".to_owned(), id_property()),
            ("idempotency_key".to_owned(), id_property()),
        ]),
    )
}

fn optimization_proposal_schema() -> Value {
    object_schema(
        OPTIMIZATION_PROPOSAL_FIELDS.to_vec(),
        Map::from_iter([
            ("project_id".to_owned(), id_property()),
            ("session_id".to_owned(), id_property()),
            ("candidate_id".to_owned(), id_property()),
            ("run_id".to_owned(), id_property()),
            ("job_id".to_owned(), id_property()),
            ("view_spec".to_owned(), json!({"type":"object"})),
            ("input_sha256".to_owned(), sha256_property()),
            ("approved".to_owned(), json!({"const": true})),
            ("approval_receipt_id".to_owned(), id_property()),
            ("approval_summary".to_owned(), safe_text_property(512)),
            ("approval_expires_at".to_owned(), safe_text_property(64)),
            ("approval_session_id".to_owned(), id_property()),
            ("idempotency_key".to_owned(), id_property()),
        ]),
    )
}

fn repair_apply_schema() -> Value {
    object_schema(
        REPAIR_APPLY_FIELDS.to_vec(),
        Map::from_iter([
            ("project_id".to_owned(), id_property()),
            ("session_id".to_owned(), id_property()),
            ("candidate_id".to_owned(), id_property()),
            ("proposal_candidate_id".to_owned(), id_property()),
            ("run_id".to_owned(), id_property()),
            (
                "source_candidate_state_sha256".to_owned(),
                sha256_property(),
            ),
            ("intent_sha256".to_owned(), sha256_property()),
            ("intent_object_sha256".to_owned(), sha256_property()),
            (
                "proposal_candidate_state_sha256".to_owned(),
                sha256_property(),
            ),
            ("prepared_object_id".to_owned(), id_property()),
            ("prepared_object_sha256".to_owned(), sha256_property()),
            ("quality_report_id".to_owned(), id_property()),
            (
                "cross_view_evidence_sha256".to_owned(),
                nullable_sha256_property(),
            ),
            ("base_version_id".to_owned(), nullable_id_property()),
            ("input_sha256".to_owned(), sha256_property()),
            ("approved".to_owned(), json!({"const": true})),
            ("approval_receipt_id".to_owned(), id_property()),
            ("approval_summary".to_owned(), safe_text_property(512)),
            ("approval_expires_at".to_owned(), safe_text_property(64)),
            ("approval_session_id".to_owned(), id_property()),
            ("idempotency_key".to_owned(), id_property()),
        ]),
    )
}

fn repair_apply_confirm_schema() -> Value {
    object_schema(
        REPAIR_APPLY_CONFIRM_FIELDS.to_vec(),
        Map::from_iter([
            ("project_id".to_owned(), id_property()),
            ("session_id".to_owned(), id_property()),
            ("candidate_id".to_owned(), id_property()),
            ("proposal_candidate_id".to_owned(), id_property()),
            ("run_id".to_owned(), id_property()),
            ("apply_intent_object_sha256".to_owned(), sha256_property()),
            (
                "apply_intent_canonical_sha256".to_owned(),
                sha256_property(),
            ),
            ("approved".to_owned(), json!({"const": true})),
            ("approval_receipt_id".to_owned(), id_property()),
            ("approval_summary".to_owned(), safe_text_property(512)),
            ("approval_expires_at".to_owned(), safe_text_property(64)),
            ("approval_session_id".to_owned(), id_property()),
            ("idempotency_key".to_owned(), id_property()),
        ]),
    )
}

pub fn bounded_action_schema() -> Value {
    let mut schema = object_schema(
        ACTION_FIELDS.to_vec(),
        Map::from_iter([
            ("action_id".to_owned(), id_property()),
            (
                "action_kind".to_owned(),
                json!({"enum": BOUNDED_ACTION_KINDS}),
            ),
            (
                "scope_kind".to_owned(),
                json!({"enum":["session","part","material-zone","reference"]}),
            ),
            ("target_id".to_owned(), nullable_id_property()),
            ("operator_id".to_owned(), operator_id_property()),
            ("parameter_changes".to_owned(), parameter_changes_property()),
            ("bounded".to_owned(), json!({"const": true})),
            ("description".to_owned(), safe_text_property(512)),
        ]),
    );
    schema["allOf"] = json!([
        {
            "if": {"properties":{"scope_kind":{"const":"session"}},"required":["scope_kind"]},
            "then": {"properties":{"target_id":{"const":null}}}
        },
        {
            "if": {"properties":{"scope_kind":{"enum":["part","material-zone","reference"]}},"required":["scope_kind"]},
            "then": {"properties":{"target_id":id_property()}}
        },
        {
            "if": {"properties":{"action_kind":{"const":"request-reference"}},"required":["action_kind"]},
            "then": {"properties":{"scope_kind":{"const":"reference"}}}
        }
    ]);
    schema
}

fn object_schema(required: Vec<&str>, properties: Map<String, Value>) -> Value {
    json!({
        "type": "object",
        "required": required,
        "properties": properties,
        "additionalProperties": false
    })
}

fn id_property() -> Value {
    json!({
        "type": "string",
        "pattern": "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
    })
}

fn nullable_id_property() -> Value {
    json!({
        "type": ["string", "null"],
        "pattern": "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
    })
}

fn operator_id_property() -> Value {
    let mut allowed = vec![Value::Null];
    allowed.extend(
        OPERATOR_IDS
            .iter()
            .map(|operator_id| Value::String((*operator_id).to_owned())),
    );
    json!({
        "type": ["string", "null"],
        "enum": allowed
    })
}

fn parameter_changes_property() -> Value {
    json!({
        "type": "array",
        "maxItems": 8,
        "uniqueItems": true,
        "items": {
            "type": "object",
            "required": ["parameter_id", "before", "after", "minimum", "maximum", "unit"],
            "properties": {
                "parameter_id": id_property(),
                "before": {"type":"number","minimum":-1000,"maximum":1000},
                "after": {"type":"number","minimum":-1000,"maximum":1000},
                "minimum": {"type":"number","minimum":-1000,"maximum":1000},
                "maximum": {"type":"number","minimum":-1000,"maximum":1000},
                "unit": {"enum":["meter","radian","ratio","count"]}
            },
            "additionalProperties": false
        }
    })
}

fn repair_proposal_property() -> Value {
    json!({
        "oneOf": [
            {
                "type": "object",
                "required": ["repair_intent", "geometry_program", "view_spec", "camera"],
                "properties": {
                    "repair_intent": {"type":"object"},
                    "geometry_program": {"type":"object"},
                    "view_spec": {"type":"object"},
                    "camera": {"type":"object"},
                    "view_evaluations": {
                        "type":"array",
                        "minItems":2,
                        "maxItems":8,
                        "items": {
                            "type":"object",
                            "required":["view_id","reference_id","reference_sha256","view_spec","camera"],
                            "properties": {
                                "view_id": id_property(),
                                "reference_id": id_property(),
                                "reference_sha256": sha256_property(),
                                "view_spec": {"type":"object"},
                                "camera": {"type":"object"}
                            },
                            "additionalProperties": false
                        }
                    }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["parameter_patch", "view_spec", "camera"],
                "properties": {
                    "parameter_patch": {
                        "type": "object",
                        "required": ["schema_version", "strategy"],
                        "properties": {
                            "schema_version": {"const": "RuntimeParameterPatch@1"},
                            "strategy": {"enum": ["primitive-dimensions-v1", "surface-control-points-v1", "hard-surface-finish-v1"]}
                        },
                        "additionalProperties": false
                    },
                    "view_spec": {"type":"object"},
                    "camera": {"type":"object"}
                },
                "additionalProperties": false
            }
        ]
    })
}

fn repair_proposal_without_intent_property() -> Value {
    json!({
        "type": "object",
        "required": ["geometry_program", "view_spec", "camera"],
        "properties": {
            "geometry_program": {"type":"object"},
            "view_spec": {"type":"object"},
            "camera": {"type":"object"},
            "view_evaluations": {
                "type":"array",
                "minItems":2,
                "maxItems":8,
                "items": {
                    "type":"object",
                    "required":["view_id","reference_id","reference_sha256","view_spec","camera"],
                    "properties": {
                        "view_id": id_property(),
                        "reference_id": id_property(),
                        "reference_sha256": sha256_property(),
                        "view_spec": {"type":"object"},
                        "camera": {"type":"object"}
                    },
                    "additionalProperties": false
                }
            }
        },
        "additionalProperties": false
    })
}

fn sha256_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn nullable_sha256_property() -> Value {
    json!({"type":["string","null"],"pattern":"^[0-9a-f]{64}$"})
}

fn stage_property() -> Value {
    json!({"enum": DESIGN_STAGES})
}

fn safe_text_property(max_length: usize) -> Value {
    json!({"type":"string","minLength":1,"maxLength":max_length})
}

pub fn validate_call(name: &str, arguments: &Value, binding: &Binding) -> Result<(), String> {
    let Some(tool) = AgenticActionTool::from_name(name) else {
        return Ok(());
    };
    let object = arguments
        .as_object()
        .ok_or_else(|| "AGENTIC_ACTION_INVALID_INPUT: arguments must be an object".to_owned())?;
    let allowed = match tool {
        AgenticActionTool::DesignActionRunGet => &READ_FIELDS[..],
        AgenticActionTool::DesignActionRunPrepare => &WRITE_FIELDS[..],
        AgenticActionTool::DesignActionOptimizationProposalPrepare => {
            &OPTIMIZATION_PROPOSAL_FIELDS[..]
        }
        AgenticActionTool::RepairIntentRunPrepare => &REPAIR_INTENT_RUN_FIELDS[..],
        AgenticActionTool::RepairApplyPrepare => &REPAIR_APPLY_FIELDS[..],
        AgenticActionTool::RepairApplyConfirm => &REPAIR_APPLY_CONFIRM_FIELDS[..],
    };
    reject_unknown_keys(object, allowed)?;

    if matches!(
        tool,
        AgenticActionTool::RepairApplyPrepare | AgenticActionTool::RepairApplyConfirm
    ) {
        validate_repair_apply(object, binding, tool)?;
    } else if matches!(tool, AgenticActionTool::RepairIntentRunPrepare) {
        validate_scope(object, binding)?;
        validate_repair_intent_run(object)?;
    } else if matches!(
        tool,
        AgenticActionTool::DesignActionOptimizationProposalPrepare
    ) {
        validate_scope(object, binding)?;
        validate_optimization_proposal_prepare(object)?;
    } else {
        validate_scope(object, binding)?;
        if tool.is_write() {
            validate_prepare(object)?;
        }
    }
    Ok(())
}

pub fn validate_parameters(name: &str, arguments: &Value, binding: &Binding) -> Result<(), String> {
    validate_call(name, arguments, binding)
}

pub fn validate_action_run_call(
    name: &str,
    arguments: &Value,
    binding: &Binding,
) -> Result<(), String> {
    validate_call(name, arguments, binding)
}

/// Validate the Runtime response before allowing it to establish the MCP
/// session's action binding.  Runtime is still the authority; this adapter
/// check prevents a malformed or cross-scope response from becoming the
/// client-side binding.
pub fn validate_response(name: &str, value: &Value, binding: &Binding) -> Result<(), String> {
    if !is_tool(name) {
        return Ok(());
    }
    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_ACTION_RUNTIME_OUTPUT_INVALID: response must be an object".to_owned()
    })?;
    for (key, expected) in [
        ("project_id", binding.project_id.as_deref()),
        ("session_id", binding.session_id.as_deref()),
        ("candidate_id", binding.candidate_id.as_deref()),
        ("run_id", binding.run_id.as_deref()),
    ] {
        if let Some(expected) = expected {
            if object.get(key).and_then(Value::as_str) != Some(expected) {
                return Err(format!(
                    "AGENTIC_ACTION_RESPONSE_SCOPE_MISMATCH: {key} differs from bound action"
                ));
            }
        }
    }
    if name == "design_action_run_prepare"
        && object.get("schema_version").and_then(Value::as_str) != Some("DesignActionRun@1")
    {
        return Err(
            "AGENTIC_ACTION_RUNTIME_OUTPUT_INVALID: prepare response is not DesignActionRun@1"
                .to_owned(),
        );
    }
    if name == "design_action_optimization_proposal_prepare"
        && object.get("schema_version").and_then(Value::as_str)
            != Some("OptimizationProposalPrepareResult@1")
    {
        return Err(
                "AGENTIC_ACTION_RUNTIME_OUTPUT_INVALID: optimization proposal response has an invalid schema"
                .to_owned(),
        );
    }
    if name == "repair_intent_run_prepare"
        && (object.get("schema_version").and_then(Value::as_str) != Some("RepairIntentRunResult@1")
            || object.get("confirm_allowed") != Some(&Value::Bool(false))
            || object.get("source_candidate_unchanged") != Some(&Value::Bool(true)))
    {
        return Err(
            "AGENTIC_ACTION_RUNTIME_OUTPUT_INVALID: RepairIntentRun response is not fail-closed"
                .to_owned(),
        );
    }
    if matches!(name, "repair_apply_prepare" | "repair_apply_confirm") {
        if object.get("schema_version").and_then(Value::as_str)
            != Some(if name == "repair_apply_prepare" {
                "RepairApplyPrepareResult@1"
            } else {
                "RepairApplyConfirmResult@1"
            })
            || object.get("source_candidate_id").and_then(Value::as_str)
                != object.get("candidate_id").and_then(Value::as_str)
        {
            return Err(
                "AGENTIC_ACTION_RUNTIME_OUTPUT_INVALID: repair apply response is not source-bound"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

pub fn bind_response(name: &str, value: &Value, binding: &mut Binding) -> Result<(), String> {
    validate_response(name, value, binding)?;
    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_ACTION_RUNTIME_OUTPUT_INVALID: response must be an object".to_owned()
    })?;
    for key in ["project_id", "session_id", "candidate_id", "run_id"] {
        let value = object
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("AGENTIC_ACTION_RUNTIME_OUTPUT_INVALID: {key} is missing"))?;
        if !is_opaque_id(value) {
            return Err(format!(
                "AGENTIC_ACTION_RUNTIME_OUTPUT_INVALID: {key} is malformed"
            ));
        }
        let slot = match key {
            "project_id" => &mut binding.project_id,
            "session_id" => &mut binding.session_id,
            "candidate_id" => &mut binding.candidate_id,
            "run_id" => &mut binding.run_id,
            _ => unreachable!(),
        };
        if slot.as_deref().is_some_and(|expected| expected != value) {
            return Err(format!(
                "AGENTIC_ACTION_RESPONSE_SCOPE_MISMATCH: {key} cannot be rebound"
            ));
        }
        *slot = Some(value.to_owned());
    }
    Ok(())
}

fn validate_scope(object: &Map<String, Value>, binding: &Binding) -> Result<(), String> {
    let project_id = required_id(object, "project_id")?;
    let session_id = required_id(object, "session_id")?;
    let candidate_id = required_id(object, "candidate_id")?;
    let run_id = required_id(object, "run_id")?;

    for (key, expected, actual) in [
        ("project_id", binding.project_id.as_deref(), project_id),
        ("session_id", binding.session_id.as_deref(), session_id),
        (
            "candidate_id",
            binding.candidate_id.as_deref(),
            candidate_id,
        ),
        ("run_id", binding.run_id.as_deref(), run_id),
    ] {
        if let Some(expected) = expected {
            if expected != actual {
                return Err(format!(
                    "AGENTIC_ACTION_SCOPE_MISMATCH: {key} is outside the bound action run"
                ));
            }
        }
    }
    Ok(())
}

fn validate_repair_apply(
    object: &Map<String, Value>,
    binding: &Binding,
    tool: AgenticActionTool,
) -> Result<(), String> {
    validate_scope(object, binding)?;
    required_id(object, "proposal_candidate_id")?;
    if matches!(tool, AgenticActionTool::RepairApplyPrepare) {
        for key in [
            "source_candidate_state_sha256",
            "intent_sha256",
            "intent_object_sha256",
            "proposal_candidate_state_sha256",
            "prepared_object_sha256",
            "quality_report_id",
            "input_sha256",
        ] {
            if key == "quality_report_id" {
                required_id(object, key)?;
            } else {
                required_sha256(object, key)?;
            }
        }
        required_id(object, "prepared_object_id")?;
    } else {
        required_sha256(object, "apply_intent_object_sha256")?;
        required_sha256(object, "apply_intent_canonical_sha256")?;
    }
    if matches!(tool, AgenticActionTool::RepairApplyPrepare) {
        if let Some(value) = object.get("cross_view_evidence_sha256") {
            if !value.is_null() && !value.as_str().is_some_and(is_sha256) {
                return Err(
                    "AGENTIC_REPAIR_APPLY_INVALID_INPUT: cross_view_evidence_sha256 must be null or SHA-256"
                        .to_owned(),
                );
            }
        }
    }
    if object.get("approved") != Some(&Value::Bool(true)) {
        return Err("AGENTIC_REPAIR_APPLY_APPROVAL_REQUIRED: approved=true is required".to_owned());
    }
    for key in [
        "approval_receipt_id",
        "approval_session_id",
        "idempotency_key",
    ] {
        required_id(object, key)?;
    }
    required_safe_text(object, "approval_summary", 512)?;
    required_safe_text(object, "approval_expires_at", 64)?;
    if object.get("approval_session_id") != object.get("session_id") {
        return Err(
            "AGENTIC_REPAIR_APPLY_SCOPE_MISMATCH: approval_session_id must match session_id"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_repair_intent_run(object: &Map<String, Value>) -> Result<(), String> {
    for key in [
        "intent_sha256",
        "intent_object_sha256",
        "observation_sha256",
        "source_evidence_sha256",
        "reference_sha256",
        "input_sha256",
    ] {
        required_sha256(object, key)?;
    }
    let action = object
        .get("action")
        .and_then(Value::as_object)
        .ok_or_else(|| "AGENTIC_REPAIR_INTENT_RUN_ACTION_REQUIRED".to_owned())?;
    if action.get("action_kind").and_then(Value::as_str) != Some("bounded-repair")
        || action.get("scope_kind").and_then(Value::as_str) != Some("part")
        || action.get("target_id").and_then(Value::as_str).is_none()
        || action.get("bounded") != Some(&Value::Bool(true))
    {
        return Err("AGENTIC_REPAIR_INTENT_RUN_ACTION_INVALID".to_owned());
    }
    if !object.get("proposal").is_some_and(Value::is_object) {
        return Err("AGENTIC_REPAIR_INTENT_RUN_PROPOSAL_REQUIRED".to_owned());
    }
    if object.get("approved") != Some(&Value::Bool(true)) {
        return Err("AGENTIC_REPAIR_INTENT_RUN_APPROVAL_REQUIRED".to_owned());
    }
    for key in [
        "approval_receipt_id",
        "approval_session_id",
        "idempotency_key",
    ] {
        required_id(object, key)?;
    }
    required_safe_text(object, "approval_summary", 512)?;
    required_safe_text(object, "approval_expires_at", 64)?;
    if object.get("approval_session_id") != object.get("session_id") {
        return Err(
            "AGENTIC_REPAIR_INTENT_RUN_SCOPE_MISMATCH: approval_session_id must match session_id"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_prepare(object: &Map<String, Value>) -> Result<(), String> {
    let requested_stage = required_stage(object, "requested_stage")?;
    if requested_stage != "primary-form" {
        return Err(
            "AGENTIC_ACTION_STAGE_UNSUPPORTED: requested stage is not executable in this slice; only primary-form is supported"
                .to_owned(),
        );
    }
    let input_sha256 = required_sha256(object, "input_sha256")?;
    if input_sha256.is_empty() {
        return Err("AGENTIC_ACTION_INVALID_INPUT: input_sha256 is required".to_owned());
    }

    if object.get("approved") != Some(&Value::Bool(true)) {
        return Err(
            "AGENTIC_ACTION_APPROVAL_REQUIRED: approved=true is required for action prepare"
                .to_owned(),
        );
    }
    for key in ["approval_receipt_id", "approval_summary", "idempotency_key"] {
        if object.get(key).is_none() {
            return Err(format!(
                "AGENTIC_ACTION_APPROVAL_REQUIRED: {key} is required"
            ));
        }
    }
    let approval_receipt_id = required_id(object, "approval_receipt_id")?;
    let approval_summary = required_safe_text(object, "approval_summary", 512)?;
    let idempotency_key = required_id(object, "idempotency_key")?;
    if approval_receipt_id.is_empty() || idempotency_key.is_empty() {
        return Err(
            "AGENTIC_ACTION_APPROVAL_REQUIRED: approval receipt and idempotency key are required"
                .to_owned(),
        );
    }
    validate_safe_text(approval_summary, "approval_summary")?;

    if let Some(expires_at) = object.get("approval_expires_at") {
        let expires_at = expires_at.as_str().ok_or_else(|| {
            "AGENTIC_ACTION_APPROVAL_REQUIRED: approval_expires_at must be a string".to_owned()
        })?;
        validate_safe_text_bounded(expires_at, "approval_expires_at", 64)?;
    }
    if let Some(approval_session_id) = object.get("approval_session_id") {
        let approval_session_id = approval_session_id.as_str().ok_or_else(|| {
            "AGENTIC_ACTION_APPROVAL_REQUIRED: approval_session_id must be a string".to_owned()
        })?;
        let session_id = required_id(object, "session_id")?;
        if approval_session_id != session_id {
            return Err(
                "AGENTIC_ACTION_SCOPE_MISMATCH: approval_session_id must match session_id"
                    .to_owned(),
            );
        }
        validate_opaque_id(approval_session_id, "approval_session_id")?;
    }
    let action = object
        .get("action")
        .and_then(Value::as_object)
        .ok_or_else(|| "AGENTIC_ACTION_INVALID_INPUT: action must be an object".to_owned())?;
    reject_unknown_keys(action, &ACTION_FIELDS)?;
    for key in REQUIRED_ACTION_FIELDS {
        if !action.contains_key(key) {
            return Err(format!(
                "AGENTIC_ACTION_INVALID_INPUT: action.{key} is required"
            ));
        }
    }

    validate_opaque_id(required_id(action, "action_id")?, "action.action_id")?;
    let action_kind = required_nonempty_string(action, "action_kind")?;
    if !BOUNDED_ACTION_KINDS.contains(&action_kind) {
        return Err(format!(
            "AGENTIC_ACTION_NOT_BOUNDED: action_kind {action_kind} is not allowlisted"
        ));
    }
    let scope_kind = required_nonempty_string(action, "scope_kind")?;
    if !matches!(
        scope_kind,
        "session" | "part" | "material-zone" | "reference"
    ) {
        return Err(
            "AGENTIC_ACTION_INVALID_INPUT: action.scope_kind is not a bounded scope".to_owned(),
        );
    }
    if scope_kind == "session" {
        if action.get("target_id") != Some(&Value::Null) {
            return Err(
                "AGENTIC_ACTION_SCOPE_MISMATCH: session action target_id must be null".to_owned(),
            );
        }
    } else {
        required_id(action, "target_id")?;
    }
    if action_kind == "request-reference" && scope_kind != "reference" {
        return Err(
            "AGENTIC_ACTION_SCOPE_MISMATCH: request-reference must target a reference".to_owned(),
        );
    }
    if action.get("bounded") != Some(&Value::Bool(true)) {
        return Err("AGENTIC_ACTION_NOT_BOUNDED: action.bounded=true is required".to_owned());
    }
    validate_operator_id(action)?;
    validate_parameter_changes(action)?;
    if let Some(description) = action.get("description") {
        let description = description.as_str().ok_or_else(|| {
            "AGENTIC_ACTION_INVALID_INPUT: action.description must be a string".to_owned()
        })?;
        validate_safe_text_bounded(description, "action.description", 512)?;
    }
    let _ = requested_stage;
    Ok(())
}

fn validate_optimization_proposal_prepare(object: &Map<String, Value>) -> Result<(), String> {
    required_id(object, "job_id")?;
    required_sha256(object, "input_sha256")?;
    if object.get("view_spec").and_then(Value::as_object).is_none() {
        return Err("AGENTIC_ACTION_INVALID_INPUT: view_spec must be an object".to_owned());
    }
    if object.get("approved") != Some(&Value::Bool(true)) {
        return Err(
            "AGENTIC_ACTION_APPROVAL_REQUIRED: approved=true is required for optimization proposal prepare"
                .to_owned(),
        );
    }
    for key in ["approval_receipt_id", "approval_summary", "idempotency_key"] {
        if object.get(key).is_none() {
            return Err(format!(
                "AGENTIC_ACTION_APPROVAL_REQUIRED: {key} is required"
            ));
        }
    }
    required_id(object, "approval_receipt_id")?;
    required_safe_text(object, "approval_summary", 512)?;
    required_safe_text(object, "approval_expires_at", 64)?;
    required_id(object, "approval_session_id")?;
    required_id(object, "idempotency_key")?;
    if object.get("approval_session_id") != object.get("session_id") {
        return Err(
            "AGENTIC_ACTION_SCOPE_MISMATCH: approval_session_id must match session_id".to_owned(),
        );
    }
    Ok(())
}

fn validate_operator_id(action: &Map<String, Value>) -> Result<(), String> {
    let Some(operator_id) = action.get("operator_id") else {
        return Err("AGENTIC_ACTION_INVALID_INPUT: action.operator_id is required".to_owned());
    };
    if operator_id.is_null() {
        return Ok(());
    }
    let operator_id = operator_id.as_str().ok_or_else(|| {
        "AGENTIC_ACTION_INVALID_INPUT: action.operator_id must be a string or null".to_owned()
    })?;
    if !OPERATOR_IDS.contains(&operator_id) {
        return Err(format!(
            "AGENTIC_ACTION_NOT_BOUNDED: operator_id {operator_id} is not allowlisted"
        ));
    }
    Ok(())
}

fn validate_parameter_changes(action: &Map<String, Value>) -> Result<(), String> {
    let changes = action
        .get("parameter_changes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_ACTION_INVALID_INPUT: action.parameter_changes must be an array".to_owned()
        })?;
    if changes.len() > 8 {
        return Err(
            "AGENTIC_ACTION_NOT_BOUNDED: parameter_changes may contain at most 8 entries"
                .to_owned(),
        );
    }
    for (index, change) in changes.iter().enumerate() {
        let change = change.as_object().ok_or_else(|| {
            format!(
                "AGENTIC_ACTION_INVALID_INPUT: action.parameter_changes[{index}] must be an object"
            )
        })?;
        const FIELDS: [&str; 6] = [
            "parameter_id",
            "before",
            "after",
            "minimum",
            "maximum",
            "unit",
        ];
        reject_unknown_keys(change, &FIELDS)?;
        for field in FIELDS {
            if !change.contains_key(field) {
                return Err(format!(
                    "AGENTIC_ACTION_INVALID_INPUT: action.parameter_changes[{index}].{field} is required"
                ));
            }
        }
        validate_opaque_id(
            change
                .get("parameter_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    format!(
                        "AGENTIC_ACTION_INVALID_INPUT: action.parameter_changes[{index}].parameter_id is invalid"
                    )
                })?,
            "action.parameter_changes.parameter_id",
        )?;
        let minimum = bounded_number(change, "minimum", index)?;
        let maximum = bounded_number(change, "maximum", index)?;
        let before = bounded_number(change, "before", index)?;
        let after = bounded_number(change, "after", index)?;
        if minimum > maximum
            || before < minimum
            || before > maximum
            || after < minimum
            || after > maximum
        {
            return Err(format!(
                "AGENTIC_ACTION_NOT_BOUNDED: action.parameter_changes[{index}] exceeds its declared bounds"
            ));
        }
        let unit = change.get("unit").and_then(Value::as_str).ok_or_else(|| {
            format!(
                "AGENTIC_ACTION_INVALID_INPUT: action.parameter_changes[{index}].unit is invalid"
            )
        })?;
        if !matches!(unit, "meter" | "radian" | "ratio" | "count") {
            return Err(format!(
                "AGENTIC_ACTION_NOT_BOUNDED: action.parameter_changes[{index}].unit is not allowlisted"
            ));
        }
    }
    Ok(())
}

fn bounded_number(object: &Map<String, Value>, key: &str, index: usize) -> Result<f64, String> {
    let value = object
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| {
            format!(
                "AGENTIC_ACTION_INVALID_INPUT: action.parameter_changes[{index}].{key} must be a number"
            )
        })?;
    if !value.is_finite() || !(-1000.0..=1000.0).contains(&value) {
        return Err(format!(
            "AGENTIC_ACTION_NOT_BOUNDED: action.parameter_changes[{index}].{key} is outside [-1000, 1000]"
        ));
    }
    Ok(value)
}

fn reject_unknown_keys(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    if let Some(key) = object
        .keys()
        .find(|key| !allowed.iter().any(|allowed_key| allowed_key == key))
    {
        return Err(format!("AGENTIC_ACTION_INVALID_INPUT: unknown field {key}"));
    }
    Ok(())
}

fn required_id<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    let value = required_nonempty_string(object, key)?;
    validate_opaque_id(value, key)?;
    Ok(value)
}

fn required_nonempty_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("AGENTIC_ACTION_INVALID_INPUT: {key} is required"))
}

fn required_stage<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    let value = required_nonempty_string(object, key)?;
    if !DESIGN_STAGES.contains(&value) {
        return Err(format!(
            "AGENTIC_ACTION_INVALID_INPUT: {key} is not a valid DesignStage"
        ));
    }
    Ok(value)
}

fn required_sha256<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    let value = required_nonempty_string(object, key)?;
    if !is_sha256(value) {
        return Err(format!(
            "AGENTIC_ACTION_INVALID_INPUT: {key} must be a lowercase SHA-256"
        ));
    }
    Ok(value)
}

fn validate_sha256(object: &Map<String, Value>, key: &str) -> Result<(), String> {
    required_sha256(object, key).map(|_| ())
}

fn required_safe_text<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    max_length: usize,
) -> Result<&'a str, String> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("AGENTIC_ACTION_INVALID_INPUT: {key} is required"))?;
    validate_safe_text_bounded(value, key, max_length)?;
    Ok(value)
}

fn validate_safe_text(value: &str, key: &str) -> Result<(), String> {
    validate_safe_text_bounded(value, key, 512)
}

fn validate_safe_text_bounded(value: &str, key: &str, max_length: usize) -> Result<(), String> {
    let lower = value.to_ascii_lowercase();
    if value.trim().is_empty()
        || value.len() > max_length
        || value.starts_with('/')
        || value.starts_with('\\')
        || lower.contains("://")
        || lower.starts_with("file:")
        || lower.contains("password")
        || lower.contains("api_key")
        || lower.contains("secret")
        || lower.contains("token")
    {
        return Err(format!(
            "AGENTIC_ACTION_INVALID_INPUT: {key} contains empty or unsafe text"
        ));
    }
    Ok(())
}

fn validate_opaque_id(value: &str, key: &str) -> Result<(), String> {
    if !is_opaque_id(value) {
        return Err(format!(
            "AGENTIC_ACTION_INVALID_INPUT: {key} must be an opaque identifier"
        ));
    }
    Ok(())
}

fn is_opaque_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> Binding {
        Binding {
            session_id: Some("session-1".to_owned()),
            project_id: Some("project-1".to_owned()),
            candidate_id: Some("candidate-1".to_owned()),
            run_id: Some("run-1".to_owned()),
        }
    }

    fn action() -> Value {
        json!({
            "action_id": "action-1",
            "action_kind": "bounded-repair",
            "scope_kind": "session",
            "target_id": null,
            "operator_id": null,
            "parameter_changes": [],
            "bounded": true,
            "description": "Prepare one bounded repair for the current stage"
        })
    }

    fn prepare() -> Value {
        json!({
            "project_id": "project-1",
            "session_id": "session-1",
            "candidate_id": "candidate-1",
            "run_id": "run-1",
            "action": action(),
            "input_sha256": "a".repeat(64),
            "requested_stage": "primary-form",
            "approved": true,
            "approval_receipt_id": "approval-1",
            "approval_summary": "Approve one bounded action run",
            "approval_expires_at": "2030-01-01T00:00:00Z",
            "approval_session_id": "session-1",
            "observation_sha256": "b".repeat(64),
            "idempotency_key": "action-run-1"
        })
    }

    fn repair_intent_run_prepare() -> Value {
        json!({
            "project_id":"project-1",
            "session_id":"session-1",
            "candidate_id":"candidate-1",
            "run_id":"run-1",
            "intent_sha256":"a".repeat(64),
            "intent_object_sha256":"b".repeat(64),
            "observation_sha256":"c".repeat(64),
            "source_evidence_sha256":"d".repeat(64),
            "reference_sha256":"e".repeat(64),
            "action":{
                "action_id":"repair-action-1",
                "action_kind":"bounded-repair",
                "scope_kind":"part",
                "target_id":"main-body",
                "operator_id":"forgecad.geometry.transform@2",
                "parameter_changes":[{
                    "parameter_id":"body-width",
                    "before":1.0,
                    "after":1.05,
                    "minimum":0.5,
                    "maximum":1.5,
                    "unit":"ratio"
                }],
                "bounded":true,
                "description":"Adjust one bounded body parameter"
            },
            "proposal":{"geometry_program":{},"view_spec":{},"camera":{}},
            "requested_stage":"primary-form",
            "input_sha256":"f".repeat(64),
            "approved":true,
            "approval_receipt_id":"approval-repair-1",
            "approval_summary":"Approve one CAS-bound bounded RepairIntent run",
            "approval_expires_at":"2030-01-01T00:00:00Z",
            "approval_session_id":"session-1",
            "idempotency_key":"repair-intent-run-1"
        })
    }

    fn repair_apply_prepare() -> Value {
        json!({
            "project_id": "project-1",
            "session_id": "session-1",
            "candidate_id": "candidate-1",
            "proposal_candidate_id": "candidate-2",
            "run_id": "run-1",
            "source_candidate_state_sha256": "a".repeat(64),
            "intent_sha256": "b".repeat(64),
            "intent_object_sha256": "c".repeat(64),
            "proposal_candidate_state_sha256": "d".repeat(64),
            "prepared_object_id": "artifact-1",
            "prepared_object_sha256": "e".repeat(64),
            "quality_report_id": "quality-1",
            "cross_view_evidence_sha256": null,
            "base_version_id": null,
            "input_sha256": "f".repeat(64),
            "approved": true,
            "approval_receipt_id": "approval-apply-1",
            "approval_summary": "Approve one bounded Repair apply preparation",
            "approval_expires_at": "2030-01-01T00:00:00Z",
            "approval_session_id": "session-1",
            "idempotency_key": "repair-apply-1"
        })
    }

    fn repair_apply_confirm() -> Value {
        json!({
            "project_id": "project-1",
            "session_id": "session-1",
            "candidate_id": "candidate-1",
            "proposal_candidate_id": "candidate-2",
            "run_id": "run-1",
            "apply_intent_object_sha256": "a".repeat(64),
            "apply_intent_canonical_sha256": "b".repeat(64),
            "approved": true,
            "approval_receipt_id": "approval-confirm-1",
            "approval_summary": "Consume one approved single-view Repair intent",
            "approval_expires_at": "2030-01-01T00:00:00Z",
            "approval_session_id": "session-1",
            "idempotency_key": "repair-confirm-1"
        })
    }

    #[test]
    fn definitions_keep_read_and_approval_boundaries_distinct() {
        let read = read_tools();
        let write = write_tools();
        assert_eq!(read.len(), 1);
        assert_eq!(write.len(), 5);
        assert_eq!(read[0]["name"], "design_action_run_get");
        assert_eq!(write[0]["name"], "design_action_run_prepare");
        assert_eq!(
            write[1]["name"],
            "design_action_optimization_proposal_prepare"
        );
        assert_eq!(write[2]["name"], "repair_intent_run_prepare");
        assert_eq!(write[3]["name"], "repair_apply_prepare");
        assert_eq!(write[4]["name"], "repair_apply_confirm");
        assert_eq!(read[0]["annotations"]["readOnlyHint"], true);
        assert_eq!(read[0]["annotations"]["writeIntent"], false);
        assert_eq!(read[0]["annotations"]["approvalRequired"], false);
        assert_eq!(read[0]["annotations"]["destructiveHint"], false);
        assert_eq!(write[0]["annotations"]["readOnlyHint"], false);
        assert_eq!(write[0]["annotations"]["writeIntent"], true);
        assert_eq!(write[0]["annotations"]["approvalRequired"], true);
        assert_eq!(write[0]["annotations"]["destructiveHint"], false);
    }

    #[test]
    fn schemas_require_scope_action_hash_stage_and_approval() {
        let read_tools = read_tools();
        let read_required = read_tools[0]["inputSchema"]["required"]
            .as_array()
            .expect("read required");
        for key in READ_FIELDS {
            assert!(read_required.iter().any(|value| value == key));
        }
        let write_schema = &write_tools()[0]["inputSchema"];
        assert_eq!(write_schema["additionalProperties"], false);
        for key in [
            "project_id",
            "session_id",
            "candidate_id",
            "run_id",
            "action",
            "input_sha256",
            "requested_stage",
            "approved",
            "approval_receipt_id",
            "approval_summary",
            "approval_expires_at",
            "approval_session_id",
            "observation_sha256",
            "idempotency_key",
        ] {
            assert!(write_schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == key));
        }
        assert_eq!(
            write_schema["properties"]["action"]["additionalProperties"],
            false
        );
        assert_eq!(
            write_schema["properties"]["action"]["properties"]["bounded"]["const"],
            true
        );
        assert_eq!(write_schema["properties"]["view_spec"]["type"], "object");
    }

    #[test]
    fn repair_proposal_schema_exposes_runtime_owned_parameter_patch_variant() {
        let schema = repair_proposal_property();
        let variants = schema["oneOf"]
            .as_array()
            .expect("repair proposal variants");
        assert_eq!(variants.len(), 2);
        assert!(variants[0]["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "geometry_program"));
        assert_eq!(
            variants[1]["properties"]["parameter_patch"]["properties"]["schema_version"]["const"],
            "RuntimeParameterPatch@1"
        );
        let strategies = variants[1]["properties"]["parameter_patch"]["properties"]["strategy"]
            ["enum"]
            .as_array()
            .expect("runtime parameter patch strategies");
        assert!(strategies
            .iter()
            .any(|value| value == "primitive-dimensions-v1"));
        assert!(strategies
            .iter()
            .any(|value| value == "surface-control-points-v1"));
        assert!(strategies
            .iter()
            .any(|value| value == "hard-surface-finish-v1"));
        assert_eq!(variants[1]["additionalProperties"], false);
    }

    #[test]
    fn valid_read_and_prepare_calls_pass_for_the_same_binding() {
        let read = json!({
            "project_id": "project-1",
            "session_id": "session-1",
            "candidate_id": "candidate-1",
            "run_id": "run-1"
        });
        assert!(validate_call("design_action_run_get", &read, &binding()).is_ok());
        assert!(validate_call("design_action_run_prepare", &prepare(), &binding()).is_ok());
        assert!(validate_call(
            "repair_intent_run_prepare",
            &repair_intent_run_prepare(),
            &binding()
        )
        .is_ok());
    }

    #[test]
    fn optimization_proposal_prepare_requires_explicit_view_and_approval() {
        let view_spec = json!({
            "schema_version":"ReferenceViewSpec@1",
            "reference_id":"reference-1",
            "reference_sha256":"a".repeat(64),
            "view_id":"view-1"
        });
        let input_binding = json!({
            "project_id":"project-1",
            "session_id":"session-1",
            "candidate_id":"candidate-1",
            "run_id":"run-1",
            "job_id":"job-1",
            "view_spec":view_spec,
            "idempotency_key":"optimization-proposal-1"
        });
        let mut request = json!({
            "project_id":"project-1",
            "session_id":"session-1",
            "candidate_id":"candidate-1",
            "run_id":"run-1",
            "job_id":"job-1",
            "view_spec":input_binding["view_spec"].clone(),
            "input_sha256":forgecad_runtime::canonical_json_hash(&input_binding),
            "approved":true,
            "approval_receipt_id":"approval-optimization-proposal-1",
            "approval_summary":"Materialize one bounded optimizer proposal for review",
            "approval_expires_at":"2030-01-01T00:00:00Z",
            "approval_session_id":"session-1",
            "idempotency_key":"optimization-proposal-1"
        });
        assert!(validate_call(
            "design_action_optimization_proposal_prepare",
            &request,
            &binding()
        )
        .is_ok());
        request["approved"] = Value::Bool(false);
        assert!(validate_call(
            "design_action_optimization_proposal_prepare",
            &request,
            &binding()
        )
        .unwrap_err()
        .contains("APPROVAL_REQUIRED"));
    }

    #[test]
    fn repair_apply_prepare_requires_approval_and_exact_scope() {
        assert!(validate_call("repair_apply_prepare", &repair_apply_prepare(), &binding()).is_ok());

        let mut not_approved = repair_apply_prepare();
        not_approved["approved"] = Value::Bool(false);
        assert!(
            validate_call("repair_apply_prepare", &not_approved, &binding())
                .unwrap_err()
                .contains("APPROVAL_REQUIRED")
        );

        let mut cross_session = repair_apply_prepare();
        cross_session["approval_session_id"] = Value::String("session-2".to_owned());
        assert!(
            validate_call("repair_apply_prepare", &cross_session, &binding())
                .unwrap_err()
                .contains("SCOPE_MISMATCH")
        );

        let mut malformed_evidence = repair_apply_prepare();
        malformed_evidence["cross_view_evidence_sha256"] = Value::String("not-a-hash".to_owned());
        assert!(
            validate_call("repair_apply_prepare", &malformed_evidence, &binding())
                .unwrap_err()
                .contains("INVALID_INPUT")
        );
    }

    #[test]
    fn repair_apply_confirm_requires_intent_hashes_and_fresh_approval() {
        assert!(validate_call("repair_apply_confirm", &repair_apply_confirm(), &binding()).is_ok());

        let mut not_approved = repair_apply_confirm();
        not_approved["approved"] = Value::Bool(false);
        assert!(
            validate_call("repair_apply_confirm", &not_approved, &binding())
                .unwrap_err()
                .contains("APPROVAL_REQUIRED")
        );

        let mut malformed_intent = repair_apply_confirm();
        malformed_intent["apply_intent_object_sha256"] = Value::String("not-a-hash".to_owned());
        assert!(validate_call("repair_apply_confirm", &malformed_intent, &binding()).is_err());

        let mut cross_session = repair_apply_confirm();
        cross_session["approval_session_id"] = Value::String("session-2".to_owned());
        assert!(
            validate_call("repair_apply_confirm", &cross_session, &binding())
                .unwrap_err()
                .contains("SCOPE_MISMATCH")
        );
    }

    #[test]
    fn unknown_fields_empty_values_and_scope_drift_fail_closed() {
        let mut unknown = prepare();
        unknown["unexpected"] = Value::String("nope".to_owned());
        assert!(
            validate_call("design_action_run_prepare", &unknown, &binding())
                .unwrap_err()
                .contains("unknown field")
        );

        let mut empty = prepare();
        empty["run_id"] = Value::String("   ".to_owned());
        assert!(validate_call("design_action_run_prepare", &empty, &binding()).is_err());

        let mut cross_project = prepare();
        cross_project["project_id"] = Value::String("project-2".to_owned());
        assert!(
            validate_call("design_action_run_prepare", &cross_project, &binding())
                .unwrap_err()
                .contains("SCOPE_MISMATCH")
        );

        let mut cross_approval = prepare();
        cross_approval["approval_session_id"] = Value::String("session-2".to_owned());
        assert!(
            validate_call("design_action_run_prepare", &cross_approval, &binding())
                .unwrap_err()
                .contains("SCOPE_MISMATCH")
        );
    }

    #[test]
    fn stage_hash_approval_and_bounded_action_guards_fail_closed() {
        let mut stage_drift = prepare();
        stage_drift["requested_stage"] = Value::String("not-a-design-stage".to_owned());
        assert!(
            validate_call("design_action_run_prepare", &stage_drift, &binding())
                .unwrap_err()
                .contains("stage")
        );

        let mut bad_hash = prepare();
        bad_hash["input_sha256"] = Value::String("A".repeat(64));
        assert!(validate_call("design_action_run_prepare", &bad_hash, &binding()).is_err());

        let mut not_approved = prepare();
        not_approved["approved"] = Value::Bool(false);
        assert!(
            validate_call("design_action_run_prepare", &not_approved, &binding())
                .unwrap_err()
                .contains("APPROVAL_REQUIRED")
        );

        let mut not_bounded = prepare();
        not_bounded["action"]["bounded"] = Value::Bool(false);
        assert!(
            validate_call("design_action_run_prepare", &not_bounded, &binding())
                .unwrap_err()
                .contains("NOT_BOUNDED")
        );

        let mut dangerous_kind = prepare();
        dangerous_kind["action"]["action_kind"] = Value::String("confirm".to_owned());
        assert!(
            validate_call("design_action_run_prepare", &dangerous_kind, &binding())
                .unwrap_err()
                .contains("NOT_BOUNDED")
        );
    }

    #[test]
    fn names_are_partitioned_without_unknown_aliases() {
        assert!(is_tool("design_action_run_get"));
        assert!(is_read_tool("design_action_run_get"));
        assert!(!is_write_tool("design_action_run_get"));
        assert!(is_write_tool("design_action_run_prepare"));
        assert_eq!(classify_name("unknown"), None);
        assert_eq!(
            all_tool_names(),
            vec![
                "design_action_run_get".to_owned(),
                "design_action_run_prepare".to_owned(),
                "design_action_optimization_proposal_prepare".to_owned(),
                "repair_intent_run_prepare".to_owned(),
                "repair_apply_prepare".to_owned(),
                "repair_apply_confirm".to_owned()
            ]
        );
    }
}
