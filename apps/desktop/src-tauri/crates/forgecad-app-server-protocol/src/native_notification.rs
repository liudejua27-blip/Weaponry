use serde::{Deserialize, Serialize};

use crate::{
    contract_validation::{
        first_forbidden_key, require_optional_stable_id, require_schema, require_stable_id,
    },
    AgentApproval, AgentItem, AgentThreadStatus, AgentThreadSummary, AgentTurn, AgentTurnStatus,
    AppServerCursor, ApprovalStatus, CursorPhase, RpcError, NOTIFICATION_APPROVAL_CREATED,
    NOTIFICATION_APPROVAL_RESOLVED, NOTIFICATION_ITEM_UPDATED, NOTIFICATION_THREAD_ARCHIVED,
    NOTIFICATION_THREAD_CREATED, NOTIFICATION_THREAD_UPDATED, NOTIFICATION_TURN_CANCELLED,
    NOTIFICATION_TURN_COMPLETED, NOTIFICATION_TURN_FAILED, NOTIFICATION_TURN_STARTED,
};

pub const NATIVE_AGENT_NOTIFICATION_SCHEMA_VERSION: &str = "NativeAgentNotification@1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum NativeAgentNotificationEvent {
    ThreadCreated { thread: AgentThreadSummary },
    ThreadUpdated { thread: AgentThreadSummary },
    ThreadArchived { thread: AgentThreadSummary },
    TurnStarted { turn: AgentTurn },
    ItemUpdated { item: AgentItem },
    ApprovalCreated { approval: AgentApproval },
    ApprovalResolved { approval: AgentApproval },
    TurnCompleted { turn: AgentTurn },
    TurnFailed { turn: AgentTurn },
    TurnCancelled { turn: AgentTurn },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NativeAgentNotification {
    pub schema_version: String,
    pub notification_id: String,
    pub cursor: String,
    pub sequence: u64,
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<String>,
    pub payload: NativeAgentNotificationEvent,
}

impl NativeAgentNotification {
    pub fn method(&self) -> &'static str {
        match self.payload {
            NativeAgentNotificationEvent::ThreadCreated { .. } => NOTIFICATION_THREAD_CREATED,
            NativeAgentNotificationEvent::ThreadUpdated { .. } => NOTIFICATION_THREAD_UPDATED,
            NativeAgentNotificationEvent::ThreadArchived { .. } => NOTIFICATION_THREAD_ARCHIVED,
            NativeAgentNotificationEvent::TurnStarted { .. } => NOTIFICATION_TURN_STARTED,
            NativeAgentNotificationEvent::ItemUpdated { .. } => NOTIFICATION_ITEM_UPDATED,
            NativeAgentNotificationEvent::ApprovalCreated { .. } => NOTIFICATION_APPROVAL_CREATED,
            NativeAgentNotificationEvent::ApprovalResolved { .. } => NOTIFICATION_APPROVAL_RESOLVED,
            NativeAgentNotificationEvent::TurnCompleted { .. } => NOTIFICATION_TURN_COMPLETED,
            NativeAgentNotificationEvent::TurnFailed { .. } => NOTIFICATION_TURN_FAILED,
            NativeAgentNotificationEvent::TurnCancelled { .. } => NOTIFICATION_TURN_CANCELLED,
        }
    }

    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "native_notification.schema_version",
            &self.schema_version,
            NATIVE_AGENT_NOTIFICATION_SCHEMA_VERSION,
        )?;
        require_stable_id("native_notification.notification_id", &self.notification_id)?;
        require_stable_id("native_notification.thread_id", &self.thread_id)?;
        require_optional_stable_id("native_notification.turn_id", self.turn_id.as_deref())?;
        require_optional_stable_id("native_notification.item_id", self.item_id.as_deref())?;
        require_optional_stable_id(
            "native_notification.approval_id",
            self.approval_id.as_deref(),
        )?;
        if self.sequence == 0 {
            return Err(RpcError::invalid_params(
                "native_notification.sequence must be greater than zero.",
            ));
        }

        let cursor = AppServerCursor::decode(&self.cursor)?;
        if cursor.thread_id != self.thread_id
            || cursor.turn_id != self.turn_id
            || cursor.source_sequence != self.sequence
            || cursor.item_id != self.item_id
        {
            return Err(RpcError::invalid_params(
                "Native notification cursor must preserve top-level identity and sequence.",
            ));
        }

        match &self.payload {
            NativeAgentNotificationEvent::ThreadCreated { thread }
            | NativeAgentNotificationEvent::ThreadUpdated { thread }
            | NativeAgentNotificationEvent::ThreadArchived { thread } => {
                thread.validate()?;
                require_thread_identity(self, &cursor, &thread.thread_id)?;
                if matches!(
                    self.payload,
                    NativeAgentNotificationEvent::ThreadArchived { .. }
                ) && thread.status != AgentThreadStatus::Archived
                {
                    return Err(RpcError::invalid_params(
                        "thread_archived notification requires archived thread status.",
                    ));
                }
            }
            NativeAgentNotificationEvent::TurnStarted { turn } => {
                turn.validate()?;
                require_turn_identity(self, &cursor, turn, CursorPhase::TurnStarted)?;
                if !matches!(
                    turn.status,
                    AgentTurnStatus::Queued | AgentTurnStatus::Running
                ) {
                    return Err(RpcError::invalid_params(
                        "turn_started notification requires queued or running status.",
                    ));
                }
            }
            NativeAgentNotificationEvent::ItemUpdated { item } => {
                item.validate()?;
                require_item_identity(self, &cursor, item)?;
            }
            NativeAgentNotificationEvent::ApprovalCreated { approval } => {
                approval.validate()?;
                require_approval_identity(self, &cursor, approval)?;
                if approval.status != ApprovalStatus::Pending || approval.resolved_at.is_some() {
                    return Err(RpcError::invalid_params(
                        "approval_created notification requires a pending unresolved approval.",
                    ));
                }
            }
            NativeAgentNotificationEvent::ApprovalResolved { approval } => {
                approval.validate()?;
                require_approval_identity(self, &cursor, approval)?;
                if approval.status == ApprovalStatus::Pending || approval.resolved_at.is_none() {
                    return Err(RpcError::invalid_params(
                        "approval_resolved notification requires a resolved decision and timestamp.",
                    ));
                }
            }
            NativeAgentNotificationEvent::TurnCompleted { turn } => {
                turn.validate()?;
                require_turn_identity(self, &cursor, turn, CursorPhase::TurnTerminal)?;
                require_terminal_status(turn, AgentTurnStatus::Completed)?;
            }
            NativeAgentNotificationEvent::TurnFailed { turn } => {
                turn.validate()?;
                require_turn_identity(self, &cursor, turn, CursorPhase::TurnTerminal)?;
                require_terminal_status(turn, AgentTurnStatus::Failed)?;
            }
            NativeAgentNotificationEvent::TurnCancelled { turn } => {
                turn.validate()?;
                require_turn_identity(self, &cursor, turn, CursorPhase::TurnTerminal)?;
                require_terminal_status(turn, AgentTurnStatus::Cancelled)?;
            }
        }

        reject_reasoning_content(self)
    }
}

fn require_thread_identity(
    notification: &NativeAgentNotification,
    cursor: &AppServerCursor,
    payload_thread_id: &str,
) -> Result<(), RpcError> {
    if payload_thread_id != notification.thread_id
        || notification.turn_id.is_some()
        || notification.item_id.is_some()
        || notification.approval_id.is_some()
        || cursor.phase != CursorPhase::TurnStarted
    {
        return Err(RpcError::invalid_params(
            "Thread notification must contain only matching thread identity.",
        ));
    }
    Ok(())
}

fn require_turn_identity(
    notification: &NativeAgentNotification,
    cursor: &AppServerCursor,
    turn: &AgentTurn,
    phase: CursorPhase,
) -> Result<(), RpcError> {
    if turn.thread_id != notification.thread_id
        || notification.turn_id.as_deref() != Some(turn.turn_id.as_str())
        || notification.item_id.is_some()
        || notification.approval_id.is_some()
        || cursor.phase != phase
    {
        return Err(RpcError::invalid_params(
            "Turn notification must contain matching thread and turn identity.",
        ));
    }
    Ok(())
}

fn require_item_identity(
    notification: &NativeAgentNotification,
    cursor: &AppServerCursor,
    item: &AgentItem,
) -> Result<(), RpcError> {
    if item.thread_id != notification.thread_id
        || notification.turn_id.as_deref() != Some(item.turn_id.as_str())
        || notification.item_id.as_deref() != Some(item.item_id.as_str())
        || notification.approval_id.is_some()
        || item.sequence != notification.sequence
        || cursor.phase != CursorPhase::Item
    {
        return Err(RpcError::invalid_params(
            "Item notification must preserve thread, turn, item and sequence identity.",
        ));
    }
    Ok(())
}

fn require_approval_identity(
    notification: &NativeAgentNotification,
    cursor: &AppServerCursor,
    approval: &AgentApproval,
) -> Result<(), RpcError> {
    if approval.thread_id != notification.thread_id
        || notification.turn_id.as_deref() != Some(approval.turn_id.as_str())
        || notification.item_id.as_deref() != Some(approval.item_id.as_str())
        || notification.approval_id.as_deref() != Some(approval.approval_id.as_str())
        || cursor.phase != CursorPhase::Approval
    {
        return Err(RpcError::invalid_params(
            "Approval notification must preserve thread, turn, item and approval identity.",
        ));
    }
    Ok(())
}

fn require_terminal_status(turn: &AgentTurn, expected: AgentTurnStatus) -> Result<(), RpcError> {
    if turn.status != expected {
        return Err(RpcError::invalid_params(
            "Turn terminal notification status does not match its event.",
        ));
    }
    Ok(())
}

fn reject_reasoning_content(notification: &NativeAgentNotification) -> Result<(), RpcError> {
    let value = serde_json::to_value(notification).map_err(|error| {
        RpcError::invalid_params(format!("native_notification is not JSON: {error}"))
    })?;
    if first_forbidden_key(&value, &["reasoning_content"]).is_some() {
        return Err(RpcError::invalid_params(
            "Native notifications cannot contain reasoning_content.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::{AgentItemStatus, AgentItemType};

    fn item() -> AgentItem {
        AgentItem {
            item_id: "item_1".into(),
            thread_id: "thread_1".into(),
            turn_id: "turn_1".into(),
            sequence: 1,
            item_type: AgentItemType::ToolResult,
            status: AgentItemStatus::Completed,
            payload: BTreeMap::new(),
            created_at: "2026-07-17T00:00:01Z".into(),
        }
    }

    fn item_notification(item: AgentItem) -> NativeAgentNotification {
        let cursor = AppServerCursor::new(
            "thread_1",
            Some("turn_1".into()),
            1,
            CursorPhase::Item,
            Some("item_1".into()),
        )
        .encode()
        .unwrap();
        NativeAgentNotification {
            schema_version: NATIVE_AGENT_NOTIFICATION_SCHEMA_VERSION.into(),
            notification_id: "notification_1".into(),
            cursor,
            sequence: 1,
            thread_id: "thread_1".into(),
            turn_id: Some("turn_1".into()),
            item_id: Some("item_1".into()),
            approval_id: None,
            payload: NativeAgentNotificationEvent::ItemUpdated { item },
        }
    }

    #[test]
    fn native_item_notification_preserves_cursor_identity_and_method() {
        let notification = item_notification(item());
        notification.validate().unwrap();
        assert_eq!(notification.method(), "item/updated");
    }

    #[test]
    fn native_notification_rejects_identity_drift_and_reasoning() {
        let mut drift = item_notification(item());
        drift.item_id = Some("item_other".into());
        assert!(drift.validate().is_err());

        let mut reasoning_item = item();
        reasoning_item.payload.insert(
            "provider".into(),
            json!({"nested": {"reasoning_content": "private"}}),
        );
        assert!(item_notification(reasoning_item).validate().is_err());
    }
}
