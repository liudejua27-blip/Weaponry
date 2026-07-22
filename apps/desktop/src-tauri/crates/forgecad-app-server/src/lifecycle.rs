//! Rust-owned Thread/Turn/Item/Approval state transitions for K002.
//!
//! The state machine owns lifecycle decisions. Persistence remains behind
//! [`LifecyclePersistencePort`] until K003 moves the database into Rust, so a
//! compatibility implementation can remain the only writer without giving
//! Python ownership of Agent orchestration.

use std::{future::Future, pin::Pin};

use forgecad_app_server_protocol::{
    AgentApproval, AgentEvent, AgentItem, AgentItemStatus, AgentItemType, AgentThreadDetail,
    AgentThreadStatus, AgentThreadSummary, AgentTurn, AgentTurnStatus, ApprovalDecision,
    ApprovalStatus, LifecyclePersistenceCommand, LifecyclePersistenceResult,
};
use serde::{Deserialize, Serialize};

use crate::CancellationToken;

pub type LifecyclePortFuture<T> =
    Pin<Box<dyn Future<Output = Result<T, LifecyclePortError>> + Send + 'static>>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LifecyclePortErrorKind {
    NotFound,
    Conflict,
    Unavailable,
    Cancelled,
    InvalidData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LifecyclePortError {
    pub code: String,
    pub kind: LifecyclePortErrorKind,
    pub message: String,
    pub recoverable: bool,
}

impl LifecyclePortError {
    pub fn cancelled() -> Self {
        Self {
            code: "LIFECYCLE_PERSISTENCE_CANCELLED".into(),
            kind: LifecyclePortErrorKind::Cancelled,
            message: "Lifecycle persistence was cancelled.".into(),
            recoverable: true,
        }
    }
}

/// Transitional single-writer boundary. Implementations may bridge to the
/// Python compatibility store, but the request contains no database path,
/// Provider credential, Product state write token, or geometry payload.
pub trait LifecyclePersistencePort: Send + Sync + 'static {
    fn execute(
        &self,
        command: LifecyclePersistenceCommand,
        cancellation: CancellationToken,
    ) -> LifecyclePortFuture<LifecyclePersistenceResult>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleErrorKind {
    InvalidTransition,
    InvalidIdentity,
    InvalidSequence,
    NotFound,
    Conflict,
    InvalidPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LifecycleError {
    pub code: String,
    pub kind: LifecycleErrorKind,
    pub message: String,
}

impl LifecycleError {
    fn new(code: impl Into<String>, kind: LifecycleErrorKind, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            kind,
            message: message.into(),
        }
    }

    fn invalid_transition(message: impl Into<String>) -> Self {
        Self::new(
            "AGENT_LIFECYCLE_INVALID_TRANSITION",
            LifecycleErrorKind::InvalidTransition,
            message,
        )
    }
}

#[derive(Debug, Clone)]
pub struct AgentLifecycleMachine {
    thread: AgentThreadDetail,
}

impl AgentLifecycleMachine {
    pub fn create(summary: AgentThreadSummary) -> Result<Self, LifecycleError> {
        if summary.last_turn_id.is_some() {
            return Err(LifecycleError::new(
                "AGENT_THREAD_LAST_TURN_INVALID",
                LifecycleErrorKind::InvalidPayload,
                "A new Thread cannot reference a last Turn.",
            ));
        }
        if summary.status == AgentThreadStatus::Archived {
            return Err(LifecycleError::invalid_transition(
                "A new Thread cannot start archived.",
            ));
        }
        let thread = AgentThreadDetail {
            summary,
            turns: Vec::new(),
        };
        validate_thread(&thread)?;
        Ok(Self { thread })
    }

    pub fn restore(thread: AgentThreadDetail) -> Result<Self, LifecycleError> {
        validate_thread(&thread)?;
        Ok(Self { thread })
    }

    pub fn snapshot(&self) -> AgentThreadDetail {
        self.thread.clone()
    }

    pub fn thread(&self) -> &AgentThreadDetail {
        &self.thread
    }

    pub fn enqueue_turn(&mut self, turn: AgentTurn) -> Result<(), LifecycleError> {
        self.ensure_mutable_thread()?;
        if self
            .thread
            .turns
            .iter()
            .any(|existing| !is_terminal(&existing.status))
        {
            return Err(LifecycleError::new(
                "AGENT_THREAD_ALREADY_RUNNING",
                LifecycleErrorKind::Conflict,
                "A Thread may have only one non-terminal Turn.",
            ));
        }
        if turn.thread_id != self.thread.summary.thread_id {
            return Err(LifecycleError::new(
                "AGENT_TURN_PARENT_MISMATCH",
                LifecycleErrorKind::InvalidIdentity,
                "Turn thread_id does not match the parent Thread.",
            ));
        }
        if turn.status != AgentTurnStatus::Queued
            || !turn.items.is_empty()
            || !turn.approvals.is_empty()
            || turn.error_code.is_some()
            || turn.error_message.is_some()
        {
            return Err(LifecycleError::new(
                "AGENT_TURN_INITIAL_STATE_INVALID",
                LifecycleErrorKind::InvalidPayload,
                "A new Turn must be queued without Items, Approvals, or errors.",
            ));
        }
        if self
            .thread
            .turns
            .iter()
            .any(|existing| existing.turn_id == turn.turn_id)
        {
            return Err(LifecycleError::new(
                "AGENT_TURN_ALREADY_EXISTS",
                LifecycleErrorKind::Conflict,
                "Turn ID already exists in this Thread.",
            ));
        }
        turn.validate().map_err(|error| {
            LifecycleError::new(
                "AGENT_TURN_INVALID",
                LifecycleErrorKind::InvalidPayload,
                error.message,
            )
        })?;
        self.thread.summary.last_turn_id = Some(turn.turn_id.clone());
        self.thread.summary.updated_at = turn.updated_at.clone();
        self.thread.summary.status = AgentThreadStatus::Active;
        self.thread.turns.push(turn);
        self.validate_after_mutation()
    }

    pub fn start_turn(&mut self, turn_id: &str, updated_at: &str) -> Result<(), LifecycleError> {
        self.transition_turn(turn_id, AgentTurnStatus::Running, updated_at, None)
    }

    pub fn append_item(&mut self, item: AgentItem) -> Result<AgentEvent, LifecycleError> {
        self.ensure_mutable_thread()?;
        let turn = self.turn_mut(&item.turn_id)?;
        if item.thread_id != turn.thread_id {
            return Err(LifecycleError::new(
                "AGENT_ITEM_PARENT_MISMATCH",
                LifecycleErrorKind::InvalidIdentity,
                "Item thread_id does not match its Turn.",
            ));
        }
        if !matches!(
            turn.status,
            AgentTurnStatus::Running
                | AgentTurnStatus::WaitingForApproval
                | AgentTurnStatus::WaitingForClarification
        ) {
            return Err(LifecycleError::invalid_transition(
                "Items may only be appended to an active Turn.",
            ));
        }
        let expected_sequence = turn
            .items
            .last()
            .map_or(1, |previous| previous.sequence.saturating_add(1));
        if item.sequence != expected_sequence {
            return Err(LifecycleError::new(
                "AGENT_ITEM_SEQUENCE_INVALID",
                LifecycleErrorKind::InvalidSequence,
                format!(
                    "Item sequence must be {expected_sequence}; received {}.",
                    item.sequence
                ),
            ));
        }
        if turn
            .items
            .iter()
            .any(|existing| existing.item_id == item.item_id)
        {
            return Err(LifecycleError::new(
                "AGENT_ITEM_ALREADY_EXISTS",
                LifecycleErrorKind::Conflict,
                "Item ID already exists in this Turn.",
            ));
        }
        item.validate().map_err(|error| {
            LifecycleError::new(
                "AGENT_ITEM_INVALID",
                LifecycleErrorKind::InvalidPayload,
                error.message,
            )
        })?;
        turn.updated_at = item.created_at.clone();
        let event = AgentEvent {
            sequence: item.sequence,
            thread_id: item.thread_id.clone(),
            turn_id: item.turn_id.clone(),
            item: item.clone(),
        };
        turn.items.push(item);
        self.thread.summary.updated_at = event.item.created_at.clone();
        self.validate_after_mutation()?;
        Ok(event)
    }

    pub fn request_approval(&mut self, approval: AgentApproval) -> Result<(), LifecycleError> {
        self.ensure_mutable_thread()?;
        let turn = self.turn_mut(&approval.turn_id)?;
        if turn.status != AgentTurnStatus::Running {
            return Err(LifecycleError::invalid_transition(
                "Approval may only be requested by a running Turn.",
            ));
        }
        if approval.thread_id != turn.thread_id {
            return Err(LifecycleError::new(
                "AGENT_APPROVAL_PARENT_MISMATCH",
                LifecycleErrorKind::InvalidIdentity,
                "Approval thread_id does not match its Turn.",
            ));
        }
        if approval.status != ApprovalStatus::Pending || approval.resolved_at.is_some() {
            return Err(LifecycleError::new(
                "AGENT_APPROVAL_INITIAL_STATE_INVALID",
                LifecycleErrorKind::InvalidPayload,
                "A new Approval must be pending and unresolved.",
            ));
        }
        let Some(item) = turn
            .items
            .iter()
            .find(|item| item.item_id == approval.item_id)
        else {
            return Err(LifecycleError::new(
                "AGENT_APPROVAL_ITEM_NOT_FOUND",
                LifecycleErrorKind::NotFound,
                "Approval request Item does not exist in this Turn.",
            ));
        };
        if item.item_type != AgentItemType::ApprovalRequest
            || item.status != AgentItemStatus::Pending
        {
            return Err(LifecycleError::new(
                "AGENT_APPROVAL_ITEM_INVALID",
                LifecycleErrorKind::InvalidPayload,
                "Approval must reference a pending approval_request Item.",
            ));
        }
        if turn
            .approvals
            .iter()
            .any(|existing| existing.approval_id == approval.approval_id)
        {
            return Err(LifecycleError::new(
                "AGENT_APPROVAL_ALREADY_EXISTS",
                LifecycleErrorKind::Conflict,
                "Approval ID already exists in this Turn.",
            ));
        }
        approval.validate().map_err(|error| {
            LifecycleError::new(
                "AGENT_APPROVAL_INVALID",
                LifecycleErrorKind::InvalidPayload,
                error.message,
            )
        })?;
        turn.updated_at = approval.created_at.clone();
        turn.status = AgentTurnStatus::WaitingForApproval;
        turn.approvals.push(approval);
        self.thread.summary.updated_at = turn.updated_at.clone();
        self.validate_after_mutation()
    }

    pub fn resolve_approval(
        &mut self,
        approval_id: &str,
        decision: ApprovalDecision,
        resolved_at: &str,
    ) -> Result<(), LifecycleError> {
        self.ensure_mutable_thread()?;
        let turn_index = self
            .thread
            .turns
            .iter()
            .position(|turn| {
                turn.approvals
                    .iter()
                    .any(|approval| approval.approval_id == approval_id)
            })
            .ok_or_else(|| {
                LifecycleError::new(
                    "AGENT_APPROVAL_NOT_FOUND",
                    LifecycleErrorKind::NotFound,
                    "Approval does not exist.",
                )
            })?;
        let turn = &mut self.thread.turns[turn_index];
        if turn.status != AgentTurnStatus::WaitingForApproval {
            return Err(LifecycleError::invalid_transition(
                "Approval can only be resolved while its Turn is waiting.",
            ));
        }
        let approval = turn
            .approvals
            .iter_mut()
            .find(|approval| approval.approval_id == approval_id)
            .expect("approval position was checked");
        if approval.status != ApprovalStatus::Pending {
            return Err(LifecycleError::new(
                "AGENT_APPROVAL_ALREADY_RESOLVED",
                LifecycleErrorKind::Conflict,
                "Approval has already been resolved.",
            ));
        }
        let item = turn
            .items
            .iter_mut()
            .find(|item| item.item_id == approval.item_id)
            .expect("validated approval references an Item");
        match decision {
            ApprovalDecision::Approved => {
                approval.status = ApprovalStatus::Approved;
                item.status = AgentItemStatus::Completed;
                turn.status = AgentTurnStatus::Running;
            }
            ApprovalDecision::Rejected => {
                approval.status = ApprovalStatus::Rejected;
                item.status = AgentItemStatus::Cancelled;
                turn.status = AgentTurnStatus::Cancelled;
            }
        }
        approval.resolved_at = Some(resolved_at.to_string());
        turn.updated_at = resolved_at.to_string();
        self.thread.summary.updated_at = resolved_at.to_string();
        self.recompute_thread_status();
        self.validate_after_mutation()
    }

    pub fn complete_turn(&mut self, turn_id: &str, updated_at: &str) -> Result<(), LifecycleError> {
        self.transition_turn(turn_id, AgentTurnStatus::Completed, updated_at, None)
    }

    pub fn fail_turn(
        &mut self,
        turn_id: &str,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
        updated_at: &str,
    ) -> Result<(), LifecycleError> {
        self.transition_turn(
            turn_id,
            AgentTurnStatus::Failed,
            updated_at,
            Some((error_code.into(), error_message.into())),
        )
    }

    pub fn cancel_turn(&mut self, turn_id: &str, updated_at: &str) -> Result<(), LifecycleError> {
        self.transition_turn(turn_id, AgentTurnStatus::Cancelled, updated_at, None)
    }

    pub fn archive_thread(&mut self, updated_at: &str) -> Result<(), LifecycleError> {
        self.ensure_mutable_thread()?;
        if self
            .thread
            .turns
            .iter()
            .any(|turn| !is_terminal(&turn.status))
        {
            return Err(LifecycleError::invalid_transition(
                "A Thread with a non-terminal Turn cannot be archived.",
            ));
        }
        self.thread.summary.status = AgentThreadStatus::Archived;
        self.thread.summary.updated_at = updated_at.to_string();
        self.validate_after_mutation()
    }

    fn transition_turn(
        &mut self,
        turn_id: &str,
        next: AgentTurnStatus,
        updated_at: &str,
        error: Option<(String, String)>,
    ) -> Result<(), LifecycleError> {
        self.ensure_mutable_thread()?;
        let turn = self.turn_mut(turn_id)?;
        if !valid_turn_transition(&turn.status, &next) {
            return Err(LifecycleError::invalid_transition(format!(
                "Turn cannot transition from {:?} to {:?}.",
                turn.status, next
            )));
        }
        if next == AgentTurnStatus::Completed
            && turn
                .approvals
                .iter()
                .any(|approval| approval.status == ApprovalStatus::Pending)
        {
            return Err(LifecycleError::invalid_transition(
                "A Turn with pending Approval cannot complete.",
            ));
        }
        turn.status = next.clone();
        turn.updated_at = updated_at.to_string();
        if let Some((code, message)) = error {
            turn.error_code = Some(code);
            turn.error_message = Some(message);
        } else if next != AgentTurnStatus::Failed {
            turn.error_code = None;
            turn.error_message = None;
        }
        self.thread.summary.updated_at = updated_at.to_string();
        self.recompute_thread_status();
        self.validate_after_mutation()
    }

    fn ensure_mutable_thread(&self) -> Result<(), LifecycleError> {
        if self.thread.summary.status == AgentThreadStatus::Archived {
            return Err(LifecycleError::invalid_transition(
                "Archived Threads are read-only.",
            ));
        }
        Ok(())
    }

    fn turn_mut(&mut self, turn_id: &str) -> Result<&mut AgentTurn, LifecycleError> {
        self.thread
            .turns
            .iter_mut()
            .find(|turn| turn.turn_id == turn_id)
            .ok_or_else(|| {
                LifecycleError::new(
                    "AGENT_TURN_NOT_FOUND",
                    LifecycleErrorKind::NotFound,
                    "Turn does not exist in this Thread.",
                )
            })
    }

    fn recompute_thread_status(&mut self) {
        let status = self
            .thread
            .turns
            .last()
            .map(|turn| match turn.status {
                AgentTurnStatus::Queued
                | AgentTurnStatus::Running
                | AgentTurnStatus::WaitingForApproval
                | AgentTurnStatus::WaitingForClarification => AgentThreadStatus::Active,
                AgentTurnStatus::Failed => AgentThreadStatus::Error,
                AgentTurnStatus::Completed | AgentTurnStatus::Cancelled => AgentThreadStatus::Idle,
            })
            .unwrap_or(AgentThreadStatus::Idle);
        self.thread.summary.status = status;
    }

    fn validate_after_mutation(&self) -> Result<(), LifecycleError> {
        validate_thread(&self.thread)
    }
}

fn is_terminal(status: &AgentTurnStatus) -> bool {
    matches!(
        status,
        AgentTurnStatus::Completed | AgentTurnStatus::Failed | AgentTurnStatus::Cancelled
    )
}

fn valid_turn_transition(current: &AgentTurnStatus, next: &AgentTurnStatus) -> bool {
    matches!(
        (current, next),
        (AgentTurnStatus::Queued, AgentTurnStatus::Running)
            | (AgentTurnStatus::Queued, AgentTurnStatus::Cancelled)
            | (
                AgentTurnStatus::Running,
                AgentTurnStatus::WaitingForApproval
            )
            | (
                AgentTurnStatus::Running,
                AgentTurnStatus::WaitingForClarification
            )
            | (AgentTurnStatus::Running, AgentTurnStatus::Completed)
            | (AgentTurnStatus::Running, AgentTurnStatus::Failed)
            | (AgentTurnStatus::Running, AgentTurnStatus::Cancelled)
            | (
                AgentTurnStatus::WaitingForApproval,
                AgentTurnStatus::Running
            )
            | (
                AgentTurnStatus::WaitingForApproval,
                AgentTurnStatus::Cancelled
            )
            | (AgentTurnStatus::WaitingForApproval, AgentTurnStatus::Failed)
            | (
                AgentTurnStatus::WaitingForClarification,
                AgentTurnStatus::Running
            )
            | (
                AgentTurnStatus::WaitingForClarification,
                AgentTurnStatus::Cancelled
            )
    )
}

fn validate_thread(thread: &AgentThreadDetail) -> Result<(), LifecycleError> {
    thread.validate().map_err(|error| {
        LifecycleError::new(
            "AGENT_LIFECYCLE_INVALID",
            LifecycleErrorKind::InvalidPayload,
            error.message,
        )
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;

    fn summary() -> AgentThreadSummary {
        AgentThreadSummary {
            thread_id: "thread_k002".into(),
            project_id: Some("prj_k002".into()),
            title: "生产概念外观".into(),
            status: AgentThreadStatus::Idle,
            summary: String::new(),
            provider_id: "deepseek".into(),
            created_at: "2026-07-17T00:00:00Z".into(),
            updated_at: "2026-07-17T00:00:00Z".into(),
            last_turn_id: None,
        }
    }

    fn turn() -> AgentTurn {
        AgentTurn {
            turn_id: "turn_k002".into(),
            thread_id: "thread_k002".into(),
            request_text: "设计一个非功能性的未来机械概念外观".into(),
            status: AgentTurnStatus::Queued,
            error_code: None,
            error_message: None,
            usage: BTreeMap::new(),
            created_at: "2026-07-17T00:00:01Z".into(),
            updated_at: "2026-07-17T00:00:01Z".into(),
            items: Vec::new(),
            approvals: Vec::new(),
        }
    }

    fn item(sequence: u64, item_type: AgentItemType, status: AgentItemStatus) -> AgentItem {
        AgentItem {
            item_id: format!("item_{sequence}"),
            thread_id: "thread_k002".into(),
            turn_id: "turn_k002".into(),
            sequence,
            item_type,
            status,
            payload: BTreeMap::new(),
            created_at: format!("2026-07-17T00:00:0{}Z", sequence + 1),
        }
    }

    #[test]
    fn lifecycle_enforces_ordered_items_and_terminal_transitions() {
        let mut machine = AgentLifecycleMachine::create(summary()).unwrap();
        machine.enqueue_turn(turn()).unwrap();
        machine
            .start_turn("turn_k002", "2026-07-17T00:00:02Z")
            .unwrap();
        let event = machine
            .append_item(item(
                1,
                AgentItemType::UserMessage,
                AgentItemStatus::Completed,
            ))
            .unwrap();
        assert_eq!(event.sequence, 1);

        let out_of_order =
            machine.append_item(item(3, AgentItemType::ToolCall, AgentItemStatus::Pending));
        assert_eq!(
            out_of_order.unwrap_err().code,
            "AGENT_ITEM_SEQUENCE_INVALID"
        );
        machine
            .complete_turn("turn_k002", "2026-07-17T00:00:04Z")
            .unwrap();
        assert_eq!(machine.thread().summary.status, AgentThreadStatus::Idle);
        assert!(machine
            .start_turn("turn_k002", "2026-07-17T00:00:05Z")
            .is_err());
    }

    #[test]
    fn approval_requires_explicit_resolution_and_rejection_cancels_without_side_effect() {
        let mut machine = AgentLifecycleMachine::create(summary()).unwrap();
        machine.enqueue_turn(turn()).unwrap();
        machine
            .start_turn("turn_k002", "2026-07-17T00:00:02Z")
            .unwrap();
        machine
            .append_item(item(
                1,
                AgentItemType::ApprovalRequest,
                AgentItemStatus::Pending,
            ))
            .unwrap();
        machine
            .request_approval(AgentApproval {
                approval_id: "approval_k002".into(),
                thread_id: "thread_k002".into(),
                turn_id: "turn_k002".into(),
                item_id: "item_1".into(),
                action: "confirm_preview".into(),
                status: ApprovalStatus::Pending,
                payload: BTreeMap::from([("permanent_side_effects".into(), json!(0))]),
                created_at: "2026-07-17T00:00:03Z".into(),
                resolved_at: None,
            })
            .unwrap();
        assert_eq!(
            machine.thread().turns[0].status,
            AgentTurnStatus::WaitingForApproval
        );
        assert!(machine
            .complete_turn("turn_k002", "2026-07-17T00:00:04Z")
            .is_err());

        machine
            .resolve_approval(
                "approval_k002",
                ApprovalDecision::Rejected,
                "2026-07-17T00:00:05Z",
            )
            .unwrap();
        let snapshot = machine.snapshot();
        assert_eq!(snapshot.turns[0].status, AgentTurnStatus::Cancelled);
        assert_eq!(
            snapshot.turns[0].items[0].status,
            AgentItemStatus::Cancelled
        );
        assert_eq!(
            snapshot.turns[0].approvals[0].status,
            ApprovalStatus::Rejected
        );
    }

    #[test]
    fn restore_validates_parent_identity_and_persistence_uses_protocol_dto() {
        let machine = AgentLifecycleMachine::create(summary()).unwrap();
        let mut invalid = machine.snapshot();
        invalid.summary.last_turn_id = Some("turn_missing".into());
        assert!(AgentLifecycleMachine::restore(invalid).is_err());

        let request = forgecad_app_server_protocol::LifecyclePersistenceCommand {
            schema_version:
                forgecad_app_server_protocol::LIFECYCLE_PERSISTENCE_COMMAND_SCHEMA_VERSION.into(),
            command_id: "replay_items_1".into(),
            idempotency_key: "a".repeat(64),
            expected_revision: Some("opaque:revision/1".into()),
            command: forgecad_app_server_protocol::LifecyclePersistenceOperation::ReplayItems {
                thread_id: "thread_k002".into(),
                after_sequence: 4,
                limit: 128,
            },
        };
        request.validate().unwrap();
        let value = serde_json::to_value(request).unwrap();
        assert_eq!(value["command"]["after_sequence"], 4);
        assert!(value.get("database_path").is_none());
    }
}
