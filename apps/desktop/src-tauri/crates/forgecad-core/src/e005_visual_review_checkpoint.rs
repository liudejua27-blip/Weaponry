//! Restart-safe handoff between the accounted E005 Author call and the one
//! permitted visual Patch call.
//!
//! The validated bounded author source is product candidate state, not raw
//! Provider payload. Images, credentials, prompts and raw responses remain
//! memory-only. Recovery may resume only when no visual Patch dispatch was
//! attempted; any attempted or uncertain visual call requires reconciliation.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    canonical_json, lower_forge_visual_author_source_v1, semantic_sha256, CoreError,
    CoreRepository, CoreResult, E005ProviderBudgetEvidence, E005ProviderCallKind,
};

pub const E005_VISUAL_REVIEW_CHECKPOINT_SCHEMA_VERSION: &str = "E005VisualReviewCheckpoint@1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005ProviderUsageCheckpoint {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub prompt_cache_hit_tokens: u64,
    pub prompt_cache_miss_tokens: u64,
    pub estimated_cost_microusd: u64,
}

impl E005ProviderUsageCheckpoint {
    pub fn validate_against(&self, evidence: &E005ProviderBudgetEvidence) -> CoreResult<()> {
        if self.input_tokens == 0
            || self.output_tokens == 0
            || self.input_tokens > evidence.reserved_input_tokens
            || self.output_tokens > evidence.reserved_output_tokens
            || self.estimated_cost_microusd > evidence.reserved_cost_ceiling_microusd
            || self
                .prompt_cache_hit_tokens
                .checked_add(self.prompt_cache_miss_tokens)
                .is_none_or(|total| total > self.input_tokens)
        {
            return Err(invalid(
                "E005_R2_CHECKPOINT_USAGE_INVALID",
                "E005 checkpoint Provider usage exceeds its accounted reservation.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum E005VisualReviewCheckpointState {
    AwaitingVisualReview,
    Completed,
    ReconciliationRequired,
}

impl E005VisualReviewCheckpointState {
    fn parse(value: &str) -> CoreResult<Self> {
        match value {
            "awaiting_visual_review" => Ok(Self::AwaitingVisualReview),
            "completed" => Ok(Self::Completed),
            "reconciliation_required" => Ok(Self::ReconciliationRequired),
            _ => Err(invalid(
                "E005_R2_CHECKPOINT_STATE_INVALID",
                "Persisted E005 visual-review checkpoint state is invalid.",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct E005VisualReviewCheckpoint {
    pub schema_version: String,
    pub authorization_id: String,
    pub task_id: String,
    pub task_payload_sha256: String,
    pub state: E005VisualReviewCheckpointState,
    pub author_source: Value,
    pub author_source_sha256: String,
    pub author_reservation_id: String,
    pub author_budget_evidence: E005ProviderBudgetEvidence,
    pub author_budget_evidence_sha256: String,
    pub author_provider_usage: E005ProviderUsageCheckpoint,
    pub author_provider_usage_sha256: String,
    pub visual_reservation_id: Option<String>,
    pub visual_budget_evidence_sha256: Option<String>,
    pub visual_review_evidence_sha256: Option<String>,
}

impl E005VisualReviewCheckpoint {
    pub fn validate(&self) -> CoreResult<()> {
        let lowering = lower_forge_visual_author_source_v1(&self.author_source)?;
        if self.schema_version != E005_VISUAL_REVIEW_CHECKPOINT_SCHEMA_VERSION
            || !valid_id(&self.authorization_id)
            || !valid_id(&self.task_id)
            || !valid_sha256(&self.task_payload_sha256)
            || !valid_sha256(&self.author_source_sha256)
            || !valid_id(&self.author_reservation_id)
            || !valid_sha256(&self.author_budget_evidence_sha256)
            || !valid_sha256(&self.author_provider_usage_sha256)
            || lowering.source_program_sha256 != self.author_source_sha256
            || self.author_budget_evidence.reservation_id != self.author_reservation_id
            || self.author_budget_evidence.authorization_id != self.authorization_id
            || self.author_budget_evidence.task_id != self.task_id
            || semantic_sha256(&self.author_budget_evidence)? != self.author_budget_evidence_sha256
            || semantic_sha256(&self.author_provider_usage)? != self.author_provider_usage_sha256
        {
            return Err(invalid(
                "E005_R2_CHECKPOINT_INVALID",
                "E005 visual-review checkpoint identity or author lineage is invalid.",
            ));
        }
        self.author_provider_usage
            .validate_against(&self.author_budget_evidence)?;
        let completed_fields = self.visual_reservation_id.as_deref().is_some_and(valid_id)
            && self
                .visual_budget_evidence_sha256
                .as_deref()
                .is_some_and(valid_sha256)
            && self
                .visual_review_evidence_sha256
                .as_deref()
                .is_some_and(valid_sha256);
        match self.state {
            E005VisualReviewCheckpointState::AwaitingVisualReview
                if self.visual_reservation_id.is_none()
                    && self.visual_budget_evidence_sha256.is_none()
                    && self.visual_review_evidence_sha256.is_none() => {}
            E005VisualReviewCheckpointState::Completed if completed_fields => {}
            E005VisualReviewCheckpointState::ReconciliationRequired => {}
            _ => {
                return Err(invalid(
                    "E005_R2_CHECKPOINT_STATE_INVALID",
                    "E005 visual-review checkpoint fields do not match its state.",
                ))
            }
        }
        Ok(())
    }
}

impl CoreRepository {
    pub fn checkpoint_e005_author_awaiting_visual_review(
        &self,
        author_evidence: &E005ProviderBudgetEvidence,
        author_provider_usage: &E005ProviderUsageCheckpoint,
        author_source: &Value,
    ) -> CoreResult<E005VisualReviewCheckpoint> {
        self.verify_e005_provider_budget_evidence(author_evidence)?;
        author_provider_usage.validate_against(author_evidence)?;
        let lowering = lower_forge_visual_author_source_v1(author_source)?;
        if author_evidence.call_kind != E005ProviderCallKind::Author
            || author_evidence.settlement != "accounted"
            || !author_evidence.network_call_made
            || author_evidence.outcome_code != "PROVIDER_COMPLETED_REPAIRABLE"
            || author_evidence.output_source_sha256.as_deref()
                != Some(lowering.source_program_sha256.as_str())
        {
            return Err(CoreError::conflict(
                "E005_R2_AUTHOR_CHECKPOINT_INELIGIBLE",
                "Only one persisted repairable unified Author result can await visual review.",
            ));
        }
        let author_source_json = canonical_json(author_source)?;
        if author_source_json.len() > 2 * 1024 * 1024 {
            return Err(invalid(
                "E005_R2_AUTHOR_CHECKPOINT_TOO_LARGE",
                "Validated E005 author source exceeds the checkpoint bound.",
            ));
        }
        let author_budget_evidence_sha256 = semantic_sha256(author_evidence)?;
        let author_budget_evidence_json = canonical_json(author_evidence)?;
        let author_provider_usage_sha256 = semantic_sha256(author_provider_usage)?;
        let author_provider_usage_json = canonical_json(author_provider_usage)?;
        let now = unix_ms()?;
        self.write(|transaction| {
            let existing: Option<(String, String, String)> = transaction
                .query_row(
                    "SELECT state, author_source_sha256, author_budget_evidence_sha256 FROM e005_visual_review_checkpoints WHERE authorization_id=? AND task_id=?",
                    params![author_evidence.authorization_id, author_evidence.task_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            if let Some((state, source_sha256, evidence_sha256)) = existing {
                if state != "awaiting_visual_review"
                    || source_sha256 != lowering.source_program_sha256
                    || evidence_sha256 != author_budget_evidence_sha256
                {
                    return Err(CoreError::conflict(
                        "E005_R2_AUTHOR_CHECKPOINT_CONFLICT",
                        "E005 visual-review checkpoint already contains different lineage.",
                    ));
                }
                return read_checkpoint(
                    transaction,
                    &author_evidence.authorization_id,
                    &author_evidence.task_id,
                )?
                .ok_or_else(|| CoreError::not_found("E005 visual-review checkpoint"));
            }
            let persisted: (String, String, String, String, String) = transaction.query_row(
                "SELECT state, call_kind, task_payload_sha256, output_source_sha256, settlement_evidence_sha256 FROM e005_provider_call_reservations WHERE reservation_id=? AND authorization_id=? AND task_id=?",
                params![author_evidence.reservation_id, author_evidence.authorization_id, author_evidence.task_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )?;
            if persisted.0 != "accounted"
                || persisted.1 != "author"
                || persisted.2 != author_evidence.task_payload_sha256
                || persisted.3 != lowering.source_program_sha256
                || persisted.4 != author_budget_evidence_sha256
            {
                return Err(CoreError::conflict(
                    "E005_R2_AUTHOR_CHECKPOINT_PERSISTED_LINEAGE_INVALID",
                    "Persisted Author reservation does not match the visual-review checkpoint.",
                ));
            }
            transaction.execute(
                "INSERT INTO e005_visual_review_checkpoints(authorization_id, task_id, task_payload_sha256, state, author_source_json, author_source_sha256, author_reservation_id, author_budget_evidence_json, author_budget_evidence_sha256, author_provider_usage_json, author_provider_usage_sha256, created_at_unix_ms, updated_at_unix_ms) VALUES (?,?,?,'awaiting_visual_review',?,?,?,?,?,?,?,?,?)",
                params![
                    author_evidence.authorization_id,
                    author_evidence.task_id,
                    author_evidence.task_payload_sha256,
                    author_source_json,
                    lowering.source_program_sha256,
                    author_evidence.reservation_id,
                    author_budget_evidence_json,
                    author_budget_evidence_sha256,
                    author_provider_usage_json,
                    author_provider_usage_sha256,
                    now,
                    now,
                ],
            )?;
            read_checkpoint(
                transaction,
                &author_evidence.authorization_id,
                &author_evidence.task_id,
            )?
            .ok_or_else(|| CoreError::not_found("E005 visual-review checkpoint"))
        })
    }

    pub fn complete_e005_visual_review_checkpoint(
        &self,
        visual_evidence: &E005ProviderBudgetEvidence,
        visual_review_evidence_sha256: &str,
    ) -> CoreResult<E005VisualReviewCheckpoint> {
        self.verify_e005_provider_budget_evidence(visual_evidence)?;
        if visual_evidence.call_kind != E005ProviderCallKind::Patch
            || visual_evidence.settlement != "accounted"
            || !visual_evidence.network_call_made
            || visual_evidence.outcome_code != "PROVIDER_COMPLETED_PASSED"
            || !valid_sha256(visual_review_evidence_sha256)
        {
            return Err(CoreError::conflict(
                "E005_R2_VISUAL_CHECKPOINT_INELIGIBLE",
                "Only one persisted successful visual Patch result can complete the checkpoint.",
            ));
        }
        let visual_budget_evidence_sha256 = semantic_sha256(visual_evidence)?;
        let now = unix_ms()?;
        self.write(|transaction| {
            let existing = read_checkpoint(
                transaction,
                &visual_evidence.authorization_id,
                &visual_evidence.task_id,
            )?
            .ok_or_else(|| CoreError::not_found("E005 visual-review checkpoint"))?;
            if existing.state == E005VisualReviewCheckpointState::Completed {
                if existing.visual_reservation_id.as_deref()
                    == Some(visual_evidence.reservation_id.as_str())
                    && existing.visual_budget_evidence_sha256.as_deref()
                        == Some(visual_budget_evidence_sha256.as_str())
                    && existing.visual_review_evidence_sha256.as_deref()
                        == Some(visual_review_evidence_sha256)
                {
                    return Ok(existing);
                }
                return Err(CoreError::conflict(
                    "E005_R2_VISUAL_CHECKPOINT_CONFLICT",
                    "Completed E005 visual-review checkpoint has different evidence.",
                ));
            }
            if existing.state != E005VisualReviewCheckpointState::AwaitingVisualReview
                || existing.task_payload_sha256 != visual_evidence.task_payload_sha256
            {
                return Err(CoreError::conflict(
                    "E005_R2_VISUAL_CHECKPOINT_STATE_INVALID",
                    "E005 visual-review checkpoint cannot accept this result.",
                ));
            }
            let changed = transaction.execute(
                "UPDATE e005_visual_review_checkpoints SET state='completed', visual_reservation_id=?, visual_budget_evidence_sha256=?, visual_review_evidence_sha256=?, updated_at_unix_ms=? WHERE authorization_id=? AND task_id=? AND state='awaiting_visual_review'",
                params![
                    visual_evidence.reservation_id,
                    visual_budget_evidence_sha256,
                    visual_review_evidence_sha256,
                    now,
                    visual_evidence.authorization_id,
                    visual_evidence.task_id,
                ],
            )?;
            if changed != 1 {
                return Err(CoreError::conflict(
                    "E005_R2_VISUAL_CHECKPOINT_UPDATE_NOT_ACQUIRED",
                    "E005 visual-review checkpoint completion was not atomically acquired.",
                ));
            }
            read_checkpoint(
                transaction,
                &visual_evidence.authorization_id,
                &visual_evidence.task_id,
            )?
            .ok_or_else(|| CoreError::not_found("E005 visual-review checkpoint"))
        })
    }

    /// Must run after 0045 Provider reservation recovery. An awaiting
    /// checkpoint with no attempted Patch remains resumable; any dispatching
    /// or accounted Patch becomes reconciliation-required and is never reset.
    pub fn recover_e005_visual_review_checkpoints_after_provider_recovery(
        &self,
    ) -> CoreResult<Vec<E005VisualReviewCheckpoint>> {
        self.write(|transaction| {
            let awaiting = {
                let mut statement = transaction.prepare(
                    "SELECT authorization_id, task_id FROM e005_visual_review_checkpoints WHERE state='awaiting_visual_review' ORDER BY authorization_id, task_id",
                )?;
                let rows = statement
                    .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
                    .collect::<Result<Vec<_>, _>>()?;
                rows
            };
            let now = unix_ms()?;
            for (authorization_id, task_id) in &awaiting {
                let attempted_patch: u8 = transaction.query_row(
                    "SELECT COUNT(*) FROM e005_provider_call_reservations WHERE authorization_id=? AND task_id=? AND call_kind='patch' AND (network_call_made=1 OR state IN ('dispatching','accounted'))",
                    params![authorization_id, task_id],
                    |row| row.get(0),
                )?;
                if attempted_patch > 0 {
                    transaction.execute(
                        "UPDATE e005_visual_review_checkpoints SET state='reconciliation_required', updated_at_unix_ms=? WHERE authorization_id=? AND task_id=? AND state='awaiting_visual_review'",
                        params![now, authorization_id, task_id],
                    )?;
                }
            }
            let mut statement = transaction.prepare(
                "SELECT authorization_id, task_id FROM e005_visual_review_checkpoints ORDER BY authorization_id, task_id",
            )?;
            let ids = statement
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            ids.into_iter()
                .map(|(authorization_id, task_id)| {
                    read_checkpoint(transaction, &authorization_id, &task_id)?.ok_or_else(|| {
                        CoreError::not_found("E005 visual-review checkpoint")
                    })
                })
                .collect()
        })
    }

    pub fn e005_visual_review_checkpoint(
        &self,
        authorization_id: &str,
        task_id: &str,
    ) -> CoreResult<Option<E005VisualReviewCheckpoint>> {
        if !valid_id(authorization_id) || !valid_id(task_id) {
            return Err(invalid(
                "E005_R2_CHECKPOINT_ID_INVALID",
                "E005 visual-review checkpoint identity is invalid.",
            ));
        }
        let connection = Connection::open(self.db_path())?;
        connection.busy_timeout(Duration::from_millis(5_000))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        read_checkpoint(&connection, authorization_id, task_id)
    }
}

fn read_checkpoint(
    connection: &Connection,
    authorization_id: &str,
    task_id: &str,
) -> CoreResult<Option<E005VisualReviewCheckpoint>> {
    let row: Option<(String, String, String, String, String, String, String, String, String, Option<String>, Option<String>, Option<String>)> = connection
        .query_row(
            "SELECT task_payload_sha256, state, author_source_json, author_source_sha256, author_reservation_id, author_budget_evidence_json, author_budget_evidence_sha256, author_provider_usage_json, author_provider_usage_sha256, visual_reservation_id, visual_budget_evidence_sha256, visual_review_evidence_sha256 FROM e005_visual_review_checkpoints WHERE authorization_id=? AND task_id=?",
            params![authorization_id, task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?)),
        )
        .optional()?;
    let Some((
        task_payload_sha256,
        state,
        source_json,
        author_source_sha256,
        author_reservation_id,
        author_budget_evidence_json,
        author_budget_evidence_sha256,
        author_provider_usage_json,
        author_provider_usage_sha256,
        visual_reservation_id,
        visual_budget_evidence_sha256,
        visual_review_evidence_sha256,
    )) = row
    else {
        return Ok(None);
    };
    let checkpoint = E005VisualReviewCheckpoint {
        schema_version: E005_VISUAL_REVIEW_CHECKPOINT_SCHEMA_VERSION.into(),
        authorization_id: authorization_id.into(),
        task_id: task_id.into(),
        task_payload_sha256,
        state: E005VisualReviewCheckpointState::parse(&state)?,
        author_source: serde_json::from_str(&source_json).map_err(|_| {
            invalid(
                "E005_R2_CHECKPOINT_SOURCE_INVALID",
                "Persisted E005 author source JSON is invalid.",
            )
        })?,
        author_source_sha256,
        author_reservation_id,
        author_budget_evidence: serde_json::from_str(&author_budget_evidence_json).map_err(
            |_| {
                invalid(
                    "E005_R2_CHECKPOINT_AUTHOR_EVIDENCE_INVALID",
                    "Persisted E005 Author budget evidence JSON is invalid.",
                )
            },
        )?,
        author_budget_evidence_sha256,
        author_provider_usage: serde_json::from_str(&author_provider_usage_json).map_err(|_| {
            invalid(
                "E005_R2_CHECKPOINT_AUTHOR_USAGE_INVALID",
                "Persisted E005 Author Provider usage JSON is invalid.",
            )
        })?,
        author_provider_usage_sha256,
        visual_reservation_id,
        visual_budget_evidence_sha256,
        visual_review_evidence_sha256,
    };
    checkpoint.validate()?;
    Ok(Some(checkpoint))
}

fn unix_ms() -> CoreResult<i64> {
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| {
            invalid(
                "E005_R2_CHECKPOINT_CLOCK_INVALID",
                "System clock is invalid.",
            )
        })?
        .as_millis();
    i64::try_from(value).map_err(|_| {
        invalid(
            "E005_R2_CHECKPOINT_CLOCK_INVALID",
            "System clock exceeded SQLite range.",
        )
    })
}

fn valid_id(value: &str) -> bool {
    (3..=160).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'@'))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid(code: &'static str, message: &'static str) -> CoreError {
    CoreError::invalid_data(code, message)
}

#[cfg(test)]
mod tests {
    use chrono::{SecondsFormat, Utc};
    use serde_json::Value;
    use tempfile::{tempdir, TempDir};

    use crate::{
        E005FormalBatchStatus, E005FormalBatchTaskState, E005ProviderCallOutcome,
        E005ProviderCallReservationRequest, E005ProviderCallSettlement,
        E005ProviderRunAuthorizationContract, E005_PROVIDER_RUN_AUTHORIZATION_SCHEMA_VERSION,
    };

    use super::*;

    struct Fixture {
        _root: TempDir,
        repository: CoreRepository,
        authorization: E005ProviderRunAuthorizationContract,
        task_set: Value,
        author_source: Value,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempdir().unwrap();
            let repository = CoreRepository::open(
                root.path().join("library.db"),
                root.path().join("library"),
                "e005-r2-checkpoint-test",
            )
            .unwrap();
            let task_set: Value = serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../packages/concept-spec/fixtures/e005-unseen-mechanical-hard-surface-task-set.json"
            )))
            .unwrap();
            let author_source: Value = serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../packages/concept-spec/fixtures/e005-r1-unified-service-console.json"
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
                authorization_id: "e005_auth_r2_checkpoint".into(),
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
                maximum_batch_wall_time_ms: 600_000,
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
                task_set,
                author_source,
            }
        }

        fn first_task(&self) -> (&str, String) {
            let task = &self.task_set["tasks"][0];
            (
                task["task_id"].as_str().unwrap(),
                semantic_sha256(task).unwrap(),
            )
        }

        fn usage() -> E005ProviderUsageCheckpoint {
            E005ProviderUsageCheckpoint {
                input_tokens: 80,
                output_tokens: 60,
                prompt_cache_hit_tokens: 0,
                prompt_cache_miss_tokens: 80,
                estimated_cost_microusd: 50,
            }
        }

        fn accounted_author(&self) -> E005ProviderBudgetEvidence {
            let (task_id, task_payload_sha256) = self.first_task();
            let source_sha256 = lower_forge_visual_author_source_v1(&self.author_source)
                .unwrap()
                .source_program_sha256;
            let reservation = self
                .repository
                .reserve_e005_provider_call(&E005ProviderCallReservationRequest {
                    authorization_id: self.authorization.authorization_id.clone(),
                    authorization_binding_sha256: self
                        .authorization
                        .authorization_binding_sha256
                        .clone(),
                    provider_id: "provider_test".into(),
                    model_id: "model_test_v1".into(),
                    task_id: task_id.into(),
                    task_payload_sha256,
                    call_kind: E005ProviderCallKind::Author,
                    request_sha256: "a".repeat(64),
                    patch_base_source_sha256: None,
                    failed_gate_sha256: None,
                    reserved_input_tokens: 100,
                    reserved_output_tokens: 100,
                    reserved_cost_ceiling_microusd: 100,
                })
                .unwrap();
            self.repository
                .mark_e005_provider_call_dispatching(&reservation.reservation_id)
                .unwrap();
            self.repository
                .settle_e005_provider_call(
                    &reservation.reservation_id,
                    &E005ProviderCallSettlement {
                        outcome: E005ProviderCallOutcome::ProviderCompletedRepairable,
                        output_source_sha256: Some(source_sha256),
                        output_gate_sha256: Some("b".repeat(64)),
                    },
                )
                .unwrap()
        }

        fn reserve_patch(
            &self,
            author_evidence: &E005ProviderBudgetEvidence,
        ) -> CoreResult<crate::E005ProviderCallReservation> {
            self.repository
                .reserve_e005_provider_call(&E005ProviderCallReservationRequest {
                    authorization_id: self.authorization.authorization_id.clone(),
                    authorization_binding_sha256: self
                        .authorization
                        .authorization_binding_sha256
                        .clone(),
                    provider_id: "provider_test".into(),
                    model_id: "model_test_v1".into(),
                    task_id: author_evidence.task_id.clone(),
                    task_payload_sha256: author_evidence.task_payload_sha256.clone(),
                    call_kind: E005ProviderCallKind::Patch,
                    request_sha256: "c".repeat(64),
                    patch_base_source_sha256: author_evidence.output_source_sha256.clone(),
                    failed_gate_sha256: author_evidence.output_gate_sha256.clone(),
                    reserved_input_tokens: 100,
                    reserved_output_tokens: 100,
                    reserved_cost_ceiling_microusd: 100,
                })
        }
    }

    #[test]
    fn e005_r2_checkpoint_completes_exact_author_to_visual_lineage_idempotently() {
        let fixture = Fixture::new();
        let author_evidence = fixture.accounted_author();
        let awaiting = fixture
            .repository
            .checkpoint_e005_author_awaiting_visual_review(
                &author_evidence,
                &Fixture::usage(),
                &fixture.author_source,
            )
            .unwrap();
        assert_eq!(
            awaiting.state,
            E005VisualReviewCheckpointState::AwaitingVisualReview
        );
        assert_eq!(
            awaiting.author_source_sha256,
            author_evidence.output_source_sha256.as_deref().unwrap()
        );

        let patch = fixture.reserve_patch(&author_evidence).unwrap();
        fixture
            .repository
            .mark_e005_provider_call_dispatching(&patch.reservation_id)
            .unwrap();
        let visual_evidence = fixture
            .repository
            .settle_e005_provider_call(
                &patch.reservation_id,
                &E005ProviderCallSettlement {
                    outcome: E005ProviderCallOutcome::ProviderCompletedPassed,
                    output_source_sha256: Some("d".repeat(64)),
                    output_gate_sha256: Some("e".repeat(64)),
                },
            )
            .unwrap();
        let completed = fixture
            .repository
            .complete_e005_visual_review_checkpoint(&visual_evidence, &"f".repeat(64))
            .unwrap();
        assert_eq!(completed.state, E005VisualReviewCheckpointState::Completed);
        let replay = fixture
            .repository
            .complete_e005_visual_review_checkpoint(&visual_evidence, &"f".repeat(64))
            .unwrap();
        assert_eq!(replay, completed);
    }

    #[test]
    fn e005_r2_checkpoint_restart_never_retries_an_attempted_visual_call() {
        let fixture = Fixture::new();
        let author_evidence = fixture.accounted_author();
        fixture
            .repository
            .checkpoint_e005_author_awaiting_visual_review(
                &author_evidence,
                &Fixture::usage(),
                &fixture.author_source,
            )
            .unwrap();
        let patch = fixture.reserve_patch(&author_evidence).unwrap();
        fixture
            .repository
            .mark_e005_provider_call_dispatching(&patch.reservation_id)
            .unwrap();

        fixture
            .repository
            .recover_e005_provider_budget_after_restart()
            .unwrap();
        let recovered = fixture
            .repository
            .recover_e005_visual_review_checkpoints_after_provider_recovery()
            .unwrap();
        assert_eq!(
            recovered[0].state,
            E005VisualReviewCheckpointState::ReconciliationRequired
        );
        let error = fixture.reserve_patch(&author_evidence).unwrap_err();
        assert_eq!(error.code(), "E005_PROVIDER_TASK_CALL_ALREADY_USED");
    }

    #[test]
    fn e005_r2_checkpoint_allows_batch_to_reclaim_only_the_unattempted_visual_handoff() {
        let fixture = Fixture::new();
        let batch = fixture
            .repository
            .start_e005_formal_batch(
                "e005_batch_r2_checkpoint",
                &fixture.authorization.authorization_id,
            )
            .unwrap();
        let claim = fixture
            .repository
            .claim_next_e005_formal_batch_task(&batch.batch_id)
            .unwrap()
            .unwrap();
        let author_evidence = fixture.accounted_author();
        assert_eq!(claim.task_id, author_evidence.task_id);
        fixture
            .repository
            .checkpoint_e005_author_awaiting_visual_review(
                &author_evidence,
                &Fixture::usage(),
                &fixture.author_source,
            )
            .unwrap();

        fixture
            .repository
            .recover_e005_provider_budget_after_restart()
            .unwrap();
        fixture
            .repository
            .recover_e005_visual_review_checkpoints_after_provider_recovery()
            .unwrap();
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
        assert_eq!(replay.task_id, claim.task_id);
    }
}
