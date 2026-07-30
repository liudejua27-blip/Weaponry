//! Rust-owned E005 formal batch checkpoints.
//!
//! This module persists only bounded task identities and sealed receipt JSON.
//! A task is claimed before Provider work. Restart recovery may return a task
//! to `pending` only when no network-attempted reservation exists; otherwise
//! it becomes `reconciliation_required` and can never be auto-retried.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    canonical_json, semantic_sha256, CoreError, CoreRepository, CoreResult,
    E005_FORMAL_TASK_SET_SHA256, E005_TASK_COUNT,
};

pub const E005_FORMAL_BATCH_SCHEMA_VERSION: &str = "E005FormalBatchCheckpoint@1";
pub const E005_FORMAL_BATCH_TASK_SCHEMA_VERSION: &str = "E005FormalBatchTaskCheckpoint@1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum E005FormalBatchStatus {
    Ready,
    Running,
    ReconciliationRequired,
    Completed,
    Cancelled,
}

impl E005FormalBatchStatus {
    fn parse(value: &str) -> CoreResult<Self> {
        match value {
            "ready" => Ok(Self::Ready),
            "running" => Ok(Self::Running),
            "reconciliation_required" => Ok(Self::ReconciliationRequired),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(invalid(
                "E005_FORMAL_BATCH_STATUS_INVALID",
                "Persisted E005 batch status is invalid.",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum E005FormalBatchTaskState {
    Pending,
    Running,
    ReceiptSealed,
    ReconciliationRequired,
}

impl E005FormalBatchTaskState {
    fn parse(value: &str) -> CoreResult<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "receipt_sealed" => Ok(Self::ReceiptSealed),
            "reconciliation_required" => Ok(Self::ReconciliationRequired),
            _ => Err(invalid(
                "E005_FORMAL_BATCH_TASK_STATE_INVALID",
                "Persisted E005 task checkpoint state is invalid.",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005FormalBatchTaskCheckpoint {
    pub schema_version: String,
    pub task_id: String,
    pub task_payload_sha256: String,
    pub task_ordinal: u8,
    pub state: E005FormalBatchTaskState,
    pub receipt_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005FormalBatchCheckpoint {
    pub schema_version: String,
    pub batch_id: String,
    pub authorization_id: String,
    pub task_set_sha256: String,
    pub status: E005FormalBatchStatus,
    pub total_task_count: u8,
    pub sealed_receipt_count: u8,
    pub tasks: Vec<E005FormalBatchTaskCheckpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005FormalBatchTaskClaim {
    pub batch_id: String,
    pub authorization_id: String,
    pub task_id: String,
    pub task_payload_sha256: String,
    pub task_ordinal: u8,
}

impl CoreRepository {
    pub fn start_e005_formal_batch(
        &self,
        batch_id: &str,
        authorization_id: &str,
    ) -> CoreResult<E005FormalBatchCheckpoint> {
        require_id(batch_id)?;
        require_id(authorization_id)?;
        self.write(|transaction| {
            if let Some(existing) = read_batch(transaction, batch_id)? {
                if existing.authorization_id != authorization_id {
                    return Err(CoreError::conflict(
                        "E005_FORMAL_BATCH_IDEMPOTENCY_CONFLICT",
                        "E005 batch ID is already bound to another authorization.",
                    ));
                }
                return Ok(existing);
            }
            if transaction
                .query_row(
                    "SELECT batch_id FROM e005_formal_batches WHERE authorization_id=?",
                    [authorization_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .is_some()
            {
                return Err(CoreError::conflict(
                    "E005_FORMAL_BATCH_AUTHORIZATION_ALREADY_BOUND",
                    "E005 authorization is already bound to another formal batch.",
                ));
            }
            let (task_set_sha256, status): (String, String) = transaction
                .query_row(
                    "SELECT task_set_sha256, status FROM e005_provider_run_authorizations WHERE authorization_id=?",
                    [authorization_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(|error| match error {
                    rusqlite::Error::QueryReturnedNoRows => CoreError::not_found("E005 Provider authorization"),
                    other => CoreError::Sqlite(other),
                })?;
            if status != "authorized" || task_set_sha256 != E005_FORMAL_TASK_SET_SHA256 {
                return Err(CoreError::conflict(
                    "E005_FORMAL_BATCH_AUTHORIZATION_INVALID",
                    "E005 batch requires one active authorization for the frozen task set.",
                ));
            }
            let task_count: u8 = transaction.query_row(
                "SELECT COUNT(*) FROM e005_provider_authorized_tasks WHERE authorization_id=?",
                [authorization_id],
                |row| row.get(0),
            )?;
            if task_count != E005_TASK_COUNT as u8 {
                return Err(invalid(
                    "E005_FORMAL_BATCH_TASK_COUNT_INVALID",
                    "E005 authorization does not contain exactly 30 frozen tasks.",
                ));
            }
            let now = unix_ms()?;
            transaction.execute(
                "INSERT INTO e005_formal_batches(batch_id, authorization_id, task_set_sha256, status, total_task_count, sealed_receipt_count, created_at_unix_ms, updated_at_unix_ms) VALUES (?,?,?,'ready',30,0,?,?)",
                params![batch_id, authorization_id, task_set_sha256, now, now],
            )?;
            transaction.execute(
                "INSERT INTO e005_formal_batch_tasks(batch_id, authorization_id, task_id, task_payload_sha256, task_ordinal, state) SELECT ?, authorization_id, task_id, task_payload_sha256, task_ordinal, 'pending' FROM e005_provider_authorized_tasks WHERE authorization_id=? ORDER BY task_ordinal",
                params![batch_id, authorization_id],
            )?;
            read_batch(transaction, batch_id)?.ok_or_else(|| CoreError::not_found("E005 formal batch"))
        })
    }

    pub fn claim_next_e005_formal_batch_task(
        &self,
        batch_id: &str,
    ) -> CoreResult<Option<E005FormalBatchTaskClaim>> {
        require_id(batch_id)?;
        self.write(|transaction| {
            let (authorization_id, status): (String, String) = transaction
                .query_row(
                    "SELECT authorization_id, status FROM e005_formal_batches WHERE batch_id=?",
                    [batch_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(|error| match error {
                    rusqlite::Error::QueryReturnedNoRows => CoreError::not_found("E005 formal batch"),
                    other => CoreError::Sqlite(other),
                })?;
            if matches!(status.as_str(), "reconciliation_required" | "cancelled") {
                return Err(CoreError::conflict(
                    "E005_FORMAL_BATCH_NOT_RUNNABLE",
                    "E005 batch requires reconciliation or is cancelled.",
                ));
            }
            if status == "completed" {
                return Ok(None);
            }
            let running: u8 = transaction.query_row(
                "SELECT COUNT(*) FROM e005_formal_batch_tasks WHERE batch_id=? AND state='running'",
                [batch_id],
                |row| row.get(0),
            )?;
            if running != 0 {
                return Err(CoreError::conflict(
                    "E005_FORMAL_BATCH_TASK_ALREADY_RUNNING",
                    "E005 batch permits only one claimed task at a time.",
                ));
            }
            let next: Option<(String, String, u8)> = transaction
                .query_row(
                    "SELECT task_id, task_payload_sha256, task_ordinal FROM e005_formal_batch_tasks WHERE batch_id=? AND state='pending' ORDER BY task_ordinal LIMIT 1",
                    [batch_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            let Some((task_id, task_payload_sha256, task_ordinal)) = next else {
                let sealed: u8 = transaction.query_row(
                    "SELECT COUNT(*) FROM e005_formal_batch_tasks WHERE batch_id=? AND state='receipt_sealed'",
                    [batch_id],
                    |row| row.get(0),
                )?;
                if sealed == E005_TASK_COUNT as u8 {
                    transaction.execute(
                        "UPDATE e005_formal_batches SET status='completed', sealed_receipt_count=30, updated_at_unix_ms=? WHERE batch_id=?",
                        params![unix_ms()?, batch_id],
                    )?;
                    return Ok(None);
                }
                return Err(invalid(
                    "E005_FORMAL_BATCH_PENDING_TASK_MISSING",
                    "E005 batch has neither a pending task nor 30 sealed receipts.",
                ));
            };
            let now = unix_ms()?;
            let changed = transaction.execute(
                "UPDATE e005_formal_batch_tasks SET state='running', started_at_unix_ms=? WHERE batch_id=? AND task_id=? AND state='pending'",
                params![now, batch_id, task_id],
            )?;
            if changed != 1 {
                return Err(CoreError::conflict(
                    "E005_FORMAL_BATCH_TASK_CLAIM_NOT_ACQUIRED",
                    "E005 task claim was not atomically acquired.",
                ));
            }
            transaction.execute(
                "UPDATE e005_formal_batches SET status='running', updated_at_unix_ms=? WHERE batch_id=?",
                params![now, batch_id],
            )?;
            Ok(Some(E005FormalBatchTaskClaim {
                batch_id: batch_id.into(),
                authorization_id,
                task_id,
                task_payload_sha256,
                task_ordinal,
            }))
        })
    }

    pub fn seal_e005_formal_batch_receipt(
        &self,
        batch_id: &str,
        task_id: &str,
        receipt: &Value,
    ) -> CoreResult<E005FormalBatchCheckpoint> {
        require_id(batch_id)?;
        require_id(task_id)?;
        let receipt_json = canonical_json(receipt)?;
        let receipt_sha256 = semantic_sha256(receipt)?;
        self.write(|transaction| {
            let (authorization_id, task_set_sha256, task_payload_sha256, state, existing_sha): (String, String, String, String, Option<String>) = transaction.query_row(
                "SELECT b.authorization_id, b.task_set_sha256, t.task_payload_sha256, t.state, t.receipt_sha256 FROM e005_formal_batches b JOIN e005_formal_batch_tasks t ON t.batch_id=b.batch_id WHERE b.batch_id=? AND t.task_id=?",
                params![batch_id, task_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            ).map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => CoreError::not_found("E005 formal batch task"),
                other => CoreError::Sqlite(other),
            })?;
            if state == "receipt_sealed" {
                if existing_sha.as_deref() != Some(receipt_sha256.as_str()) {
                    return Err(CoreError::conflict(
                        "E005_FORMAL_BATCH_RECEIPT_IDEMPOTENCY_CONFLICT",
                        "E005 task receipt was already sealed with different evidence.",
                    ));
                }
                return read_batch(transaction, batch_id)?.ok_or_else(|| CoreError::not_found("E005 formal batch"));
            }
            if state != "running" {
                return Err(CoreError::conflict(
                    "E005_FORMAL_BATCH_RECEIPT_STATE_INVALID",
                    "Only the currently running E005 task can seal a receipt.",
                ));
            }
            validate_receipt_binding(
                transaction,
                receipt,
                &authorization_id,
                &task_set_sha256,
                task_id,
                &task_payload_sha256,
            )?;
            let now = unix_ms()?;
            let changed = transaction.execute(
                "UPDATE e005_formal_batch_tasks SET state='receipt_sealed', receipt_json=?, receipt_sha256=?, sealed_at_unix_ms=? WHERE batch_id=? AND task_id=? AND state='running'",
                params![receipt_json, receipt_sha256, now, batch_id, task_id],
            )?;
            if changed != 1 {
                return Err(CoreError::conflict(
                    "E005_FORMAL_BATCH_RECEIPT_SEAL_NOT_ACQUIRED",
                    "E005 receipt seal was not atomically acquired.",
                ));
            }
            let sealed: u8 = transaction.query_row(
                "SELECT COUNT(*) FROM e005_formal_batch_tasks WHERE batch_id=? AND state='receipt_sealed'",
                [batch_id],
                |row| row.get(0),
            )?;
            let status = if sealed == E005_TASK_COUNT as u8 { "completed" } else { "running" };
            transaction.execute(
                "UPDATE e005_formal_batches SET status=?, sealed_receipt_count=?, updated_at_unix_ms=? WHERE batch_id=?",
                params![status, sealed, now, batch_id],
            )?;
            read_batch(transaction, batch_id)?.ok_or_else(|| CoreError::not_found("E005 formal batch"))
        })
    }

    pub fn recover_e005_formal_batches_after_provider_recovery(
        &self,
    ) -> CoreResult<Vec<E005FormalBatchCheckpoint>> {
        self.write(|transaction| {
            let running = {
                let mut statement = transaction.prepare(
                    "SELECT batch_id, task_id, authorization_id FROM e005_formal_batch_tasks WHERE state='running' ORDER BY batch_id, task_ordinal",
                )?;
                let rows = statement
                    .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))?
                    .collect::<Result<Vec<_>, _>>()?;
                rows
            };
            for (batch_id, task_id, authorization_id) in running {
                let resumable_visual_review: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM e005_visual_review_checkpoints c WHERE c.authorization_id=? AND c.task_id=? AND c.state='awaiting_visual_review') AND NOT EXISTS(SELECT 1 FROM e005_provider_call_reservations r WHERE r.authorization_id=? AND r.task_id=? AND r.call_kind='patch' AND (r.network_call_made=1 OR r.state IN ('dispatching','accounted')))",
                    params![authorization_id, task_id, authorization_id, task_id],
                    |row| row.get(0),
                )?;
                let attempted: u8 = transaction.query_row(
                    "SELECT COUNT(*) FROM e005_provider_call_reservations WHERE authorization_id=? AND task_id=? AND (network_call_made=1 OR state IN ('dispatching','accounted'))",
                    params![authorization_id, task_id],
                    |row| row.get(0),
                )?;
                if attempted == 0 || resumable_visual_review {
                    transaction.execute(
                        "UPDATE e005_formal_batch_tasks SET state='pending', started_at_unix_ms=NULL WHERE batch_id=? AND task_id=? AND state='running'",
                        params![batch_id, task_id],
                    )?;
                } else {
                    transaction.execute(
                        "UPDATE e005_formal_batch_tasks SET state='reconciliation_required' WHERE batch_id=? AND task_id=? AND state='running'",
                        params![batch_id, task_id],
                    )?;
                }
            }
            let batch_ids = {
                let mut statement = transaction.prepare(
                    "SELECT batch_id FROM e005_formal_batches WHERE status IN ('ready','running','reconciliation_required') ORDER BY batch_id",
                )?;
                let rows = statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                rows
            };
            let now = unix_ms()?;
            let mut recovered = Vec::new();
            for batch_id in batch_ids {
                let reconciliation: u8 = transaction.query_row(
                    "SELECT COUNT(*) FROM e005_formal_batch_tasks WHERE batch_id=? AND state='reconciliation_required'",
                    [batch_id.as_str()],
                    |row| row.get(0),
                )?;
                let sealed: u8 = transaction.query_row(
                    "SELECT COUNT(*) FROM e005_formal_batch_tasks WHERE batch_id=? AND state='receipt_sealed'",
                    [batch_id.as_str()],
                    |row| row.get(0),
                )?;
                let status = if reconciliation > 0 {
                    "reconciliation_required"
                } else if sealed == E005_TASK_COUNT as u8 {
                    "completed"
                } else {
                    "ready"
                };
                transaction.execute(
                    "UPDATE e005_formal_batches SET status=?, sealed_receipt_count=?, updated_at_unix_ms=? WHERE batch_id=?",
                    params![status, sealed, now, batch_id],
                )?;
                recovered.push(read_batch(transaction, &batch_id)?.ok_or_else(|| CoreError::not_found("E005 formal batch"))?);
            }
            Ok(recovered)
        })
    }

    pub fn e005_formal_batch_checkpoint(
        &self,
        batch_id: &str,
    ) -> CoreResult<Option<E005FormalBatchCheckpoint>> {
        require_id(batch_id)?;
        let connection = Connection::open(self.db_path())?;
        connection.busy_timeout(Duration::from_millis(5_000))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        read_batch(&connection, batch_id)
    }
}

fn validate_receipt_binding(
    connection: &Connection,
    receipt: &Value,
    authorization_id: &str,
    task_set_sha256: &str,
    task_id: &str,
    task_payload_sha256: &str,
) -> CoreResult<()> {
    let authorization_json: String = connection.query_row(
        "SELECT authorization_json FROM e005_provider_run_authorizations WHERE authorization_id=?",
        [authorization_id],
        |row| row.get(0),
    )?;
    let authorization: Value = serde_json::from_str(&authorization_json).map_err(|_| {
        invalid(
            "E005_FORMAL_BATCH_AUTHORIZATION_JSON_INVALID",
            "Persisted E005 authorization JSON is invalid.",
        )
    })?;
    let authorization_sha256 = semantic_sha256(&authorization)?;
    let provider_evidence = receipt
        .get("provider_call_evidence")
        .and_then(Value::as_array);
    let production = receipt
        .get("production_review_evidence")
        .and_then(Value::as_object);
    let production_value = receipt.get("production_review_evidence");
    let fixed_views = receipt.get("fixed_views").and_then(Value::as_object);
    let phases = receipt.get("phase_receipts").and_then(Value::as_array);
    let usage = receipt.get("usage").and_then(Value::as_object);
    let status = receipt.get("status").and_then(Value::as_str);
    let patch_count = receipt.get("patch_count").and_then(Value::as_u64);
    let visual_status = receipt
        .get("visual_review_evidence")
        .and_then(Value::as_object)
        .and_then(|visual| visual.get("status"))
        .and_then(Value::as_str);
    let expected_phase_count = match (status, patch_count) {
        (Some("passed_without_patch"), Some(0)) => Some(8usize),
        (Some("passed_after_patch"), Some(1)) => Some(13usize),
        _ => None,
    };
    let provider_evidence_sha256 = provider_evidence.map(semantic_sha256).transpose()?;
    let production_sha256 = production_value.map(semantic_sha256).transpose()?;
    let required_top_level_hashes = [
        "request_sha256",
        "provider_call_evidence_sha256",
        "source_program_sha256",
        "expanded_program_sha256",
        "shape_program_sha256",
        "structural_descriptor_sha256",
        "semantic_structure_sha256",
        "normalized_geometry_sha256",
        "topology_signature_sha256",
        "operation_sequence_sha256",
        "profile_signature_sha256",
        "part_zone_signature_sha256",
        "glb_sha256",
        "fixed_view_sha256",
        "visual_session_sha256",
        "visual_session_receipt_sha256",
        "gate_outcome_sha256",
        "compile_readback_sha256",
        "restricted_geometry_evidence_sha256",
        "production_review_evidence_sha256",
    ];
    let valid = receipt.get("schema_version").and_then(Value::as_str) == Some("E005RunReceipt@1")
        && receipt.get("run_mode").and_then(Value::as_str) == Some("formal_provider")
        && receipt
            .get("distribution_eligible")
            .and_then(Value::as_bool)
            == Some(true)
        && receipt
            .get("provider_authorization_id")
            .and_then(Value::as_str)
            == Some(authorization_id)
        && receipt
            .get("provider_authorization_sha256")
            .and_then(Value::as_str)
            == Some(authorization_sha256.as_str())
        && receipt.get("task_set_sha256").and_then(Value::as_str) == Some(task_set_sha256)
        && receipt.get("task_id").and_then(Value::as_str) == Some(task_id)
        && receipt.get("task_payload_sha256").and_then(Value::as_str) == Some(task_payload_sha256)
        && receipt.get("author_source_mode").and_then(Value::as_str)
            == Some("provider_authored_v2")
        && receipt.get("authoring_count").and_then(Value::as_u64) == Some(1)
        && receipt
            .get("network_provider_calls")
            .and_then(Value::as_u64)
            == Some(2)
        && receipt.get("human_review_status").and_then(Value::as_str) == Some("pending")
        && receipt.get("artifact_profile_id").and_then(Value::as_str) == Some("production_concept")
        && receipt
            .get("failure_codes")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        && receipt.get("elapsed_ms").and_then(Value::as_u64).is_some()
        && receipt
            .get("triangle_count")
            .and_then(Value::as_u64)
            .is_some_and(|value| value > 0)
        && receipt
            .get("mesh_count")
            .and_then(Value::as_u64)
            .is_some_and(|value| value > 0)
        && receipt
            .get("primitive_count")
            .and_then(Value::as_u64)
            .is_some_and(|value| value > 0)
        && receipt
            .get("material_count")
            .and_then(Value::as_u64)
            .is_some_and(|value| value > 0)
        && receipt
            .get("bounds_mm")
            .and_then(Value::as_array)
            .is_some_and(|bounds| {
                bounds.len() == 3
                    && bounds.iter().all(|value| {
                        value
                            .as_f64()
                            .is_some_and(|value| value.is_finite() && value > 0.0)
                    })
            })
        && required_top_level_hashes.iter().all(|field| {
            receipt
                .get(*field)
                .and_then(Value::as_str)
                .is_some_and(valid_sha256)
        })
        && receipt.get("vp204_session_sha256").is_none()
        && receipt.get("vp204_receipt_sha256").is_none()
        && provider_evidence.is_some_and(|evidence| {
            evidence.len() == 2
                && ["author", "patch"]
                    .iter()
                    .zip(evidence)
                    .all(|(kind, item)| {
                        item.get("authorization_id").and_then(Value::as_str)
                            == Some(authorization_id)
                            && item.get("task_id").and_then(Value::as_str) == Some(task_id)
                            && item.get("task_payload_sha256").and_then(Value::as_str)
                                == Some(task_payload_sha256)
                            && item.get("call_kind").and_then(Value::as_str) == Some(*kind)
                            && item.get("network_call_made").and_then(Value::as_bool) == Some(true)
                    })
        })
        && provider_evidence_sha256.as_deref()
            == receipt
                .get("provider_call_evidence_sha256")
                .and_then(Value::as_str)
        && production_sha256.as_deref()
            == receipt
                .get("production_review_evidence_sha256")
                .and_then(Value::as_str)
        && production.is_some_and(|production| {
            production.get("schema_version").and_then(Value::as_str)
                == Some("E005ProductionReview@1")
                && production
                    .get("artifact_profile_id")
                    .and_then(Value::as_str)
                    == Some("production_concept")
                && production
                    .get("visual_texture_provenance_verified")
                    .and_then(Value::as_bool)
                    == Some(true)
                && production
                    .get("surface_adornment_count")
                    .and_then(Value::as_u64)
                    .is_some_and(|value| (1..=32).contains(&value))
                && production
                    .get("visual_texture_set_count")
                    .and_then(Value::as_u64)
                    == production
                        .get("surface_adornment_count")
                        .and_then(Value::as_u64)
                && production
                    .get("visual_texture_map_count")
                    .and_then(Value::as_u64)
                    == production
                        .get("surface_adornment_count")
                        .and_then(Value::as_u64)
                        .map(|value| value * 5)
                && [
                    "source_program_sha256",
                    "glb_sha256",
                    "normalized_geometry_sha256",
                    "fixed_view_sha256",
                    "compile_readback_sha256",
                    "restricted_geometry_evidence_sha256",
                ]
                .iter()
                .all(|field| production.get(*field) == receipt.get(*field))
        })
        && fixed_views.is_some_and(valid_turntable_views)
        && production.and_then(|production| production.get("fixed_views"))
            == receipt.get("fixed_views")
        && usage.is_some_and(|usage| {
            usage.get("provider_requests").and_then(Value::as_u64) == Some(2)
                && usage.get("estimated_cost_microusd").and_then(Value::as_u64)
                    == receipt
                        .get("billable_cost_microusd")
                        .and_then(Value::as_u64)
        })
        && phases.is_some_and(|phases| {
            Some(phases.len()) == expected_phase_count
                && phases.iter().enumerate().all(|(index, phase)| {
                    phase.get("sequence").and_then(Value::as_u64) == Some((index + 1) as u64)
                        && phase.get("duration_ms").and_then(Value::as_u64).is_some()
                        && phase
                            .get("input_sha256")
                            .and_then(Value::as_str)
                            .is_some_and(valid_sha256)
                        && phase
                            .get("output_sha256")
                            .and_then(Value::as_str)
                            .is_some_and(valid_sha256)
                })
        })
        && matches!(
            (status, visual_status),
            (
                Some("passed_without_patch"),
                Some("accepted_by_visual_review")
            ) | (
                Some("passed_after_patch"),
                Some("patched_pending_visual_confirmation")
            )
        );
    if !valid {
        return Err(invalid(
            "E005_FORMAL_BATCH_RECEIPT_BINDING_INVALID",
            "Formal receipt does not bind the exact authorization and frozen task checkpoint.",
        ));
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_turntable_views(views: &serde_json::Map<String, Value>) -> bool {
    let required = [
        "turntable_000",
        "turntable_045",
        "turntable_090",
        "turntable_135",
        "turntable_180",
        "turntable_225",
        "turntable_270",
        "turntable_315",
    ];
    views.len() == required.len()
        && required.iter().all(|view| {
            views
                .get(*view)
                .and_then(Value::as_str)
                .is_some_and(valid_sha256)
        })
}

fn read_batch(
    connection: &Connection,
    batch_id: &str,
) -> CoreResult<Option<E005FormalBatchCheckpoint>> {
    let header: Option<(String, String, String, u8, u8)> = connection
        .query_row(
            "SELECT authorization_id, task_set_sha256, status, total_task_count, sealed_receipt_count FROM e005_formal_batches WHERE batch_id=?",
            [batch_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((authorization_id, task_set_sha256, status, total_task_count, sealed_receipt_count)) =
        header
    else {
        return Ok(None);
    };
    let tasks = {
        let mut statement = connection.prepare(
            "SELECT task_id, task_payload_sha256, task_ordinal, state, receipt_sha256 FROM e005_formal_batch_tasks WHERE batch_id=? ORDER BY task_ordinal",
        )?;
        let raw_tasks = statement
            .query_map([batch_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u8>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        raw_tasks
            .into_iter()
            .map(
                |(task_id, task_payload_sha256, task_ordinal, state, receipt_sha256)| {
                    Ok(E005FormalBatchTaskCheckpoint {
                        schema_version: E005_FORMAL_BATCH_TASK_SCHEMA_VERSION.into(),
                        task_id,
                        task_payload_sha256,
                        task_ordinal,
                        state: E005FormalBatchTaskState::parse(&state)?,
                        receipt_sha256,
                    })
                },
            )
            .collect::<CoreResult<Vec<_>>>()?
    };
    if tasks.len() != total_task_count as usize || sealed_receipt_count as usize > tasks.len() {
        return Err(invalid(
            "E005_FORMAL_BATCH_CHECKPOINT_INVALID",
            "Persisted E005 batch checkpoint counts are inconsistent.",
        ));
    }
    Ok(Some(E005FormalBatchCheckpoint {
        schema_version: E005_FORMAL_BATCH_SCHEMA_VERSION.into(),
        batch_id: batch_id.into(),
        authorization_id,
        task_set_sha256,
        status: E005FormalBatchStatus::parse(&status)?,
        total_task_count,
        sealed_receipt_count,
        tasks,
    }))
}

fn require_id(value: &str) -> CoreResult<()> {
    if (3..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'@'))
    {
        Ok(())
    } else {
        Err(invalid(
            "E005_FORMAL_BATCH_ID_INVALID",
            "E005 batch identity is invalid.",
        ))
    }
}

fn unix_ms() -> CoreResult<i64> {
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| {
            invalid(
                "E005_FORMAL_BATCH_CLOCK_INVALID",
                "System time is before the Unix epoch.",
            )
        })?
        .as_millis();
    i64::try_from(value).map_err(|_| {
        invalid(
            "E005_FORMAL_BATCH_CLOCK_INVALID",
            "System time exceeded SQLite range.",
        )
    })
}

fn invalid(code: &'static str, message: &'static str) -> CoreError {
    CoreError::invalid_data(code, message)
}

#[cfg(test)]
mod tests {
    use chrono::{SecondsFormat, Utc};
    use serde_json::json;
    use tempfile::{tempdir, TempDir};

    use crate::{
        E005ProviderCallKind, E005ProviderCallReservationRequest,
        E005ProviderRunAuthorizationContract, E005_PROVIDER_RUN_AUTHORIZATION_SCHEMA_VERSION,
    };

    use super::*;

    struct Fixture {
        _root: TempDir,
        repository: CoreRepository,
        authorization: E005ProviderRunAuthorizationContract,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempdir().unwrap();
            let repository = CoreRepository::open(
                root.path().join("library.db"),
                root.path().join("library"),
                "e005-batch-test",
            )
            .unwrap();
            let task_set: Value = serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../packages/concept-spec/fixtures/e005-unseen-mechanical-hard-surface-task-set.json"
            )))
            .unwrap();
            let authorized_at = Utc::now()
                .checked_sub_signed(chrono::Duration::seconds(10))
                .unwrap()
                .to_rfc3339_opts(SecondsFormat::Millis, true);
            let expires_at = Utc::now()
                .checked_add_signed(chrono::Duration::minutes(10))
                .unwrap()
                .to_rfc3339_opts(SecondsFormat::Millis, true);
            let mut authorization = E005ProviderRunAuthorizationContract {
                schema_version: E005_PROVIDER_RUN_AUTHORIZATION_SCHEMA_VERSION.into(),
                authorization_id: "e005_auth_batch_test".into(),
                task_set_sha256: semantic_sha256(&task_set).unwrap(),
                status: "authorized".into(),
                grant_mode: "explicit_user_confirmation".into(),
                provider_id: Some("provider_test".into()),
                model_id: Some("model_test_v1".into()),
                source_policy_sha256: Some("1".repeat(64)),
                pricing_snapshot_sha256: Some("2".repeat(64)),
                disclosure_sha256: Some("3".repeat(64)),
                authorized_at: Some(authorized_at),
                expires_at: Some(expires_at),
                maximum_author_calls: 30,
                maximum_patch_calls: 30,
                maximum_total_calls: 60,
                maximum_input_tokens: 600_000,
                maximum_output_tokens: 300_000,
                maximum_variable_cost_microusd: 60_000_000,
                maximum_batch_wall_time_ms: 3_150_000,
                maximum_single_call_wall_time_ms: 105_000,
                whole_object_template_policy: "forbidden".into(),
                authorization_binding_sha256: String::new(),
            };
            let mut binding = serde_json::to_value(&authorization).unwrap();
            binding
                .as_object_mut()
                .unwrap()
                .remove("authorization_binding_sha256");
            authorization.authorization_binding_sha256 = semantic_sha256(&binding).unwrap();
            repository
                .issue_e005_provider_run_authorization(&authorization, &task_set)
                .unwrap();
            Self {
                _root: root,
                repository,
                authorization,
            }
        }
    }

    fn complete_production_receipt(
        authorization: &E005ProviderRunAuthorizationContract,
        task_id: &str,
        task_payload_sha256: &str,
    ) -> Value {
        let hash = "a".repeat(64);
        let views = json!({
            "turntable_000":hash,
            "turntable_045":hash,
            "turntable_090":hash,
            "turntable_135":hash,
            "turntable_180":hash,
            "turntable_225":hash,
            "turntable_270":hash,
            "turntable_315":hash,
        });
        let production = json!({
            "schema_version":"E005ProductionReview@1",
            "source_program_sha256":hash,
            "surface_plan_sha256":hash,
            "surface_adornment_sha256":hash,
            "restricted_geometry_input_sha256":hash,
            "surface_adornment_count":2,
            "glb_sha256":hash,
            "normalized_geometry_sha256":hash,
            "fixed_view_sha256":hash,
            "fixed_views":views,
            "compile_readback_sha256":hash,
            "restricted_geometry_evidence_sha256":hash,
            "artifact_profile_id":"production_concept",
            "material_zone_count":2,
            "visual_texture_set_count":2,
            "visual_texture_map_count":10,
            "visual_texture_provenance_verified":true,
            "lower_duration_ms":1,
            "compile_duration_ms":1,
            "render_duration_ms":1,
            "elapsed_ms":4,
        });
        let evidence = json!([
            {
                "authorization_id":authorization.authorization_id,
                "task_id":task_id,
                "task_payload_sha256":task_payload_sha256,
                "call_kind":"author",
                "network_call_made":true
            },
            {
                "authorization_id":authorization.authorization_id,
                "task_id":task_id,
                "task_payload_sha256":task_payload_sha256,
                "call_kind":"patch",
                "network_call_made":true
            }
        ]);
        let phases = (1..=8)
            .map(|sequence| {
                json!({
                    "sequence":sequence,
                    "phase":if sequence == 1 {"author"} else {"preview"},
                    "duration_ms":1,
                    "input_sha256":hash,
                    "output_sha256":hash,
                    "cache":"not_applicable"
                })
            })
            .collect::<Vec<_>>();
        let evidence_sha256 = semantic_sha256(&evidence).unwrap();
        let production_sha256 = semantic_sha256(&production).unwrap();
        let mut receipt = json!({
            "schema_version":"E005RunReceipt@1",
            "run_id":format!("run_{task_id}"),
            "task_set_sha256":E005_FORMAL_TASK_SET_SHA256,
            "task_id":task_id,
            "status":"passed_without_patch",
            "run_mode":"formal_provider",
            "distribution_eligible":true,
            "author_source_mode":"provider_authored_v2",
            "task_payload_sha256":task_payload_sha256,
            "request_sha256":hash,
            "authoring_count":1,
            "patch_count":0,
            "provider_authorization_id":authorization.authorization_id,
            "provider_authorization_sha256":semantic_sha256(authorization).unwrap(),
            "provider_call_evidence":evidence,
            "provider_call_evidence_sha256":evidence_sha256,
            "visual_review_evidence":{"status":"accepted_by_visual_review"},
            "production_review_evidence":production,
            "production_review_evidence_sha256":production_sha256,
            "source_program_sha256":hash,
            "expanded_program_sha256":hash,
            "shape_program_sha256":hash,
            "structural_descriptor_sha256":hash,
            "semantic_structure_sha256":hash,
        });
        let artifacts = json!({
            "normalized_geometry_sha256":hash,
            "topology_signature_sha256":hash,
            "operation_sequence_sha256":hash,
            "profile_signature_sha256":hash,
            "part_zone_signature_sha256":hash,
            "glb_sha256":hash,
            "fixed_view_sha256":hash,
            "fixed_views":views,
            "visual_session_sha256":hash,
            "visual_session_receipt_sha256":hash,
            "gate_outcome_sha256":hash,
            "compile_readback_sha256":hash,
            "restricted_geometry_evidence_sha256":hash,
        });
        let runtime = json!({
            "artifact_profile_id":"production_concept",
            "runtime_manifest_version":"forgecad-geometry-runtime@1",
            "triangle_count":100,
            "bounds_mm":[100.0,100.0,100.0],
            "mesh_count":1,
            "primitive_count":1,
            "material_count":2,
            "usage":{"provider_requests":2,"estimated_cost_microusd":0},
            "phase_receipts":phases,
            "elapsed_ms":20,
            "network_provider_calls":2,
            "billable_cost_microusd":0,
            "failure_codes":[],
            "human_review_status":"pending"
        });
        for extension in [artifacts, runtime] {
            receipt
                .as_object_mut()
                .unwrap()
                .extend(extension.as_object().unwrap().clone());
        }
        receipt
    }

    #[test]
    fn e005_batch_claim_and_predispatch_restart_return_the_same_task_to_pending() {
        let fixture = Fixture::new();
        let batch = fixture
            .repository
            .start_e005_formal_batch(
                "e005_batch_test_001",
                &fixture.authorization.authorization_id,
            )
            .unwrap();
        assert_eq!(batch.total_task_count, 30);
        assert_eq!(batch.tasks.len(), 30);
        let first = fixture
            .repository
            .claim_next_e005_formal_batch_task(&batch.batch_id)
            .unwrap()
            .unwrap();
        assert_eq!(first.task_ordinal, 1);
        let recovered = fixture
            .repository
            .recover_e005_formal_batches_after_provider_recovery()
            .unwrap();
        assert_eq!(recovered[0].status, E005FormalBatchStatus::Ready);
        assert_eq!(
            recovered[0].tasks[0].state,
            E005FormalBatchTaskState::Pending
        );
        let replay = fixture
            .repository
            .claim_next_e005_formal_batch_task(&batch.batch_id)
            .unwrap()
            .unwrap();
        assert_eq!(replay.task_id, first.task_id);
    }

    #[test]
    fn e005_batch_rejects_binding_one_authorization_to_two_batch_ids() {
        let fixture = Fixture::new();
        fixture
            .repository
            .start_e005_formal_batch(
                "e005_batch_test_auth_a",
                &fixture.authorization.authorization_id,
            )
            .unwrap();
        let error = fixture
            .repository
            .start_e005_formal_batch(
                "e005_batch_test_auth_b",
                &fixture.authorization.authorization_id,
            )
            .unwrap_err();
        assert_eq!(
            error.code(),
            "E005_FORMAL_BATCH_AUTHORIZATION_ALREADY_BOUND"
        );
    }

    #[test]
    fn e005_batch_network_attempt_restart_requires_reconciliation_and_never_reclaims() {
        let fixture = Fixture::new();
        let batch = fixture
            .repository
            .start_e005_formal_batch(
                "e005_batch_test_002",
                &fixture.authorization.authorization_id,
            )
            .unwrap();
        let claim = fixture
            .repository
            .claim_next_e005_formal_batch_task(&batch.batch_id)
            .unwrap()
            .unwrap();
        let reservation = fixture
            .repository
            .reserve_e005_provider_call(&E005ProviderCallReservationRequest {
                authorization_id: claim.authorization_id.clone(),
                authorization_binding_sha256: fixture
                    .authorization
                    .authorization_binding_sha256
                    .clone(),
                provider_id: "provider_test".into(),
                model_id: "model_test_v1".into(),
                task_id: claim.task_id.clone(),
                task_payload_sha256: claim.task_payload_sha256.clone(),
                call_kind: E005ProviderCallKind::Author,
                request_sha256: "a".repeat(64),
                patch_base_source_sha256: None,
                failed_gate_sha256: None,
                reserved_input_tokens: 100,
                reserved_output_tokens: 100,
                reserved_cost_ceiling_microusd: 100,
            })
            .unwrap();
        fixture
            .repository
            .mark_e005_provider_call_dispatching(&reservation.reservation_id)
            .unwrap();
        fixture
            .repository
            .recover_e005_provider_budget_after_restart()
            .unwrap();
        let recovered = fixture
            .repository
            .recover_e005_formal_batches_after_provider_recovery()
            .unwrap();
        assert_eq!(
            recovered[0].status,
            E005FormalBatchStatus::ReconciliationRequired
        );
        assert_eq!(
            recovered[0].tasks[0].state,
            E005FormalBatchTaskState::ReconciliationRequired
        );
        let error = fixture
            .repository
            .claim_next_e005_formal_batch_task(&batch.batch_id)
            .unwrap_err();
        assert_eq!(error.code(), "E005_FORMAL_BATCH_NOT_RUNNABLE");
    }

    #[test]
    fn e005_batch_seals_exact_receipt_atomically_and_rejects_conflicting_replay() {
        let fixture = Fixture::new();
        let batch = fixture
            .repository
            .start_e005_formal_batch(
                "e005_batch_test_003",
                &fixture.authorization.authorization_id,
            )
            .unwrap();
        let claim = fixture
            .repository
            .claim_next_e005_formal_batch_task(&batch.batch_id)
            .unwrap()
            .unwrap();
        let incomplete = json!({
            "schema_version":"E005RunReceipt@1",
            "run_mode":"formal_provider",
            "distribution_eligible":true,
            "provider_authorization_id":fixture.authorization.authorization_id,
            "provider_authorization_sha256":semantic_sha256(&fixture.authorization).unwrap(),
            "task_set_sha256":E005_FORMAL_TASK_SET_SHA256,
            "task_id":claim.task_id,
            "task_payload_sha256":claim.task_payload_sha256,
        });
        assert_eq!(
            fixture
                .repository
                .seal_e005_formal_batch_receipt(
                    &batch.batch_id,
                    incomplete["task_id"].as_str().unwrap(),
                    &incomplete,
                )
                .unwrap_err()
                .code(),
            "E005_FORMAL_BATCH_RECEIPT_BINDING_INVALID"
        );
        let receipt = complete_production_receipt(
            &fixture.authorization,
            &claim.task_id,
            &claim.task_payload_sha256,
        );
        let sealed = fixture
            .repository
            .seal_e005_formal_batch_receipt(
                &batch.batch_id,
                receipt["task_id"].as_str().unwrap(),
                &receipt,
            )
            .unwrap();
        assert_eq!(sealed.sealed_receipt_count, 1);
        assert_eq!(
            sealed.tasks[0].state,
            E005FormalBatchTaskState::ReceiptSealed
        );
        let replay = fixture
            .repository
            .seal_e005_formal_batch_receipt(
                &batch.batch_id,
                receipt["task_id"].as_str().unwrap(),
                &receipt,
            )
            .unwrap();
        assert_eq!(
            replay.tasks[0].receipt_sha256,
            sealed.tasks[0].receipt_sha256
        );
        let mut tampered = receipt;
        tampered["run_id"] = json!("tampered");
        let error = fixture
            .repository
            .seal_e005_formal_batch_receipt(
                &batch.batch_id,
                tampered["task_id"].as_str().unwrap(),
                &tampered,
            )
            .unwrap_err();
        assert_eq!(
            error.code(),
            "E005_FORMAL_BATCH_RECEIPT_IDEMPOTENCY_CONFLICT"
        );
    }
}
