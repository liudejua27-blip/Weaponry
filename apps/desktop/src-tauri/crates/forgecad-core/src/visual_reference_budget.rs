use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{semantic_sha256, CoreError, CoreResult, VisualReferenceComparisonInput};

pub const VISUAL_REFERENCE_COMPARISON_AUTHORIZATION_SCHEMA_VERSION: &str =
    "VisualReferenceComparisonAuthorization@1";
pub const VISUAL_REFERENCE_COMPARISON_RESERVATION_SCHEMA_VERSION: &str =
    "VisualReferenceComparisonReservation@1";
pub const VISUAL_REFERENCE_COMPARISON_BUDGET_EVIDENCE_SCHEMA_VERSION: &str =
    "VisualReferenceComparisonBudgetEvidence@1";
pub const VISUAL_REFERENCE_COMPARISON_MAXIMUM_CALLS: u8 = 3;
pub const VISUAL_REFERENCE_COMPARISON_MAXIMUM_VARIABLE_COST_MICROUSD: u64 = 100_000;
pub const VISUAL_REFERENCE_COMPARISON_AUTHORIZATION_LIFETIME_MS: i64 = 15 * 60 * 1_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualReferenceComparisonAuthorization {
    pub schema_version: String,
    pub authorization_id: String,
    pub client_request_id: String,
    pub project_id: String,
    pub request_sha256: String,
    pub evidence_graph_sha256: String,
    pub acceptance_policy_sha256: String,
    pub authorization_binding_sha256: String,
    pub bound_turn_id: Option<String>,
    pub status: String,
    pub maximum_calls: u8,
    pub maximum_variable_cost_microusd: u64,
    pub reservations_created: u32,
    pub calls_accounted: u8,
    pub accounted_cost_ceiling_microusd: u64,
    pub reserved_cost_ceiling_microusd: u64,
    pub authorized_at_unix_ms: i64,
    pub expires_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualReferenceComparisonReservation {
    pub schema_version: String,
    pub reservation_id: String,
    pub authorization_id: String,
    pub authorization_binding_sha256: String,
    pub turn_id: String,
    pub comparison_input_sha256: String,
    pub call_number: u8,
    pub reservation_ordinal: u32,
    pub reserved_cost_ceiling_microusd: u64,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualReferenceComparisonBudgetEvidence {
    pub schema_version: String,
    pub authorization_id: String,
    pub authorization_binding_sha256: String,
    pub reservation_id: String,
    pub turn_id: String,
    pub comparison_input_sha256: String,
    pub call_number: u8,
    pub maximum_calls: u8,
    pub maximum_variable_cost_microusd: u64,
    pub reserved_cost_ceiling_microusd: u64,
    pub settlement: String,
    pub network_call_made: bool,
    pub calls_accounted_after: u8,
    pub accounted_cost_ceiling_microusd_after: u64,
    pub settled_at_unix_ms: i64,
}

impl VisualReferenceComparisonAuthorization {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != VISUAL_REFERENCE_COMPARISON_AUTHORIZATION_SCHEMA_VERSION
            || !valid_prefixed_id(&self.authorization_id, "visauth_")
            || self.client_request_id.is_empty()
            || self.client_request_id.len() > 128
            || !valid_prefixed_id(&self.project_id, "project_")
            || !valid_sha256(&self.request_sha256)
            || !valid_sha256(&self.evidence_graph_sha256)
            || !valid_sha256(&self.acceptance_policy_sha256)
            || !valid_sha256(&self.authorization_binding_sha256)
            || self
                .bound_turn_id
                .as_deref()
                .is_some_and(|value| !valid_prefixed_id(value, "turn_"))
            || !matches!(
                self.status.as_str(),
                "authorized" | "consumed" | "cancelled" | "expired"
            )
            || self.maximum_calls != VISUAL_REFERENCE_COMPARISON_MAXIMUM_CALLS
            || self.maximum_variable_cost_microusd
                != VISUAL_REFERENCE_COMPARISON_MAXIMUM_VARIABLE_COST_MICROUSD
            || self.calls_accounted > self.maximum_calls
            || self.accounted_cost_ceiling_microusd + self.reserved_cost_ceiling_microusd
                > self.maximum_variable_cost_microusd
            || self.expires_at_unix_ms <= self.authorized_at_unix_ms
            || self.updated_at_unix_ms < self.authorized_at_unix_ms
        {
            return Err(invalid("Visual comparison authorization is malformed."));
        }
        if self.authorization_binding_sha256
            != visual_reference_authorization_binding_sha256(
                &self.project_id,
                &self.request_sha256,
                &self.evidence_graph_sha256,
                &self.acceptance_policy_sha256,
                self.maximum_calls,
                self.maximum_variable_cost_microusd,
            )?
        {
            return Err(invalid(
                "Visual comparison authorization binding hash does not match its exact scope.",
            ));
        }
        Ok(())
    }
}

impl VisualReferenceComparisonReservation {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != VISUAL_REFERENCE_COMPARISON_RESERVATION_SCHEMA_VERSION
            || !valid_prefixed_id(&self.reservation_id, "visreserve_")
            || !valid_prefixed_id(&self.authorization_id, "visauth_")
            || !valid_sha256(&self.authorization_binding_sha256)
            || !valid_prefixed_id(&self.turn_id, "turn_")
            || !valid_sha256(&self.comparison_input_sha256)
            || self.call_number == 0
            || self.call_number > VISUAL_REFERENCE_COMPARISON_MAXIMUM_CALLS
            || self.reservation_ordinal == 0
            || self.reserved_cost_ceiling_microusd == 0
            || self.reserved_cost_ceiling_microusd
                > VISUAL_REFERENCE_COMPARISON_MAXIMUM_VARIABLE_COST_MICROUSD
        {
            return Err(invalid("Visual comparison reservation is malformed."));
        }
        Ok(())
    }
}

impl VisualReferenceComparisonBudgetEvidence {
    pub fn validate_against(&self, input: &VisualReferenceComparisonInput) -> CoreResult<()> {
        if self.schema_version != VISUAL_REFERENCE_COMPARISON_BUDGET_EVIDENCE_SCHEMA_VERSION
            || !valid_prefixed_id(&self.authorization_id, "visauth_")
            || !valid_prefixed_id(&self.reservation_id, "visreserve_")
            || !valid_prefixed_id(&self.turn_id, "turn_")
            || !valid_sha256(&self.authorization_binding_sha256)
            || self.comparison_input_sha256 != semantic_sha256(input)?
            || self.call_number == 0
            || self.call_number > self.maximum_calls
            || self.maximum_calls != VISUAL_REFERENCE_COMPARISON_MAXIMUM_CALLS
            || self.maximum_variable_cost_microusd
                != VISUAL_REFERENCE_COMPARISON_MAXIMUM_VARIABLE_COST_MICROUSD
            || self.reserved_cost_ceiling_microusd == 0
            || self.reserved_cost_ceiling_microusd > self.maximum_variable_cost_microusd
            || !matches!(self.settlement.as_str(), "accounted" | "released")
            || (self.settlement == "accounted") != self.network_call_made
            || self.calls_accounted_after > self.maximum_calls
            || self.accounted_cost_ceiling_microusd_after > self.maximum_variable_cost_microusd
        {
            return Err(invalid("Visual comparison budget evidence is malformed."));
        }
        Ok(())
    }
}

pub fn visual_reference_authorization_binding_sha256(
    project_id: &str,
    request_sha256: &str,
    evidence_graph_sha256: &str,
    acceptance_policy_sha256: &str,
    maximum_calls: u8,
    maximum_variable_cost_microusd: u64,
) -> CoreResult<String> {
    semantic_sha256(&json!({
        "schema_version": "VisualReferenceComparisonAuthorizationBinding@1",
        "project_id": project_id,
        "request_sha256": request_sha256,
        "evidence_graph_sha256": evidence_graph_sha256,
        "acceptance_policy_sha256": acceptance_policy_sha256,
        "maximum_calls": maximum_calls,
        "maximum_variable_cost_microusd": maximum_variable_cost_microusd,
    }))
}

pub(crate) fn visual_reference_reservation_id(
    authorization_id: &str,
    turn_id: &str,
    comparison_input_sha256: &str,
    reservation_ordinal: u32,
) -> CoreResult<String> {
    let digest = semantic_sha256(&json!({
        "schema_version": "VisualReferenceComparisonReservationIdentity@1",
        "authorization_id": authorization_id,
        "turn_id": turn_id,
        "comparison_input_sha256": comparison_input_sha256,
        "reservation_ordinal": reservation_ordinal,
    }))?;
    Ok(format!("visreserve_{}", &digest[..24]))
}

fn valid_prefixed_id(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix)
        && value.len() > prefix.len()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid(message: impl Into<String>) -> CoreError {
    CoreError::invalid_data("VISUAL_REFERENCE_COMPARISON_BUDGET_INVALID", message)
}
