use forgecad_app_server_protocol::{
    AgentApproval, AgentItem, AgentItemType, AgentThreadDetail, AgentThreadStatus,
    AgentThreadSummary, AgentTurn, AgentTurnStatus, ApprovalStatus, LifecyclePersistenceCommand,
    LifecyclePersistenceOperation, LifecyclePersistenceOutcome, LifecyclePersistenceResult,
    LIFECYCLE_PERSISTENCE_RESULT_SCHEMA_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::Value;

use crate::{semantic_sha256, CoreError, CoreRepository, CoreResult};

const IDEMPOTENCY_SCOPE: &str = "K002 LifecyclePersistenceCommand@1";

/// Rust-owned implementation of the K002 sealed lifecycle persistence port.
///
/// It intentionally consumes and returns the protocol crate DTOs directly so
/// the desktop adapter is a thin async wrapper with no duplicate lifecycle or
/// SQLite logic.
#[derive(Debug, Clone)]
pub struct LifecycleStore {
    repository: CoreRepository,
}

impl LifecycleStore {
    pub fn new(repository: CoreRepository) -> Self {
        Self { repository }
    }

    pub fn execute_lifecycle(
        &self,
        command: LifecyclePersistenceCommand,
    ) -> CoreResult<LifecyclePersistenceResult> {
        command
            .validate()
            .map_err(|error| CoreError::invalid_data("LIFECYCLE_COMMAND_INVALID", error.message))?;
        let mut command_value = serde_json::to_value(&command).map_err(|_| {
            CoreError::invalid_data(
                "LIFECYCLE_COMMAND_INVALID",
                "Lifecycle command could not be canonicalized.",
            )
        })?;
        command_value
            .as_object_mut()
            .expect("serialized lifecycle command is an object")
            .remove("command_id");
        let command_hash = semantic_sha256(&command_value)?;

        self.repository.write(|transaction| {
            let replay: Option<(String, String)> = transaction
                .query_row(
                    "SELECT request_hash, response_json FROM idempotency_records WHERE scope=? AND idempotency_key=?",
                    params![IDEMPOTENCY_SCOPE, command.idempotency_key],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((stored_hash, response_json)) = replay {
                if stored_hash != command_hash {
                    return Err(conflict(
                        "LIFECYCLE_IDEMPOTENCY_CONFLICT",
                        "Lifecycle idempotency key was reused for another sealed command.",
                    ));
                }
                let mut result: LifecyclePersistenceResult =
                    serde_json::from_str(&response_json).map_err(|_| {
                        CoreError::invalid_data(
                            "LIFECYCLE_REPLAY_CORRUPT",
                            "Stored lifecycle replay payload is invalid.",
                        )
                    })?;
                result.command_id = command.command_id.clone();
                result.replayed = true;
                result.validate().map_err(|error| {
                    CoreError::invalid_data("LIFECYCLE_REPLAY_CORRUPT", error.message)
                })?;
                return Ok(result);
            }

            let result = apply(transaction, &command)?;
            result.validate().map_err(|error| {
                CoreError::invalid_data("LIFECYCLE_RESULT_INVALID", error.message)
            })?;
            transaction.execute(
                "INSERT INTO idempotency_records(scope, idempotency_key, request_hash, response_json, created_at) VALUES (?, ?, ?, ?, datetime('now'))",
                params![
                    IDEMPOTENCY_SCOPE,
                    command.idempotency_key,
                    command_hash,
                    serde_json::to_string(&result).map_err(|_| CoreError::invalid_data(
                        "LIFECYCLE_RESULT_INVALID",
                        "Lifecycle result could not be serialized."
                    ))?,
                ],
            )?;
            Ok(result)
        })
    }

    /// Seals every non-terminal Turn after a process restart. Normal desktop
    /// startup currently performs this through LoadThread + SetTurnTerminal;
    /// this bulk primitive is available for recovery tooling and direct tests.
    pub fn recover_orphaned_turns(&self, updated_at: &str) -> CoreResult<Vec<String>> {
        self.repository.write(|transaction| {
            let mut statement = transaction.prepare(
                "SELECT turn_id, thread_id FROM agent_turns WHERE status IN ('queued', 'running', 'waiting_for_approval', 'waiting_for_clarification') ORDER BY created_at, turn_id",
            )?;
            let rows = statement
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            drop(statement);
            for (turn_id, thread_id) in &rows {
                transaction.execute(
                    "UPDATE agent_turns SET status='failed', error_code='AGENT_RUNTIME_RESTARTED', error_message='The Agent runtime restarted before this Turn completed; the orphaned execution was not resumed.', updated_at=? WHERE turn_id=?",
                    params![updated_at, turn_id],
                )?;
                transaction.execute(
                    "UPDATE agent_threads SET status='error', last_turn_id=?, updated_at=? WHERE thread_id=?",
                    params![turn_id, updated_at, thread_id],
                )?;
            }
            Ok(rows.into_iter().map(|(turn_id, _)| turn_id).collect())
        })
    }
}

fn apply(
    connection: &Connection,
    command: &LifecyclePersistenceCommand,
) -> CoreResult<LifecyclePersistenceResult> {
    match &command.command {
        LifecyclePersistenceOperation::LoadThread { thread_id } => {
            let thread = thread_detail(connection, thread_id)?;
            let revision = thread_revision(&thread)?;
            require_optional_revision(command, &revision)?;
            result(
                command,
                revision,
                LifecyclePersistenceOutcome::ThreadLoaded { thread },
            )
        }
        LifecyclePersistenceOperation::ListThreads {
            project_id,
            include_archived,
            limit,
        } => {
            let threads =
                list_threads(connection, project_id.as_deref(), *include_archived, *limit)?;
            let revision = collection_revision(&threads)?;
            require_optional_revision(command, &revision)?;
            result(
                command,
                revision,
                LifecyclePersistenceOutcome::ThreadsListed { threads },
            )
        }
        LifecyclePersistenceOperation::CreateThread { thread } => {
            if thread_detail(connection, &thread.thread_id)?.is_some() {
                return Err(conflict(
                    "LIFECYCLE_THREAD_EXISTS",
                    "CreateThread targets an existing Thread.",
                ));
            }
            require_optional_revision(command, &thread_revision(&None)?)?;
            if thread.status != AgentThreadStatus::Idle || thread.last_turn_id.is_some() {
                return Err(conflict(
                    "LIFECYCLE_CREATE_THREAD_STATE_INVALID",
                    "A persisted Thread must start idle without a last Turn.",
                ));
            }
            connection.execute(
                "INSERT INTO agent_threads(thread_id, project_id, title, status, summary, provider_id, created_at, updated_at, last_turn_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![thread.thread_id, thread.project_id, thread.title, enum_text(&thread.status)?, thread.summary, thread.provider_id, thread.created_at, thread.updated_at, thread.last_turn_id],
            )?;
            applied(
                connection,
                command,
                &thread.thread_id,
                None,
                None,
                None,
                None,
            )
        }
        LifecyclePersistenceOperation::ArchiveThread { thread } => {
            let current = require_cas(connection, command, &thread.thread_id)?;
            let mut expected = current.summary;
            expected.status = AgentThreadStatus::Archived;
            expected.updated_at = thread.updated_at.clone();
            if &expected != thread {
                return Err(conflict(
                    "LIFECYCLE_ARCHIVE_IDENTITY_DRIFT",
                    "ArchiveThread may update only status and updated_at.",
                ));
            }
            connection.execute(
                "UPDATE agent_threads SET status='archived', updated_at=? WHERE thread_id=?",
                params![thread.updated_at, thread.thread_id],
            )?;
            applied(
                connection,
                command,
                &thread.thread_id,
                None,
                None,
                None,
                None,
            )
        }
        LifecyclePersistenceOperation::CreateTurn { thread_id, turn } => {
            let current = require_cas(connection, command, thread_id)?;
            if turn.status != AgentTurnStatus::Running {
                return Err(conflict(
                    "LIFECYCLE_CREATE_TURN_STATE_INVALID",
                    "A new persisted Turn must start running.",
                ));
            }
            if current.turns.iter().any(|turn| !terminal(&turn.status)) {
                return Err(conflict(
                    "LIFECYCLE_TURN_ALREADY_ACTIVE",
                    "A Thread can have only one non-terminal Turn.",
                ));
            }
            connection.execute(
                "INSERT INTO agent_turns(turn_id, thread_id, request_text, status, error_code, error_message, usage_json, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![turn.turn_id, turn.thread_id, turn.request_text, enum_text(&turn.status)?, turn.error_code, turn.error_message, json_text(&turn.usage)?, turn.created_at, turn.updated_at],
            )?;
            connection.execute(
                "UPDATE agent_threads SET status='active', last_turn_id=?, updated_at=? WHERE thread_id=?",
                params![turn.turn_id, turn.updated_at, thread_id],
            )?;
            applied(
                connection,
                command,
                thread_id,
                Some(&turn.turn_id),
                None,
                None,
                None,
            )
        }
        LifecyclePersistenceOperation::AppendItem {
            item,
            expected_previous_sequence,
        } => {
            require_cas(connection, command, &item.thread_id)?;
            let actual_previous: u64 = connection.query_row(
                "SELECT COALESCE(MAX(sequence), 0) FROM agent_items WHERE thread_id=?",
                [&item.thread_id],
                |row| row.get(0),
            )?;
            if actual_previous != *expected_previous_sequence {
                return Err(conflict(
                    "LIFECYCLE_ITEM_SEQUENCE_CONFLICT",
                    "AppendItem sequence is stale for the thread event stream.",
                ));
            }
            let turn: Option<(String, String)> = connection
                .query_row(
                    "SELECT thread_id, status FROM agent_turns WHERE turn_id=?",
                    [&item.turn_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((turn_thread, turn_status)) = turn else {
                return Err(CoreError::not_found("lifecycle Turn"));
            };
            if turn_thread != item.thread_id || terminal_text(&turn_status) {
                return Err(conflict(
                    "LIFECYCLE_TURN_TERMINAL",
                    "AppendItem targets a missing, foreign or terminal Turn.",
                ));
            }
            connection.execute(
                "INSERT INTO agent_items(item_id, thread_id, turn_id, sequence, item_type, status, payload_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                params![item.item_id, item.thread_id, item.turn_id, item.sequence, enum_text(&item.item_type)?, enum_text(&item.status)?, json_text(&item.payload)?, item.created_at],
            )?;
            let turn_status = if item.item_type == AgentItemType::Clarification {
                "waiting_for_clarification"
            } else {
                turn_status.as_str()
            };
            connection.execute(
                "UPDATE agent_turns SET status=?, updated_at=? WHERE turn_id=?",
                params![turn_status, item.created_at, item.turn_id],
            )?;
            connection.execute(
                "UPDATE agent_threads SET status='active', last_turn_id=?, updated_at=? WHERE thread_id=?",
                params![item.turn_id, item.created_at, item.thread_id],
            )?;
            applied(
                connection,
                command,
                &item.thread_id,
                Some(&item.turn_id),
                Some(&item.item_id),
                None,
                Some(item.sequence),
            )
        }
        LifecyclePersistenceOperation::CreateApproval { approval } => {
            require_cas(connection, command, &approval.thread_id)?;
            let item: Option<(String, String, String, String, u64)> = connection
                .query_row(
                    "SELECT thread_id, turn_id, item_type, status, sequence FROM agent_items WHERE item_id=?",
                    [&approval.item_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
                )
                .optional()?;
            let Some((thread_id, turn_id, item_type, status, sequence)) = item else {
                return Err(CoreError::not_found("approval request Item"));
            };
            if thread_id != approval.thread_id
                || turn_id != approval.turn_id
                || item_type != "approval_request"
                || status != "pending"
            {
                return Err(conflict(
                    "LIFECYCLE_APPROVAL_ITEM_INVALID",
                    "CreateApproval must reference its pending approval_request Item.",
                ));
            }
            connection.execute(
                "INSERT INTO agent_approvals(approval_id, thread_id, turn_id, item_id, action, status, payload_json, created_at, resolved_at) VALUES (?, ?, ?, ?, ?, 'pending', ?, ?, NULL)",
                params![approval.approval_id, approval.thread_id, approval.turn_id, approval.item_id, approval.action, json_text(&approval.payload)?, approval.created_at],
            )?;
            connection.execute(
                "UPDATE agent_turns SET status='waiting_for_approval', updated_at=? WHERE turn_id=?",
                params![approval.created_at, approval.turn_id],
            )?;
            connection.execute(
                "UPDATE agent_threads SET status='active', last_turn_id=?, updated_at=? WHERE thread_id=?",
                params![approval.turn_id, approval.created_at, approval.thread_id],
            )?;
            applied(
                connection,
                command,
                &approval.thread_id,
                Some(&approval.turn_id),
                Some(&approval.item_id),
                Some(&approval.approval_id),
                Some(sequence),
            )
        }
        LifecyclePersistenceOperation::ResolveApproval { approval } => {
            require_cas(connection, command, &approval.thread_id)?;
            let existing = approval_from_connection(connection, &approval.approval_id)?
                .ok_or_else(|| CoreError::not_found("lifecycle Approval"))?;
            if existing.status != ApprovalStatus::Pending {
                return Err(conflict(
                    "LIFECYCLE_APPROVAL_NOT_PENDING",
                    "ResolveApproval requires a pending approval.",
                ));
            }
            let mut expected = existing.clone();
            expected.status = approval.status.clone();
            expected.resolved_at = approval.resolved_at.clone();
            if &expected != approval {
                return Err(conflict(
                    "LIFECYCLE_APPROVAL_IDENTITY_DRIFT",
                    "ResolveApproval changed immutable persisted identity.",
                ));
            }
            let resolved_at = approval.resolved_at.as_deref().ok_or_else(|| {
                conflict(
                    "LIFECYCLE_APPROVAL_RESOLUTION_INVALID",
                    "Resolved approval is missing resolved_at.",
                )
            })?;
            let approved = approval.status == ApprovalStatus::Approved;
            connection.execute(
                "UPDATE agent_approvals SET status=?, resolved_at=? WHERE approval_id=? AND status='pending'",
                params![enum_text(&approval.status)?, resolved_at, approval.approval_id],
            )?;
            connection.execute(
                "UPDATE agent_items SET status=? WHERE item_id=?",
                params![
                    if approved { "completed" } else { "cancelled" },
                    approval.item_id
                ],
            )?;
            connection.execute(
                "UPDATE agent_turns SET status=?, error_code=NULL, error_message=NULL, updated_at=? WHERE turn_id=?",
                params![if approved { "running" } else { "cancelled" }, resolved_at, approval.turn_id],
            )?;
            connection.execute(
                "UPDATE agent_threads SET status=?, last_turn_id=?, updated_at=? WHERE thread_id=?",
                params![
                    if approved { "active" } else { "idle" },
                    approval.turn_id,
                    resolved_at,
                    approval.thread_id
                ],
            )?;
            let sequence: u64 = connection.query_row(
                "SELECT sequence FROM agent_items WHERE item_id=?",
                [&approval.item_id],
                |row| row.get(0),
            )?;
            applied(
                connection,
                command,
                &approval.thread_id,
                Some(&approval.turn_id),
                Some(&approval.item_id),
                Some(&approval.approval_id),
                Some(sequence),
            )
        }
        LifecyclePersistenceOperation::SetTurnTerminal { turn } => {
            let thread = require_cas(connection, command, &turn.thread_id)?;
            let current = thread
                .turns
                .iter()
                .find(|item| item.turn_id == turn.turn_id)
                .ok_or_else(|| CoreError::not_found("lifecycle Turn"))?;
            if !valid_terminal_transition(&current.status, &turn.status) {
                return Err(conflict(
                    "LIFECYCLE_TURN_TRANSITION_INVALID",
                    "SetTurnTerminal is not an allowed lifecycle transition.",
                ));
            }
            let mut expected = current.clone();
            expected.status = turn.status.clone();
            expected.error_code = turn.error_code.clone();
            expected.error_message = turn.error_message.clone();
            expected.usage = turn.usage.clone();
            expected.updated_at = turn.updated_at.clone();
            if &expected != turn {
                return Err(conflict(
                    "LIFECYCLE_TURN_IDENTITY_DRIFT",
                    "Terminal update may change only status, error, usage/trace and updated_at.",
                ));
            }
            if turn.status == AgentTurnStatus::Completed
                && current
                    .approvals
                    .iter()
                    .any(|item| item.status == ApprovalStatus::Pending)
            {
                return Err(conflict(
                    "LIFECYCLE_APPROVAL_PENDING",
                    "A Turn with pending approval cannot complete.",
                ));
            }
            connection.execute(
                "UPDATE agent_turns SET status=?, error_code=?, error_message=?, usage_json=?, updated_at=? WHERE turn_id=?",
                params![enum_text(&turn.status)?, turn.error_code, turn.error_message, json_text(&turn.usage)?, turn.updated_at, turn.turn_id],
            )?;
            connection.execute(
                "UPDATE agent_threads SET status=?, last_turn_id=?, updated_at=? WHERE thread_id=?",
                params![
                    if turn.status == AgentTurnStatus::Failed {
                        "error"
                    } else {
                        "idle"
                    },
                    turn.turn_id,
                    turn.updated_at,
                    turn.thread_id
                ],
            )?;
            applied(
                connection,
                command,
                &turn.thread_id,
                Some(&turn.turn_id),
                None,
                None,
                None,
            )
        }
        LifecyclePersistenceOperation::ReplayItems {
            thread_id,
            after_sequence,
            limit,
        } => {
            let thread = thread_detail(connection, thread_id)?;
            let revision = thread_revision(&thread)?;
            require_optional_revision(command, &revision)?;
            let mut statement = connection.prepare(
                "SELECT item_id, thread_id, turn_id, sequence, item_type, status, payload_json, created_at FROM agent_items WHERE thread_id=? AND sequence>? ORDER BY sequence LIMIT ?",
            )?;
            let rows = statement
                .query_map(
                    params![thread_id, after_sequence, u64::from(*limit) + 1],
                    item_from_row,
                )?
                .collect::<Result<Vec<_>, _>>()?;
            let next_sequence =
                (rows.len() > usize::from(*limit)).then(|| rows[usize::from(*limit)].sequence);
            let items = rows.into_iter().take(usize::from(*limit)).collect();
            result(
                command,
                revision,
                LifecyclePersistenceOutcome::ItemsReplayed {
                    thread_id: thread_id.clone(),
                    items,
                    next_sequence,
                },
            )
        }
    }
}

fn thread_detail(
    connection: &Connection,
    thread_id: &str,
) -> CoreResult<Option<AgentThreadDetail>> {
    let summary = connection
        .query_row(
            "SELECT thread_id, project_id, title, status, summary, provider_id, created_at, updated_at, last_turn_id FROM agent_threads WHERE thread_id=?",
            [thread_id],
            thread_summary_from_row,
        )
        .optional()?;
    let Some(summary) = summary else {
        return Ok(None);
    };

    let mut item_statement = connection.prepare(
        "SELECT item_id, thread_id, turn_id, sequence, item_type, status, payload_json, created_at FROM agent_items WHERE thread_id=? ORDER BY sequence",
    )?;
    let items = item_statement
        .query_map([thread_id], item_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    let mut approval_statement = connection.prepare(
        "SELECT approval_id, thread_id, turn_id, item_id, action, status, payload_json, created_at, resolved_at FROM agent_approvals WHERE thread_id=? ORDER BY created_at, approval_id",
    )?;
    let approvals = approval_statement
        .query_map([thread_id], approval_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    let mut turn_statement = connection.prepare(
        "SELECT turn_id, thread_id, request_text, status, error_code, error_message, usage_json, created_at, updated_at FROM agent_turns WHERE thread_id=? ORDER BY created_at, turn_id",
    )?;
    let raw_turns = turn_statement
        .query_map([thread_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let turns = raw_turns
        .into_iter()
        .map(|row| {
            let turn_id = row.0;
            Ok(AgentTurn {
                turn_id: turn_id.clone(),
                thread_id: row.1,
                request_text: row.2,
                status: enum_parse(&row.3)?,
                error_code: row.4,
                error_message: row.5,
                usage: parse_json(row.6)?,
                created_at: row.7,
                updated_at: row.8,
                items: items
                    .iter()
                    .filter(|item| item.turn_id == turn_id)
                    .cloned()
                    .collect(),
                approvals: approvals
                    .iter()
                    .filter(|item| item.turn_id == turn_id)
                    .cloned()
                    .collect(),
            })
        })
        .collect::<CoreResult<Vec<_>>>()?;
    let detail = AgentThreadDetail { summary, turns };
    detail
        .validate()
        .map_err(|error| CoreError::invalid_data("LIFECYCLE_ROW_INVALID", error.message))?;
    Ok(Some(detail))
}

fn list_threads(
    connection: &Connection,
    project_id: Option<&str>,
    include_archived: bool,
    limit: u16,
) -> CoreResult<Vec<AgentThreadSummary>> {
    let mut sql = String::from("SELECT thread_id, project_id, title, status, summary, provider_id, created_at, updated_at, last_turn_id FROM agent_threads WHERE 1=1");
    let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(project_id) = project_id {
        sql.push_str(" AND project_id=?");
        values.push(Box::new(project_id.to_string()));
    }
    if !include_archived {
        sql.push_str(" AND status!='archived'");
    }
    sql.push_str(" ORDER BY updated_at DESC, thread_id DESC LIMIT ?");
    values.push(Box::new(limit));
    let refs = values
        .iter()
        .map(|value| value.as_ref())
        .collect::<Vec<_>>();
    let mut statement = connection.prepare(&sql)?;
    let threads = statement
        .query_map(refs.as_slice(), thread_summary_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(threads)
}

fn require_cas(
    connection: &Connection,
    command: &LifecyclePersistenceCommand,
    thread_id: &str,
) -> CoreResult<AgentThreadDetail> {
    let thread = thread_detail(connection, thread_id)?
        .ok_or_else(|| CoreError::not_found("lifecycle Thread"))?;
    require_expected(command, &thread_revision(&Some(thread.clone()))?)?;
    Ok(thread)
}

fn require_expected(command: &LifecyclePersistenceCommand, actual: &str) -> CoreResult<()> {
    if command.expected_revision.as_deref() == Some(actual) {
        Ok(())
    } else {
        Err(conflict(
            "LIFECYCLE_CAS_CONFLICT",
            "Lifecycle expected_revision is stale.",
        ))
    }
}

fn require_optional_revision(
    command: &LifecyclePersistenceCommand,
    actual: &str,
) -> CoreResult<()> {
    if command
        .expected_revision
        .as_deref()
        .is_none_or(|expected| expected == actual)
    {
        Ok(())
    } else {
        Err(conflict(
            "LIFECYCLE_CAS_CONFLICT",
            "Lifecycle expected_revision is stale.",
        ))
    }
}

fn thread_revision(thread: &Option<AgentThreadDetail>) -> CoreResult<String> {
    Ok(format!("sha256:{}", semantic_sha256(thread)?))
}
fn collection_revision(threads: &[AgentThreadSummary]) -> CoreResult<String> {
    Ok(format!("sha256:{}", semantic_sha256(threads)?))
}

fn applied(
    connection: &Connection,
    command: &LifecyclePersistenceCommand,
    thread_id: &str,
    turn_id: Option<&str>,
    item_id: Option<&str>,
    approval_id: Option<&str>,
    sequence: Option<u64>,
) -> CoreResult<LifecyclePersistenceResult> {
    let revision = thread_revision(&thread_detail(connection, thread_id)?)?;
    result(
        command,
        revision,
        LifecyclePersistenceOutcome::Applied {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.map(str::to_string),
            item_id: item_id.map(str::to_string),
            approval_id: approval_id.map(str::to_string),
            sequence,
        },
    )
}

fn result(
    command: &LifecyclePersistenceCommand,
    revision: String,
    outcome: LifecyclePersistenceOutcome,
) -> CoreResult<LifecyclePersistenceResult> {
    Ok(LifecyclePersistenceResult {
        schema_version: LIFECYCLE_PERSISTENCE_RESULT_SCHEMA_VERSION.to_string(),
        command_id: command.command_id.clone(),
        revision,
        replayed: false,
        result: outcome,
    })
}

fn thread_summary_from_row(row: &Row<'_>) -> rusqlite::Result<AgentThreadSummary> {
    let status: String = row.get(3)?;
    Ok(AgentThreadSummary {
        thread_id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        status: enum_parse(&status).map_err(to_sql_error)?,
        summary: row.get(4)?,
        provider_id: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        last_turn_id: row.get(8)?,
    })
}

fn item_from_row(row: &Row<'_>) -> rusqlite::Result<AgentItem> {
    let item_type: String = row.get(4)?;
    let status: String = row.get(5)?;
    Ok(AgentItem {
        item_id: row.get(0)?,
        thread_id: row.get(1)?,
        turn_id: row.get(2)?,
        sequence: row.get(3)?,
        item_type: enum_parse(&item_type).map_err(to_sql_error)?,
        status: enum_parse(&status).map_err(to_sql_error)?,
        payload: parse_json(row.get::<_, String>(6)?).map_err(to_sql_error)?,
        created_at: row.get(7)?,
    })
}

fn approval_from_connection(
    connection: &Connection,
    id: &str,
) -> CoreResult<Option<AgentApproval>> {
    connection.query_row("SELECT approval_id, thread_id, turn_id, item_id, action, status, payload_json, created_at, resolved_at FROM agent_approvals WHERE approval_id=?", [id], approval_from_row).optional().map_err(Into::into)
}

fn approval_from_row(row: &Row<'_>) -> rusqlite::Result<AgentApproval> {
    let status: String = row.get(5)?;
    Ok(AgentApproval {
        approval_id: row.get(0)?,
        thread_id: row.get(1)?,
        turn_id: row.get(2)?,
        item_id: row.get(3)?,
        action: row.get(4)?,
        status: enum_parse(&status).map_err(to_sql_error)?,
        payload: parse_json(row.get::<_, String>(6)?).map_err(to_sql_error)?,
        created_at: row.get(7)?,
        resolved_at: row.get(8)?,
    })
}

fn enum_text<T: Serialize>(value: &T) -> CoreResult<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .ok_or_else(|| {
            CoreError::invalid_data(
                "LIFECYCLE_ENUM_INVALID",
                "Lifecycle enum could not be serialized.",
            )
        })
}

fn enum_parse<T: serde::de::DeserializeOwned>(value: &str) -> CoreResult<T> {
    serde_json::from_value(Value::String(value.to_string())).map_err(|_| {
        CoreError::invalid_data(
            "LIFECYCLE_ENUM_INVALID",
            "Persisted lifecycle enum is invalid.",
        )
    })
}

fn json_text<T: Serialize>(value: &T) -> CoreResult<String> {
    serde_json::to_string(value).map_err(|_| {
        CoreError::invalid_data(
            "LIFECYCLE_JSON_INVALID",
            "Lifecycle JSON could not be serialized.",
        )
    })
}
fn parse_json<T: serde::de::DeserializeOwned>(value: String) -> CoreResult<T> {
    serde_json::from_str(&value).map_err(|_| {
        CoreError::invalid_data(
            "LIFECYCLE_ROW_INVALID",
            "Persisted lifecycle JSON is invalid.",
        )
    })
}
fn to_sql_error(_error: CoreError) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}
fn conflict(code: &'static str, message: &'static str) -> CoreError {
    CoreError::conflict(code, message)
}
fn terminal(status: &AgentTurnStatus) -> bool {
    matches!(
        status,
        AgentTurnStatus::Completed | AgentTurnStatus::Failed | AgentTurnStatus::Cancelled
    )
}
fn terminal_text(status: &str) -> bool {
    matches!(status, "completed" | "failed" | "cancelled")
}

fn valid_terminal_transition(current: &AgentTurnStatus, target: &AgentTurnStatus) -> bool {
    matches!(
        (current, target),
        (AgentTurnStatus::Queued, AgentTurnStatus::Cancelled)
            | (
                AgentTurnStatus::Running,
                AgentTurnStatus::Completed | AgentTurnStatus::Failed | AgentTurnStatus::Cancelled
            )
            | (
                AgentTurnStatus::WaitingForApproval,
                AgentTurnStatus::Failed | AgentTurnStatus::Cancelled
            )
            | (
                AgentTurnStatus::WaitingForClarification,
                AgentTurnStatus::Cancelled
            )
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use forgecad_app_server_protocol::{
        AgentItemStatus, LifecyclePersistenceOperation,
        LIFECYCLE_PERSISTENCE_COMMAND_SCHEMA_VERSION,
    };
    use tempfile::tempdir;

    use super::*;
    use crate::{ContentAddressedObjectStore, MigrationRunner, StateOwner, WriterLease};

    fn store() -> (tempfile::TempDir, LifecycleStore) {
        let root = tempdir().unwrap();
        let db = root.path().join("library.db");
        MigrationRunner::new(&db).run().unwrap();
        let lease = WriterLease::acquire(
            &db,
            root.path(),
            "lifecycle-test",
            StateOwner::PythonCompatibilityAdapter,
        )
        .unwrap();
        let repo = CoreRepository::new(
            lease,
            ContentAddressedObjectStore::new(root.path()).unwrap(),
        )
        .unwrap();
        (root, LifecycleStore::new(repo))
    }

    fn command(
        id: &str,
        key: char,
        revision: Option<String>,
        operation: LifecyclePersistenceOperation,
    ) -> LifecyclePersistenceCommand {
        LifecyclePersistenceCommand {
            schema_version: LIFECYCLE_PERSISTENCE_COMMAND_SCHEMA_VERSION.into(),
            command_id: id.into(),
            idempotency_key: key.to_string().repeat(64),
            expected_revision: revision,
            command: operation,
        }
    }

    fn thread() -> AgentThreadSummary {
        AgentThreadSummary {
            thread_id: "thread_a".into(),
            project_id: None,
            title: "Test".into(),
            status: AgentThreadStatus::Idle,
            summary: String::new(),
            provider_id: "deepseek".into(),
            created_at: "2026-07-17T00:00:00Z".into(),
            updated_at: "2026-07-17T00:00:00Z".into(),
            last_turn_id: None,
        }
    }

    #[test]
    fn lifecycle_create_replay_cas_sequence_terminal_usage_and_restart_readback() {
        let (_root, store) = store();
        let created = store
            .execute_lifecycle(command(
                "cmd_create",
                'a',
                None,
                LifecyclePersistenceOperation::CreateThread { thread: thread() },
            ))
            .unwrap();
        let replay = store
            .execute_lifecycle(command(
                "cmd_replay",
                'a',
                None,
                LifecyclePersistenceOperation::CreateThread { thread: thread() },
            ))
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.revision, created.revision);

        let turn = AgentTurn {
            turn_id: "turn_a".into(),
            thread_id: "thread_a".into(),
            request_text: "Design a concept".into(),
            status: AgentTurnStatus::Running,
            error_code: None,
            error_message: None,
            usage: BTreeMap::new(),
            created_at: "2026-07-17T00:00:01Z".into(),
            updated_at: "2026-07-17T00:00:01Z".into(),
            items: vec![],
            approvals: vec![],
        };
        let started = store
            .execute_lifecycle(command(
                "cmd_turn",
                'b',
                Some(created.revision),
                LifecyclePersistenceOperation::CreateTurn {
                    thread_id: "thread_a".into(),
                    turn: turn.clone(),
                },
            ))
            .unwrap();
        let item = AgentItem {
            item_id: "item_a".into(),
            thread_id: "thread_a".into(),
            turn_id: "turn_a".into(),
            sequence: 1,
            item_type: AgentItemType::AssistantMessage,
            status: AgentItemStatus::Completed,
            payload: BTreeMap::from([("text".into(), Value::String("working".into()))]),
            created_at: "2026-07-17T00:00:02Z".into(),
        };
        let appended = store
            .execute_lifecycle(command(
                "cmd_item",
                'c',
                Some(started.revision),
                LifecyclePersistenceOperation::AppendItem {
                    item: item.clone(),
                    expected_previous_sequence: 0,
                },
            ))
            .unwrap();
        let mut terminal = turn;
        terminal.status = AgentTurnStatus::Completed;
        terminal.updated_at = "2026-07-17T00:00:03Z".into();
        terminal.items.push(item);
        terminal
            .usage
            .insert("input_tokens".into(), Value::from(12));
        terminal.usage.insert(
            "provider_trace".into(),
            Value::String("trace-redacted".into()),
        );
        let finished = store
            .execute_lifecycle(command(
                "cmd_terminal",
                'd',
                Some(appended.revision),
                LifecyclePersistenceOperation::SetTurnTerminal {
                    turn: terminal.clone(),
                },
            ))
            .unwrap();
        let loaded = store
            .execute_lifecycle(command(
                "cmd_load",
                'e',
                Some(finished.revision),
                LifecyclePersistenceOperation::LoadThread {
                    thread_id: "thread_a".into(),
                },
            ))
            .unwrap();
        let LifecyclePersistenceOutcome::ThreadLoaded {
            thread: Some(thread),
        } = loaded.result
        else {
            panic!("thread missing")
        };
        assert_eq!(thread.turns[0].usage, terminal.usage);
        assert_eq!(thread.turns[0].items.len(), 1);
    }

    #[test]
    fn orphan_recovery_is_terminal_and_preserves_existing_usage_trace() {
        let (_root, store) = store();
        let created = store
            .execute_lifecycle(command(
                "cmd_create",
                'f',
                None,
                LifecyclePersistenceOperation::CreateThread { thread: thread() },
            ))
            .unwrap();
        let mut turn = AgentTurn {
            turn_id: "turn_orphan".into(),
            thread_id: "thread_a".into(),
            request_text: "orphan".into(),
            status: AgentTurnStatus::Running,
            error_code: None,
            error_message: None,
            usage: BTreeMap::from([("provider_trace".into(), Value::String("kept".into()))]),
            created_at: "2026-07-17T00:00:01Z".into(),
            updated_at: "2026-07-17T00:00:01Z".into(),
            items: vec![],
            approvals: vec![],
        };
        store
            .execute_lifecycle(command(
                "cmd_turn",
                '1',
                Some(created.revision),
                LifecyclePersistenceOperation::CreateTurn {
                    thread_id: "thread_a".into(),
                    turn: turn.clone(),
                },
            ))
            .unwrap();
        assert_eq!(
            store
                .recover_orphaned_turns("2026-07-17T00:00:02Z")
                .unwrap(),
            vec!["turn_orphan"]
        );
        let loaded = store
            .execute_lifecycle(command(
                "cmd_load",
                '2',
                None,
                LifecyclePersistenceOperation::LoadThread {
                    thread_id: "thread_a".into(),
                },
            ))
            .unwrap();
        let LifecyclePersistenceOutcome::ThreadLoaded {
            thread: Some(thread),
        } = loaded.result
        else {
            panic!("thread missing")
        };
        turn = thread.turns[0].clone();
        assert_eq!(turn.status, AgentTurnStatus::Failed);
        assert_eq!(turn.error_code.as_deref(), Some("AGENT_RUNTIME_RESTARTED"));
        assert_eq!(
            turn.usage.get("provider_trace"),
            Some(&Value::String("kept".into()))
        );
    }
}
