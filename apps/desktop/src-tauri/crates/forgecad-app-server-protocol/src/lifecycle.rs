use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{valid_stable_id, RpcError};

const MAX_CLIENT_REQUEST_ID_CHARS: usize = 120;
const MAX_SHORT_TEXT_CHARS: usize = 160;
const MAX_PROVIDER_ID_CHARS: usize = 120;
const MAX_ACTION_CHARS: usize = 120;
const MAX_REQUEST_TEXT_CHARS: usize = 8_000;
const MAX_SUMMARY_TEXT_CHARS: usize = 8_000;
const MAX_ERROR_MESSAGE_CHARS: usize = 8_000;
const MAX_APPROVAL_NOTE_CHARS: usize = 1_000;
const MAX_TIMESTAMP_CHARS: usize = 64;

fn invalid_field(field: &str, requirement: impl AsRef<str>) -> RpcError {
    RpcError::invalid_params(format!("{field} {}.", requirement.as_ref()))
}

fn validate_stable_id(field: &str, value: &str) -> Result<(), RpcError> {
    if !valid_stable_id(value) {
        return Err(invalid_field(
            field,
            "must be a stable ID containing 1 to 160 ASCII letters, digits, '_', '-' or '.'",
        ));
    }
    Ok(())
}

fn validate_stable_id_with_max(field: &str, value: &str, max_chars: usize) -> Result<(), RpcError> {
    validate_stable_id(field, value)?;
    if value.chars().count() > max_chars {
        return Err(invalid_field(
            field,
            format!("must contain at most {max_chars} characters"),
        ));
    }
    Ok(())
}

fn validate_optional_stable_id(field: &str, value: Option<&str>) -> Result<(), RpcError> {
    if let Some(value) = value {
        validate_stable_id(field, value)?;
    }
    Ok(())
}

fn validate_text(
    field: &str,
    value: &str,
    min_chars: usize,
    max_chars: usize,
) -> Result<(), RpcError> {
    let count = value.chars().count();
    if !(min_chars..=max_chars).contains(&count) {
        return Err(invalid_field(
            field,
            format!("must contain between {min_chars} and {max_chars} characters"),
        ));
    }
    Ok(())
}

fn validate_timestamp(field: &str, value: &str) -> Result<(), RpcError> {
    validate_text(field, value, 1, MAX_TIMESTAMP_CHARS)?;
    if !value.is_ascii() || value.chars().any(char::is_control) {
        return Err(invalid_field(
            field,
            "must be bounded printable ASCII timestamp text",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentThreadStatus {
    Idle,
    Active,
    Error,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentTurnStatus {
    Queued,
    Running,
    WaitingForApproval,
    WaitingForClarification,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentItemType {
    UserMessage,
    AssistantMessage,
    Plan,
    ToolCall,
    ToolResult,
    Preview,
    ApprovalRequest,
    Clarification,
    Artifact,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentItemStatus {
    Pending,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentItem {
    pub item_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub sequence: u64,
    pub item_type: AgentItemType,
    pub status: AgentItemStatus,
    #[serde(default)]
    pub payload: BTreeMap<String, Value>,
    pub created_at: String,
}

impl AgentItem {
    pub fn validate(&self) -> Result<(), RpcError> {
        validate_stable_id("item.item_id", &self.item_id)?;
        validate_stable_id("item.thread_id", &self.thread_id)?;
        validate_stable_id("item.turn_id", &self.turn_id)?;
        if self.sequence == 0 {
            return Err(invalid_field("item.sequence", "must be greater than zero"));
        }
        validate_timestamp("item.created_at", &self.created_at)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentApproval {
    pub approval_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub action: String,
    pub status: ApprovalStatus,
    #[serde(default)]
    pub payload: BTreeMap<String, Value>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<String>,
}

impl AgentApproval {
    pub fn validate(&self) -> Result<(), RpcError> {
        validate_stable_id("approval.approval_id", &self.approval_id)?;
        validate_stable_id("approval.thread_id", &self.thread_id)?;
        validate_stable_id("approval.turn_id", &self.turn_id)?;
        validate_stable_id("approval.item_id", &self.item_id)?;
        validate_text("approval.action", &self.action, 1, MAX_ACTION_CHARS)?;
        validate_timestamp("approval.created_at", &self.created_at)?;
        if let Some(resolved_at) = self.resolved_at.as_deref() {
            validate_timestamp("approval.resolved_at", resolved_at)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentTurn {
    pub turn_id: String,
    pub thread_id: String,
    pub request_text: String,
    pub status: AgentTurnStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(default)]
    pub usage: BTreeMap<String, Value>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub items: Vec<AgentItem>,
    #[serde(default)]
    pub approvals: Vec<AgentApproval>,
}

impl AgentTurn {
    pub fn validate(&self) -> Result<(), RpcError> {
        validate_stable_id("turn.turn_id", &self.turn_id)?;
        validate_stable_id("turn.thread_id", &self.thread_id)?;
        validate_text(
            "turn.request_text",
            &self.request_text,
            1,
            MAX_REQUEST_TEXT_CHARS,
        )?;
        if let Some(error_code) = self.error_code.as_deref() {
            validate_stable_id("turn.error_code", error_code)?;
        }
        if let Some(error_message) = self.error_message.as_deref() {
            validate_text(
                "turn.error_message",
                error_message,
                0,
                MAX_ERROR_MESSAGE_CHARS,
            )?;
        }
        validate_timestamp("turn.created_at", &self.created_at)?;
        validate_timestamp("turn.updated_at", &self.updated_at)?;

        let mut previous_sequence = 0;
        let mut item_ids = BTreeSet::new();
        for item in &self.items {
            item.validate()?;
            if item.thread_id != self.thread_id || item.turn_id != self.turn_id {
                return Err(invalid_field(
                    "turn.items",
                    "must preserve the parent thread_id and turn_id",
                ));
            }
            if item.sequence <= previous_sequence {
                return Err(invalid_field(
                    "turn.items.sequence",
                    "must be strictly increasing",
                ));
            }
            previous_sequence = item.sequence;
            if !item_ids.insert(item.item_id.as_str()) {
                return Err(invalid_field("turn.items.item_id", "must be unique"));
            }
        }

        let mut approval_ids = BTreeSet::new();
        for approval in &self.approvals {
            approval.validate()?;
            if approval.thread_id != self.thread_id || approval.turn_id != self.turn_id {
                return Err(invalid_field(
                    "turn.approvals",
                    "must preserve the parent thread_id and turn_id",
                ));
            }
            let approval_item = self
                .items
                .iter()
                .find(|item| item.item_id == approval.item_id)
                .ok_or_else(|| {
                    invalid_field(
                        "turn.approvals.item_id",
                        "must reference an item in the same turn",
                    )
                })?;
            if approval_item.item_type != AgentItemType::ApprovalRequest {
                return Err(invalid_field(
                    "turn.approvals.item_id",
                    "must reference an approval_request item",
                ));
            }
            if !approval_ids.insert(approval.approval_id.as_str()) {
                return Err(invalid_field(
                    "turn.approvals.approval_id",
                    "must be unique",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentThreadSummary {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub title: String,
    pub status: AgentThreadStatus,
    pub summary: String,
    pub provider_id: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_turn_id: Option<String>,
}

impl AgentThreadSummary {
    pub fn validate(&self) -> Result<(), RpcError> {
        validate_stable_id("thread.thread_id", &self.thread_id)?;
        validate_optional_stable_id("thread.project_id", self.project_id.as_deref())?;
        validate_text("thread.title", &self.title, 1, MAX_SHORT_TEXT_CHARS)?;
        validate_text("thread.summary", &self.summary, 0, MAX_SUMMARY_TEXT_CHARS)?;
        validate_stable_id_with_max(
            "thread.provider_id",
            &self.provider_id,
            MAX_PROVIDER_ID_CHARS,
        )?;
        validate_timestamp("thread.created_at", &self.created_at)?;
        validate_timestamp("thread.updated_at", &self.updated_at)?;
        validate_optional_stable_id("thread.last_turn_id", self.last_turn_id.as_deref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentThreadDetail {
    #[serde(flatten)]
    pub summary: AgentThreadSummary,
    #[serde(default)]
    pub turns: Vec<AgentTurn>,
}

impl AgentThreadDetail {
    pub fn validate(&self) -> Result<(), RpcError> {
        self.summary.validate()?;
        let mut turn_ids = BTreeSet::new();
        for turn in &self.turns {
            turn.validate()?;
            if turn.thread_id != self.summary.thread_id {
                return Err(invalid_field(
                    "thread.turns.thread_id",
                    "must preserve the parent thread_id",
                ));
            }
            if !turn_ids.insert(turn.turn_id.as_str()) {
                return Err(invalid_field("thread.turns.turn_id", "must be unique"));
            }
        }
        if let Some(last_turn_id) = self.summary.last_turn_id.as_deref() {
            if !turn_ids.contains(last_turn_id) {
                return Err(invalid_field(
                    "thread.last_turn_id",
                    "must reference a turn in thread.turns",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateAgentThreadRequest {
    pub client_request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
}

impl CreateAgentThreadRequest {
    pub fn validate(&self) -> Result<(), RpcError> {
        validate_stable_id_with_max(
            "create_thread.client_request_id",
            &self.client_request_id,
            MAX_CLIENT_REQUEST_ID_CHARS,
        )?;
        validate_optional_stable_id("create_thread.project_id", self.project_id.as_deref())?;
        if let Some(title) = self.title.as_deref() {
            validate_text("create_thread.title", title, 1, MAX_SHORT_TEXT_CHARS)?;
        }
        if let Some(provider_id) = self.provider_id.as_deref() {
            validate_stable_id_with_max(
                "create_thread.provider_id",
                provider_id,
                MAX_PROVIDER_ID_CHARS,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StartAgentTurnRequest {
    pub client_request_id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clarification_domain_pack_id: Option<String>,
}

impl StartAgentTurnRequest {
    pub fn validate(&self) -> Result<(), RpcError> {
        validate_stable_id_with_max(
            "start_turn.client_request_id",
            &self.client_request_id,
            MAX_CLIENT_REQUEST_ID_CHARS,
        )?;
        validate_text(
            "start_turn.message",
            &self.message,
            1,
            MAX_REQUEST_TEXT_CHARS,
        )?;
        validate_optional_stable_id(
            "start_turn.clarification_domain_pack_id",
            self.clarification_domain_pack_id.as_deref(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CreateAgentApprovalRequest {
    pub client_request_id: String,
    pub turn_id: String,
    pub action: String,
    #[serde(default)]
    pub payload: BTreeMap<String, Value>,
}

impl CreateAgentApprovalRequest {
    pub fn validate(&self) -> Result<(), RpcError> {
        validate_stable_id_with_max(
            "create_approval.client_request_id",
            &self.client_request_id,
            MAX_CLIENT_REQUEST_ID_CHARS,
        )?;
        validate_stable_id("create_approval.turn_id", &self.turn_id)?;
        validate_text("create_approval.action", &self.action, 1, MAX_ACTION_CHARS)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResolveAgentApprovalRequest {
    pub client_request_id: String,
    pub decision: ApprovalDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl ResolveAgentApprovalRequest {
    pub fn validate(&self) -> Result<(), RpcError> {
        validate_stable_id_with_max(
            "resolve_approval.client_request_id",
            &self.client_request_id,
            MAX_CLIENT_REQUEST_ID_CHARS,
        )?;
        if let Some(note) = self.note.as_deref() {
            validate_text("resolve_approval.note", note, 0, MAX_APPROVAL_NOTE_CHARS)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentEvent {
    pub sequence: u64,
    pub thread_id: String,
    pub turn_id: String,
    pub item: AgentItem,
}

impl AgentEvent {
    pub fn validate(&self) -> Result<(), RpcError> {
        if self.sequence == 0 {
            return Err(invalid_field("event.sequence", "must be greater than zero"));
        }
        validate_stable_id("event.thread_id", &self.thread_id)?;
        validate_stable_id("event.turn_id", &self.turn_id)?;
        self.item.validate()?;
        if self.sequence != self.item.sequence
            || self.thread_id != self.item.thread_id
            || self.turn_id != self.item.turn_id
        {
            return Err(invalid_field(
                "event.item",
                "must preserve the event sequence, thread_id and turn_id",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn complete_thread_fixture() -> AgentThreadDetail {
        AgentThreadDetail {
            summary: AgentThreadSummary {
                thread_id: "thread_1".into(),
                project_id: Some("prj_1".into()),
                title: "未来交通工具外观".into(),
                status: AgentThreadStatus::Active,
                summary: "等待用户确认表面细节".into(),
                provider_id: "deterministic_kernel".into(),
                created_at: "2026-07-17T00:00:00Z".into(),
                updated_at: "2026-07-17T00:00:02Z".into(),
                last_turn_id: Some("turn_1".into()),
            },
            turns: vec![AgentTurn {
                turn_id: "turn_1".into(),
                thread_id: "thread_1".into(),
                request_text: "创建一个非功能性的未来机械概念外观".into(),
                status: AgentTurnStatus::WaitingForApproval,
                error_code: None,
                error_message: None,
                usage: BTreeMap::new(),
                created_at: "2026-07-17T00:00:00Z".into(),
                updated_at: "2026-07-17T00:00:02Z".into(),
                items: vec![
                    AgentItem {
                        item_id: "item_1".into(),
                        thread_id: "thread_1".into(),
                        turn_id: "turn_1".into(),
                        sequence: 1,
                        item_type: AgentItemType::UserMessage,
                        status: AgentItemStatus::Completed,
                        payload: BTreeMap::new(),
                        created_at: "2026-07-17T00:00:00Z".into(),
                    },
                    AgentItem {
                        item_id: "item_2".into(),
                        thread_id: "thread_1".into(),
                        turn_id: "turn_1".into(),
                        sequence: 2,
                        item_type: AgentItemType::ApprovalRequest,
                        status: AgentItemStatus::Pending,
                        payload: BTreeMap::new(),
                        created_at: "2026-07-17T00:00:01Z".into(),
                    },
                ],
                approvals: vec![AgentApproval {
                    approval_id: "approval_1".into(),
                    thread_id: "thread_1".into(),
                    turn_id: "turn_1".into(),
                    item_id: "item_2".into(),
                    action: "confirm_preview".into(),
                    status: ApprovalStatus::Pending,
                    payload: BTreeMap::new(),
                    created_at: "2026-07-17T00:00:02Z".into(),
                    resolved_at: None,
                }],
            }],
        }
    }

    #[test]
    fn complete_thread_turn_item_and_approval_fixture_validates() {
        let thread = complete_thread_fixture();
        thread.validate().expect("complete lifecycle is valid");
        let decoded: AgentThreadDetail =
            serde_json::from_value(serde_json::to_value(&thread).unwrap()).unwrap();
        decoded.validate().expect("wire round trip stays valid");

        let event = AgentEvent {
            sequence: 1,
            thread_id: "thread_1".into(),
            turn_id: "turn_1".into(),
            item: thread.turns[0].items[0].clone(),
        };
        event.validate().expect("event identity is valid");
    }

    #[test]
    fn current_python_item_shape_round_trips_without_renaming() {
        let fixture = json!({
            "item_id": "item_1",
            "thread_id": "thread_1",
            "turn_id": "turn_1",
            "sequence": 3,
            "item_type": "tool_result",
            "status": "completed",
            "payload": {"schema_version": "AgentActionToolEvent@1", "tool_name": "compile_readback_candidate"},
            "created_at": "2026-07-17T00:00:00Z"
        });
        let item: AgentItem =
            serde_json::from_value(fixture.clone()).expect("fixture is compatible");
        item.validate().expect("fixture satisfies lifecycle bounds");
        assert_eq!(serde_json::to_value(item).unwrap(), fixture);
    }

    #[test]
    fn create_start_and_resolve_requests_validate_text_and_id_bounds() {
        CreateAgentThreadRequest {
            client_request_id: "request_create_1".into(),
            project_id: Some("prj_1".into()),
            title: Some("新建概念".into()),
            provider_id: Some("deterministic_kernel".into()),
        }
        .validate()
        .unwrap();
        StartAgentTurnRequest {
            client_request_id: "request_turn_1".into(),
            message: "继续优化表面流线".into(),
            clarification_domain_pack_id: Some("vehicle".into()),
        }
        .validate()
        .unwrap();
        CreateAgentApprovalRequest {
            client_request_id: "request_approval_1".into(),
            turn_id: "turn_1".into(),
            action: "confirm_preview".into(),
            payload: BTreeMap::new(),
        }
        .validate()
        .unwrap();
        ResolveAgentApprovalRequest {
            client_request_id: "request_resolve_1".into(),
            decision: ApprovalDecision::Approved,
            note: Some("确认".into()),
        }
        .validate()
        .unwrap();

        let empty_id = CreateAgentThreadRequest {
            client_request_id: String::new(),
            project_id: None,
            title: None,
            provider_id: None,
        };
        assert!(empty_id.validate().is_err());

        let oversized_message = StartAgentTurnRequest {
            client_request_id: "request_turn_2".into(),
            message: "x".repeat(MAX_REQUEST_TEXT_CHARS + 1),
            clarification_domain_pack_id: None,
        };
        assert!(oversized_message.validate().is_err());

        let oversized_note = ResolveAgentApprovalRequest {
            client_request_id: "request_resolve_2".into(),
            decision: ApprovalDecision::Rejected,
            note: Some("x".repeat(MAX_APPROVAL_NOTE_CHARS + 1)),
        };
        assert!(oversized_note.validate().is_err());
    }

    #[test]
    fn serde_rejects_unknown_lifecycle_fields() {
        let request = json!({
            "client_request_id": "request_1",
            "message": "valid",
            "unexpected": true
        });
        assert!(serde_json::from_value::<StartAgentTurnRequest>(request).is_err());

        let mut thread = serde_json::to_value(complete_thread_fixture()).unwrap();
        thread
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), Value::Bool(true));
        assert!(serde_json::from_value::<AgentThreadDetail>(thread).is_err());
    }

    #[test]
    fn lifecycle_rejects_empty_or_oversized_timestamp_text() {
        let mut thread = complete_thread_fixture();
        thread.turns[0].items[0].created_at.clear();
        assert!(thread.validate().is_err());

        let mut thread = complete_thread_fixture();
        thread.summary.updated_at = "x".repeat(MAX_TIMESTAMP_CHARS + 1);
        assert!(thread.validate().is_err());
    }

    #[test]
    fn lifecycle_rejects_parent_child_identity_drift() {
        let mut thread = complete_thread_fixture();
        thread.turns[0].items[0].thread_id = "thread_other".into();
        assert!(thread.validate().is_err());

        let mut thread = complete_thread_fixture();
        thread.turns[0].approvals[0].item_id = "item_1".into();
        assert!(thread.validate().is_err());

        let mut thread = complete_thread_fixture();
        thread.turns[0].thread_id = "thread_other".into();
        assert!(thread.validate().is_err());

        let mut event = AgentEvent {
            sequence: 2,
            thread_id: "thread_1".into(),
            turn_id: "turn_1".into(),
            item: complete_thread_fixture().turns.remove(0).items.remove(0),
        };
        assert!(event.validate().is_err());
        event.sequence = 1;
        event.validate().unwrap();
    }

    #[test]
    fn lifecycle_rejects_zero_and_non_increasing_item_sequences() {
        let mut thread = complete_thread_fixture();
        thread.turns[0].items[0].sequence = 0;
        assert!(thread.validate().is_err());

        let mut thread = complete_thread_fixture();
        thread.turns[0].items[1].sequence = 1;
        assert!(thread.validate().is_err());
    }
}
