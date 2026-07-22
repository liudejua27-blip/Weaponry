//! V003 single-synthesis decision contract.
//!
//! This is deliberately a pure Core state machine: it can only describe a
//! transient preview that has already passed every hard gate.  It has no
//! `CoreRepository`, SQLite, CAS, or `ActiveDesignSnapshot` handle, therefore
//! a failed, cancelled, or undetermined attempt has no route to create a
//! Version or mutate a Snapshot.  Promotion remains the existing explicit
//! preview -> confirm transaction.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{CoreError, CoreResult};

pub const SINGLE_GENERATION_ATTEMPT_SCHEMA_VERSION: &str = "SingleGenerationAttempt@1";
pub const GENERATION_GATE_REPORT_SCHEMA_VERSION: &str = "GenerationGateReport@1";
pub const REPAIR_ATTEMPT_SCHEMA_VERSION: &str = "RepairAttempt@1";
pub const SINGLE_RESULT_DECISION_SCHEMA_VERSION: &str = "SingleResultDecision@1";
pub const MAX_SAME_INTENT_REPAIR_ATTEMPTS: usize = 2;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationOutcome {
    Pass,
    Fail,
    Undetermined,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GenerationAttemptKind {
    Initial,
    Repair,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SingleGenerationAttempt {
    pub schema_version: String,
    pub attempt_id: String,
    pub turn_id: String,
    pub project_id: String,
    pub attempt_kind: GenerationAttemptKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_attempt_id: Option<String>,
    /// Hashes pin the same Brief, Domain Pack, core Recipe/profile intent and
    /// runtime manifest across bounded repairs without storing prompts or
    /// provider reasoning.
    pub brief_sha256: String,
    pub domain_pack_id: String,
    pub domain_pack_sha256: String,
    pub core_recipe_or_profile_sha256: String,
    pub runtime_manifest_sha256: String,
    pub shape_program_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_provenance_sha256: Option<String>,
}

impl SingleGenerationAttempt {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != SINGLE_GENERATION_ATTEMPT_SCHEMA_VERSION {
            return Err(invalid(
                "SINGLE_GENERATION_ATTEMPT_SCHEMA_INVALID",
                "Single generation attempt must use its exact v1 schema.",
            ));
        }
        for (field, value) in [
            ("attempt_id", self.attempt_id.as_str()),
            ("turn_id", self.turn_id.as_str()),
            ("project_id", self.project_id.as_str()),
            ("domain_pack_id", self.domain_pack_id.as_str()),
        ] {
            require_id(field, value)?;
        }
        for (field, value) in [
            ("brief_sha256", self.brief_sha256.as_str()),
            ("domain_pack_sha256", self.domain_pack_sha256.as_str()),
            (
                "core_recipe_or_profile_sha256",
                self.core_recipe_or_profile_sha256.as_str(),
            ),
            (
                "runtime_manifest_sha256",
                self.runtime_manifest_sha256.as_str(),
            ),
            ("shape_program_sha256", self.shape_program_sha256.as_str()),
        ] {
            require_sha256(field, value)?;
        }
        if let Some(value) = &self.recipe_provenance_sha256 {
            require_sha256("recipe_provenance_sha256", value)?;
        }
        match (self.attempt_kind, self.parent_attempt_id.as_deref()) {
            (GenerationAttemptKind::Initial, None) => Ok(()),
            (GenerationAttemptKind::Repair, Some(parent)) => require_id("parent_attempt_id", parent),
            _ => Err(invalid(
                "SINGLE_GENERATION_ATTEMPT_LINEAGE_INVALID",
                "Initial attempts have no parent and repair attempts require exactly one parent attempt.",
            )),
        }
    }

    fn same_intent_as(&self, parent: &Self) -> bool {
        self.turn_id == parent.turn_id
            && self.project_id == parent.project_id
            && self.brief_sha256 == parent.brief_sha256
            && self.domain_pack_id == parent.domain_pack_id
            && self.domain_pack_sha256 == parent.domain_pack_sha256
            && self.core_recipe_or_profile_sha256 == parent.core_recipe_or_profile_sha256
            && self.runtime_manifest_sha256 == parent.runtime_manifest_sha256
            && self.recipe_provenance_sha256 == parent.recipe_provenance_sha256
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GenerationGateCheck {
    pub gate_id: String,
    pub outcome: VerificationOutcome,
    /// Only a concrete `fail` can be repairable. An unavailable/timeout
    /// (`undetermined`) must never be converted to a soft warning.
    pub repairable: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GenerationGateReport {
    pub schema_version: String,
    pub gate_report_id: String,
    pub attempt_id: String,
    pub glb_sha256: String,
    pub compile_readback_id: String,
    pub render_fingerprint: String,
    pub gate_profile_version: String,
    pub checks: Vec<GenerationGateCheck>,
    pub summary: String,
}

impl GenerationGateReport {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != GENERATION_GATE_REPORT_SCHEMA_VERSION {
            return Err(invalid(
                "GENERATION_GATE_REPORT_SCHEMA_INVALID",
                "Generation gate report must use its exact v1 schema.",
            ));
        }
        for (field, value) in [
            ("gate_report_id", self.gate_report_id.as_str()),
            ("attempt_id", self.attempt_id.as_str()),
            ("compile_readback_id", self.compile_readback_id.as_str()),
            ("render_fingerprint", self.render_fingerprint.as_str()),
            ("gate_profile_version", self.gate_profile_version.as_str()),
        ] {
            require_id(field, value)?;
        }
        require_sha256("glb_sha256", &self.glb_sha256)?;
        require_summary("summary", &self.summary)?;
        if self.checks.is_empty() {
            return Err(invalid(
                "GENERATION_GATE_REPORT_EMPTY",
                "Generation gate reports require at least one hard-gate fact.",
            ));
        }
        let mut ids = BTreeSet::new();
        for check in &self.checks {
            require_id("gate_id", &check.gate_id)?;
            require_summary("check.summary", &check.summary)?;
            if !ids.insert(check.gate_id.as_str()) {
                return Err(invalid(
                    "GENERATION_GATE_REPORT_DUPLICATE_CHECK",
                    "Generation gate report contains a duplicate gate id.",
                ));
            }
            if check.repairable && check.outcome != VerificationOutcome::Fail {
                return Err(invalid(
                    "GENERATION_GATE_REPAIRABILITY_INVALID",
                    "Only a concrete failed hard gate can be repaired.",
                ));
            }
        }
        Ok(())
    }

    pub fn is_passed(&self) -> bool {
        !self.checks.is_empty()
            && self
                .checks
                .iter()
                .all(|check| check.outcome == VerificationOutcome::Pass)
    }

    pub fn has_undetermined(&self) -> bool {
        self.checks
            .iter()
            .any(|check| check.outcome == VerificationOutcome::Undetermined)
    }

    pub fn repairable_gate_ids(&self) -> BTreeSet<&str> {
        self.checks
            .iter()
            .filter(|check| check.outcome == VerificationOutcome::Fail && check.repairable)
            .map(|check| check.gate_id.as_str())
            .collect()
    }

    pub fn allows_repair(&self) -> bool {
        let mut has_repairable_failure = false;
        for check in &self.checks {
            match check.outcome {
                VerificationOutcome::Pass => {}
                VerificationOutcome::Fail if check.repairable => {
                    has_repairable_failure = true;
                }
                VerificationOutcome::Fail | VerificationOutcome::Undetermined => return false,
            }
        }
        has_repairable_failure
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RepairAttempt {
    pub schema_version: String,
    pub repair_id: String,
    pub parent_attempt_id: String,
    pub parent_gate_report_id: String,
    pub repaired_gate_ids: Vec<String>,
    pub repaired_attempt: SingleGenerationAttempt,
}

impl RepairAttempt {
    pub fn validate_against(
        &self,
        parent: &SingleGenerationAttempt,
        report: &GenerationGateReport,
    ) -> CoreResult<()> {
        if self.schema_version != REPAIR_ATTEMPT_SCHEMA_VERSION {
            return Err(invalid(
                "REPAIR_ATTEMPT_SCHEMA_INVALID",
                "Repair attempt must use its exact v1 schema.",
            ));
        }
        require_id("repair_id", &self.repair_id)?;
        require_id("parent_attempt_id", &self.parent_attempt_id)?;
        require_id("parent_gate_report_id", &self.parent_gate_report_id)?;
        self.repaired_attempt.validate()?;
        if self.parent_attempt_id != parent.attempt_id
            || self.parent_gate_report_id != report.gate_report_id
            || self.repaired_attempt.attempt_kind != GenerationAttemptKind::Repair
            || self.repaired_attempt.parent_attempt_id.as_deref()
                != Some(parent.attempt_id.as_str())
            || !self.repaired_attempt.same_intent_as(parent)
        {
            return Err(CoreError::conflict(
                "REPAIR_ATTEMPT_INTENT_DRIFT",
                "Repair attempts must preserve the parent brief, domain, recipe/profile intent, runtime manifest, and provenance.",
            ));
        }
        let allowed = report.repairable_gate_ids();
        if !report.allows_repair() || self.repaired_gate_ids.is_empty() {
            return Err(CoreError::conflict(
                "REPAIR_ATTEMPT_NOT_ALLOWED",
                "The parent report does not authorize an in-place repair.",
            ));
        }
        let mut requested = BTreeSet::new();
        for gate_id in &self.repaired_gate_ids {
            require_id("repaired_gate_ids", gate_id)?;
            if !requested.insert(gate_id.as_str()) || !allowed.contains(gate_id.as_str()) {
                return Err(CoreError::conflict(
                    "REPAIR_ATTEMPT_SCOPE_INVALID",
                    "Repair may modify only the concrete failed fields authorized by the parent gate report.",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SingleResultState {
    ReadyForPreview,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SingleResultOutcome {
    Passed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GenerationPreview {
    pub preview_id: String,
    pub artifact_sha256: String,
    pub artifact_profile_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GenerationFailure {
    pub code: String,
    pub message: String,
    pub repair_attempts_used: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GenerationCancel {
    pub code: String,
    pub message: String,
}

/// The only V003 payload intended for the result card. It intentionally
/// excludes raw gate JSON, provider reasoning, variant IDs, and any Version
/// or Snapshot write identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SingleResultDecision {
    pub schema_version: String,
    pub decision_id: String,
    pub turn_id: String,
    pub project_id: String,
    pub state: SingleResultState,
    pub outcome: SingleResultOutcome,
    pub summary: String,
    pub attempt_id: String,
    pub gate_report_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<GenerationPreview>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<GenerationFailure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel: Option<GenerationCancel>,
}

impl SingleResultDecision {
    pub fn passed(
        decision_id: String,
        attempt: &SingleGenerationAttempt,
        report: &GenerationGateReport,
        summary: String,
        preview: GenerationPreview,
    ) -> CoreResult<Self> {
        attempt.validate()?;
        report.validate()?;
        if report.attempt_id != attempt.attempt_id || !report.is_passed() {
            return Err(CoreError::conflict(
                "SINGLE_RESULT_GATE_REQUIRED",
                "A result preview requires every hard gate to pass for the same attempt.",
            ));
        }
        if preview.artifact_sha256 != report.glb_sha256 {
            return Err(CoreError::conflict(
                "SINGLE_RESULT_PREVIEW_ARTIFACT_MISMATCH",
                "A result preview must reference the exact GLB verified by its hard-gate report.",
            ));
        }
        let decision = Self {
            schema_version: SINGLE_RESULT_DECISION_SCHEMA_VERSION.into(),
            decision_id,
            turn_id: attempt.turn_id.clone(),
            project_id: attempt.project_id.clone(),
            state: SingleResultState::ReadyForPreview,
            outcome: SingleResultOutcome::Passed,
            summary,
            attempt_id: attempt.attempt_id.clone(),
            gate_report_id: report.gate_report_id.clone(),
            preview: Some(preview),
            failure: None,
            cancel: None,
        };
        decision.validate()?;
        Ok(decision)
    }

    pub fn failed(
        decision_id: String,
        attempt: &SingleGenerationAttempt,
        report: &GenerationGateReport,
        summary: String,
        failure: GenerationFailure,
    ) -> CoreResult<Self> {
        attempt.validate()?;
        report.validate()?;
        if report.attempt_id != attempt.attempt_id || report.is_passed() {
            return Err(CoreError::conflict(
                "SINGLE_RESULT_FAILURE_INVALID",
                "A failed result must refer to the same non-passing hard-gate report.",
            ));
        }
        let decision = Self {
            schema_version: SINGLE_RESULT_DECISION_SCHEMA_VERSION.into(),
            decision_id,
            turn_id: attempt.turn_id.clone(),
            project_id: attempt.project_id.clone(),
            state: SingleResultState::Failed,
            outcome: SingleResultOutcome::Failed,
            summary,
            attempt_id: attempt.attempt_id.clone(),
            gate_report_id: report.gate_report_id.clone(),
            preview: None,
            failure: Some(failure),
            cancel: None,
        };
        decision.validate()?;
        Ok(decision)
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != SINGLE_RESULT_DECISION_SCHEMA_VERSION {
            return Err(invalid(
                "SINGLE_RESULT_DECISION_SCHEMA_INVALID",
                "Single result decisions must use their exact v1 schema.",
            ));
        }
        for (field, value) in [
            ("decision_id", self.decision_id.as_str()),
            ("turn_id", self.turn_id.as_str()),
            ("project_id", self.project_id.as_str()),
            ("attempt_id", self.attempt_id.as_str()),
            ("gate_report_id", self.gate_report_id.as_str()),
        ] {
            require_id(field, value)?;
        }
        require_summary("summary", &self.summary)?;
        match (
            &self.state,
            &self.outcome,
            &self.preview,
            &self.failure,
            &self.cancel,
        ) {
            (
                SingleResultState::ReadyForPreview,
                SingleResultOutcome::Passed,
                Some(preview),
                None,
                None,
            ) => {
                require_id("preview.preview_id", &preview.preview_id)?;
                require_sha256("preview.artifact_sha256", &preview.artifact_sha256)?;
                if !matches!(
                    preview.artifact_profile_id.as_str(),
                    "interactive_preview" | "production_concept"
                ) {
                    return Err(invalid(
                        "SINGLE_RESULT_PREVIEW_PROFILE_INVALID",
                        "Result previews require a reviewed ForgeCAD artifact profile.",
                    ));
                }
                if preview
                    .expires_at
                    .as_deref()
                    .is_some_and(|value| value.trim().is_empty() || value.len() > 128)
                {
                    return Err(invalid(
                        "SINGLE_RESULT_PREVIEW_EXPIRY_INVALID",
                        "Result preview expiry must be a bounded non-empty timestamp when present.",
                    ));
                }
                Ok(())
            }
            (SingleResultState::Failed, SingleResultOutcome::Failed, None, Some(failure), None) => {
                require_id("failure.code", &failure.code)?;
                require_summary("failure.message", &failure.message)?;
                if usize::from(failure.repair_attempts_used) > MAX_SAME_INTENT_REPAIR_ATTEMPTS {
                    return Err(invalid(
                        "SINGLE_RESULT_REPAIR_COUNT_INVALID",
                        "A failed result cannot report more than two same-intent repairs.",
                    ));
                }
                Ok(())
            }
            (
                SingleResultState::Cancelled,
                SingleResultOutcome::Cancelled,
                None,
                None,
                Some(cancel),
            ) => {
                require_id("cancel.code", &cancel.code)?;
                require_summary("cancel.message", &cancel.message)
            }
            _ => Err(invalid(
                "SINGLE_RESULT_DECISION_STATE_INVALID",
                "Result state, outcome, preview, failure, and cancel payload must agree.",
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SingleGenerationSessionState {
    Running,
    AwaitingRepair { parent_gate_report_id: String },
    ReadyForPreview(SingleResultDecision),
    Failed(SingleResultDecision),
    Cancelled(SingleResultDecision),
}

/// In-memory Turn state. Persist only the resulting lifecycle Items and
/// decision evidence through their existing ports; this session deliberately
/// cannot create Versions, snapshots, ChangeSets, or CAS references.
#[derive(Debug, Clone)]
pub struct SingleGenerationSession {
    initial_attempt: SingleGenerationAttempt,
    current_attempt: SingleGenerationAttempt,
    repair_attempts: Vec<RepairAttempt>,
    state: SingleGenerationSessionState,
}

impl SingleGenerationSession {
    pub fn begin(initial_attempt: SingleGenerationAttempt) -> CoreResult<Self> {
        initial_attempt.validate()?;
        if initial_attempt.attempt_kind != GenerationAttemptKind::Initial {
            return Err(invalid(
                "SINGLE_GENERATION_INITIAL_ATTEMPT_REQUIRED",
                "A V003 session must start with exactly one initial synthesis attempt.",
            ));
        }
        Ok(Self {
            initial_attempt: initial_attempt.clone(),
            current_attempt: initial_attempt,
            repair_attempts: Vec::new(),
            state: SingleGenerationSessionState::Running,
        })
    }

    pub fn current_attempt(&self) -> &SingleGenerationAttempt {
        &self.current_attempt
    }
    pub fn repair_attempts(&self) -> &[RepairAttempt] {
        &self.repair_attempts
    }
    pub fn state(&self) -> &SingleGenerationSessionState {
        &self.state
    }

    pub fn record_gate_report(
        &mut self,
        report: GenerationGateReport,
        decision_id: String,
        summary: String,
        preview: Option<GenerationPreview>,
    ) -> CoreResult<()> {
        if !matches!(self.state, SingleGenerationSessionState::Running) {
            return Err(CoreError::conflict(
                "SINGLE_GENERATION_SESSION_NOT_RUNNING",
                "A gate report may only be recorded for the active synthesis attempt.",
            ));
        }
        report.validate()?;
        if report.attempt_id != self.current_attempt.attempt_id {
            return Err(CoreError::conflict(
                "GENERATION_GATE_ATTEMPT_MISMATCH",
                "Gate reports must bind the currently active synthesis attempt.",
            ));
        }
        if report.is_passed() {
            let preview = preview.ok_or_else(|| {
                invalid(
                    "SINGLE_RESULT_PREVIEW_REQUIRED",
                    "A passing result requires a transient preview descriptor.",
                )
            })?;
            self.state =
                SingleGenerationSessionState::ReadyForPreview(SingleResultDecision::passed(
                    decision_id,
                    &self.current_attempt,
                    &report,
                    summary,
                    preview,
                )?);
        } else if report.allows_repair()
            && self.repair_attempts.len() < MAX_SAME_INTENT_REPAIR_ATTEMPTS
        {
            self.state = SingleGenerationSessionState::AwaitingRepair {
                parent_gate_report_id: report.gate_report_id,
            };
        } else {
            let code = if report.has_undetermined() {
                "GENERATION_GATE_UNDETERMINED"
            } else {
                "GENERATION_GATE_FAILED"
            };
            self.state = SingleGenerationSessionState::Failed(SingleResultDecision::failed(
                decision_id,
                &self.current_attempt,
                &report,
                summary,
                GenerationFailure {
                    code: code.into(),
                    message: "The single synthesis did not pass every required verification."
                        .into(),
                    repair_attempts_used: self.repair_attempts.len() as u8,
                },
            )?);
        }
        Ok(())
    }

    pub fn apply_repair(
        &mut self,
        repair: RepairAttempt,
        parent_report: &GenerationGateReport,
    ) -> CoreResult<()> {
        let SingleGenerationSessionState::AwaitingRepair {
            parent_gate_report_id,
        } = &self.state
        else {
            return Err(CoreError::conflict(
                "REPAIR_ATTEMPT_STATE_INVALID",
                "Repairs are allowed only after the current report requests one.",
            ));
        };
        if parent_gate_report_id != &parent_report.gate_report_id
            || parent_report.attempt_id != self.current_attempt.attempt_id
        {
            return Err(CoreError::conflict(
                "REPAIR_ATTEMPT_PARENT_MISMATCH",
                "Repair parent report no longer matches the active synthesis attempt.",
            ));
        }
        if self.repair_attempts.len() >= MAX_SAME_INTENT_REPAIR_ATTEMPTS {
            return Err(CoreError::conflict(
                "REPAIR_ATTEMPT_LIMIT_REACHED",
                "A V003 synthesis permits at most two same-intent repair attempts.",
            ));
        }
        repair.validate_against(&self.current_attempt, parent_report)?;
        self.current_attempt = repair.repaired_attempt.clone();
        self.repair_attempts.push(repair);
        self.state = SingleGenerationSessionState::Running;
        Ok(())
    }

    pub fn cancel(
        &mut self,
        decision_id: String,
        report_id: String,
        summary: String,
        cancel: GenerationCancel,
    ) -> CoreResult<()> {
        if matches!(
            self.state,
            SingleGenerationSessionState::ReadyForPreview(_)
                | SingleGenerationSessionState::Failed(_)
                | SingleGenerationSessionState::Cancelled(_)
        ) {
            return Err(CoreError::conflict(
                "SINGLE_GENERATION_SESSION_TERMINAL",
                "A terminal V003 session cannot be cancelled again.",
            ));
        }
        let decision = SingleResultDecision {
            schema_version: SINGLE_RESULT_DECISION_SCHEMA_VERSION.into(),
            decision_id,
            turn_id: self.current_attempt.turn_id.clone(),
            project_id: self.current_attempt.project_id.clone(),
            state: SingleResultState::Cancelled,
            outcome: SingleResultOutcome::Cancelled,
            summary,
            attempt_id: self.current_attempt.attempt_id.clone(),
            gate_report_id: report_id,
            preview: None,
            failure: None,
            cancel: Some(cancel),
        };
        decision.validate()?;
        self.state = SingleGenerationSessionState::Cancelled(decision);
        Ok(())
    }

    pub fn initial_attempt(&self) -> &SingleGenerationAttempt {
        &self.initial_attempt
    }
}

fn invalid(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message)
}

fn require_id(field: &str, value: &str) -> CoreResult<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'@'))
    {
        return Err(invalid(
            "SINGLE_GENERATION_ID_INVALID",
            format!("{field} must be a bounded stable identifier."),
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> CoreResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid(
            "SINGLE_GENERATION_SHA256_INVALID",
            format!("{field} must be a lowercase SHA-256 digest."),
        ));
    }
    Ok(())
}

fn require_summary(field: &str, value: &str) -> CoreResult<()> {
    if value.trim().is_empty() || value.len() > 1_024 {
        return Err(invalid(
            "SINGLE_GENERATION_SUMMARY_INVALID",
            format!("{field} must be a bounded non-empty summary."),
        ));
    }
    Ok(())
}
