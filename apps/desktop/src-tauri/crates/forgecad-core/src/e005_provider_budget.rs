use std::{
    collections::BTreeSet,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::DateTime;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{migration::open_connection, semantic_sha256, CoreError, CoreRepository, CoreResult};

pub const E005_PROVIDER_RUN_AUTHORIZATION_SCHEMA_VERSION: &str = "E005ProviderRunAuthorization@1";
pub const E005_PROVIDER_LEDGER_SCHEMA_VERSION: &str = "E005ProviderBudgetLedger@1";
pub const E005_PROVIDER_RESERVATION_SCHEMA_VERSION: &str = "E005ProviderCallReservation@1";
pub const E005_PROVIDER_BUDGET_EVIDENCE_SCHEMA_VERSION: &str = "E005ProviderBudgetEvidence@1";
pub const E005_MAXIMUM_AUTHOR_CALLS: u8 = 30;
pub const E005_MAXIMUM_PATCH_CALLS: u8 = 30;
pub const E005_MAXIMUM_TOTAL_CALLS: u8 = 60;
pub const E005_TASK_COUNT: usize = 30;
pub const E005_FORMAL_TASK_SET_SHA256: &str =
    "471c592b5f328f6e899b430b49eb042d3c6955f498b14fd1d2558a0934e18dde";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005ProviderRunAuthorizationContract {
    pub schema_version: String,
    pub authorization_id: String,
    pub task_set_sha256: String,
    pub status: String,
    pub grant_mode: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub source_policy_sha256: Option<String>,
    pub pricing_snapshot_sha256: Option<String>,
    pub disclosure_sha256: Option<String>,
    pub authorized_at: Option<String>,
    pub expires_at: Option<String>,
    pub maximum_author_calls: u8,
    pub maximum_patch_calls: u8,
    pub maximum_total_calls: u8,
    pub maximum_input_tokens: u64,
    pub maximum_output_tokens: u64,
    pub maximum_variable_cost_microusd: u64,
    pub maximum_batch_wall_time_ms: u64,
    pub maximum_single_call_wall_time_ms: u64,
    pub whole_object_template_policy: String,
    pub authorization_binding_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum E005ProviderCallKind {
    Author,
    Patch,
}

impl E005ProviderCallKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Author => "author",
            Self::Patch => "patch",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005ProviderBudgetLedger {
    pub schema_version: String,
    pub authorization: E005ProviderRunAuthorizationContract,
    pub status: String,
    pub reservations_created: u32,
    pub author_calls_accounted: u8,
    pub patch_calls_accounted: u8,
    pub calls_accounted: u8,
    pub reserved_input_tokens: u64,
    pub reserved_output_tokens: u64,
    pub reserved_cost_ceiling_microusd: u64,
    pub accounted_input_tokens: u64,
    pub accounted_output_tokens: u64,
    pub accounted_cost_ceiling_microusd: u64,
    pub authorized_at_unix_ms: i64,
    pub expires_at_unix_ms: i64,
    pub batch_deadline_unix_ms: i64,
    pub updated_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005ProviderCallReservationRequest {
    pub authorization_id: String,
    pub authorization_binding_sha256: String,
    pub provider_id: String,
    pub model_id: String,
    pub task_id: String,
    pub task_payload_sha256: String,
    pub call_kind: E005ProviderCallKind,
    pub request_sha256: String,
    pub patch_base_source_sha256: Option<String>,
    pub failed_gate_sha256: Option<String>,
    pub reserved_input_tokens: u64,
    pub reserved_output_tokens: u64,
    pub reserved_cost_ceiling_microusd: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005ProviderCallReservation {
    pub schema_version: String,
    pub reservation_id: String,
    pub authorization_id: String,
    pub authorization_binding_sha256: String,
    pub task_id: String,
    pub task_payload_sha256: String,
    pub call_kind: E005ProviderCallKind,
    pub call_number: u8,
    pub kind_call_number: u8,
    pub reservation_ordinal: u32,
    pub request_sha256: String,
    pub patch_base_source_sha256: Option<String>,
    pub failed_gate_sha256: Option<String>,
    pub reserved_input_tokens: u64,
    pub reserved_output_tokens: u64,
    pub reserved_cost_ceiling_microusd: u64,
    pub deadline_unix_ms: i64,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum E005ProviderCallOutcome {
    PreDispatchReleased,
    ProviderCompletedPassed,
    ProviderCompletedRepairable,
    ProviderCompletedFailed,
    ProviderTimeout,
    ProviderCancelled,
    ProviderTransportFailed,
    RecoveredUncertainDispatch,
}

impl E005ProviderCallOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::PreDispatchReleased => "PRE_DISPATCH_RELEASED",
            Self::ProviderCompletedPassed => "PROVIDER_COMPLETED_PASSED",
            Self::ProviderCompletedRepairable => "PROVIDER_COMPLETED_REPAIRABLE",
            Self::ProviderCompletedFailed => "PROVIDER_COMPLETED_FAILED",
            Self::ProviderTimeout => "PROVIDER_TIMEOUT",
            Self::ProviderCancelled => "PROVIDER_CANCELLED",
            Self::ProviderTransportFailed => "PROVIDER_TRANSPORT_FAILED",
            Self::RecoveredUncertainDispatch => "RECOVERED_UNCERTAIN_DISPATCH",
        }
    }

    fn is_pre_dispatch(self) -> bool {
        self == Self::PreDispatchReleased
    }

    fn requires_output(self) -> bool {
        matches!(
            self,
            Self::ProviderCompletedPassed | Self::ProviderCompletedRepairable
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005ProviderCallSettlement {
    pub outcome: E005ProviderCallOutcome,
    pub output_source_sha256: Option<String>,
    pub output_gate_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005ProviderBudgetEvidence {
    pub schema_version: String,
    pub authorization_id: String,
    pub authorization_binding_sha256: String,
    pub reservation_id: String,
    pub task_id: String,
    pub task_payload_sha256: String,
    pub request_sha256: String,
    pub provider_id: String,
    pub model_id: String,
    pub call_kind: E005ProviderCallKind,
    pub call_number: u8,
    pub kind_call_number: u8,
    pub settlement: String,
    pub network_call_made: bool,
    pub outcome_code: String,
    pub output_source_sha256: Option<String>,
    pub output_gate_sha256: Option<String>,
    pub reserved_input_tokens: u64,
    pub reserved_output_tokens: u64,
    pub reserved_cost_ceiling_microusd: u64,
    pub author_calls_accounted_after: u8,
    pub patch_calls_accounted_after: u8,
    pub calls_accounted_after: u8,
    pub accounted_input_tokens_after: u64,
    pub accounted_output_tokens_after: u64,
    pub accounted_cost_ceiling_microusd_after: u64,
    pub settled_at_unix_ms: i64,
}

impl E005ProviderRunAuthorizationContract {
    pub fn validate_authorized(&self) -> CoreResult<()> {
        let optional_values = [
            self.provider_id.as_deref(),
            self.model_id.as_deref(),
            self.source_policy_sha256.as_deref(),
            self.pricing_snapshot_sha256.as_deref(),
            self.disclosure_sha256.as_deref(),
            self.authorized_at.as_deref(),
            self.expires_at.as_deref(),
        ];
        if self.schema_version != E005_PROVIDER_RUN_AUTHORIZATION_SCHEMA_VERSION
            || !valid_id(&self.authorization_id)
            || !valid_sha256(&self.task_set_sha256)
            || self.status != "authorized"
            || self.grant_mode != "explicit_user_confirmation"
            || optional_values.iter().any(|value| value.is_none())
            || !valid_id(self.provider_id.as_deref().unwrap_or_default())
            || !valid_id(self.model_id.as_deref().unwrap_or_default())
            || !valid_sha256(self.source_policy_sha256.as_deref().unwrap_or_default())
            || !valid_sha256(self.pricing_snapshot_sha256.as_deref().unwrap_or_default())
            || !valid_sha256(self.disclosure_sha256.as_deref().unwrap_or_default())
            || self.maximum_author_calls != E005_MAXIMUM_AUTHOR_CALLS
            || self.maximum_patch_calls != E005_MAXIMUM_PATCH_CALLS
            || self.maximum_total_calls != E005_MAXIMUM_TOTAL_CALLS
            || self.maximum_input_tokens == 0
            || self.maximum_output_tokens == 0
            || self.maximum_variable_cost_microusd == 0
            || self.maximum_batch_wall_time_ms == 0
            || self.maximum_batch_wall_time_ms > 10_800_000
            || self.maximum_single_call_wall_time_ms == 0
            || self.maximum_single_call_wall_time_ms > 105_000
            || self.maximum_single_call_wall_time_ms > self.maximum_batch_wall_time_ms
            || self.whole_object_template_policy != "forbidden"
            || !valid_sha256(&self.authorization_binding_sha256)
            || [
                self.maximum_input_tokens,
                self.maximum_output_tokens,
                self.maximum_variable_cost_microusd,
                self.maximum_batch_wall_time_ms,
                self.maximum_single_call_wall_time_ms,
            ]
            .into_iter()
            .any(|value| value > i64::MAX as u64)
        {
            return Err(invalid(
                "E005 Provider authorization is malformed or not explicit.",
            ));
        }
        self.authorized_time_range_unix_ms()?;
        let mut binding = serde_json::to_value(self).map_err(|_| {
            invalid("E005 Provider authorization could not be canonicalized for binding.")
        })?;
        binding
            .as_object_mut()
            .expect("authorization serializes as object")
            .remove("authorization_binding_sha256");
        if semantic_sha256(&binding)? != self.authorization_binding_sha256 {
            return Err(invalid(
                "E005 Provider authorization binding hash does not match its exact scope.",
            ));
        }
        Ok(())
    }

    fn authorized_time_range_unix_ms(&self) -> CoreResult<(i64, i64)> {
        let authorized_at =
            DateTime::parse_from_rfc3339(self.authorized_at.as_deref().unwrap_or_default())
                .map_err(|_| invalid("E005 authorized_at is not RFC 3339."))?
                .timestamp_millis();
        let expires_at =
            DateTime::parse_from_rfc3339(self.expires_at.as_deref().unwrap_or_default())
                .map_err(|_| invalid("E005 expires_at is not RFC 3339."))?
                .timestamp_millis();
        if authorized_at < 0 || expires_at <= authorized_at {
            return Err(invalid(
                "E005 Provider authorization has an invalid bound time range.",
            ));
        }
        Ok((authorized_at, expires_at))
    }
}

impl E005ProviderBudgetLedger {
    pub fn validate(&self) -> CoreResult<()> {
        self.authorization.validate_authorized()?;
        if self.schema_version != E005_PROVIDER_LEDGER_SCHEMA_VERSION
            || !matches!(
                self.status.as_str(),
                "authorized" | "consumed" | "cancelled" | "expired"
            )
            || self.author_calls_accounted > E005_MAXIMUM_AUTHOR_CALLS
            || self.patch_calls_accounted > E005_MAXIMUM_PATCH_CALLS
            || self.calls_accounted != self.author_calls_accounted + self.patch_calls_accounted
            || self.calls_accounted > E005_MAXIMUM_TOTAL_CALLS
            || sum_exceeds(
                self.authorization.maximum_input_tokens,
                &[self.reserved_input_tokens, self.accounted_input_tokens],
            )
            || sum_exceeds(
                self.authorization.maximum_output_tokens,
                &[self.reserved_output_tokens, self.accounted_output_tokens],
            )
            || sum_exceeds(
                self.authorization.maximum_variable_cost_microusd,
                &[
                    self.reserved_cost_ceiling_microusd,
                    self.accounted_cost_ceiling_microusd,
                ],
            )
            || self.expires_at_unix_ms <= self.authorized_at_unix_ms
            || self.batch_deadline_unix_ms <= self.authorized_at_unix_ms
            || self.batch_deadline_unix_ms > self.expires_at_unix_ms
            || self.updated_at_unix_ms < self.authorized_at_unix_ms
        {
            return Err(invalid(
                "Persisted E005 Provider budget ledger is malformed.",
            ));
        }
        Ok(())
    }
}

impl E005ProviderCallReservationRequest {
    fn validate(&self) -> CoreResult<()> {
        let patch_fields_valid = match self.call_kind {
            E005ProviderCallKind::Author => {
                self.patch_base_source_sha256.is_none() && self.failed_gate_sha256.is_none()
            }
            E005ProviderCallKind::Patch => {
                self.patch_base_source_sha256
                    .as_deref()
                    .is_some_and(valid_sha256)
                    && self.failed_gate_sha256.as_deref().is_some_and(valid_sha256)
            }
        };
        if !valid_id(&self.authorization_id)
            || !valid_sha256(&self.authorization_binding_sha256)
            || !valid_id(&self.provider_id)
            || !valid_id(&self.model_id)
            || !valid_id(&self.task_id)
            || !valid_sha256(&self.task_payload_sha256)
            || !valid_sha256(&self.request_sha256)
            || !patch_fields_valid
            || self.reserved_input_tokens == 0
            || self.reserved_output_tokens == 0
            || self.reserved_cost_ceiling_microusd == 0
            || [
                self.reserved_input_tokens,
                self.reserved_output_tokens,
                self.reserved_cost_ceiling_microusd,
            ]
            .into_iter()
            .any(|value| value > i64::MAX as u64)
        {
            return Err(invalid("E005 Provider reservation request is malformed."));
        }
        Ok(())
    }
}

impl E005ProviderCallSettlement {
    fn validate(&self) -> CoreResult<()> {
        let output_pair = match (
            self.output_source_sha256.as_deref(),
            self.output_gate_sha256.as_deref(),
        ) {
            (Some(source), Some(gate)) => valid_sha256(source) && valid_sha256(gate),
            (None, None) => true,
            _ => false,
        };
        if !output_pair
            || (self.outcome.requires_output() && self.output_source_sha256.is_none())
            || (self.outcome.is_pre_dispatch() && self.output_source_sha256.is_some())
        {
            return Err(invalid(
                "E005 Provider settlement output evidence is malformed.",
            ));
        }
        Ok(())
    }
}

impl CoreRepository {
    pub fn issue_e005_provider_run_authorization(
        &self,
        authorization: &E005ProviderRunAuthorizationContract,
        frozen_task_set: &Value,
    ) -> CoreResult<E005ProviderBudgetLedger> {
        authorization.validate_authorized()?;
        if authorization.task_set_sha256 != E005_FORMAL_TASK_SET_SHA256
            || frozen_task_set
                .get("schema_version")
                .and_then(Value::as_str)
                != Some("E005UnseenTaskSet@1")
            || semantic_sha256(frozen_task_set)? != authorization.task_set_sha256
        {
            return Err(CoreError::conflict(
                "E005_PROVIDER_TASK_SET_MISMATCH",
                "E005 Provider authorization does not match the exact frozen task set.",
            ));
        }
        let tasks = authorized_tasks(frozen_task_set)?;
        let (authorized_at_unix_ms, expires_at_unix_ms) =
            authorization.authorized_time_range_unix_ms()?;
        let batch_deadline_unix_ms = authorized_at_unix_ms
            .checked_add(authorization.maximum_batch_wall_time_ms as i64)
            .ok_or_else(|| invalid("E005 Provider batch deadline overflowed."))?
            .min(expires_at_unix_ms);
        let authorization_json = crate::canonical_json(authorization)?;
        self.write(|transaction| {
            if let Some(existing) = e005_authorization_from_connection(
                transaction,
                &authorization.authorization_id,
            )? {
                if existing.authorization.authorization_binding_sha256
                    != authorization.authorization_binding_sha256
                    || existing.authorized_at_unix_ms != authorized_at_unix_ms
                    || existing.expires_at_unix_ms != expires_at_unix_ms
                {
                    return Err(CoreError::conflict(
                        "E005_PROVIDER_AUTHORIZATION_IDEMPOTENCY_CONFLICT",
                        "E005 authorization ID was already bound to a different contract or time range.",
                    ));
                }
                return Ok(existing);
            }
            transaction.execute(
                "INSERT INTO e005_provider_run_authorizations(authorization_id, task_set_sha256, provider_id, model_id, source_policy_sha256, pricing_snapshot_sha256, disclosure_sha256, authorization_binding_sha256, authorization_json, status, maximum_author_calls, maximum_patch_calls, maximum_total_calls, maximum_input_tokens, maximum_output_tokens, maximum_variable_cost_microusd, maximum_batch_wall_time_ms, maximum_single_call_wall_time_ms, reservations_created, author_calls_accounted, patch_calls_accounted, calls_accounted, reserved_input_tokens, reserved_output_tokens, reserved_cost_ceiling_microusd, accounted_input_tokens, accounted_output_tokens, accounted_cost_ceiling_microusd, authorized_at_unix_ms, expires_at_unix_ms, batch_deadline_unix_ms, updated_at_unix_ms) VALUES (?,?,?,?,?,?,?,?,?,'authorized',?,?,?,?,?,?,?,?,0,0,0,0,0,0,0,0,0,0,?,?,?,?)",
                params![
                    authorization.authorization_id,
                    authorization.task_set_sha256,
                    authorization.provider_id.as_deref().expect("validated"),
                    authorization.model_id.as_deref().expect("validated"),
                    authorization.source_policy_sha256.as_deref().expect("validated"),
                    authorization.pricing_snapshot_sha256.as_deref().expect("validated"),
                    authorization.disclosure_sha256.as_deref().expect("validated"),
                    authorization.authorization_binding_sha256,
                    authorization_json,
                    authorization.maximum_author_calls,
                    authorization.maximum_patch_calls,
                    authorization.maximum_total_calls,
                    authorization.maximum_input_tokens,
                    authorization.maximum_output_tokens,
                    authorization.maximum_variable_cost_microusd,
                    authorization.maximum_batch_wall_time_ms,
                    authorization.maximum_single_call_wall_time_ms,
                    authorized_at_unix_ms,
                    expires_at_unix_ms,
                    batch_deadline_unix_ms,
                    authorized_at_unix_ms,
                ],
            )?;
            for (index, (task_id, task_payload_sha256)) in tasks.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO e005_provider_authorized_tasks(authorization_id, task_id, task_payload_sha256, task_ordinal) VALUES (?,?,?,?)",
                    params![authorization.authorization_id, task_id, task_payload_sha256, index + 1],
                )?;
            }
            require_e005_authorization(transaction, &authorization.authorization_id)
        })
    }

    pub fn e005_provider_budget_ledger(
        &self,
        authorization_id: &str,
    ) -> CoreResult<Option<E005ProviderBudgetLedger>> {
        let connection = open_connection(self.db_path())?;
        e005_authorization_from_connection(&connection, authorization_id)
    }

    pub fn reserve_e005_provider_call(
        &self,
        request: &E005ProviderCallReservationRequest,
    ) -> CoreResult<E005ProviderCallReservation> {
        self.reserve_e005_provider_call_at(request, -1)
    }

    fn reserve_e005_provider_call_at(
        &self,
        request: &E005ProviderCallReservationRequest,
        now_unix_ms: i64,
    ) -> CoreResult<E005ProviderCallReservation> {
        request.validate()?;
        self.write(|transaction| {
            let now_unix_ms = if now_unix_ms < 0 {
                system_time_unix_ms()?
            } else {
                now_unix_ms
            };
            let authorization =
                require_e005_authorization(transaction, &request.authorization_id)?;
            if authorization.status != "authorized"
                || now_unix_ms < authorization.authorized_at_unix_ms
                || now_unix_ms < authorization.updated_at_unix_ms
                || now_unix_ms >= authorization.expires_at_unix_ms
                || now_unix_ms >= authorization.batch_deadline_unix_ms
            {
                return Err(CoreError::conflict(
                    "E005_PROVIDER_AUTHORIZATION_INACTIVE",
                    "E005 Provider call stopped before network because authorization is inactive or expired.",
                ));
            }
            if authorization.authorization.authorization_binding_sha256
                != request.authorization_binding_sha256
                || authorization.authorization.provider_id.as_deref() != Some(&request.provider_id)
                || authorization.authorization.model_id.as_deref() != Some(&request.model_id)
            {
                return Err(CoreError::conflict(
                    "E005_PROVIDER_AUTHORIZATION_LINEAGE_MISMATCH",
                    "E005 Provider call does not match the authorized binding, Provider and model.",
                ));
            }
            let authorized_task_hash: Option<String> = transaction
                .query_row(
                    "SELECT task_payload_sha256 FROM e005_provider_authorized_tasks WHERE authorization_id=? AND task_id=?",
                    params![request.authorization_id, request.task_id],
                    |row| row.get(0),
                )
                .optional()?;
            if authorized_task_hash.as_deref() != Some(&request.task_payload_sha256) {
                return Err(CoreError::conflict(
                    "E005_PROVIDER_TASK_LINEAGE_MISMATCH",
                    "E005 Provider call does not match an exact task in the frozen authorized set.",
                ));
            }
            let deadline_unix_ms = now_unix_ms
                .checked_add(
                    authorization.authorization.maximum_single_call_wall_time_ms as i64,
                )
                .ok_or_else(|| invalid("E005 Provider single-call deadline overflowed."))?
                .min(authorization.expires_at_unix_ms)
                .min(authorization.batch_deadline_unix_ms);
            if deadline_unix_ms <= now_unix_ms {
                return Err(CoreError::conflict(
                    "E005_PROVIDER_DEADLINE_EXCEEDED",
                    "E005 Provider call deadline exceeds its single-call, batch or authorization ceiling.",
                ));
            }
            if matches!(request.call_kind, E005ProviderCallKind::Patch) {
                require_repairable_author(
                    transaction,
                    request,
                    request.patch_base_source_sha256.as_deref().expect("validated"),
                    request.failed_gate_sha256.as_deref().expect("validated"),
                )?;
            }
            let (active_author, active_patch, active_total): (u8, u8, u8) =
                transaction.query_row(
                    "SELECT COALESCE(SUM(CASE WHEN call_kind='author' THEN 1 ELSE 0 END),0), COALESCE(SUM(CASE WHEN call_kind='patch' THEN 1 ELSE 0 END),0), COUNT(*) FROM e005_provider_call_reservations WHERE authorization_id=? AND state IN ('reserved','dispatching','accounted')",
                    [request.authorization_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
            let (active_kind, maximum_kind) = match request.call_kind {
                E005ProviderCallKind::Author => (active_author, E005_MAXIMUM_AUTHOR_CALLS),
                E005ProviderCallKind::Patch => (active_patch, E005_MAXIMUM_PATCH_CALLS),
            };
            if active_kind >= maximum_kind || active_total >= E005_MAXIMUM_TOTAL_CALLS {
                return Err(CoreError::conflict(
                    "E005_PROVIDER_CALL_BUDGET_EXHAUSTED",
                    "E005 Provider call stopped before network because its author, patch or total call ceiling is exhausted.",
                ));
            }
            let task_kind_already_used: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM e005_provider_call_reservations WHERE authorization_id=? AND task_id=? AND call_kind=? AND state IN ('reserved','dispatching','accounted'))",
                params![request.authorization_id, request.task_id, request.call_kind.as_str()],
                |row| row.get(0),
            )?;
            if task_kind_already_used {
                return Err(CoreError::conflict(
                    "E005_PROVIDER_TASK_CALL_ALREADY_USED",
                    "E005 permits at most one network-accounted author and one eligible patch per frozen task.",
                ));
            }
            if sum_exceeds(
                authorization.authorization.maximum_input_tokens,
                &[
                    authorization.reserved_input_tokens,
                    authorization.accounted_input_tokens,
                    request.reserved_input_tokens,
                ],
            ) || sum_exceeds(
                authorization.authorization.maximum_output_tokens,
                &[
                    authorization.reserved_output_tokens,
                    authorization.accounted_output_tokens,
                    request.reserved_output_tokens,
                ],
            ) || sum_exceeds(
                authorization.authorization.maximum_variable_cost_microusd,
                &[
                    authorization.reserved_cost_ceiling_microusd,
                    authorization.accounted_cost_ceiling_microusd,
                    request.reserved_cost_ceiling_microusd,
                ],
            )
            {
                return Err(CoreError::conflict(
                    "E005_PROVIDER_USAGE_BUDGET_EXHAUSTED",
                    "E005 Provider call stopped before network because token or cost ceilings are exhausted.",
                ));
            }
            let reservation_ordinal = authorization.reservations_created.checked_add(1).ok_or_else(|| {
                CoreError::conflict(
                    "E005_PROVIDER_RESERVATION_ORDINAL_EXHAUSTED",
                    "E005 Provider reservation ordinal is exhausted.",
                )
            })?;
            let call_number = active_total + 1;
            let kind_call_number = active_kind + 1;
            let reservation_id = e005_reservation_id(request, reservation_ordinal)?;
            transaction.execute(
                "INSERT INTO e005_provider_call_reservations(reservation_id, authorization_id, authorization_binding_sha256, task_id, task_payload_sha256, call_kind, call_number, kind_call_number, reservation_ordinal, request_sha256, patch_base_source_sha256, failed_gate_sha256, reserved_input_tokens, reserved_output_tokens, reserved_cost_ceiling_microusd, deadline_unix_ms, state, network_call_made, outcome_code, output_source_sha256, output_gate_sha256, created_at_unix_ms, dispatched_at_unix_ms, settled_at_unix_ms) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,'reserved',NULL,NULL,NULL,NULL,?,NULL,NULL)",
                params![
                    reservation_id,
                    request.authorization_id,
                    request.authorization_binding_sha256,
                    request.task_id,
                    request.task_payload_sha256,
                    request.call_kind.as_str(),
                    call_number,
                    kind_call_number,
                    reservation_ordinal,
                    request.request_sha256,
                    request.patch_base_source_sha256,
                    request.failed_gate_sha256,
                    request.reserved_input_tokens,
                    request.reserved_output_tokens,
                    request.reserved_cost_ceiling_microusd,
                    deadline_unix_ms,
                    now_unix_ms,
                ],
            )?;
            transaction.execute(
                "UPDATE e005_provider_run_authorizations SET reservations_created=?, reserved_input_tokens=reserved_input_tokens+?, reserved_output_tokens=reserved_output_tokens+?, reserved_cost_ceiling_microusd=reserved_cost_ceiling_microusd+?, updated_at_unix_ms=? WHERE authorization_id=?",
                params![
                    reservation_ordinal,
                    request.reserved_input_tokens,
                    request.reserved_output_tokens,
                    request.reserved_cost_ceiling_microusd,
                    now_unix_ms,
                    request.authorization_id,
                ],
            )?;
            Ok(E005ProviderCallReservation {
                schema_version: E005_PROVIDER_RESERVATION_SCHEMA_VERSION.into(),
                reservation_id,
                authorization_id: request.authorization_id.clone(),
                authorization_binding_sha256: request.authorization_binding_sha256.clone(),
                task_id: request.task_id.clone(),
                task_payload_sha256: request.task_payload_sha256.clone(),
                call_kind: request.call_kind.clone(),
                call_number,
                kind_call_number,
                reservation_ordinal,
                request_sha256: request.request_sha256.clone(),
                patch_base_source_sha256: request.patch_base_source_sha256.clone(),
                failed_gate_sha256: request.failed_gate_sha256.clone(),
                reserved_input_tokens: request.reserved_input_tokens,
                reserved_output_tokens: request.reserved_output_tokens,
                reserved_cost_ceiling_microusd: request.reserved_cost_ceiling_microusd,
                deadline_unix_ms,
                created_at_unix_ms: now_unix_ms,
            })
        })
    }

    /// Must be committed immediately before polling the Provider future.
    /// Any persisted `dispatching` row is treated as network-attempted after a
    /// restart, even if the process died between this write and socket I/O.
    pub fn mark_e005_provider_call_dispatching(&self, reservation_id: &str) -> CoreResult<()> {
        self.mark_e005_provider_call_dispatching_at(reservation_id, -1)
    }

    fn mark_e005_provider_call_dispatching_at(
        &self,
        reservation_id: &str,
        dispatched_at_unix_ms: i64,
    ) -> CoreResult<()> {
        self.write(|transaction| {
            let dispatched_at_unix_ms = if dispatched_at_unix_ms < 0 {
                system_time_unix_ms()?
            } else {
                dispatched_at_unix_ms
            };
            let (state, deadline, created_at, authorization_status, expires_at, batch_deadline, updated_at): (String, i64, i64, String, i64, i64, i64) = transaction
                .query_row(
                    "SELECT r.state, r.deadline_unix_ms, r.created_at_unix_ms, a.status, a.expires_at_unix_ms, a.batch_deadline_unix_ms, a.updated_at_unix_ms FROM e005_provider_call_reservations r JOIN e005_provider_run_authorizations a ON a.authorization_id=r.authorization_id WHERE r.reservation_id=?",
                    [reservation_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
                )
                .map_err(|error| match error {
                    rusqlite::Error::QueryReturnedNoRows => {
                        CoreError::not_found("E005 Provider call reservation")
                    }
                    other => CoreError::Sqlite(other),
                })?;
            if state == "dispatching" {
                return Err(CoreError::conflict(
                    "E005_PROVIDER_DISPATCH_ALREADY_ATTEMPTED",
                    "E005 Provider reservation already entered dispatching and cannot authorize another network attempt.",
                ));
            }
            if state != "reserved" {
                return Err(CoreError::conflict(
                    "E005_PROVIDER_DISPATCH_STATE_INVALID",
                    "Only a reserved E005 Provider call can enter dispatching state.",
                ));
            }
            if authorization_status != "authorized"
                || dispatched_at_unix_ms < updated_at
                || dispatched_at_unix_ms >= expires_at
                || dispatched_at_unix_ms >= batch_deadline
                || dispatched_at_unix_ms < created_at
                || dispatched_at_unix_ms >= deadline
            {
                return Err(CoreError::conflict(
                    "E005_PROVIDER_DISPATCH_DEADLINE_EXPIRED",
                    "E005 Provider call stopped before network because its deadline expired.",
                ));
            }
            let changed = transaction.execute(
                "UPDATE e005_provider_call_reservations SET state='dispatching', network_call_made=1, dispatched_at_unix_ms=? WHERE reservation_id=? AND state='reserved'",
                params![dispatched_at_unix_ms, reservation_id],
            )?;
            if changed != 1 {
                return Err(CoreError::conflict(
                    "E005_PROVIDER_DISPATCH_NOT_ACQUIRED",
                    "E005 Provider dispatch permit was not atomically acquired.",
                ));
            }
            Ok(())
        })
    }

    pub fn settle_e005_provider_call(
        &self,
        reservation_id: &str,
        settlement: &E005ProviderCallSettlement,
    ) -> CoreResult<E005ProviderBudgetEvidence> {
        self.settle_e005_provider_call_at(reservation_id, settlement, -1)
    }

    /// Revalidates exported formal evidence against the canonical JSON and
    /// exact reservation row persisted by the Rust-owned ledger. Formal
    /// receipt construction must call this for every author/patch evidence
    /// item instead of trusting caller-supplied JSON.
    pub fn verify_e005_provider_budget_evidence(
        &self,
        evidence: &E005ProviderBudgetEvidence,
    ) -> CoreResult<()> {
        let connection = open_connection(self.db_path())?;
        let row = e005_reservation_row(&connection, &evidence.reservation_id)?;
        let persisted = persisted_e005_budget_evidence(&connection, &row)?;
        if &persisted != evidence {
            return Err(CoreError::conflict(
                "E005_PROVIDER_EVIDENCE_EXPORT_MISMATCH",
                "Formal Provider evidence does not match the exact persisted reservation evidence.",
            ));
        }
        Ok(())
    }

    fn settle_e005_provider_call_at(
        &self,
        reservation_id: &str,
        settlement: &E005ProviderCallSettlement,
        settled_at_unix_ms: i64,
    ) -> CoreResult<E005ProviderBudgetEvidence> {
        settlement.validate()?;
        self.write(|transaction| {
            let settled_at_unix_ms = if settled_at_unix_ms < 0 {
                system_time_unix_ms()?
            } else {
                settled_at_unix_ms
            };
            settle_e005_provider_call_transaction(
                transaction,
                reservation_id,
                settlement,
                settled_at_unix_ms,
            )
        })
    }

    /// Startup-only recovery: reservations that never reached dispatch are
    /// released, while every uncertain dispatch is conservatively accounted.
    /// A formal runner must call this successfully before constructing a
    /// Provider client.
    pub fn recover_e005_provider_budget_after_restart(
        &self,
    ) -> CoreResult<Vec<E005ProviderBudgetEvidence>> {
        self.recover_e005_provider_budget_after_restart_at(-1)
    }

    fn recover_e005_provider_budget_after_restart_at(
        &self,
        recovered_at_unix_ms: i64,
    ) -> CoreResult<Vec<E005ProviderBudgetEvidence>> {
        self.write(|transaction| {
            let recovered_at_unix_ms = if recovered_at_unix_ms < 0 {
                system_time_unix_ms()?
            } else {
                recovered_at_unix_ms
            };
            let reservations = {
                let mut statement = transaction.prepare(
                    "SELECT reservation_id, state FROM e005_provider_call_reservations WHERE state IN ('reserved','dispatching') ORDER BY authorization_id, reservation_ordinal",
                )?;
                let rows = statement
                    .query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                rows
            };
            reservations
                .iter()
                .map(|(reservation_id, state)| {
                    let outcome = if state == "reserved" {
                        E005ProviderCallOutcome::PreDispatchReleased
                    } else {
                        E005ProviderCallOutcome::RecoveredUncertainDispatch
                    };
                    settle_e005_provider_call_transaction(
                        transaction,
                        reservation_id,
                        &E005ProviderCallSettlement {
                            outcome,
                            output_source_sha256: None,
                            output_gate_sha256: None,
                        },
                        recovered_at_unix_ms,
                    )
                })
                .collect()
        })
    }
}

fn settle_e005_provider_call_transaction(
    transaction: &Transaction<'_>,
    reservation_id: &str,
    settlement: &E005ProviderCallSettlement,
    settled_at_unix_ms: i64,
) -> CoreResult<E005ProviderBudgetEvidence> {
    if settled_at_unix_ms < 0 {
        return Err(invalid("E005 Provider settlement clock is invalid."));
    }
    let row = e005_reservation_row(transaction, reservation_id)?;
    let expected_state = if settlement.outcome.is_pre_dispatch() {
        "reserved"
    } else {
        "dispatching"
    };
    if matches!(row.state.as_str(), "accounted" | "released") {
        if row.outcome_code.as_deref() != Some(settlement.outcome.as_str())
            || row.output_source_sha256 != settlement.output_source_sha256
            || row.output_gate_sha256 != settlement.output_gate_sha256
        {
            return Err(CoreError::conflict(
                "E005_PROVIDER_SETTLEMENT_CONFLICT",
                "E005 Provider call was already settled with different evidence.",
            ));
        }
        return persisted_e005_budget_evidence(transaction, &row);
    }
    if row.state != expected_state {
        return Err(CoreError::conflict(
            "E005_PROVIDER_SETTLEMENT_STATE_INVALID",
            "Only a proven pre-dispatch failure may release budget; every dispatching call must be accounted.",
        ));
    }
    let network_call_made = row.state == "dispatching";
    let final_state = if network_call_made {
        "accounted"
    } else {
        "released"
    };
    if settled_at_unix_ms < row.created_at_unix_ms
        || row
            .dispatched_at_unix_ms
            .is_some_and(|dispatched| settled_at_unix_ms < dispatched)
    {
        return Err(CoreError::conflict(
            "E005_PROVIDER_SETTLEMENT_TIME_INVALID",
            "E005 Provider settlement time precedes reservation or dispatch.",
        ));
    }
    let (author_delta, patch_delta, call_delta) = if network_call_made {
        match row.call_kind {
            E005ProviderCallKind::Author => (1, 0, 1),
            E005ProviderCallKind::Patch => (0, 1, 1),
        }
    } else {
        (0, 0, 0)
    };
    transaction.execute(
        "UPDATE e005_provider_run_authorizations SET reserved_input_tokens=reserved_input_tokens-?, reserved_output_tokens=reserved_output_tokens-?, reserved_cost_ceiling_microusd=reserved_cost_ceiling_microusd-?, author_calls_accounted=author_calls_accounted+?, patch_calls_accounted=patch_calls_accounted+?, calls_accounted=calls_accounted+?, accounted_input_tokens=accounted_input_tokens+?, accounted_output_tokens=accounted_output_tokens+?, accounted_cost_ceiling_microusd=accounted_cost_ceiling_microusd+?, status=CASE WHEN calls_accounted+? >= maximum_total_calls OR accounted_input_tokens+? >= maximum_input_tokens OR accounted_output_tokens+? >= maximum_output_tokens OR accounted_cost_ceiling_microusd+? >= maximum_variable_cost_microusd THEN 'consumed' ELSE status END, updated_at_unix_ms=? WHERE authorization_id=?",
        params![
            row.reserved_input_tokens,
            row.reserved_output_tokens,
            row.reserved_cost_ceiling_microusd,
            author_delta,
            patch_delta,
            call_delta,
            if network_call_made { row.reserved_input_tokens } else { 0 },
            if network_call_made { row.reserved_output_tokens } else { 0 },
            if network_call_made { row.reserved_cost_ceiling_microusd } else { 0 },
            call_delta,
            if network_call_made { row.reserved_input_tokens } else { 0 },
            if network_call_made { row.reserved_output_tokens } else { 0 },
            if network_call_made { row.reserved_cost_ceiling_microusd } else { 0 },
            settled_at_unix_ms,
            row.authorization_id,
        ],
    )?;
    let mut settled = row;
    settled.state = final_state.into();
    settled.outcome_code = Some(settlement.outcome.as_str().into());
    settled.output_source_sha256 = settlement.output_source_sha256.clone();
    settled.output_gate_sha256 = settlement.output_gate_sha256.clone();
    settled.settled_at_unix_ms = Some(settled_at_unix_ms);
    let evidence = build_e005_budget_evidence(transaction, &settled)?;
    let evidence_json = crate::canonical_json(&evidence)?;
    let evidence_sha256 = semantic_sha256(&evidence)?;
    let changed = transaction.execute(
        "UPDATE e005_provider_call_reservations SET state=?, network_call_made=?, outcome_code=?, output_source_sha256=?, output_gate_sha256=?, settlement_evidence_json=?, settlement_evidence_sha256=?, settled_at_unix_ms=? WHERE reservation_id=? AND state=?",
        params![
            final_state,
            network_call_made,
            settlement.outcome.as_str(),
            settlement.output_source_sha256,
            settlement.output_gate_sha256,
            evidence_json,
            evidence_sha256,
            settled_at_unix_ms,
            reservation_id,
            expected_state,
        ],
    )?;
    if changed != 1 {
        return Err(CoreError::conflict(
            "E005_PROVIDER_SETTLEMENT_NOT_ACQUIRED",
            "E005 Provider settlement was not atomically acquired.",
        ));
    }
    Ok(evidence)
}

#[derive(Debug, Clone)]
struct E005ReservationRow {
    reservation_id: String,
    authorization_id: String,
    authorization_binding_sha256: String,
    task_id: String,
    task_payload_sha256: String,
    request_sha256: String,
    call_kind: E005ProviderCallKind,
    call_number: u8,
    kind_call_number: u8,
    reserved_input_tokens: u64,
    reserved_output_tokens: u64,
    reserved_cost_ceiling_microusd: u64,
    state: String,
    outcome_code: Option<String>,
    output_source_sha256: Option<String>,
    output_gate_sha256: Option<String>,
    created_at_unix_ms: i64,
    dispatched_at_unix_ms: Option<i64>,
    settled_at_unix_ms: Option<i64>,
    settlement_evidence_json: Option<String>,
    settlement_evidence_sha256: Option<String>,
}

fn e005_reservation_row(
    connection: &Connection,
    reservation_id: &str,
) -> CoreResult<E005ReservationRow> {
    connection
        .query_row(
            "SELECT authorization_id, authorization_binding_sha256, task_id, task_payload_sha256, request_sha256, call_kind, call_number, kind_call_number, reserved_input_tokens, reserved_output_tokens, reserved_cost_ceiling_microusd, state, outcome_code, output_source_sha256, output_gate_sha256, created_at_unix_ms, dispatched_at_unix_ms, settled_at_unix_ms, settlement_evidence_json, settlement_evidence_sha256 FROM e005_provider_call_reservations WHERE reservation_id=?",
            [reservation_id],
            |row| {
                let call_kind: String = row.get(5)?;
                Ok(E005ReservationRow {
                    reservation_id: reservation_id.into(),
                    authorization_id: row.get(0)?,
                    authorization_binding_sha256: row.get(1)?,
                    task_id: row.get(2)?,
                    task_payload_sha256: row.get(3)?,
                    request_sha256: row.get(4)?,
                    call_kind: if call_kind == "author" {
                        E005ProviderCallKind::Author
                    } else {
                        E005ProviderCallKind::Patch
                    },
                    call_number: row.get(6)?,
                    kind_call_number: row.get(7)?,
                    reserved_input_tokens: row.get(8)?,
                    reserved_output_tokens: row.get(9)?,
                    reserved_cost_ceiling_microusd: row.get(10)?,
                    state: row.get(11)?,
                    outcome_code: row.get(12)?,
                    output_source_sha256: row.get(13)?,
                    output_gate_sha256: row.get(14)?,
                    created_at_unix_ms: row.get(15)?,
                    dispatched_at_unix_ms: row.get(16)?,
                    settled_at_unix_ms: row.get(17)?,
                    settlement_evidence_json: row.get(18)?,
                    settlement_evidence_sha256: row.get(19)?,
                })
            },
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => {
                CoreError::not_found("E005 Provider call reservation")
            }
            other => CoreError::Sqlite(other),
        })
}

fn build_e005_budget_evidence(
    connection: &Connection,
    row: &E005ReservationRow,
) -> CoreResult<E005ProviderBudgetEvidence> {
    let authorization = require_e005_authorization(connection, &row.authorization_id)?;
    Ok(E005ProviderBudgetEvidence {
        schema_version: E005_PROVIDER_BUDGET_EVIDENCE_SCHEMA_VERSION.into(),
        authorization_id: row.authorization_id.clone(),
        authorization_binding_sha256: row.authorization_binding_sha256.clone(),
        reservation_id: row.reservation_id.clone(),
        task_id: row.task_id.clone(),
        task_payload_sha256: row.task_payload_sha256.clone(),
        request_sha256: row.request_sha256.clone(),
        provider_id: authorization
            .authorization
            .provider_id
            .clone()
            .expect("validated authorization"),
        model_id: authorization
            .authorization
            .model_id
            .clone()
            .expect("validated authorization"),
        call_kind: row.call_kind.clone(),
        call_number: row.call_number,
        kind_call_number: row.kind_call_number,
        settlement: row.state.clone(),
        network_call_made: row.state == "accounted",
        outcome_code: row.outcome_code.clone().unwrap_or_default(),
        output_source_sha256: row.output_source_sha256.clone(),
        output_gate_sha256: row.output_gate_sha256.clone(),
        reserved_input_tokens: row.reserved_input_tokens,
        reserved_output_tokens: row.reserved_output_tokens,
        reserved_cost_ceiling_microusd: row.reserved_cost_ceiling_microusd,
        author_calls_accounted_after: authorization.author_calls_accounted,
        patch_calls_accounted_after: authorization.patch_calls_accounted,
        calls_accounted_after: authorization.calls_accounted,
        accounted_input_tokens_after: authorization.accounted_input_tokens,
        accounted_output_tokens_after: authorization.accounted_output_tokens,
        accounted_cost_ceiling_microusd_after: authorization.accounted_cost_ceiling_microusd,
        settled_at_unix_ms: row.settled_at_unix_ms.ok_or_else(|| {
            invalid("Settled E005 Provider reservation has no settlement timestamp.")
        })?,
    })
}

fn persisted_e005_budget_evidence(
    connection: &Connection,
    row: &E005ReservationRow,
) -> CoreResult<E005ProviderBudgetEvidence> {
    let json = row.settlement_evidence_json.as_deref().ok_or_else(|| {
        invalid("Settled E005 Provider reservation has no persisted budget evidence.")
    })?;
    let evidence: E005ProviderBudgetEvidence = serde_json::from_str(json)
        .map_err(|_| invalid("Persisted E005 Provider budget evidence cannot be decoded."))?;
    if semantic_sha256(&evidence)? != row.settlement_evidence_sha256.clone().unwrap_or_default() {
        return Err(invalid(
            "Persisted E005 Provider budget evidence hash does not match.",
        ));
    }
    let authorization = require_e005_authorization(connection, &row.authorization_id)?;
    let provider_id = authorization
        .authorization
        .provider_id
        .as_deref()
        .unwrap_or_default();
    let model_id = authorization
        .authorization
        .model_id
        .as_deref()
        .unwrap_or_default();
    if crate::canonical_json(&evidence)? != json
        || evidence.schema_version != E005_PROVIDER_BUDGET_EVIDENCE_SCHEMA_VERSION
        || evidence.authorization_id != row.authorization_id
        || evidence.authorization_binding_sha256 != row.authorization_binding_sha256
        || evidence.reservation_id != row.reservation_id
        || evidence.task_id != row.task_id
        || evidence.task_payload_sha256 != row.task_payload_sha256
        || evidence.request_sha256 != row.request_sha256
        || evidence.provider_id != provider_id
        || evidence.model_id != model_id
        || evidence.call_kind != row.call_kind
        || evidence.call_number != row.call_number
        || evidence.kind_call_number != row.kind_call_number
        || evidence.settlement != row.state
        || evidence.network_call_made != (row.state == "accounted")
        || evidence.outcome_code != row.outcome_code.clone().unwrap_or_default()
        || evidence.output_source_sha256 != row.output_source_sha256
        || evidence.output_gate_sha256 != row.output_gate_sha256
        || evidence.reserved_input_tokens != row.reserved_input_tokens
        || evidence.reserved_output_tokens != row.reserved_output_tokens
        || evidence.reserved_cost_ceiling_microusd != row.reserved_cost_ceiling_microusd
        || Some(evidence.settled_at_unix_ms) != row.settled_at_unix_ms
    {
        return Err(invalid(
            "Persisted E005 Provider budget evidence does not bind its reservation row.",
        ));
    }
    Ok(evidence)
}

fn require_repairable_author(
    connection: &Connection,
    request: &E005ProviderCallReservationRequest,
    patch_base_source_sha256: &str,
    failed_gate_sha256: &str,
) -> CoreResult<()> {
    let reservation_id: Option<String> = connection
        .query_row(
        "SELECT reservation_id FROM e005_provider_call_reservations WHERE authorization_id=? AND task_id=? AND call_kind='author' AND state='accounted' AND outcome_code='PROVIDER_COMPLETED_REPAIRABLE' AND output_source_sha256=? AND output_gate_sha256=?",
        params![request.authorization_id, request.task_id, patch_base_source_sha256, failed_gate_sha256],
        |row| row.get(0),
    )
        .optional()?;
    let Some(reservation_id) = reservation_id else {
        return Err(CoreError::conflict(
            "E005_PROVIDER_PATCH_NOT_ELIGIBLE",
            "E005 patch requires the exact source and repairable failed-gate evidence from the same task's accounted author call.",
        ));
    };
    let author_row = e005_reservation_row(connection, &reservation_id)?;
    persisted_e005_budget_evidence(connection, &author_row)?;
    Ok(())
}

fn authorized_tasks(task_set: &Value) -> CoreResult<Vec<(String, String)>> {
    let tasks = task_set
        .get("tasks")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("E005 frozen task set has no task array."))?;
    if tasks.len() != E005_TASK_COUNT {
        return Err(invalid(
            "E005 frozen task set must contain exactly 30 tasks.",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut result = Vec::with_capacity(tasks.len());
    for task in tasks {
        let task_id = task
            .get("task_id")
            .and_then(Value::as_str)
            .filter(|value| valid_id(value))
            .ok_or_else(|| invalid("E005 frozen task has an invalid task_id."))?;
        if !ids.insert(task_id.to_string()) {
            return Err(invalid("E005 frozen task IDs must be unique."));
        }
        result.push((task_id.to_string(), semantic_sha256(task)?));
    }
    Ok(result)
}

#[derive(Debug)]
struct PersistedE005AuthorizationScope {
    task_set_sha256: String,
    provider_id: String,
    model_id: String,
    source_policy_sha256: String,
    pricing_snapshot_sha256: String,
    disclosure_sha256: String,
    authorization_binding_sha256: String,
    maximum_author_calls: u8,
    maximum_patch_calls: u8,
    maximum_total_calls: u8,
    maximum_input_tokens: u64,
    maximum_output_tokens: u64,
    maximum_variable_cost_microusd: u64,
    maximum_batch_wall_time_ms: u64,
    maximum_single_call_wall_time_ms: u64,
}

fn e005_authorization_from_connection(
    connection: &Connection,
    authorization_id: &str,
) -> CoreResult<Option<E005ProviderBudgetLedger>> {
    let value: Option<(E005ProviderBudgetLedger, PersistedE005AuthorizationScope)> = connection
        .query_row(
            "SELECT authorization_json, status, reservations_created, author_calls_accounted, patch_calls_accounted, calls_accounted, reserved_input_tokens, reserved_output_tokens, reserved_cost_ceiling_microusd, accounted_input_tokens, accounted_output_tokens, accounted_cost_ceiling_microusd, authorized_at_unix_ms, expires_at_unix_ms, batch_deadline_unix_ms, updated_at_unix_ms, task_set_sha256, provider_id, model_id, source_policy_sha256, pricing_snapshot_sha256, disclosure_sha256, authorization_binding_sha256, maximum_author_calls, maximum_patch_calls, maximum_total_calls, maximum_input_tokens, maximum_output_tokens, maximum_variable_cost_microusd, maximum_batch_wall_time_ms, maximum_single_call_wall_time_ms FROM e005_provider_run_authorizations WHERE authorization_id=?",
            [authorization_id],
            |row| {
                let authorization_json: String = row.get(0)?;
                let authorization = serde_json::from_str(&authorization_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        authorization_json.len(),
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok((
                    E005ProviderBudgetLedger {
                        schema_version: E005_PROVIDER_LEDGER_SCHEMA_VERSION.into(),
                        authorization,
                        status: row.get(1)?,
                        reservations_created: row.get(2)?,
                        author_calls_accounted: row.get(3)?,
                        patch_calls_accounted: row.get(4)?,
                        calls_accounted: row.get(5)?,
                        reserved_input_tokens: row.get(6)?,
                        reserved_output_tokens: row.get(7)?,
                        reserved_cost_ceiling_microusd: row.get(8)?,
                        accounted_input_tokens: row.get(9)?,
                        accounted_output_tokens: row.get(10)?,
                        accounted_cost_ceiling_microusd: row.get(11)?,
                        authorized_at_unix_ms: row.get(12)?,
                        expires_at_unix_ms: row.get(13)?,
                        batch_deadline_unix_ms: row.get(14)?,
                        updated_at_unix_ms: row.get(15)?,
                    },
                    PersistedE005AuthorizationScope {
                        task_set_sha256: row.get(16)?,
                        provider_id: row.get(17)?,
                        model_id: row.get(18)?,
                        source_policy_sha256: row.get(19)?,
                        pricing_snapshot_sha256: row.get(20)?,
                        disclosure_sha256: row.get(21)?,
                        authorization_binding_sha256: row.get(22)?,
                        maximum_author_calls: row.get(23)?,
                        maximum_patch_calls: row.get(24)?,
                        maximum_total_calls: row.get(25)?,
                        maximum_input_tokens: row.get(26)?,
                        maximum_output_tokens: row.get(27)?,
                        maximum_variable_cost_microusd: row.get(28)?,
                        maximum_batch_wall_time_ms: row.get(29)?,
                        maximum_single_call_wall_time_ms: row.get(30)?,
                    },
                ))
            },
        )
        .optional()?;
    let Some((ledger, scope)) = value else {
        return Ok(None);
    };
    ledger.validate()?;
    let authorization = &ledger.authorization;
    if scope.task_set_sha256 != authorization.task_set_sha256
        || scope.provider_id != authorization.provider_id.clone().unwrap_or_default()
        || scope.model_id != authorization.model_id.clone().unwrap_or_default()
        || scope.source_policy_sha256
            != authorization
                .source_policy_sha256
                .clone()
                .unwrap_or_default()
        || scope.pricing_snapshot_sha256
            != authorization
                .pricing_snapshot_sha256
                .clone()
                .unwrap_or_default()
        || scope.disclosure_sha256 != authorization.disclosure_sha256.clone().unwrap_or_default()
        || scope.authorization_binding_sha256 != authorization.authorization_binding_sha256
        || scope.maximum_author_calls != authorization.maximum_author_calls
        || scope.maximum_patch_calls != authorization.maximum_patch_calls
        || scope.maximum_total_calls != authorization.maximum_total_calls
        || scope.maximum_input_tokens != authorization.maximum_input_tokens
        || scope.maximum_output_tokens != authorization.maximum_output_tokens
        || scope.maximum_variable_cost_microusd != authorization.maximum_variable_cost_microusd
        || scope.maximum_batch_wall_time_ms != authorization.maximum_batch_wall_time_ms
        || scope.maximum_single_call_wall_time_ms != authorization.maximum_single_call_wall_time_ms
    {
        return Err(invalid(
            "Persisted E005 authorization SQL scope diverges from its canonical JSON.",
        ));
    }
    Ok(Some(ledger))
}

fn require_e005_authorization(
    connection: &Connection,
    authorization_id: &str,
) -> CoreResult<E005ProviderBudgetLedger> {
    e005_authorization_from_connection(connection, authorization_id)?
        .ok_or_else(|| CoreError::not_found("E005 Provider run authorization"))
}

fn e005_reservation_id(
    request: &E005ProviderCallReservationRequest,
    reservation_ordinal: u32,
) -> CoreResult<String> {
    let digest = semantic_sha256(&json!({
        "schema_version": "E005ProviderCallReservationIdentity@1",
        "authorization_id": request.authorization_id,
        "task_id": request.task_id,
        "call_kind": request.call_kind,
        "request_sha256": request.request_sha256,
        "reservation_ordinal": reservation_ordinal,
    }))?;
    Ok(format!("e005reserve_{}", &digest[..24]))
}

fn valid_id(value: &str) -> bool {
    (3..=128).contains(&value.len())
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

fn sum_exceeds(limit: u64, values: &[u64]) -> bool {
    values
        .iter()
        .try_fold(0_u64, |total, value| total.checked_add(*value))
        .is_none_or(|total| total > limit)
}

fn system_time_unix_ms() -> CoreResult<i64> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid("System clock is before the Unix epoch."))?
        .as_millis();
    i64::try_from(millis).map_err(|_| invalid("System clock exceeds the E005 time range."))
}

fn invalid(message: impl Into<String>) -> CoreError {
    CoreError::invalid_data("E005_PROVIDER_BUDGET_INVALID", message)
}

#[cfg(test)]
mod tests {
    use chrono::{SecondsFormat, Utc};
    use tempfile::tempdir;

    use super::*;
    use crate::{
        migration::open_connection, ContentAddressedObjectStore, MigrationRunner, StateOwner,
        WriterLease,
    };

    struct Fixture {
        _root: tempfile::TempDir,
        repository: CoreRepository,
        task_set: Value,
        authorization: E005ProviderRunAuthorizationContract,
        task_ids: Vec<String>,
        task_hashes: Vec<String>,
        base_time_unix_ms: i64,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempdir().unwrap();
            let db = root.path().join("forgecad.db");
            MigrationRunner::new(&db).run().unwrap();
            let lease = WriterLease::acquire(
                &db,
                root.path(),
                "e005-budget-test-writer",
                StateOwner::PythonCompatibilityAdapter,
            )
            .unwrap();
            let store = ContentAddressedObjectStore::new(root.path()).unwrap();
            let repository = CoreRepository::new(lease, store).unwrap();
            let task_set: Value = serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../packages/concept-spec/fixtures/e005-unseen-mechanical-hard-surface-task-set.json"
            )))
            .unwrap();
            assert_eq!(
                semantic_sha256(&task_set).unwrap(),
                E005_FORMAL_TASK_SET_SHA256
            );
            let tasks = task_set.get("tasks").unwrap().as_array().unwrap();
            let task_ids = tasks
                .iter()
                .map(|task| task.get("task_id").unwrap().as_str().unwrap().to_string())
                .collect();
            let task_hashes = tasks
                .iter()
                .map(|task| semantic_sha256(task).unwrap())
                .collect();
            let base_time_unix_ms = system_time_unix_ms().unwrap();
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
                authorization_id: "e005_auth_test".into(),
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
            Self {
                _root: root,
                repository,
                task_set,
                authorization,
                task_ids,
                task_hashes,
                base_time_unix_ms,
            }
        }

        fn issue(&self) -> E005ProviderBudgetLedger {
            self.repository
                .issue_e005_provider_run_authorization(&self.authorization, &self.task_set)
                .unwrap()
        }

        fn request(
            &self,
            task_index: usize,
            kind: E005ProviderCallKind,
        ) -> E005ProviderCallReservationRequest {
            E005ProviderCallReservationRequest {
                authorization_id: self.authorization.authorization_id.clone(),
                authorization_binding_sha256: self
                    .authorization
                    .authorization_binding_sha256
                    .clone(),
                provider_id: self.authorization.provider_id.clone().unwrap(),
                model_id: self.authorization.model_id.clone().unwrap(),
                task_id: self.task_ids[task_index].clone(),
                task_payload_sha256: self.task_hashes[task_index].clone(),
                call_kind: kind,
                request_sha256: format!("{:064x}", task_index + 100),
                patch_base_source_sha256: None,
                failed_gate_sha256: None,
                reserved_input_tokens: 10_000,
                reserved_output_tokens: 5_000,
                reserved_cost_ceiling_microusd: 1_000_000,
            }
        }

        fn dispatch_and_settle(
            &self,
            reservation: &E005ProviderCallReservation,
            outcome: E005ProviderCallOutcome,
            source: Option<String>,
            gate: Option<String>,
        ) -> E005ProviderBudgetEvidence {
            self.repository
                .mark_e005_provider_call_dispatching(&reservation.reservation_id)
                .unwrap();
            self.repository
                .settle_e005_provider_call(
                    &reservation.reservation_id,
                    &E005ProviderCallSettlement {
                        outcome,
                        output_source_sha256: source,
                        output_gate_sha256: gate,
                    },
                )
                .unwrap()
        }
    }

    #[test]
    fn e005_budget_rejects_the_31st_author_before_network() {
        let fixture = Fixture::new();
        fixture.issue();
        for index in 0..30 {
            let request = fixture.request(index, E005ProviderCallKind::Author);
            let reservation = fixture
                .repository
                .reserve_e005_provider_call(&request)
                .unwrap();
            fixture.dispatch_and_settle(
                &reservation,
                E005ProviderCallOutcome::ProviderCompletedPassed,
                Some(format!("{:064x}", index + 1_000)),
                Some(format!("{:064x}", index + 2_000)),
            );
        }
        let mut duplicate = fixture.request(0, E005ProviderCallKind::Author);
        duplicate.request_sha256 = "f".repeat(64);
        assert_eq!(
            fixture
                .repository
                .reserve_e005_provider_call(&duplicate)
                .unwrap_err()
                .code(),
            "E005_PROVIDER_CALL_BUDGET_EXHAUSTED"
        );
        let ledger = fixture
            .repository
            .e005_provider_budget_ledger(&fixture.authorization.authorization_id)
            .unwrap()
            .unwrap();
        assert_eq!(ledger.status, "authorized");
        assert_eq!(ledger.author_calls_accounted, 30);
        assert_eq!(ledger.calls_accounted, 30);
    }

    #[test]
    fn e005_exported_evidence_must_match_the_persisted_reservation() {
        let fixture = Fixture::new();
        fixture.issue();
        let request = fixture.request(0, E005ProviderCallKind::Author);
        let reservation = fixture
            .repository
            .reserve_e005_provider_call(&request)
            .unwrap();
        let evidence = fixture.dispatch_and_settle(
            &reservation,
            E005ProviderCallOutcome::ProviderCompletedPassed,
            Some("a".repeat(64)),
            Some("b".repeat(64)),
        );
        fixture
            .repository
            .verify_e005_provider_budget_evidence(&evidence)
            .unwrap();
        let mut tampered = evidence;
        tampered.request_sha256 = "c".repeat(64);
        assert_eq!(
            fixture
                .repository
                .verify_e005_provider_budget_evidence(&tampered)
                .unwrap_err()
                .code(),
            "E005_PROVIDER_EVIDENCE_EXPORT_MISMATCH"
        );
    }

    #[test]
    fn e005_patch_requires_repairable_exact_author_evidence_and_only_one_patch() {
        let fixture = Fixture::new();
        fixture.issue();
        let author_request = fixture.request(0, E005ProviderCallKind::Author);
        let author = fixture
            .repository
            .reserve_e005_provider_call(&author_request)
            .unwrap();
        let source = "a".repeat(64);
        let gate = "b".repeat(64);
        fixture.dispatch_and_settle(
            &author,
            E005ProviderCallOutcome::ProviderCompletedRepairable,
            Some(source.clone()),
            Some(gate.clone()),
        );
        let mut patch = fixture.request(0, E005ProviderCallKind::Patch);
        patch.request_sha256 = "c".repeat(64);
        patch.patch_base_source_sha256 = Some(source);
        patch.failed_gate_sha256 = Some(gate);
        let reservation = fixture
            .repository
            .reserve_e005_provider_call(&patch)
            .unwrap();
        fixture.dispatch_and_settle(
            &reservation,
            E005ProviderCallOutcome::ProviderCompletedPassed,
            Some("d".repeat(64)),
            Some("e".repeat(64)),
        );
        assert_eq!(
            fixture
                .repository
                .reserve_e005_provider_call(&patch)
                .unwrap_err()
                .code(),
            "E005_PROVIDER_TASK_CALL_ALREADY_USED"
        );
    }

    #[test]
    fn e005_budget_rejects_the_31st_patch_and_61st_total_before_network() {
        let fixture = Fixture::new();
        fixture.issue();
        let mut repairable = Vec::new();
        for index in 0..30 {
            let request = fixture.request(index, E005ProviderCallKind::Author);
            let reservation = fixture
                .repository
                .reserve_e005_provider_call(&request)
                .unwrap();
            let source = format!("{:064x}", index + 10_000);
            let gate = format!("{:064x}", index + 20_000);
            fixture.dispatch_and_settle(
                &reservation,
                E005ProviderCallOutcome::ProviderCompletedRepairable,
                Some(source.clone()),
                Some(gate.clone()),
            );
            repairable.push((source, gate));
        }
        for (index, (source, gate)) in repairable.into_iter().enumerate() {
            let mut request = fixture.request(index, E005ProviderCallKind::Patch);
            request.request_sha256 = format!("{:064x}", index + 30_000);
            request.patch_base_source_sha256 = Some(source);
            request.failed_gate_sha256 = Some(gate);
            let reservation = fixture
                .repository
                .reserve_e005_provider_call(&request)
                .unwrap();
            fixture.dispatch_and_settle(
                &reservation,
                E005ProviderCallOutcome::ProviderCompletedPassed,
                Some(format!("{:064x}", index + 40_000)),
                Some(format!("{:064x}", index + 50_000)),
            );
        }
        let ledger = fixture
            .repository
            .e005_provider_budget_ledger(&fixture.authorization.authorization_id)
            .unwrap()
            .unwrap();
        assert_eq!(ledger.status, "consumed");
        assert_eq!(ledger.author_calls_accounted, 30);
        assert_eq!(ledger.patch_calls_accounted, 30);
        assert_eq!(ledger.calls_accounted, 60);
        let mut sixty_first = fixture.request(0, E005ProviderCallKind::Patch);
        sixty_first.patch_base_source_sha256 = Some("a".repeat(64));
        sixty_first.failed_gate_sha256 = Some("b".repeat(64));
        assert_eq!(
            fixture
                .repository
                .reserve_e005_provider_call(&sixty_first)
                .unwrap_err()
                .code(),
            "E005_PROVIDER_AUTHORIZATION_INACTIVE"
        );
    }

    #[test]
    fn e005_successful_first_pass_cannot_patch() {
        let fixture = Fixture::new();
        fixture.issue();
        let author_request = fixture.request(0, E005ProviderCallKind::Author);
        let author = fixture
            .repository
            .reserve_e005_provider_call(&author_request)
            .unwrap();
        fixture.dispatch_and_settle(
            &author,
            E005ProviderCallOutcome::ProviderCompletedPassed,
            Some("a".repeat(64)),
            Some("b".repeat(64)),
        );
        let mut patch = fixture.request(0, E005ProviderCallKind::Patch);
        patch.patch_base_source_sha256 = Some("a".repeat(64));
        patch.failed_gate_sha256 = Some("b".repeat(64));
        assert_eq!(
            fixture
                .repository
                .reserve_e005_provider_call(&patch)
                .unwrap_err()
                .code(),
            "E005_PROVIDER_PATCH_NOT_ELIGIBLE"
        );
    }

    #[test]
    fn e005_pre_dispatch_release_is_free_but_dispatch_timeout_and_restart_are_accounted() {
        let fixture = Fixture::new();
        fixture.issue();
        let released_request = fixture.request(0, E005ProviderCallKind::Author);
        let released = fixture
            .repository
            .reserve_e005_provider_call(&released_request)
            .unwrap();
        let released_evidence = fixture
            .repository
            .settle_e005_provider_call(
                &released.reservation_id,
                &E005ProviderCallSettlement {
                    outcome: E005ProviderCallOutcome::PreDispatchReleased,
                    output_source_sha256: None,
                    output_gate_sha256: None,
                },
            )
            .unwrap();
        assert!(!released_evidence.network_call_made);
        assert_eq!(released_evidence.calls_accounted_after, 0);
        let replayed_release = fixture
            .repository
            .settle_e005_provider_call(
                &released.reservation_id,
                &E005ProviderCallSettlement {
                    outcome: E005ProviderCallOutcome::PreDispatchReleased,
                    output_source_sha256: None,
                    output_gate_sha256: None,
                },
            )
            .unwrap();
        assert_eq!(replayed_release, released_evidence);

        let timeout_request = fixture.request(0, E005ProviderCallKind::Author);
        let timeout = fixture
            .repository
            .reserve_e005_provider_call(&timeout_request)
            .unwrap();
        let timeout_evidence = fixture.dispatch_and_settle(
            &timeout,
            E005ProviderCallOutcome::ProviderTimeout,
            None,
            None,
        );
        assert!(timeout_evidence.network_call_made);
        assert_eq!(timeout_evidence.calls_accounted_after, 1);

        let cancelled_request = fixture.request(1, E005ProviderCallKind::Author);
        let cancelled = fixture
            .repository
            .reserve_e005_provider_call(&cancelled_request)
            .unwrap();
        let cancelled_evidence = fixture.dispatch_and_settle(
            &cancelled,
            E005ProviderCallOutcome::ProviderCancelled,
            None,
            None,
        );
        assert_eq!(cancelled_evidence.calls_accounted_after, 2);

        let crash_request = fixture.request(2, E005ProviderCallKind::Author);
        let crash = fixture
            .repository
            .reserve_e005_provider_call(&crash_request)
            .unwrap();
        fixture
            .repository
            .mark_e005_provider_call_dispatching(&crash.reservation_id)
            .unwrap();
        assert_eq!(
            fixture
                .repository
                .mark_e005_provider_call_dispatching(&crash.reservation_id)
                .unwrap_err()
                .code(),
            "E005_PROVIDER_DISPATCH_ALREADY_ATTEMPTED"
        );
        let abandoned_request = fixture.request(3, E005ProviderCallKind::Author);
        fixture
            .repository
            .reserve_e005_provider_call(&abandoned_request)
            .unwrap();
        let recovered = fixture
            .repository
            .recover_e005_provider_budget_after_restart()
            .unwrap();
        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered[0].outcome_code, "RECOVERED_UNCERTAIN_DISPATCH");
        assert_eq!(recovered[0].calls_accounted_after, 3);
        assert_eq!(recovered[1].outcome_code, "PRE_DISPATCH_RELEASED");
        assert_eq!(recovered[1].calls_accounted_after, 3);
        let late_replay = fixture
            .repository
            .settle_e005_provider_call(
                &released.reservation_id,
                &E005ProviderCallSettlement {
                    outcome: E005ProviderCallOutcome::PreDispatchReleased,
                    output_source_sha256: None,
                    output_gate_sha256: None,
                },
            )
            .unwrap();
        assert_eq!(late_replay, released_evidence);
    }

    #[test]
    fn e005_reservation_rejects_expiry_tampering_deadline_and_usage_overrun() {
        let fixture = Fixture::new();
        fixture.issue();
        let mut wrong_provider = fixture.request(0, E005ProviderCallKind::Author);
        wrong_provider.provider_id = "provider_tampered".into();
        assert_eq!(
            fixture
                .repository
                .reserve_e005_provider_call(&wrong_provider)
                .unwrap_err()
                .code(),
            "E005_PROVIDER_AUTHORIZATION_LINEAGE_MISMATCH"
        );
        let mut wrong_binding = fixture.request(0, E005ProviderCallKind::Author);
        wrong_binding.authorization_binding_sha256 = "0".repeat(64);
        assert_eq!(
            fixture
                .repository
                .reserve_e005_provider_call(&wrong_binding)
                .unwrap_err()
                .code(),
            "E005_PROVIDER_AUTHORIZATION_LINEAGE_MISMATCH"
        );
        let mut tampered = fixture.request(0, E005ProviderCallKind::Author);
        tampered.task_payload_sha256 = "f".repeat(64);
        assert_eq!(
            fixture
                .repository
                .reserve_e005_provider_call(&tampered)
                .unwrap_err()
                .code(),
            "E005_PROVIDER_TASK_LINEAGE_MISMATCH"
        );
        let mut overrun = fixture.request(0, E005ProviderCallKind::Author);
        overrun.reserved_input_tokens = fixture.authorization.maximum_input_tokens + 1;
        assert_eq!(
            fixture
                .repository
                .reserve_e005_provider_call(&overrun)
                .unwrap_err()
                .code(),
            "E005_PROVIDER_USAGE_BUDGET_EXHAUSTED"
        );
        let expired = fixture.request(0, E005ProviderCallKind::Author);
        let ledger = fixture
            .repository
            .e005_provider_budget_ledger(&fixture.authorization.authorization_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            fixture
                .repository
                .reserve_e005_provider_call_at(&expired, ledger.expires_at_unix_ms)
                .unwrap_err()
                .code(),
            "E005_PROVIDER_AUTHORIZATION_INACTIVE"
        );
    }

    #[test]
    fn migration_creates_e005_budget_tables() {
        let fixture = Fixture::new();
        let connection = open_connection(fixture.repository.db_path()).unwrap();
        for table in [
            "e005_provider_run_authorizations",
            "e005_provider_authorized_tasks",
            "e005_provider_call_reservations",
        ] {
            let exists: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "missing {table}");
        }
    }

    #[test]
    fn e005_sql_constraints_and_canonical_scope_readback_reject_tampering() {
        let fixture = Fixture::new();
        fixture.issue();
        let connection = open_connection(fixture.repository.db_path()).unwrap();
        assert!(connection
            .execute(
                "UPDATE e005_provider_run_authorizations SET reserved_input_tokens=maximum_input_tokens, accounted_input_tokens=1 WHERE authorization_id=?",
                [&fixture.authorization.authorization_id],
            )
            .is_err());
        connection
            .execute(
                "UPDATE e005_provider_run_authorizations SET provider_id='provider_tampered' WHERE authorization_id=?",
                [&fixture.authorization.authorization_id],
            )
            .unwrap();
        assert_eq!(
            fixture
                .repository
                .e005_provider_budget_ledger(&fixture.authorization.authorization_id)
                .unwrap_err()
                .code(),
            "E005_PROVIDER_BUDGET_INVALID"
        );
    }

    #[test]
    fn e005_concurrent_dispatch_grants_exactly_one_network_attempt() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let fixture = Fixture::new();
        fixture.issue();
        let request = fixture.request(0, E005ProviderCallKind::Author);
        let reservation = fixture
            .repository
            .reserve_e005_provider_call(&request)
            .unwrap();
        let barrier = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();
        for _ in 0..2 {
            let repository = fixture.repository.clone();
            let reservation_id = reservation.reservation_id.clone();
            let barrier = barrier.clone();
            let dispatch_time = fixture.base_time_unix_ms + 10_000;
            handles.push(thread::spawn(move || {
                barrier.wait();
                repository
                    .mark_e005_provider_call_dispatching_at(&reservation_id, dispatch_time)
                    .map_err(|error| error.code().to_string())
            }));
        }
        barrier.wait();
        let results = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter_map(|result| result.as_ref().err())
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["E005_PROVIDER_DISPATCH_ALREADY_ATTEMPTED"]
        );
    }

    #[test]
    fn e005_persisted_evidence_cannot_be_swapped_between_reservations() {
        let fixture = Fixture::new();
        fixture.issue();
        let first = fixture
            .repository
            .reserve_e005_provider_call(&fixture.request(0, E005ProviderCallKind::Author))
            .unwrap();
        let second = fixture
            .repository
            .reserve_e005_provider_call(&fixture.request(1, E005ProviderCallKind::Author))
            .unwrap();
        let release = E005ProviderCallSettlement {
            outcome: E005ProviderCallOutcome::PreDispatchReleased,
            output_source_sha256: None,
            output_gate_sha256: None,
        };
        fixture
            .repository
            .settle_e005_provider_call_at(
                &first.reservation_id,
                &release,
                fixture.base_time_unix_ms + 10_000,
            )
            .unwrap();
        fixture
            .repository
            .settle_e005_provider_call_at(
                &second.reservation_id,
                &release,
                fixture.base_time_unix_ms + 10_001,
            )
            .unwrap();
        let connection = open_connection(fixture.repository.db_path()).unwrap();
        let second_evidence: (String, String) = connection
            .query_row(
                "SELECT settlement_evidence_json, settlement_evidence_sha256 FROM e005_provider_call_reservations WHERE reservation_id=?",
                [&second.reservation_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        connection
            .execute(
                "UPDATE e005_provider_call_reservations SET settlement_evidence_json=?, settlement_evidence_sha256=? WHERE reservation_id=?",
                params![second_evidence.0, second_evidence.1, &first.reservation_id],
            )
            .unwrap();
        assert_eq!(
            fixture
                .repository
                .settle_e005_provider_call(&first.reservation_id, &release)
                .unwrap_err()
                .code(),
            "E005_PROVIDER_BUDGET_INVALID"
        );
    }

    #[test]
    fn e005_output_hash_tampering_blocks_replay_and_patch_eligibility() {
        let fixture = Fixture::new();
        fixture.issue();
        let author = fixture
            .repository
            .reserve_e005_provider_call(&fixture.request(0, E005ProviderCallKind::Author))
            .unwrap();
        let source = "a".repeat(64);
        let gate = "b".repeat(64);
        fixture.dispatch_and_settle(
            &author,
            E005ProviderCallOutcome::ProviderCompletedRepairable,
            Some(source),
            Some(gate),
        );

        let tampered_source = "c".repeat(64);
        let tampered_gate = "d".repeat(64);
        let connection = open_connection(fixture.repository.db_path()).unwrap();
        connection
            .execute(
                "UPDATE e005_provider_call_reservations SET output_source_sha256=?, output_gate_sha256=? WHERE reservation_id=?",
                params![&tampered_source, &tampered_gate, &author.reservation_id],
            )
            .unwrap();

        let tampered_settlement = E005ProviderCallSettlement {
            outcome: E005ProviderCallOutcome::ProviderCompletedRepairable,
            output_source_sha256: Some(tampered_source.clone()),
            output_gate_sha256: Some(tampered_gate.clone()),
        };
        assert_eq!(
            fixture
                .repository
                .settle_e005_provider_call(&author.reservation_id, &tampered_settlement)
                .unwrap_err()
                .code(),
            "E005_PROVIDER_BUDGET_INVALID"
        );

        let mut patch = fixture.request(0, E005ProviderCallKind::Patch);
        patch.request_sha256 = "e".repeat(64);
        patch.patch_base_source_sha256 = Some(tampered_source);
        patch.failed_gate_sha256 = Some(tampered_gate);
        assert_eq!(
            fixture
                .repository
                .reserve_e005_provider_call(&patch)
                .unwrap_err()
                .code(),
            "E005_PROVIDER_BUDGET_INVALID"
        );
    }

    #[test]
    fn e005_clock_rollback_and_expired_dispatch_fail_closed() {
        let fixture = Fixture::new();
        fixture.issue();
        let request = fixture.request(0, E005ProviderCallKind::Author);
        let reservation = fixture
            .repository
            .reserve_e005_provider_call_at(&request, fixture.base_time_unix_ms)
            .unwrap();
        let ledger = fixture
            .repository
            .e005_provider_budget_ledger(&fixture.authorization.authorization_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            fixture
                .repository
                .mark_e005_provider_call_dispatching_at(
                    &reservation.reservation_id,
                    ledger.expires_at_unix_ms,
                )
                .unwrap_err()
                .code(),
            "E005_PROVIDER_DISPATCH_DEADLINE_EXPIRED"
        );
        fixture
            .repository
            .settle_e005_provider_call_at(
                &reservation.reservation_id,
                &E005ProviderCallSettlement {
                    outcome: E005ProviderCallOutcome::PreDispatchReleased,
                    output_source_sha256: None,
                    output_gate_sha256: None,
                },
                fixture.base_time_unix_ms + 100,
            )
            .unwrap();
        assert_eq!(
            fixture
                .repository
                .reserve_e005_provider_call_at(
                    &fixture.request(1, E005ProviderCallKind::Author),
                    fixture.base_time_unix_ms + 50,
                )
                .unwrap_err()
                .code(),
            "E005_PROVIDER_AUTHORIZATION_INACTIVE"
        );
    }
}
