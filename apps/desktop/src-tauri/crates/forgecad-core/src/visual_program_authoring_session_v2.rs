//! FGC-VP204 one-author / at-most-one-patch state machine.
//!
//! The session is transient evidence. It cannot write Versions, Snapshots or
//! CAS references. Callers measure phase durations and provide only redacted
//! usage; Rust validates and seals the receipt against exact source hashes.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    apply_forge_visual_geometry_patch_v2, lower_visual_runtime_source_v1, semantic_sha256,
    CoreError, CoreResult, GeometryIncrementalPlanV2, FORGE_VISUAL_AUTHOR_SOURCE_SCHEMA_VERSION,
};

pub const VISUAL_PROGRAM_AUTHORING_SESSION_SCHEMA_VERSION: &str = "VisualProgramAuthoringSession@1";
pub const VISUAL_PROGRAM_EXECUTION_RECEIPT_SCHEMA_VERSION: &str = "VisualProgramExecutionReceipt@1";
pub const VISUAL_PROGRAM_GATE_OUTCOME_SCHEMA_VERSION: &str = "VisualProgramGateOutcome@1";

fn invalid(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message.into())
}

fn require_id(value: &str) -> CoreResult<()> {
    if !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'@'))
    {
        Ok(())
    } else {
        Err(invalid(
            "FORGE_VISUAL_VP204_SESSION_ID_INVALID",
            "session identity must be bounded and stable",
        ))
    }
}

fn require_hash(value: &str) -> CoreResult<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(invalid(
            "FORGE_VISUAL_VP204_SESSION_HASH_INVALID",
            "session hash must be a lowercase SHA-256",
        ))
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum VisualProgramPhaseV2 {
    Author,
    Validate,
    Expand,
    Lower,
    CompileReadback,
    Render,
    Evaluate,
    Patch,
    Preview,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualProgramCacheDispositionV2 {
    Hit,
    Miss,
    NotApplicable,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualProgramPhaseReceiptV2 {
    pub sequence: u16,
    pub phase: VisualProgramPhaseV2,
    pub duration_ms: u64,
    pub input_sha256: String,
    pub output_sha256: String,
    pub cache: VisualProgramCacheDispositionV2,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fragment_cache_hit_operation_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fragment_cache_miss_operation_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct VisualProgramUsageV2 {
    pub provider_requests: u8,
    pub product_tool_calls: u16,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub prompt_cache_hit_tokens: u64,
    pub prompt_cache_miss_tokens: u64,
    pub estimated_cost_microusd: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualProgramExecutionReceiptV2 {
    pub schema_version: String,
    pub receipt_id: String,
    pub session_id: String,
    pub authoring_count: u8,
    pub patch_count: u8,
    pub source_program_sha256: String,
    pub expanded_program_sha256: String,
    pub shape_program_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glb_sha256: Option<String>,
    pub phases: Vec<VisualProgramPhaseReceiptV2>,
    pub usage: VisualProgramUsageV2,
    pub cancelled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_code: Option<String>,
}

impl VisualProgramExecutionReceiptV2 {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != VISUAL_PROGRAM_EXECUTION_RECEIPT_SCHEMA_VERSION {
            return Err(invalid(
                "FORGE_VISUAL_VP204_RECEIPT_SCHEMA_INVALID",
                "receipt schema version is unsupported",
            ));
        }
        require_id(&self.receipt_id)?;
        require_id(&self.session_id)?;
        for hash in [
            &self.source_program_sha256,
            &self.expanded_program_sha256,
            &self.shape_program_sha256,
        ] {
            require_hash(hash)?;
        }
        if let Some(hash) = &self.glb_sha256 {
            require_hash(hash)?;
        }
        if self.authoring_count != 1 || self.patch_count > 1 || self.usage.provider_requests > 2 {
            return Err(invalid(
                "FORGE_VISUAL_VP204_ROUNDTRIP_LIMIT",
                "receipt must record exactly one author and at most one patch/provider repair",
            ));
        }
        if self.phases.is_empty() || self.phases.len() > 32 {
            return Err(invalid(
                "FORGE_VISUAL_VP204_RECEIPT_PHASES_INVALID",
                "receipt phases must be non-empty and bounded",
            ));
        }
        let mut previous = 0_u16;
        let mut previous_output: Option<&str> = None;
        let mut sequences = BTreeSet::new();
        let mut observed_phases = BTreeSet::new();
        for phase in &self.phases {
            if phase.sequence == 0
                || phase.sequence != previous + 1
                || !sequences.insert(phase.sequence)
            {
                return Err(invalid(
                    "FORGE_VISUAL_VP204_RECEIPT_SEQUENCE_INVALID",
                    "receipt phase sequence must be unique, contiguous and strictly increasing",
                ));
            }
            require_hash(&phase.input_sha256)?;
            require_hash(&phase.output_sha256)?;
            let mut fragment_ids = BTreeSet::new();
            for operation_id in phase
                .fragment_cache_hit_operation_ids
                .iter()
                .chain(&phase.fragment_cache_miss_operation_ids)
            {
                require_id(operation_id)?;
                if !fragment_ids.insert(operation_id) {
                    return Err(invalid(
                        "FORGE_VISUAL_VP204_RECEIPT_FRAGMENT_CACHE_INVALID",
                        "fragment cache operation ids must be unique and disjoint",
                    ));
                }
            }
            if phase.phase != VisualProgramPhaseV2::CompileReadback && !fragment_ids.is_empty() {
                return Err(invalid(
                    "FORGE_VISUAL_VP204_RECEIPT_FRAGMENT_CACHE_INVALID",
                    "only compile_readback may record operation-fragment cache evidence",
                ));
            }
            if previous_output.is_some_and(|output| output != phase.input_sha256) {
                return Err(invalid(
                    "FORGE_VISUAL_VP204_RECEIPT_CHAIN_INVALID",
                    "each receipt phase must consume the previous phase output hash",
                ));
            }
            previous_output = Some(&phase.output_sha256);
            observed_phases.insert(phase.phase);
            previous = phase.sequence;
        }
        if self.cancelled && self.failure_code.is_some() {
            return Err(invalid(
                "FORGE_VISUAL_VP204_RECEIPT_TERMINAL_INVALID",
                "cancel and failure are mutually exclusive",
            ));
        }
        if let Some(code) = &self.failure_code {
            require_id(code)?;
        }
        if self.usage.provider_requests == 0
            && (self.usage.input_tokens != 0
                || self.usage.output_tokens != 0
                || self.usage.prompt_cache_hit_tokens != 0
                || self.usage.prompt_cache_miss_tokens != 0
                || self.usage.estimated_cost_microusd != 0)
        {
            return Err(invalid(
                "FORGE_VISUAL_VP204_RECEIPT_USAGE_INVALID",
                "an offline receipt cannot report provider token or cost usage",
            ));
        }
        if self.usage.prompt_cache_hit_tokens + self.usage.prompt_cache_miss_tokens
            > self.usage.input_tokens
        {
            return Err(invalid(
                "FORGE_VISUAL_VP204_RECEIPT_USAGE_INVALID",
                "prompt cache token counts cannot exceed input tokens",
            ));
        }
        let terminal_error = self.cancelled || self.failure_code.is_some();
        if terminal_error {
            if self.glb_sha256.is_some() {
                return Err(invalid(
                    "FORGE_VISUAL_VP204_RECEIPT_TERMINAL_INVALID",
                    "cancelled or failed receipts cannot promote a GLB",
                ));
            }
        } else {
            if self.glb_sha256.is_none() {
                return Err(invalid(
                    "FORGE_VISUAL_VP204_RECEIPT_GLB_MISSING",
                    "a successful receipt must bind the compiled GLB",
                ));
            }
            let required = [
                VisualProgramPhaseV2::Author,
                VisualProgramPhaseV2::Validate,
                VisualProgramPhaseV2::Expand,
                VisualProgramPhaseV2::Lower,
                VisualProgramPhaseV2::CompileReadback,
                VisualProgramPhaseV2::Render,
                VisualProgramPhaseV2::Evaluate,
            ];
            if required
                .iter()
                .any(|phase| !observed_phases.contains(phase))
                || (self.patch_count == 1
                    && !observed_phases.contains(&VisualProgramPhaseV2::Patch))
            {
                return Err(invalid(
                    "FORGE_VISUAL_VP204_RECEIPT_PHASES_INCOMPLETE",
                    "a successful receipt must cover author through preview and its optional patch",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualProgramGateVerdictV2 {
    Pass,
    Fail,
    Undetermined,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualProgramGateOutcomeV2 {
    pub schema_version: String,
    pub gate_report_id: String,
    pub source_program_sha256: String,
    pub verdict: VisualProgramGateVerdictV2,
    pub repairable: bool,
}

impl VisualProgramGateOutcomeV2 {
    fn validate(&self) -> CoreResult<()> {
        if self.schema_version != VISUAL_PROGRAM_GATE_OUTCOME_SCHEMA_VERSION {
            return Err(invalid(
                "FORGE_VISUAL_VP204_GATE_SCHEMA_INVALID",
                "gate outcome schema version is unsupported",
            ));
        }
        require_id(&self.gate_report_id)?;
        require_hash(&self.source_program_sha256)?;
        if self.repairable && self.verdict != VisualProgramGateVerdictV2::Fail {
            return Err(invalid(
                "FORGE_VISUAL_VP204_GATE_REPAIR_INVALID",
                "only a concrete failure may authorize the one patch",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualProgramAuthoringStateV2 {
    AwaitingInitialGate,
    AwaitingPatch,
    AwaitingPatchedGate,
    ReadyForPreview,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VisualProgramAuthoringSessionV2 {
    pub schema_version: String,
    pub session_id: String,
    pub idempotency_key: String,
    pub request_sha256: String,
    pub current_revision: u32,
    pub initial_source_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_source_sha256: Option<String>,
    pub current_source_sha256: String,
    pub current_source: Value,
    pub authoring_count: u8,
    pub patch_count: u8,
    pub state: VisualProgramAuthoringStateV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_patch_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incremental_plan: Option<GeometryIncrementalPlanV2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_report_id: Option<String>,
    pub receipt: VisualProgramExecutionReceiptV2,
    pub receipt_sha256: String,
}

impl VisualProgramAuthoringSessionV2 {
    pub fn begin(
        session_id: String,
        idempotency_key: String,
        request_sha256: String,
        source: Value,
        receipt: VisualProgramExecutionReceiptV2,
    ) -> CoreResult<Self> {
        require_id(&session_id)?;
        require_id(&idempotency_key)?;
        require_hash(&request_sha256)?;
        receipt.validate()?;
        if receipt.session_id != session_id
            || receipt.authoring_count != 1
            || receipt.patch_count != 0
        {
            return Err(invalid(
                "FORGE_VISUAL_VP204_RECEIPT_SESSION_MISMATCH",
                "initial receipt must bind the same one-author session",
            ));
        }
        let lowering = lower_visual_runtime_source_v1(&source)?;
        if receipt.source_program_sha256 != lowering.source_program_sha256
            || receipt.expanded_program_sha256 != lowering.expanded_program_sha256
            || receipt.shape_program_sha256 != lowering.shape_program_sha256
        {
            return Err(invalid(
                "FORGE_VISUAL_VP204_RECEIPT_HASH_MISMATCH",
                "initial receipt hashes do not bind the lowered source",
            ));
        }
        let receipt_sha256 = semantic_sha256(&receipt)?;
        let session = Self {
            schema_version: VISUAL_PROGRAM_AUTHORING_SESSION_SCHEMA_VERSION.into(),
            session_id,
            idempotency_key,
            request_sha256,
            current_revision: 1,
            initial_source_sha256: lowering.source_program_sha256.clone(),
            parent_source_sha256: None,
            current_source_sha256: lowering.source_program_sha256,
            current_source: source,
            authoring_count: 1,
            patch_count: 0,
            state: VisualProgramAuthoringStateV2::AwaitingInitialGate,
            applied_patch_sha256: None,
            incremental_plan: None,
            gate_report_id: None,
            receipt,
            receipt_sha256,
        };
        session.validate_recovery()?;
        Ok(session)
    }

    pub fn record_gate(&mut self, outcome: VisualProgramGateOutcomeV2) -> CoreResult<()> {
        outcome.validate()?;
        if outcome.source_program_sha256 != self.current_source_sha256 {
            return Err(CoreError::conflict(
                "FORGE_VISUAL_VP204_GATE_STALE",
                "gate outcome does not bind the active source",
            ));
        }
        match self.state {
            VisualProgramAuthoringStateV2::AwaitingInitialGate
            | VisualProgramAuthoringStateV2::AwaitingPatchedGate => {}
            _ => {
                return Err(CoreError::conflict(
                    "FORGE_VISUAL_VP204_GATE_STATE_INVALID",
                    "gate outcome is not allowed in the current session state",
                ))
            }
        }
        let preview_recorded = self
            .receipt
            .phases
            .iter()
            .any(|phase| phase.phase == VisualProgramPhaseV2::Preview);
        if (outcome.verdict == VisualProgramGateVerdictV2::Pass) != preview_recorded {
            return Err(invalid(
                "FORGE_VISUAL_VP204_PREVIEW_GATE_MISMATCH",
                "only a passed hard gate may carry the single preview phase",
            ));
        }
        self.gate_report_id = Some(outcome.gate_report_id);
        self.state = match outcome.verdict {
            VisualProgramGateVerdictV2::Pass => VisualProgramAuthoringStateV2::ReadyForPreview,
            VisualProgramGateVerdictV2::Fail if outcome.repairable && self.patch_count == 0 => {
                VisualProgramAuthoringStateV2::AwaitingPatch
            }
            VisualProgramGateVerdictV2::Fail | VisualProgramGateVerdictV2::Undetermined => {
                VisualProgramAuthoringStateV2::Failed
            }
        };
        Ok(())
    }

    pub fn apply_patch(
        &mut self,
        patch: &Value,
        receipt: VisualProgramExecutionReceiptV2,
    ) -> CoreResult<&GeometryIncrementalPlanV2> {
        let patch_sha256 = semantic_sha256(patch)?;
        if self.patch_count == 1
            && self.applied_patch_sha256.as_deref() == Some(patch_sha256.as_str())
        {
            return self.incremental_plan.as_ref().ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP204_RECOVERY_INVALID",
                    "idempotent patch replay lost its incremental plan",
                )
            });
        }
        if self.state != VisualProgramAuthoringStateV2::AwaitingPatch || self.patch_count != 0 {
            return Err(CoreError::conflict(
                "FORGE_VISUAL_VP204_PATCH_LIMIT_REACHED",
                "session permits at most one typed patch after a repairable gate failure",
            ));
        }
        if self
            .current_source
            .get("schema_version")
            .and_then(Value::as_str)
            == Some(FORGE_VISUAL_AUTHOR_SOURCE_SCHEMA_VERSION)
        {
            return Err(invalid(
                "FORGE_VISUAL_R1_PATCH_NOT_IMPLEMENTED",
                "the unified author source remains immutable until E005-R2 adds a hash-bound visual patch",
            ));
        }
        let patched = apply_forge_visual_geometry_patch_v2(&self.current_source, patch)?;
        receipt.validate()?;
        if receipt.session_id != self.session_id
            || receipt.authoring_count != 1
            || receipt.patch_count != 1
            || receipt.source_program_sha256 != patched.lowering.source_program_sha256
            || receipt.expanded_program_sha256
                != patched.lowering.expanded_dag.expanded_program_sha256
            || receipt.shape_program_sha256 != patched.lowering.shape_program_sha256
        {
            return Err(invalid(
                "FORGE_VISUAL_VP204_RECEIPT_HASH_MISMATCH",
                "patch receipt does not bind the one-patch result",
            ));
        }
        self.parent_source_sha256 = Some(self.current_source_sha256.clone());
        self.current_source = patched.patched_program;
        self.current_source_sha256 = patched.lowering.source_program_sha256;
        self.current_revision = 2;
        self.patch_count = 1;
        self.applied_patch_sha256 = Some(patch_sha256);
        self.incremental_plan = Some(patched.incremental_plan);
        self.gate_report_id = None;
        self.receipt_sha256 = semantic_sha256(&receipt)?;
        self.receipt = receipt;
        self.state = VisualProgramAuthoringStateV2::AwaitingPatchedGate;
        self.incremental_plan.as_ref().ok_or_else(|| {
            invalid(
                "FORGE_VISUAL_VP204_RECOVERY_INVALID",
                "patch plan is missing",
            )
        })
    }

    pub fn cancel(&mut self, receipt: VisualProgramExecutionReceiptV2) -> CoreResult<()> {
        if matches!(
            self.state,
            VisualProgramAuthoringStateV2::ReadyForPreview
                | VisualProgramAuthoringStateV2::Failed
                | VisualProgramAuthoringStateV2::Cancelled
        ) {
            return Err(CoreError::conflict(
                "FORGE_VISUAL_VP204_SESSION_TERMINAL",
                "terminal session cannot be cancelled again",
            ));
        }
        receipt.validate()?;
        if receipt.session_id != self.session_id
            || !receipt.cancelled
            || receipt.source_program_sha256 != self.current_source_sha256
        {
            return Err(invalid(
                "FORGE_VISUAL_VP204_RECEIPT_SESSION_MISMATCH",
                "cancel receipt does not bind the active session",
            ));
        }
        self.receipt_sha256 = semantic_sha256(&receipt)?;
        self.receipt = receipt;
        self.state = VisualProgramAuthoringStateV2::Cancelled;
        Ok(())
    }

    pub fn fail(&mut self, receipt: VisualProgramExecutionReceiptV2) -> CoreResult<()> {
        if matches!(
            self.state,
            VisualProgramAuthoringStateV2::ReadyForPreview
                | VisualProgramAuthoringStateV2::Failed
                | VisualProgramAuthoringStateV2::Cancelled
        ) {
            return Err(CoreError::conflict(
                "FORGE_VISUAL_VP204_SESSION_TERMINAL",
                "terminal session cannot fail again",
            ));
        }
        receipt.validate()?;
        if receipt.session_id != self.session_id
            || receipt.cancelled
            || receipt.failure_code.is_none()
            || receipt.source_program_sha256 != self.current_source_sha256
        {
            return Err(invalid(
                "FORGE_VISUAL_VP204_RECEIPT_SESSION_MISMATCH",
                "failure receipt does not bind the active session",
            ));
        }
        self.receipt_sha256 = semantic_sha256(&receipt)?;
        self.receipt = receipt;
        self.state = VisualProgramAuthoringStateV2::Failed;
        Ok(())
    }

    pub fn restore(value: &Value) -> CoreResult<Self> {
        let session: Self = serde_json::from_value(value.clone()).map_err(|error| {
            invalid(
                "FORGE_VISUAL_VP204_SESSION_PARSE_FAILED",
                format!("session restore failed closed: {error}"),
            )
        })?;
        session.validate_recovery()?;
        Ok(session)
    }

    pub fn validate_recovery(&self) -> CoreResult<()> {
        if self.schema_version != VISUAL_PROGRAM_AUTHORING_SESSION_SCHEMA_VERSION {
            return Err(invalid(
                "FORGE_VISUAL_VP204_SESSION_SCHEMA_INVALID",
                "session schema version is unsupported",
            ));
        }
        require_id(&self.session_id)?;
        require_id(&self.idempotency_key)?;
        require_hash(&self.request_sha256)?;
        require_hash(&self.initial_source_sha256)?;
        if let Some(hash) = &self.parent_source_sha256 {
            require_hash(hash)?;
        }
        require_hash(&self.current_source_sha256)?;
        require_hash(&self.receipt_sha256)?;
        self.receipt.validate()?;
        if self.authoring_count != 1
            || self.patch_count > 1
            || self.receipt.session_id != self.session_id
            || self.receipt.authoring_count != 1
            || self.receipt.patch_count != self.patch_count
            || semantic_sha256(&self.receipt)? != self.receipt_sha256
        {
            return Err(invalid(
                "FORGE_VISUAL_VP204_RECOVERY_INVALID",
                "session counters or receipt seal are inconsistent",
            ));
        }
        let lowering = lower_visual_runtime_source_v1(&self.current_source)?;
        if lowering.source_program_sha256 != self.current_source_sha256
            || self.receipt.source_program_sha256 != self.current_source_sha256
            || self.receipt.expanded_program_sha256 != lowering.expanded_program_sha256
            || self.receipt.shape_program_sha256 != lowering.shape_program_sha256
        {
            return Err(invalid(
                "FORGE_VISUAL_VP204_RECOVERY_HASH_MISMATCH",
                "restored session source/lowering/receipt hashes disagree",
            ));
        }
        match (
            self.patch_count,
            self.current_revision,
            &self.parent_source_sha256,
            &self.applied_patch_sha256,
            &self.incremental_plan,
            self.state,
        ) {
            (
                0,
                1,
                None,
                None,
                None,
                VisualProgramAuthoringStateV2::AwaitingInitialGate
                | VisualProgramAuthoringStateV2::AwaitingPatch
                | VisualProgramAuthoringStateV2::ReadyForPreview
                | VisualProgramAuthoringStateV2::Failed
                | VisualProgramAuthoringStateV2::Cancelled,
            ) => {}
            (
                1,
                2,
                Some(parent_hash),
                Some(hash),
                Some(plan),
                VisualProgramAuthoringStateV2::AwaitingPatchedGate
                | VisualProgramAuthoringStateV2::ReadyForPreview
                | VisualProgramAuthoringStateV2::Failed
                | VisualProgramAuthoringStateV2::Cancelled,
            ) => {
                require_hash(hash)?;
                if parent_hash != &self.initial_source_sha256
                    || plan.base_source_sha256 != *parent_hash
                    || plan.patched_source_sha256 != self.current_source_sha256
                    || plan.patch_sha256 != *hash
                {
                    return Err(invalid(
                        "FORGE_VISUAL_VP204_RECOVERY_HASH_MISMATCH",
                        "patch plan lineage is inconsistent",
                    ));
                }
            }
            _ => {
                return Err(invalid(
                    "FORGE_VISUAL_VP204_RECOVERY_STATE_INVALID",
                    "restored session state does not match its patch lineage",
                ))
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower_forge_visual_geometry_program_v2;
    use serde_json::json;

    fn rotor() -> Value {
        serde_json::from_str(include_str!(
            "../../../../../../packages/concept-spec/fixtures/forge-visual-geometry-v2-rotor.json"
        ))
        .unwrap()
    }

    fn receipt(
        session_id: &str,
        source: &Value,
        patch_count: u8,
        include_preview: bool,
    ) -> VisualProgramExecutionReceiptV2 {
        let lowering = lower_forge_visual_geometry_program_v2(source).unwrap();
        let mut phase_kinds = vec![
            VisualProgramPhaseV2::Author,
            VisualProgramPhaseV2::Validate,
            VisualProgramPhaseV2::Expand,
            VisualProgramPhaseV2::Lower,
            VisualProgramPhaseV2::CompileReadback,
            VisualProgramPhaseV2::Render,
            VisualProgramPhaseV2::Evaluate,
        ];
        if patch_count == 1 {
            phase_kinds.push(VisualProgramPhaseV2::Patch);
        }
        if include_preview {
            phase_kinds.push(VisualProgramPhaseV2::Preview);
        }
        let phases = phase_kinds
            .into_iter()
            .enumerate()
            .map(|(index, phase)| VisualProgramPhaseReceiptV2 {
                sequence: (index + 1) as u16,
                phase,
                duration_ms: 1,
                input_sha256: format!("{:064x}", index + 1),
                output_sha256: format!("{:064x}", index + 2),
                cache: if phase == VisualProgramPhaseV2::CompileReadback {
                    VisualProgramCacheDispositionV2::Miss
                } else {
                    VisualProgramCacheDispositionV2::NotApplicable
                },
                fragment_cache_hit_operation_ids: Vec::new(),
                fragment_cache_miss_operation_ids: Vec::new(),
            })
            .collect();
        VisualProgramExecutionReceiptV2 {
            schema_version: VISUAL_PROGRAM_EXECUTION_RECEIPT_SCHEMA_VERSION.into(),
            receipt_id: format!("receipt_{session_id}_{patch_count}"),
            session_id: session_id.into(),
            authoring_count: 1,
            patch_count,
            source_program_sha256: lowering.source_program_sha256,
            expanded_program_sha256: lowering.expanded_dag.expanded_program_sha256,
            shape_program_sha256: lowering.shape_program_sha256,
            glb_sha256: Some("f".repeat(64)),
            phases,
            usage: VisualProgramUsageV2::default(),
            cancelled: false,
            failure_code: None,
        }
    }

    #[test]
    fn vp204_session_allows_one_author_one_patch_and_idempotent_replay() {
        let source = rotor();
        let mut session = VisualProgramAuthoringSessionV2::begin(
            "vpsession_rotor".into(),
            "idem_rotor".into(),
            "3".repeat(64),
            source.clone(),
            receipt("vpsession_rotor", &source, 0, false),
        )
        .unwrap();
        session
            .record_gate(VisualProgramGateOutcomeV2 {
                schema_version: VISUAL_PROGRAM_GATE_OUTCOME_SCHEMA_VERSION.into(),
                gate_report_id: "gate_initial".into(),
                source_program_sha256: session.current_source_sha256.clone(),
                verdict: VisualProgramGateVerdictV2::Fail,
                repairable: true,
            })
            .unwrap();
        let patch = json!({"schema_version":"ForgeVisualGeometryPatch@1","patch_id":"patch_rotor","expected_source_sha256":session.current_source_sha256,"operations":[{"op":"set_array","node_id":"node_rotor_bank","count":4,"spacing":760.0}]});
        let patched = apply_forge_visual_geometry_patch_v2(&source, &patch).unwrap();
        let patched_receipt = receipt("vpsession_rotor", &patched.patched_program, 1, true);
        session.apply_patch(&patch, patched_receipt).unwrap();
        assert_eq!(session.current_revision, 2);
        assert_eq!(
            session.parent_source_sha256.as_deref(),
            Some(session.initial_source_sha256.as_str())
        );
        let count = session.patch_count;
        session
            .apply_patch(&patch, session.receipt.clone())
            .unwrap();
        assert_eq!(session.patch_count, count);
        let conflicting = json!({"schema_version":"ForgeVisualGeometryPatch@1","patch_id":"patch_second","expected_source_sha256":session.current_source_sha256,"operations":[{"op":"set_array","node_id":"node_rotor_bank","count":5,"spacing":800.0}]});
        assert_eq!(
            session
                .apply_patch(&conflicting, session.receipt.clone())
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP204_PATCH_LIMIT_REACHED"
        );
        session
            .record_gate(VisualProgramGateOutcomeV2 {
                schema_version: VISUAL_PROGRAM_GATE_OUTCOME_SCHEMA_VERSION.into(),
                gate_report_id: "gate_patched".into(),
                source_program_sha256: session.current_source_sha256.clone(),
                verdict: VisualProgramGateVerdictV2::Pass,
                repairable: false,
            })
            .unwrap();
        assert_eq!(
            session.state,
            VisualProgramAuthoringStateV2::ReadyForPreview
        );
    }

    #[test]
    fn vp204_session_restore_rejects_counter_hash_and_state_tampering() {
        let source = rotor();
        let session = VisualProgramAuthoringSessionV2::begin(
            "vpsession_restore".into(),
            "idem_restore".into(),
            "4".repeat(64),
            source.clone(),
            receipt("vpsession_restore", &source, 0, false),
        )
        .unwrap();
        let serialized = serde_json::to_value(&session).unwrap();
        assert_eq!(
            VisualProgramAuthoringSessionV2::restore(&serialized).unwrap(),
            session
        );
        let mut counter = serialized.clone();
        counter["authoring_count"] = json!(2);
        assert_eq!(
            VisualProgramAuthoringSessionV2::restore(&counter)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP204_RECOVERY_INVALID"
        );
        let mut hash = serialized.clone();
        hash["current_source_sha256"] = json!("0".repeat(64));
        assert_eq!(
            VisualProgramAuthoringSessionV2::restore(&hash)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP204_RECOVERY_HASH_MISMATCH"
        );
    }

    #[test]
    fn vp204_session_records_cancel_and_failure_without_promoting_artifacts() {
        let source = rotor();
        for (session_id, cancelled, failure_code, expected_state) in [
            (
                "vpsession_cancel",
                true,
                None,
                VisualProgramAuthoringStateV2::Cancelled,
            ),
            (
                "vpsession_timeout",
                false,
                Some("GEOMETRY_TIMEOUT".to_string()),
                VisualProgramAuthoringStateV2::Failed,
            ),
        ] {
            let mut session = VisualProgramAuthoringSessionV2::begin(
                session_id.into(),
                format!("idem_{session_id}"),
                "5".repeat(64),
                source.clone(),
                receipt(session_id, &source, 0, false),
            )
            .unwrap();
            let mut terminal = receipt(session_id, &source, 0, false);
            terminal.receipt_id = format!("receipt_{session_id}_terminal");
            terminal.glb_sha256 = None;
            terminal.phases.truncate(4);
            terminal.cancelled = cancelled;
            terminal.failure_code = failure_code;
            if cancelled {
                session.cancel(terminal).unwrap();
            } else {
                session.fail(terminal).unwrap();
            }
            assert_eq!(session.state, expected_state);
            let restored =
                VisualProgramAuthoringSessionV2::restore(&serde_json::to_value(&session).unwrap())
                    .unwrap();
            assert_eq!(restored.state, expected_state);
            assert!(restored.receipt.glb_sha256.is_none());
        }
    }
}
