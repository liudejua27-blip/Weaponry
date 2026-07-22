use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    contract_validation::{
        require_optional_stable_id, require_schema, require_stable_id, require_text, MAX_NOTE_CHARS,
    },
    AgentApproval, AgentItem, AgentThreadDetail, AgentThreadSummary, AgentTurn,
    CreateAgentApprovalRequest, CreateAgentThreadRequest, ResolveAgentApprovalRequest, RpcError,
    StartAgentTurnRequest,
};

pub const THREAD_COMMAND_SCHEMA_VERSION: &str = "AgentThreadCommand@1";
pub const THREAD_COMMAND_RESULT_SCHEMA_VERSION: &str = "AgentThreadCommandResult@1";
pub const TURN_COMMAND_SCHEMA_VERSION: &str = "AgentTurnCommand@1";
pub const TURN_COMMAND_RESULT_SCHEMA_VERSION: &str = "AgentTurnCommandResult@1";
pub const ITEM_COMMAND_SCHEMA_VERSION: &str = "AgentItemCommand@1";
pub const ITEM_COMMAND_RESULT_SCHEMA_VERSION: &str = "AgentItemCommandResult@1";
pub const APPROVAL_COMMAND_SCHEMA_VERSION: &str = "AgentApprovalCommand@1";
pub const APPROVAL_COMMAND_RESULT_SCHEMA_VERSION: &str = "AgentApprovalCommandResult@1";

const MAX_LIST_LIMIT: u16 = 200;

fn validate_limit(field: &str, limit: u16) -> Result<(), RpcError> {
    if limit == 0 || limit > MAX_LIST_LIMIT {
        return Err(RpcError::invalid_params(format!(
            "{field} must be between 1 and {MAX_LIST_LIMIT}."
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ThreadCommandOperation {
    Create {
        request: CreateAgentThreadRequest,
    },
    List {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project_id: Option<String>,
        #[serde(default)]
        include_archived: bool,
        limit: u16,
    },
    Read {
        thread_id: String,
    },
    Archive {
        client_request_id: String,
        thread_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ThreadCommand {
    pub schema_version: String,
    pub command_id: String,
    pub command: ThreadCommandOperation,
}

impl ThreadCommand {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "thread_command.schema_version",
            &self.schema_version,
            THREAD_COMMAND_SCHEMA_VERSION,
        )?;
        require_stable_id("thread_command.command_id", &self.command_id)?;
        match &self.command {
            ThreadCommandOperation::Create { request } => request.validate(),
            ThreadCommandOperation::List {
                project_id, limit, ..
            } => {
                require_optional_stable_id("thread_command.project_id", project_id.as_deref())?;
                validate_limit("thread_command.limit", *limit)
            }
            ThreadCommandOperation::Read { thread_id } => {
                require_stable_id("thread_command.thread_id", thread_id)
            }
            ThreadCommandOperation::Archive {
                client_request_id,
                thread_id,
            } => {
                require_stable_id("thread_command.client_request_id", client_request_id)?;
                require_stable_id("thread_command.thread_id", thread_id)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum ThreadCommandOutcome {
    Thread { thread: AgentThreadDetail },
    Threads { threads: Vec<AgentThreadSummary> },
    Archived { thread: AgentThreadSummary },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ThreadCommandResult {
    pub schema_version: String,
    pub command_id: String,
    pub result: ThreadCommandOutcome,
}

impl ThreadCommandResult {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "thread_result.schema_version",
            &self.schema_version,
            THREAD_COMMAND_RESULT_SCHEMA_VERSION,
        )?;
        require_stable_id("thread_result.command_id", &self.command_id)?;
        match &self.result {
            ThreadCommandOutcome::Thread { thread } => thread.validate(),
            ThreadCommandOutcome::Threads { threads } => {
                if threads.len() > MAX_LIST_LIMIT as usize {
                    return Err(RpcError::invalid_params(
                        "thread_result.threads exceeds the negotiated list limit.",
                    ));
                }
                let mut ids = BTreeSet::new();
                for thread in threads {
                    thread.validate()?;
                    if !ids.insert(thread.thread_id.as_str()) {
                        return Err(RpcError::invalid_params(
                            "thread_result.threads must have unique thread_id values.",
                        ));
                    }
                }
                Ok(())
            }
            ThreadCommandOutcome::Archived { thread } => thread.validate(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum TurnCommandOperation {
    Start {
        thread_id: String,
        request: StartAgentTurnRequest,
    },
    Read {
        thread_id: String,
        turn_id: String,
    },
    Cancel {
        thread_id: String,
        turn_id: String,
        cancellation_id: String,
        cancellation_token: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TurnCommand {
    pub schema_version: String,
    pub command_id: String,
    pub command: TurnCommandOperation,
}

impl TurnCommand {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "turn_command.schema_version",
            &self.schema_version,
            TURN_COMMAND_SCHEMA_VERSION,
        )?;
        require_stable_id("turn_command.command_id", &self.command_id)?;
        match &self.command {
            TurnCommandOperation::Start { thread_id, request } => {
                require_stable_id("turn_command.thread_id", thread_id)?;
                request.validate()
            }
            TurnCommandOperation::Read { thread_id, turn_id } => {
                require_stable_id("turn_command.thread_id", thread_id)?;
                require_stable_id("turn_command.turn_id", turn_id)
            }
            TurnCommandOperation::Cancel {
                thread_id,
                turn_id,
                cancellation_id,
                cancellation_token,
                reason,
            } => {
                require_stable_id("turn_command.thread_id", thread_id)?;
                require_stable_id("turn_command.turn_id", turn_id)?;
                require_stable_id("turn_command.cancellation_id", cancellation_id)?;
                require_stable_id("turn_command.cancellation_token", cancellation_token)?;
                if let Some(reason) = reason.as_deref() {
                    require_text("turn_command.reason", reason, 0, MAX_NOTE_CHARS)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum TurnCommandOutcome {
    Started {
        turn: AgentTurn,
        cancellation_id: String,
        cancellation_token: String,
    },
    Turn {
        turn: AgentTurn,
    },
    CancellationAccepted {
        thread_id: String,
        turn_id: String,
        cancellation_id: String,
        accepted: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TurnCommandResult {
    pub schema_version: String,
    pub command_id: String,
    pub result: TurnCommandOutcome,
}

impl TurnCommandResult {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "turn_result.schema_version",
            &self.schema_version,
            TURN_COMMAND_RESULT_SCHEMA_VERSION,
        )?;
        require_stable_id("turn_result.command_id", &self.command_id)?;
        match &self.result {
            TurnCommandOutcome::Started {
                turn,
                cancellation_id,
                cancellation_token,
            } => {
                turn.validate()?;
                require_stable_id("turn_result.cancellation_id", cancellation_id)?;
                require_stable_id("turn_result.cancellation_token", cancellation_token)?;
                if !matches!(
                    turn.status,
                    crate::AgentTurnStatus::Queued | crate::AgentTurnStatus::Running
                ) {
                    return Err(RpcError::invalid_params(
                        "Started Turn result requires queued or running status.",
                    ));
                }
                Ok(())
            }
            TurnCommandOutcome::Turn { turn } => turn.validate(),
            TurnCommandOutcome::CancellationAccepted {
                thread_id,
                turn_id,
                cancellation_id,
                ..
            } => {
                require_stable_id("turn_result.thread_id", thread_id)?;
                require_stable_id("turn_result.turn_id", turn_id)?;
                require_stable_id("turn_result.cancellation_id", cancellation_id)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ItemCommandOperation {
    List {
        thread_id: String,
        turn_id: String,
        #[serde(default)]
        after_sequence: u64,
        limit: u16,
    },
    Read {
        thread_id: String,
        turn_id: String,
        item_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ItemCommand {
    pub schema_version: String,
    pub command_id: String,
    pub command: ItemCommandOperation,
}

impl ItemCommand {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "item_command.schema_version",
            &self.schema_version,
            ITEM_COMMAND_SCHEMA_VERSION,
        )?;
        require_stable_id("item_command.command_id", &self.command_id)?;
        match &self.command {
            ItemCommandOperation::List {
                thread_id,
                turn_id,
                limit,
                ..
            } => {
                require_stable_id("item_command.thread_id", thread_id)?;
                require_stable_id("item_command.turn_id", turn_id)?;
                validate_limit("item_command.limit", *limit)
            }
            ItemCommandOperation::Read {
                thread_id,
                turn_id,
                item_id,
            } => {
                require_stable_id("item_command.thread_id", thread_id)?;
                require_stable_id("item_command.turn_id", turn_id)?;
                require_stable_id("item_command.item_id", item_id)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum ItemCommandOutcome {
    Item {
        item: AgentItem,
    },
    Items {
        items: Vec<AgentItem>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next_sequence: Option<u64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ItemCommandResult {
    pub schema_version: String,
    pub command_id: String,
    pub result: ItemCommandOutcome,
}

impl ItemCommandResult {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "item_result.schema_version",
            &self.schema_version,
            ITEM_COMMAND_RESULT_SCHEMA_VERSION,
        )?;
        require_stable_id("item_result.command_id", &self.command_id)?;
        match &self.result {
            ItemCommandOutcome::Item { item } => item.validate(),
            ItemCommandOutcome::Items {
                items,
                next_sequence,
            } => {
                if items.len() > MAX_LIST_LIMIT as usize {
                    return Err(RpcError::invalid_params(
                        "item_result.items exceeds the negotiated list limit.",
                    ));
                }
                let mut previous = 0;
                for item in items {
                    item.validate()?;
                    if item.sequence <= previous {
                        return Err(RpcError::invalid_params(
                            "item_result.items sequence must be strictly increasing.",
                        ));
                    }
                    previous = item.sequence;
                }
                if next_sequence.is_some_and(|next| next <= previous) {
                    return Err(RpcError::invalid_params(
                        "item_result.next_sequence must follow the last returned item.",
                    ));
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApprovalCommandOperation {
    Create {
        thread_id: String,
        request: CreateAgentApprovalRequest,
    },
    Read {
        thread_id: String,
        turn_id: String,
        approval_id: String,
    },
    Resolve {
        thread_id: String,
        turn_id: String,
        approval_id: String,
        request: ResolveAgentApprovalRequest,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ApprovalCommand {
    pub schema_version: String,
    pub command_id: String,
    pub command: ApprovalCommandOperation,
}

impl ApprovalCommand {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "approval_command.schema_version",
            &self.schema_version,
            APPROVAL_COMMAND_SCHEMA_VERSION,
        )?;
        require_stable_id("approval_command.command_id", &self.command_id)?;
        match &self.command {
            ApprovalCommandOperation::Create { thread_id, request } => {
                require_stable_id("approval_command.thread_id", thread_id)?;
                request.validate()
            }
            ApprovalCommandOperation::Read {
                thread_id,
                turn_id,
                approval_id,
            }
            | ApprovalCommandOperation::Resolve {
                thread_id,
                turn_id,
                approval_id,
                ..
            } => {
                require_stable_id("approval_command.thread_id", thread_id)?;
                require_stable_id("approval_command.turn_id", turn_id)?;
                require_stable_id("approval_command.approval_id", approval_id)?;
                if let ApprovalCommandOperation::Resolve { request, .. } = &self.command {
                    request.validate()?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApprovalCommandOutcome {
    Approval { approval: AgentApproval },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ApprovalCommandResult {
    pub schema_version: String,
    pub command_id: String,
    pub result: ApprovalCommandOutcome,
}

impl ApprovalCommandResult {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "approval_result.schema_version",
            &self.schema_version,
            APPROVAL_COMMAND_RESULT_SCHEMA_VERSION,
        )?;
        require_stable_id("approval_result.command_id", &self.command_id)?;
        match &self.result {
            ApprovalCommandOutcome::Approval { approval } => approval.validate(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versioned_commands_validate_native_lifecycle_identity() {
        ThreadCommand {
            schema_version: THREAD_COMMAND_SCHEMA_VERSION.into(),
            command_id: "cmd_thread_create_1".into(),
            command: ThreadCommandOperation::Create {
                request: CreateAgentThreadRequest {
                    client_request_id: "client_thread_1".into(),
                    project_id: Some("project_1".into()),
                    title: Some("未来概念".into()),
                    provider_id: Some("deepseek".into()),
                },
            },
        }
        .validate()
        .unwrap();

        TurnCommand {
            schema_version: TURN_COMMAND_SCHEMA_VERSION.into(),
            command_id: "cmd_turn_start_1".into(),
            command: TurnCommandOperation::Start {
                thread_id: "thread_1".into(),
                request: StartAgentTurnRequest {
                    client_request_id: "client_turn_1".into(),
                    message: "继续细化完整外观".into(),
                    clarification_domain_pack_id: None,
                },
            },
        }
        .validate()
        .unwrap();

        ItemCommand {
            schema_version: ITEM_COMMAND_SCHEMA_VERSION.into(),
            command_id: "cmd_item_list_1".into(),
            command: ItemCommandOperation::List {
                thread_id: "thread_1".into(),
                turn_id: "turn_1".into(),
                after_sequence: 0,
                limit: 100,
            },
        }
        .validate()
        .unwrap();
    }

    #[test]
    fn commands_reject_unknown_fields_and_invalid_bounds() {
        let unknown = serde_json::json!({
            "schema_version": "AgentTurnCommand@1",
            "command_id": "cmd_1",
            "command": {
                "operation": "read",
                "thread_id": "thread_1",
                "turn_id": "turn_1",
                "provider_key": "forbidden"
            }
        });
        assert!(serde_json::from_value::<TurnCommand>(unknown).is_err());

        let command = ItemCommand {
            schema_version: ITEM_COMMAND_SCHEMA_VERSION.into(),
            command_id: "cmd_item_list_1".into(),
            command: ItemCommandOperation::List {
                thread_id: "thread_1".into(),
                turn_id: "turn_1".into(),
                after_sequence: 0,
                limit: 0,
            },
        };
        assert!(command.validate().is_err());
    }

    #[test]
    fn started_turn_returns_ephemeral_cancellation_capability() {
        let result: TurnCommandResult = serde_json::from_value(serde_json::json!({
            "schema_version": TURN_COMMAND_RESULT_SCHEMA_VERSION,
            "command_id": "cmd_turn_start_1",
            "result": {
                "outcome": "started",
                "turn": {
                    "turn_id": "turn_1",
                    "thread_id": "thread_1",
                    "request_text": "生成完整概念外观",
                    "status": "running",
                    "usage": {},
                    "created_at": "2026-07-17T00:00:00Z",
                    "updated_at": "2026-07-17T00:00:00Z",
                    "items": [],
                    "approvals": []
                },
                "cancellation_id": "cancel_turn_1",
                "cancellation_token": "token_turn_1"
            }
        }))
        .unwrap();
        result.validate().unwrap();

        let mut terminal = serde_json::to_value(&result).unwrap();
        terminal["result"]["turn"]["status"] = serde_json::json!("completed");
        let terminal: TurnCommandResult = serde_json::from_value(terminal).unwrap();
        assert!(terminal.validate().is_err());
    }
}
