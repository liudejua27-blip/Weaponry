use serde::{Deserialize, Serialize};

use crate::{
    contract_validation::{
        first_forbidden_key, require_optional_stable_id, require_schema, require_sha256,
        require_stable_id,
    },
    AgentApproval, AgentItem, AgentThreadDetail, AgentThreadSummary, AgentTurn, AgentTurnStatus,
    ApprovalStatus, RpcError,
};

pub const LIFECYCLE_PERSISTENCE_COMMAND_SCHEMA_VERSION: &str = "LifecyclePersistenceCommand@1";
pub const LIFECYCLE_PERSISTENCE_RESULT_SCHEMA_VERSION: &str = "LifecyclePersistenceResult@1";

const MAX_PERSISTENCE_LIMIT: u16 = 200;
const MAX_OPAQUE_REVISION_CHARS: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum LifecyclePersistenceOperation {
    LoadThread {
        thread_id: String,
    },
    ListThreads {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project_id: Option<String>,
        #[serde(default)]
        include_archived: bool,
        limit: u16,
    },
    CreateThread {
        thread: AgentThreadSummary,
    },
    ArchiveThread {
        thread: AgentThreadSummary,
    },
    CreateTurn {
        thread_id: String,
        turn: AgentTurn,
    },
    AppendItem {
        item: AgentItem,
        expected_previous_sequence: u64,
    },
    CreateApproval {
        approval: AgentApproval,
    },
    ResolveApproval {
        approval: AgentApproval,
    },
    SetTurnTerminal {
        turn: AgentTurn,
    },
    ReplayItems {
        thread_id: String,
        #[serde(default)]
        after_sequence: u64,
        limit: u16,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LifecyclePersistenceCommand {
    pub schema_version: String,
    pub command_id: String,
    pub idempotency_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<String>,
    pub command: LifecyclePersistenceOperation,
}

impl LifecyclePersistenceCommand {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "persistence_command.schema_version",
            &self.schema_version,
            LIFECYCLE_PERSISTENCE_COMMAND_SCHEMA_VERSION,
        )?;
        require_stable_id("persistence_command.command_id", &self.command_id)?;
        require_sha256("persistence_command.idempotency_key", &self.idempotency_key)?;
        if let Some(revision) = self.expected_revision.as_deref() {
            validate_opaque_revision("persistence_command.expected_revision", revision)?;
        }

        match &self.command {
            LifecyclePersistenceOperation::LoadThread { thread_id } => {
                require_stable_id("persistence_command.thread_id", thread_id)?;
            }
            LifecyclePersistenceOperation::ListThreads {
                project_id, limit, ..
            } => {
                require_optional_stable_id(
                    "persistence_command.project_id",
                    project_id.as_deref(),
                )?;
                validate_limit("persistence_command.limit", *limit)?;
            }
            LifecyclePersistenceOperation::CreateThread { thread } => {
                thread.validate()?;
            }
            LifecyclePersistenceOperation::ArchiveThread { thread } => {
                thread.validate()?;
                if thread.status != crate::AgentThreadStatus::Archived {
                    return Err(RpcError::invalid_params(
                        "Archive-thread persistence requires archived thread status.",
                    ));
                }
            }
            LifecyclePersistenceOperation::CreateTurn { thread_id, turn } => {
                require_stable_id("persistence_command.thread_id", thread_id)?;
                turn.validate()?;
                if turn.thread_id != *thread_id {
                    return Err(RpcError::invalid_params(
                        "Create-turn persistence identity must preserve thread_id.",
                    ));
                }
                if !turn.items.is_empty() || !turn.approvals.is_empty() {
                    return Err(RpcError::invalid_params(
                        "Create-turn persistence starts empty; items and approvals append through dedicated commands.",
                    ));
                }
            }
            LifecyclePersistenceOperation::AppendItem {
                item,
                expected_previous_sequence,
            } => {
                item.validate()?;
                if expected_previous_sequence.checked_add(1) != Some(item.sequence) {
                    return Err(RpcError::invalid_params(
                        "Append-item sequence must immediately follow expected_previous_sequence.",
                    ));
                }
            }
            LifecyclePersistenceOperation::CreateApproval { approval } => {
                approval.validate()?;
                if approval.status != ApprovalStatus::Pending || approval.resolved_at.is_some() {
                    return Err(RpcError::invalid_params(
                        "Create-approval persistence requires a pending unresolved approval.",
                    ));
                }
            }
            LifecyclePersistenceOperation::ResolveApproval { approval } => {
                approval.validate()?;
                if approval.status == ApprovalStatus::Pending || approval.resolved_at.is_none() {
                    return Err(RpcError::invalid_params(
                        "Resolve-approval persistence requires a resolved decision and timestamp.",
                    ));
                }
            }
            LifecyclePersistenceOperation::SetTurnTerminal { turn } => {
                turn.validate()?;
                if !matches!(
                    turn.status,
                    AgentTurnStatus::Completed
                        | AgentTurnStatus::Failed
                        | AgentTurnStatus::Cancelled
                ) {
                    return Err(RpcError::invalid_params(
                        "Set-turn-terminal persistence requires completed, failed or cancelled status.",
                    ));
                }
            }
            LifecyclePersistenceOperation::ReplayItems {
                thread_id, limit, ..
            } => {
                require_stable_id("persistence_command.thread_id", thread_id)?;
                validate_limit("persistence_command.limit", *limit)?;
            }
        }

        reject_reasoning_content("persistence_command", self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum LifecyclePersistenceOutcome {
    Applied {
        thread_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        turn_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        approval_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sequence: Option<u64>,
    },
    ThreadLoaded {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thread: Option<AgentThreadDetail>,
    },
    ThreadsListed {
        threads: Vec<AgentThreadSummary>,
    },
    ItemsReplayed {
        thread_id: String,
        items: Vec<AgentItem>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next_sequence: Option<u64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LifecyclePersistenceResult {
    pub schema_version: String,
    pub command_id: String,
    pub revision: String,
    pub replayed: bool,
    pub result: LifecyclePersistenceOutcome,
}

impl LifecyclePersistenceResult {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "persistence_result.schema_version",
            &self.schema_version,
            LIFECYCLE_PERSISTENCE_RESULT_SCHEMA_VERSION,
        )?;
        require_stable_id("persistence_result.command_id", &self.command_id)?;
        validate_opaque_revision("persistence_result.revision", &self.revision)?;

        match &self.result {
            LifecyclePersistenceOutcome::Applied {
                thread_id,
                turn_id,
                item_id,
                approval_id,
                sequence,
            } => {
                require_stable_id("persistence_result.thread_id", thread_id)?;
                require_optional_stable_id("persistence_result.turn_id", turn_id.as_deref())?;
                require_optional_stable_id("persistence_result.item_id", item_id.as_deref())?;
                require_optional_stable_id(
                    "persistence_result.approval_id",
                    approval_id.as_deref(),
                )?;
                if item_id.is_some() && turn_id.is_none() {
                    return Err(RpcError::invalid_params(
                        "Applied item identity requires turn_id.",
                    ));
                }
                if approval_id.is_some() && (turn_id.is_none() || item_id.is_none()) {
                    return Err(RpcError::invalid_params(
                        "Applied approval identity requires turn_id and item_id.",
                    ));
                }
                if sequence.is_some_and(|value| value == 0)
                    || (sequence.is_some() != item_id.is_some())
                {
                    return Err(RpcError::invalid_params(
                        "Applied item identity requires one positive sequence.",
                    ));
                }
            }
            LifecyclePersistenceOutcome::ThreadLoaded { thread } => {
                if let Some(thread) = thread {
                    thread.validate()?;
                }
            }
            LifecyclePersistenceOutcome::ThreadsListed { threads } => {
                if threads.len() > MAX_PERSISTENCE_LIMIT as usize {
                    return Err(RpcError::invalid_params(
                        "Persistence thread list exceeds the contract limit.",
                    ));
                }
                for thread in threads {
                    thread.validate()?;
                }
            }
            LifecyclePersistenceOutcome::ItemsReplayed {
                thread_id,
                items,
                next_sequence,
            } => {
                require_stable_id("persistence_result.thread_id", thread_id)?;
                if items.len() > MAX_PERSISTENCE_LIMIT as usize {
                    return Err(RpcError::invalid_params(
                        "Persistence item replay exceeds the contract limit.",
                    ));
                }
                let mut previous = 0;
                for item in items {
                    item.validate()?;
                    if item.thread_id != *thread_id || item.sequence <= previous {
                        return Err(RpcError::invalid_params(
                            "Replayed items must preserve thread identity and strictly increasing thread-scoped sequence.",
                        ));
                    }
                    previous = item.sequence;
                }
                if next_sequence.is_some_and(|next| next <= previous) {
                    return Err(RpcError::invalid_params(
                        "Replay next_sequence must follow the last returned item.",
                    ));
                }
            }
        }

        reject_reasoning_content("persistence_result", self)
    }
}

fn validate_limit(field: &str, value: u16) -> Result<(), RpcError> {
    if value == 0 || value > MAX_PERSISTENCE_LIMIT {
        return Err(RpcError::invalid_params(format!(
            "{field} must be between 1 and {MAX_PERSISTENCE_LIMIT}."
        )));
    }
    Ok(())
}

fn validate_opaque_revision(field: &str, value: &str) -> Result<(), RpcError> {
    let chars = value.chars().count();
    if chars == 0
        || chars > MAX_OPAQUE_REVISION_CHARS
        || !value.is_ascii()
        || value.chars().any(char::is_control)
    {
        return Err(RpcError::invalid_params(format!(
            "{field} must be 1 to {MAX_OPAQUE_REVISION_CHARS} printable ASCII characters."
        )));
    }
    Ok(())
}

fn reject_reasoning_content<T: Serialize>(field: &str, value: &T) -> Result<(), RpcError> {
    let value = serde_json::to_value(value)
        .map_err(|error| RpcError::invalid_params(format!("{field} is not JSON: {error}")))?;
    if first_forbidden_key(&value, &["reasoning_content"]).is_some() {
        return Err(RpcError::invalid_params(format!(
            "{field} cannot persist reasoning_content."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::{AgentItemStatus, AgentItemType};

    fn item(sequence: u64) -> AgentItem {
        AgentItem {
            item_id: format!("item_{sequence}"),
            thread_id: "thread_1".into(),
            turn_id: "turn_1".into(),
            sequence,
            item_type: AgentItemType::ToolResult,
            status: AgentItemStatus::Completed,
            payload: BTreeMap::new(),
            created_at: "2026-07-17T00:00:01Z".into(),
        }
    }

    #[test]
    fn append_item_requires_cas_sequence_and_opaque_revision() {
        let command = LifecyclePersistenceCommand {
            schema_version: LIFECYCLE_PERSISTENCE_COMMAND_SCHEMA_VERSION.into(),
            command_id: "persist_item_1".into(),
            idempotency_key: "a".repeat(64),
            expected_revision: Some("opaque:revision/7".into()),
            command: LifecyclePersistenceOperation::AppendItem {
                item: item(2),
                expected_previous_sequence: 1,
            },
        };
        command.validate().unwrap();

        let result = LifecyclePersistenceResult {
            schema_version: LIFECYCLE_PERSISTENCE_RESULT_SCHEMA_VERSION.into(),
            command_id: "persist_item_1".into(),
            revision: "opaque:revision/8".into(),
            replayed: false,
            result: LifecyclePersistenceOutcome::Applied {
                thread_id: "thread_1".into(),
                turn_id: Some("turn_1".into()),
                item_id: Some("item_2".into()),
                approval_id: None,
                sequence: Some(2),
            },
        };
        result.validate().unwrap();
    }

    #[test]
    fn persistence_rejects_reasoning_content_at_any_depth() {
        let mut secret_item = item(1);
        secret_item.payload.insert(
            "provider_envelope".into(),
            json!({"nested": {"reasoning_content": "private"}}),
        );
        let command = LifecyclePersistenceCommand {
            schema_version: LIFECYCLE_PERSISTENCE_COMMAND_SCHEMA_VERSION.into(),
            command_id: "persist_item_1".into(),
            idempotency_key: "a".repeat(64),
            expected_revision: None,
            command: LifecyclePersistenceOperation::AppendItem {
                item: secret_item,
                expected_previous_sequence: 0,
            },
        };
        assert!(command.validate().is_err());
    }
}
