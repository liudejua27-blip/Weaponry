//! Rust-owned FGC-E005 prepare-once Provider permit runner.
//!
//! This module deliberately stops short of claiming a formal 30-task run. It
//! owns the non-bypassable per-task call and receipt boundary used by the
//! future batch command:
//! startup recovery -> code-owned request -> prepare once -> reserve -> mark
//! dispatching -> one Provider dispatch -> local typed validation -> verifier
//! -> conservative settlement.

use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use forgecad_core::{
    apply_forge_visual_geometry_patch_v2, lower_forge_visual_author_source_v1, semantic_sha256,
    CoreError, CoreRepository, E005ProviderBudgetEvidence, E005ProviderBudgetLedger,
    E005ProviderCallKind, E005ProviderCallOutcome, E005ProviderCallReservation,
    E005ProviderCallReservationRequest, E005ProviderCallSettlement,
    E005ProviderRunAuthorizationContract, E005ProviderUsageCheckpoint, E005VisualReviewCheckpoint,
    E005VisualReviewCheckpointState, VisualProgramAuthoringSessionV2,
    VisualProgramAuthoringStateV2, VisualProgramPhaseV2, VisualProgramUsageV2,
    E005_FORMAL_TASK_SET_SHA256,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    e005_offline_harness::{
        adapt_r2_formal_result, adapt_result, upgrade_r3_formal_receipt,
        validate_source_operation_allowlist, E005EngineeringGateEvaluator, E005HumanReviewStatus,
        E005OfflineHarnessRequest, E005RunReceipt, E005RunStatus, E005_TASK_SET_SHA256,
    },
    CancellationToken, E005PreparedVisualReviewProviderPort, E005PreparedVisualReviewV1,
    E005ProductionReviewCoordinatorV1, E005ProductionReviewResultV1, E005VisualReviewCoordinatorV1,
    E005VisualReviewEvidenceV1, E005VisualReviewRequestV1, E005VisualReviewResultV1,
    PreparedE005VisualReviewProviderRequest, ProviderClient, ProviderError, ProviderErrorCategory,
    ProviderFinishReason, ProviderMessage, ProviderRequest, ProviderResponse, ProviderRole,
    ProviderToolDefinition, ProviderUsage, RestrictedGeometryPort, Vp204RuntimeContinuation,
    Vp204RuntimeCoordinator, Vp204RuntimeInitialOutcome, Vp204RuntimeRequest, Vp204RuntimeResult,
    E005_VISUAL_REVIEW_MAX_OUTPUT_TOKENS, E005_VISUAL_REVIEW_SYSTEM_PROMPT,
};

pub const E005_AUTHOR_TOOL_NAME: &str = "submit_e005_visual_program";
pub const E005_PATCH_TOOL_NAME: &str = "submit_e005_visual_patch";
pub const E005_AUTHOR_MAX_OUTPUT_TOKENS: u64 = 8_192;
pub const E005_PATCH_MAX_OUTPUT_TOKENS: u64 = 2_048;

const E005_AUTHOR_SYSTEM_PROMPT: &str = "You are ForgeCAD's bounded mechanical hard-surface visual-program author. Return exactly one submit_e005_visual_program tool call. Build the whole visible object from the frozen task using only ForgeVisualAuthorSource@1 and the task operation allowlist. Use compact typed parameters, reusable geometry macros, bounded repeat, one rigid Part hierarchy, Material Zones, typed surface bindings, and explicit detail motifs so primitive count does not force linear author output. Preserve strong silhouette, coherent attachment, mechanical hierarchy and visible construction detail. Whole-object catalog templates, prose-only answers, arbitrary code, paths, URLs, and manufacturing instructions are forbidden.";
const E005_PATCH_SYSTEM_PROMPT: &str = "E005-R1 does not authorize a Provider patch for ForgeVisualAuthorSource@1. A hash-bound visual patch contract will be introduced by E005-R2; do not dispatch this tool under the R1 source policy.";

const AUTHOR_SCHEMA_TEXT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/schemas/forge-visual-author-source-v1.schema.json"
));
const GEOMETRY_TEMPLATE_SCHEMA_TEXT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/schemas/forge-visual-geometry-program-v2.schema.json"
));
const PATCH_SCHEMA_TEXT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/schemas/forge-visual-geometry-patch-v1.schema.json"
));
const E005_VISUAL_PATCH_PROPOSAL_SCHEMA_TEXT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/schemas/e005-visual-patch-proposal-v1.schema.json"
));
const E005_VISUAL_PATCH_SCHEMA_TEXT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/schemas/e005-visual-patch-v1.schema.json"
));

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct E005ProviderCallInput {
    pub authorization_id: String,
    pub task_id: String,
    pub task_payload: Value,
    pub call_kind: E005ProviderCallKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_base_source: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_gate: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum E005ProviderVerificationVerdict {
    Passed,
    Repairable,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct E005ProviderAuthoredOutput {
    pub task_id: String,
    pub call_kind: E005ProviderCallKind,
    pub tool_name: String,
    pub authored_value: Value,
    pub final_source: Value,
    pub final_source_sha256: String,
    pub provider_usage: ProviderUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005ProviderVerification {
    pub verdict: E005ProviderVerificationVerdict,
    pub source_program_sha256: Option<String>,
    pub gate_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_gate: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005ProviderCallExecution {
    pub task_id: String,
    pub call_kind: E005ProviderCallKind,
    pub request_sha256: String,
    pub pricing_snapshot_sha256: String,
    pub provider_usage: ProviderUsage,
    pub authored_value: Value,
    pub final_source: Value,
    pub verification: E005ProviderVerification,
    pub budget_evidence: E005ProviderBudgetEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005ProviderRunnerFailure {
    pub code: String,
    pub message: String,
    pub network_call_made: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_evidence: Option<E005ProviderBudgetEvidence>,
}

impl std::fmt::Display for E005ProviderRunnerFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for E005ProviderRunnerFailure {}

pub type E005VerificationFuture = Pin<
    Box<dyn Future<Output = Result<E005ProviderVerification, E005ProviderRunnerFailure>> + Send>,
>;

pub trait E005ProviderOutputVerifier: Send + Sync + 'static {
    fn verify(
        &self,
        output: E005ProviderAuthoredOutput,
        cancellation: CancellationToken,
    ) -> E005VerificationFuture;
}

/// R2 author verifier. A valid unified source is intentionally not rendered
/// here: its first and only candidate build belongs to the eight-view visual
/// review stage. The repairable gate means "awaiting the authorized visual
/// decision", not an engineering failure and not permission for a legacy
/// geometry-patch tool call.
#[derive(Debug, Default)]
pub struct E005PendingVisualReviewVerifier;

impl E005ProviderOutputVerifier for E005PendingVisualReviewVerifier {
    fn verify(
        &self,
        output: E005ProviderAuthoredOutput,
        _cancellation: CancellationToken,
    ) -> E005VerificationFuture {
        Box::pin(async move {
            if output.call_kind != E005ProviderCallKind::Author
                || lower_forge_visual_author_source_v1(&output.final_source)
                    .map_err(core_failure_after_provider)?
                    .source_program_sha256
                    != output.final_source_sha256
            {
                return Err(verifier_failure("E005_R2_AUTHOR_SOURCE_INVALID"));
            }
            let pending_gate = json!({
                "schema_version": "VisualProgramGateOutcome@1",
                "gate_report_id": "gate_e005_r2_awaiting_visual_review",
                "source_program_sha256": output.final_source_sha256,
                "verdict": "fail",
                "repairable": true,
            });
            let gate_sha256 =
                semantic_sha256(&pending_gate).map_err(core_failure_after_provider)?;
            Ok(E005ProviderVerification {
                verdict: E005ProviderVerificationVerdict::Repairable,
                source_program_sha256: pending_gate
                    .get("source_program_sha256")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                gate_sha256: Some(gate_sha256),
                failed_gate: Some(pending_gate),
            })
        })
    }
}

struct E005PendingVp204Verification {
    continuation: Vp204RuntimeContinuation,
    author_usage: ProviderUsage,
}

#[derive(Default)]
struct E005Vp204VerifierState {
    pending: BTreeMap<String, E005PendingVp204Verification>,
    completed: BTreeMap<String, Vp204RuntimeResult>,
}

/// Production VP204 verifier for formal E005 calls.
///
/// A repairable author result retains one opaque, non-cloneable VP204
/// continuation. The patch call must consume that exact continuation, so the
/// initial lowering/compile/render/gate stage cannot be repeated accidentally.
#[derive(Clone)]
pub struct E005Vp204OutputVerifier {
    coordinator: Vp204RuntimeCoordinator,
    state: Arc<Mutex<E005Vp204VerifierState>>,
}

impl E005Vp204OutputVerifier {
    pub fn new(geometry: Arc<dyn RestrictedGeometryPort>) -> Self {
        Self {
            coordinator: Vp204RuntimeCoordinator::new(
                geometry,
                Arc::new(E005EngineeringGateEvaluator),
            ),
            state: Arc::new(Mutex::new(E005Vp204VerifierState::default())),
        }
    }

    pub fn take_completed_result(
        &self,
        task_id: &str,
    ) -> Result<Option<Vp204RuntimeResult>, E005ProviderRunnerFailure> {
        self.state
            .lock()
            .map_err(|_| verifier_failure("E005_VP204_VERIFIER_STATE_POISONED"))
            .map(|mut state| state.completed.remove(task_id))
    }

    pub fn has_pending_patch(&self, task_id: &str) -> Result<bool, E005ProviderRunnerFailure> {
        self.state
            .lock()
            .map_err(|_| verifier_failure("E005_VP204_VERIFIER_STATE_POISONED"))
            .map(|state| state.pending.contains_key(task_id))
    }

    pub fn discard_task(&self, task_id: &str) -> Result<(), E005ProviderRunnerFailure> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| verifier_failure("E005_VP204_VERIFIER_STATE_POISONED"))?;
        state.pending.remove(task_id);
        state.completed.remove(task_id);
        Ok(())
    }

    async fn verify_output(
        &self,
        output: E005ProviderAuthoredOutput,
        cancellation: CancellationToken,
    ) -> Result<E005ProviderVerification, E005ProviderRunnerFailure> {
        match output.call_kind {
            E005ProviderCallKind::Author => self.verify_author(output, cancellation).await,
            E005ProviderCallKind::Patch => self.verify_patch(output, cancellation).await,
        }
    }

    async fn verify_author(
        &self,
        output: E005ProviderAuthoredOutput,
        cancellation: CancellationToken,
    ) -> Result<E005ProviderVerification, E005ProviderRunnerFailure> {
        {
            let state = self
                .state
                .lock()
                .map_err(|_| verifier_failure("E005_VP204_VERIFIER_STATE_POISONED"))?;
            if state.pending.contains_key(&output.task_id)
                || state.completed.contains_key(&output.task_id)
            {
                return Err(verifier_failure("E005_VP204_AUTHOR_ALREADY_VERIFIED"));
            }
        }
        let request_sha256 = semantic_sha256(&json!({
            "schema_version": "E005Vp204VerificationRequest@1",
            "task_id": output.task_id,
            "call_kind": "author",
            "source_program_sha256": output.final_source_sha256,
        }))
        .map_err(core_failure_after_provider)?;
        let suffix = &request_sha256[..16];
        let initial = self
            .coordinator
            .execute_initial(
                Vp204RuntimeRequest {
                    session_id: format!("vpsession_e005_formal_{suffix}"),
                    idempotency_key: format!("idem_e005_formal_{suffix}"),
                    request_sha256,
                    source: output.final_source,
                    patch: None,
                    usage: provider_usage(&output.provider_usage, 1, 1),
                },
                cancellation,
            )
            .await
            .map_err(vp204_failure_after_provider)?;
        if initial.session().receipt.source_program_sha256 != output.final_source_sha256 {
            return Err(verifier_failure(
                "E005_VP204_AUTHOR_SOURCE_LINEAGE_MISMATCH",
            ));
        }
        match initial {
            Vp204RuntimeInitialOutcome::AwaitingPatch(continuation) => {
                let verification = verification_from_session(
                    continuation.session(),
                    E005ProviderVerificationVerdict::Repairable,
                )?;
                self.state
                    .lock()
                    .map_err(|_| verifier_failure("E005_VP204_VERIFIER_STATE_POISONED"))?
                    .pending
                    .insert(
                        output.task_id,
                        E005PendingVp204Verification {
                            continuation,
                            author_usage: output.provider_usage,
                        },
                    );
                Ok(verification)
            }
            Vp204RuntimeInitialOutcome::Complete(result) => {
                let verdict =
                    if result.session.state == VisualProgramAuthoringStateV2::ReadyForPreview {
                        E005ProviderVerificationVerdict::Passed
                    } else {
                        E005ProviderVerificationVerdict::Failed
                    };
                let verification = verification_from_session(&result.session, verdict)?;
                self.state
                    .lock()
                    .map_err(|_| verifier_failure("E005_VP204_VERIFIER_STATE_POISONED"))?
                    .completed
                    .insert(output.task_id, result);
                Ok(verification)
            }
        }
    }

    async fn verify_patch(
        &self,
        output: E005ProviderAuthoredOutput,
        cancellation: CancellationToken,
    ) -> Result<E005ProviderVerification, E005ProviderRunnerFailure> {
        let pending = self
            .state
            .lock()
            .map_err(|_| verifier_failure("E005_VP204_VERIFIER_STATE_POISONED"))?
            .pending
            .remove(&output.task_id)
            .ok_or_else(|| verifier_failure("E005_VP204_PATCH_CONTINUATION_MISSING"))?;
        let cumulative_usage =
            combined_provider_usage(&pending.author_usage, &output.provider_usage)?;
        let result = self
            .coordinator
            .resume_with_patch(
                pending.continuation,
                output.authored_value,
                cumulative_usage,
                cancellation,
            )
            .await
            .map_err(vp204_failure_after_provider)?;
        if result.session.receipt.source_program_sha256 != output.final_source_sha256 {
            return Err(verifier_failure("E005_VP204_PATCH_SOURCE_LINEAGE_MISMATCH"));
        }
        let verdict = if result.session.state == VisualProgramAuthoringStateV2::ReadyForPreview {
            E005ProviderVerificationVerdict::Passed
        } else {
            E005ProviderVerificationVerdict::Failed
        };
        let verification = verification_from_session(&result.session, verdict)?;
        self.state
            .lock()
            .map_err(|_| verifier_failure("E005_VP204_VERIFIER_STATE_POISONED"))?
            .completed
            .insert(output.task_id, result);
        Ok(verification)
    }
}

impl E005ProviderOutputVerifier for E005Vp204OutputVerifier {
    fn verify(
        &self,
        output: E005ProviderAuthoredOutput,
        cancellation: CancellationToken,
    ) -> E005VerificationFuture {
        let verifier = self.clone();
        Box::pin(async move { verifier.verify_output(output, cancellation).await })
    }
}

fn provider_usage(
    usage: &ProviderUsage,
    provider_requests: u8,
    product_tool_calls: u16,
) -> VisualProgramUsageV2 {
    VisualProgramUsageV2 {
        provider_requests,
        product_tool_calls,
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        prompt_cache_hit_tokens: usage.prompt_cache_hit_tokens,
        prompt_cache_miss_tokens: usage.prompt_cache_miss_tokens,
        estimated_cost_microusd: usage.estimated_cost_microusd,
    }
}

fn combined_provider_usage(
    author: &ProviderUsage,
    patch: &ProviderUsage,
) -> Result<VisualProgramUsageV2, E005ProviderRunnerFailure> {
    fn add(left: u64, right: u64) -> Result<u64, E005ProviderRunnerFailure> {
        left.checked_add(right)
            .ok_or_else(|| verifier_failure("E005_VP204_PROVIDER_USAGE_OVERFLOW"))
    }
    Ok(VisualProgramUsageV2 {
        provider_requests: 2,
        product_tool_calls: 2,
        input_tokens: add(author.input_tokens, patch.input_tokens)?,
        output_tokens: add(author.output_tokens, patch.output_tokens)?,
        prompt_cache_hit_tokens: add(
            author.prompt_cache_hit_tokens,
            patch.prompt_cache_hit_tokens,
        )?,
        prompt_cache_miss_tokens: add(
            author.prompt_cache_miss_tokens,
            patch.prompt_cache_miss_tokens,
        )?,
        estimated_cost_microusd: add(
            author.estimated_cost_microusd,
            patch.estimated_cost_microusd,
        )?,
    })
}

fn verification_from_session(
    session: &VisualProgramAuthoringSessionV2,
    verdict: E005ProviderVerificationVerdict,
) -> Result<E005ProviderVerification, E005ProviderRunnerFailure> {
    match verdict {
        E005ProviderVerificationVerdict::Passed => {
            if session.state != VisualProgramAuthoringStateV2::ReadyForPreview {
                return Err(verifier_failure("E005_VP204_PASS_STATE_INVALID"));
            }
        }
        E005ProviderVerificationVerdict::Repairable => {
            if session.state != VisualProgramAuthoringStateV2::AwaitingPatch {
                return Err(verifier_failure("E005_VP204_REPAIR_STATE_INVALID"));
            }
        }
        E005ProviderVerificationVerdict::Failed => {
            return Ok(E005ProviderVerification {
                verdict,
                source_program_sha256: None,
                gate_sha256: None,
                failed_gate: None,
            });
        }
    }
    let gate_sha256 = session
        .receipt
        .phases
        .iter()
        .rev()
        .find(|phase| phase.phase == VisualProgramPhaseV2::Evaluate)
        .map(|phase| phase.output_sha256.clone())
        .filter(|hash| valid_sha256(hash))
        .ok_or_else(|| verifier_failure("E005_VP204_GATE_LINEAGE_MISSING"))?;
    let failed_gate = (verdict == E005ProviderVerificationVerdict::Repairable).then(|| {
        json!({
            "schema_version": "VisualProgramGateOutcome@1",
            "gate_report_id": session.gate_report_id,
            "source_program_sha256": session.receipt.source_program_sha256,
            "verdict": "fail",
            "repairable": true,
        })
    });
    if failed_gate
        .as_ref()
        .map(semantic_sha256)
        .transpose()
        .map_err(core_failure_after_provider)?
        .as_deref()
        .is_some_and(|hash| hash != gate_sha256)
    {
        return Err(verifier_failure("E005_VP204_GATE_RECONSTRUCTION_MISMATCH"));
    }
    Ok(E005ProviderVerification {
        verdict,
        source_program_sha256: Some(session.receipt.source_program_sha256.clone()),
        gate_sha256: Some(gate_sha256),
        failed_gate,
    })
}

fn verifier_failure(code: &'static str) -> E005ProviderRunnerFailure {
    failure(
        code,
        "The Rust-owned VP204 formal verifier rejected the E005 result.",
        true,
        None,
    )
}

fn vp204_failure_after_provider(error: crate::Vp204RuntimeFailure) -> E005ProviderRunnerFailure {
    failure(&error.code, &error.message, true, None)
}

fn core_failure_after_provider(error: CoreError) -> E005ProviderRunnerFailure {
    failure(error.code(), "VP204 evidence hashing failed.", true, None)
}

/// Narrow persistence port. The production implementation delegates every
/// operation to CoreRepository's atomic E005 ledger; there is no runner-local
/// counter or alternate source of truth.
pub trait E005ProviderBudgetPort: Send + Sync + 'static {
    fn recover_after_restart(&self) -> Result<Vec<E005ProviderBudgetEvidence>, CoreError>;
    fn ledger(&self, authorization_id: &str)
        -> Result<Option<E005ProviderBudgetLedger>, CoreError>;
    fn reserve(
        &self,
        request: &E005ProviderCallReservationRequest,
    ) -> Result<E005ProviderCallReservation, CoreError>;
    fn mark_dispatching(&self, reservation_id: &str) -> Result<(), CoreError>;
    fn settle(
        &self,
        reservation_id: &str,
        settlement: &E005ProviderCallSettlement,
    ) -> Result<E005ProviderBudgetEvidence, CoreError>;
    fn verify_evidence(&self, evidence: &E005ProviderBudgetEvidence) -> Result<(), CoreError>;
}

impl E005ProviderBudgetPort for CoreRepository {
    fn recover_after_restart(&self) -> Result<Vec<E005ProviderBudgetEvidence>, CoreError> {
        self.recover_e005_provider_budget_after_restart()
    }

    fn ledger(
        &self,
        authorization_id: &str,
    ) -> Result<Option<E005ProviderBudgetLedger>, CoreError> {
        self.e005_provider_budget_ledger(authorization_id)
    }

    fn reserve(
        &self,
        request: &E005ProviderCallReservationRequest,
    ) -> Result<E005ProviderCallReservation, CoreError> {
        self.reserve_e005_provider_call(request)
    }

    fn mark_dispatching(&self, reservation_id: &str) -> Result<(), CoreError> {
        self.mark_e005_provider_call_dispatching(reservation_id)
    }

    fn settle(
        &self,
        reservation_id: &str,
        settlement: &E005ProviderCallSettlement,
    ) -> Result<E005ProviderBudgetEvidence, CoreError> {
        self.settle_e005_provider_call(reservation_id, settlement)
    }

    fn verify_evidence(&self, evidence: &E005ProviderBudgetEvidence) -> Result<(), CoreError> {
        self.verify_e005_provider_budget_evidence(evidence)
    }
}

/// Durable R2 handoff port. It persists only the validated Author source,
/// ledger-verified evidence and exact usage needed to resume the visual stage.
/// Provider prompts, credentials, image bytes and raw responses stay memory-only.
pub trait E005VisualReviewCheckpointPort: Send + Sync + 'static {
    fn recover_after_provider_recovery(&self)
        -> Result<Vec<E005VisualReviewCheckpoint>, CoreError>;
    fn checkpoint_author(
        &self,
        evidence: &E005ProviderBudgetEvidence,
        usage: &ProviderUsage,
        source: &Value,
    ) -> Result<E005VisualReviewCheckpoint, CoreError>;
    fn checkpoint(
        &self,
        authorization_id: &str,
        task_id: &str,
    ) -> Result<Option<E005VisualReviewCheckpoint>, CoreError>;
    fn complete_visual(
        &self,
        evidence: &E005ProviderBudgetEvidence,
        visual_review_evidence_sha256: &str,
    ) -> Result<E005VisualReviewCheckpoint, CoreError>;
}

impl E005VisualReviewCheckpointPort for CoreRepository {
    fn recover_after_provider_recovery(
        &self,
    ) -> Result<Vec<E005VisualReviewCheckpoint>, CoreError> {
        self.recover_e005_visual_review_checkpoints_after_provider_recovery()
    }

    fn checkpoint_author(
        &self,
        evidence: &E005ProviderBudgetEvidence,
        usage: &ProviderUsage,
        source: &Value,
    ) -> Result<E005VisualReviewCheckpoint, CoreError> {
        self.checkpoint_e005_author_awaiting_visual_review(
            evidence,
            &E005ProviderUsageCheckpoint {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                prompt_cache_hit_tokens: usage.prompt_cache_hit_tokens,
                prompt_cache_miss_tokens: usage.prompt_cache_miss_tokens,
                estimated_cost_microusd: usage.estimated_cost_microusd,
            },
            source,
        )
    }

    fn checkpoint(
        &self,
        authorization_id: &str,
        task_id: &str,
    ) -> Result<Option<E005VisualReviewCheckpoint>, CoreError> {
        self.e005_visual_review_checkpoint(authorization_id, task_id)
    }

    fn complete_visual(
        &self,
        evidence: &E005ProviderBudgetEvidence,
        visual_review_evidence_sha256: &str,
    ) -> Result<E005VisualReviewCheckpoint, CoreError> {
        self.complete_e005_visual_review_checkpoint(evidence, visual_review_evidence_sha256)
    }
}

pub struct E005FormalProviderRunner {
    budget: Arc<dyn E005ProviderBudgetPort>,
    provider: Arc<dyn ProviderClient>,
    verifier: Arc<dyn E005ProviderOutputVerifier>,
    startup_recovery: Vec<E005ProviderBudgetEvidence>,
}

impl std::fmt::Debug for E005FormalProviderRunner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("E005FormalProviderRunner")
            .field("budget", &"[RUST_LEDGER]")
            .field("provider", &"[PREPARE_ONCE]")
            .field("verifier", &"[VP204_VERIFIER]")
            .field("startup_recovery_count", &self.startup_recovery.len())
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct E005FormalTaskRequest {
    pub authorization_id: String,
    pub task_id: String,
    pub task_payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct E005FormalVisualCallInput {
    pub authorization_id: String,
    pub task_id: String,
    pub task_payload: Value,
    pub initial_source_sha256: String,
    pub comparison_input_sha256: String,
}

#[derive(Debug, Clone)]
pub struct E005FormalVisualCallExecution {
    pub task_id: String,
    pub request_sha256: String,
    pub pricing_snapshot_sha256: String,
    pub provider_usage: ProviderUsage,
    pub review: E005VisualReviewResultV1,
    pub budget_evidence: E005ProviderBudgetEvidence,
}

#[derive(Debug, Clone)]
pub struct E005FormalR2TaskExecution {
    pub request: E005FormalTaskRequest,
    pub author: E005ProviderCallExecution,
    pub visual: E005FormalVisualCallExecution,
    pub elapsed_ms: u64,
}

impl E005FormalR2TaskExecution {
    pub fn network_provider_calls(&self) -> u8 {
        2
    }

    pub fn geometry_build_count(&self) -> u8 {
        self.visual.review.geometry_build_count
    }

    pub fn budget_evidence(&self) -> [&E005ProviderBudgetEvidence; 2] {
        [&self.author.budget_evidence, &self.visual.budget_evidence]
    }
}

fn resumed_author_execution(
    checkpoint: E005VisualReviewCheckpoint,
    budget: &dyn E005ProviderBudgetPort,
) -> Result<E005ProviderCallExecution, E005ProviderRunnerFailure> {
    checkpoint
        .validate()
        .map_err(core_failure_without_evidence)?;
    budget
        .verify_evidence(&checkpoint.author_budget_evidence)
        .map_err(core_failure_without_evidence)?;
    let ledger = budget
        .ledger(&checkpoint.authorization_id)
        .map_err(core_failure_without_evidence)?
        .ok_or_else(|| {
            failure(
                "E005_PROVIDER_AUTHORIZATION_MISSING",
                "No E005 Provider authorization exists for the visual-review checkpoint.",
                false,
                None,
            )
        })?;
    let pricing_snapshot_sha256 = ledger
        .authorization
        .pricing_snapshot_sha256
        .clone()
        .ok_or_else(|| verifier_failure("E005_R2_CHECKPOINT_PRICING_MISSING"))?;
    let failed_gate = json!({
        "schema_version": "VisualProgramGateOutcome@1",
        "gate_report_id": "gate_e005_r2_awaiting_visual_review",
        "source_program_sha256": checkpoint.author_source_sha256,
        "verdict": "fail",
        "repairable": true,
    });
    let gate_sha256 = semantic_sha256(&failed_gate).map_err(core_failure_without_evidence)?;
    if checkpoint
        .author_budget_evidence
        .output_source_sha256
        .as_deref()
        != Some(checkpoint.author_source_sha256.as_str())
        || checkpoint
            .author_budget_evidence
            .output_gate_sha256
            .as_deref()
            != Some(gate_sha256.as_str())
    {
        return Err(verifier_failure("E005_R2_CHECKPOINT_GATE_LINEAGE_MISMATCH"));
    }
    let usage = checkpoint.author_provider_usage;
    Ok(E005ProviderCallExecution {
        task_id: checkpoint.task_id,
        call_kind: E005ProviderCallKind::Author,
        request_sha256: checkpoint.author_budget_evidence.request_sha256.clone(),
        pricing_snapshot_sha256,
        provider_usage: ProviderUsage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            prompt_cache_hit_tokens: usage.prompt_cache_hit_tokens,
            prompt_cache_miss_tokens: usage.prompt_cache_miss_tokens,
            estimated_cost_microusd: usage.estimated_cost_microusd,
        },
        authored_value: checkpoint.author_source.clone(),
        final_source: checkpoint.author_source,
        verification: E005ProviderVerification {
            verdict: E005ProviderVerificationVerdict::Repairable,
            source_program_sha256: Some(checkpoint.author_source_sha256),
            gate_sha256: Some(gate_sha256),
            failed_gate: Some(failed_gate),
        },
        budget_evidence: checkpoint.author_budget_evidence,
    })
}

/// Full R2 task path for a caller that already owns true sealed image bytes:
/// one unified author call, one generic TurntableEight build, then one
/// prepare-once visual call and at most one deterministic rebuild.
pub struct E005FormalR2TaskCoordinator {
    runner: E005FormalProviderRunner,
    checkpoints: Arc<dyn E005VisualReviewCheckpointPort>,
    review: E005VisualReviewCoordinatorV1,
    visual_provider: Arc<dyn E005PreparedVisualReviewProviderPort>,
    startup_checkpoint_recovery: Vec<E005VisualReviewCheckpoint>,
}

impl std::fmt::Debug for E005FormalR2TaskCoordinator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("E005FormalR2TaskCoordinator")
            .field("runner", &self.runner)
            .field(
                "startup_checkpoint_recovery_count",
                &self.startup_checkpoint_recovery.len(),
            )
            .field("review", &"[RUST_VISUAL_REVIEW]")
            .field("visual_provider", &"[PREPARE_ONCE_MULTIMODAL]")
            .finish()
    }
}

impl E005FormalR2TaskCoordinator {
    pub fn bootstrap<F>(
        budget: Arc<dyn E005ProviderBudgetPort>,
        checkpoints: Arc<dyn E005VisualReviewCheckpointPort>,
        review: E005VisualReviewCoordinatorV1,
        visual_provider: Arc<dyn E005PreparedVisualReviewProviderPort>,
        author_provider_factory: F,
    ) -> Result<Self, E005ProviderRunnerFailure>
    where
        F: FnOnce() -> Result<Arc<dyn ProviderClient>, ProviderError>,
    {
        let runner = E005FormalProviderRunner::bootstrap(
            budget,
            Arc::new(E005PendingVisualReviewVerifier),
            author_provider_factory,
        )?;
        let startup_checkpoint_recovery = checkpoints
            .recover_after_provider_recovery()
            .map_err(core_failure_without_evidence)?;
        Ok(Self {
            runner,
            checkpoints,
            review,
            visual_provider,
            startup_checkpoint_recovery,
        })
    }

    pub fn startup_recovery(&self) -> &[E005ProviderBudgetEvidence] {
        self.runner.startup_recovery()
    }

    pub fn startup_checkpoint_recovery(&self) -> &[E005VisualReviewCheckpoint] {
        &self.startup_checkpoint_recovery
    }

    pub async fn execute_task(
        &self,
        request: E005FormalTaskRequest,
        mut visual_request: E005VisualReviewRequestV1,
        cancellation: CancellationToken,
    ) -> Result<E005FormalR2TaskExecution, E005ProviderRunnerFailure> {
        let started = Instant::now();
        if visual_request.request.turn_id != visual_request.turn_id
            || visual_request.request.project_id != visual_request.graph.project_id
        {
            return Err(failure(
                "E005_R2_FORMAL_REVIEW_REQUEST_INVALID",
                "Formal R2 visual evidence request has inconsistent turn or project lineage.",
                false,
                None,
            ));
        }
        let task_payload_sha256 =
            semantic_sha256(&request.task_payload).map_err(core_failure_without_evidence)?;
        let author = match self
            .checkpoints
            .checkpoint(&request.authorization_id, &request.task_id)
            .map_err(core_failure_without_evidence)?
        {
            Some(checkpoint)
                if checkpoint.state == E005VisualReviewCheckpointState::AwaitingVisualReview
                    && checkpoint.task_payload_sha256 == task_payload_sha256 => {
                resumed_author_execution(checkpoint, self.runner.budget.as_ref())?
            }
            Some(_) => {
                return Err(failure(
                    "E005_R2_CHECKPOINT_NOT_RESUMABLE",
                    "The persisted E005 visual-review checkpoint is completed, inconsistent or requires reconciliation.",
                    false,
                    None,
                ))
            }
            None => {
                let author = self
                    .runner
                    .execute_call(
                        E005ProviderCallInput {
                            authorization_id: request.authorization_id.clone(),
                            task_id: request.task_id.clone(),
                            task_payload: request.task_payload.clone(),
                            call_kind: E005ProviderCallKind::Author,
                            patch_base_source: None,
                            failed_gate: None,
                        },
                        cancellation.child_token(),
                    )
                    .await?;
                self.checkpoints
                    .checkpoint_author(
                        &author.budget_evidence,
                        &author.provider_usage,
                        &author.final_source,
                    )
                    .map_err(core_failure_after_provider)?;
                author
            }
        };
        self.runner
            .verify_budget_evidence(&author.budget_evidence)?;
        if author.verification.verdict != E005ProviderVerificationVerdict::Repairable
            || author
                .verification
                .failed_gate
                .as_ref()
                .and_then(|gate| gate.get("gate_report_id"))
                .and_then(Value::as_str)
                != Some("gate_e005_r2_awaiting_visual_review")
        {
            return Err(verifier_failure("E005_R2_PENDING_VISUAL_GATE_MISSING"));
        }

        visual_request.authorization_id = Some(request.authorization_id.clone());
        visual_request.source = author.final_source.clone();
        let prepared_review = self
            .review
            .prepare(visual_request, cancellation.child_token())
            .await
            .map_err(|error| failure(&error.code, &error.message, error.network_call_made, None))?;
        let initial_source_sha256 = prepared_review.initial_source_sha256().to_owned();
        if author.verification.source_program_sha256.as_deref()
            != Some(initial_source_sha256.as_str())
        {
            return Err(verifier_failure("E005_R2_AUTHOR_REVIEW_SOURCE_MISMATCH"));
        }
        let comparison_input_sha256 = prepared_review
            .comparison_input_sha256()
            .map_err(core_failure_without_evidence)?;
        let prepared_provider = self
            .visual_provider
            .prepare_e005_visual_review(prepared_review.provider_request().clone())
            .map_err(|error| failure(error.code, &error.message, error.network_call_made, None))?;
        let visual = self
            .runner
            .execute_prepared_visual_call(
                E005FormalVisualCallInput {
                    authorization_id: request.authorization_id.clone(),
                    task_id: request.task_id.clone(),
                    task_payload: request.task_payload.clone(),
                    initial_source_sha256,
                    comparison_input_sha256,
                },
                &self.review,
                prepared_review,
                prepared_provider,
                cancellation.child_token(),
            )
            .await?;
        self.runner
            .verify_budget_evidence(&visual.budget_evidence)?;
        let visual_review_evidence = E005VisualReviewEvidenceV1::from_result(&visual.review)
            .map_err(core_failure_after_provider)?;
        let visual_review_evidence_sha256 =
            semantic_sha256(&visual_review_evidence).map_err(core_failure_after_provider)?;
        self.checkpoints
            .complete_visual(&visual.budget_evidence, &visual_review_evidence_sha256)
            .map_err(core_failure_after_provider)?;
        Ok(E005FormalR2TaskExecution {
            request,
            author,
            visual,
            elapsed_ms: bounded_elapsed_ms(started),
        })
    }

    pub fn seal_receipt(
        &self,
        execution: E005FormalR2TaskExecution,
    ) -> Result<E005RunReceipt, E005ProviderRunnerFailure> {
        let (authorization, authorization_sha256) = self
            .runner
            .authorization_snapshot(&execution.request.authorization_id)?;
        if authorization.task_set_sha256 != E005_TASK_SET_SHA256 {
            return Err(verifier_failure(
                "E005_R2_FORMAL_AUTHORIZATION_SCOPE_MISMATCH",
            ));
        }
        let evidence = execution
            .budget_evidence()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        for item in &evidence {
            self.runner.verify_budget_evidence(item)?;
        }
        let task_payload_sha256 = semantic_sha256(&execution.request.task_payload)
            .map_err(core_failure_after_provider)?;
        let evidence_sha256 = semantic_sha256(&evidence).map_err(core_failure_after_provider)?;
        let visual_review_evidence =
            E005VisualReviewEvidenceV1::from_result(&execution.visual.review)
                .map_err(core_failure_after_provider)?;
        let visual_review_evidence_sha256 =
            semantic_sha256(&visual_review_evidence).map_err(core_failure_after_provider)?;
        let request_sha256 = semantic_sha256(&json!({
            "schema_version": "E005R2FormalTaskReceiptRequest@1",
            "authorization_sha256": authorization_sha256,
            "task_set_sha256": E005_TASK_SET_SHA256,
            "task_id": execution.request.task_id,
            "task_payload_sha256": task_payload_sha256,
            "author_request_sha256": execution.author.request_sha256,
            "visual_request_sha256": execution.visual.request_sha256,
            "provider_call_evidence_sha256": evidence_sha256,
            "visual_review_evidence_sha256": visual_review_evidence_sha256,
        }))
        .map_err(core_failure_after_provider)?;
        let usage = combined_r2_provider_usage(
            &execution.author.provider_usage,
            &execution.visual.provider_usage,
            execution.visual.review.geometry_build_count,
        )?;
        let visual_session = crate::E005VisualSessionV1::from_result(
            &task_payload_sha256,
            &request_sha256,
            &execution.visual.review,
            usage,
        )
        .map_err(core_failure_after_provider)?;
        adapt_r2_formal_result(
            E005_TASK_SET_SHA256.into(),
            execution.request.task_id,
            task_payload_sha256,
            request_sha256,
            authorization.authorization_id,
            authorization_sha256,
            evidence,
            execution.visual.review.final_source.clone(),
            &execution.visual.review.final_geometry,
            visual_session,
            execution.elapsed_ms,
        )
        .map_err(vp204_failure_after_provider)
    }
}

/// Same-process R3 coordinator. It deliberately remains outside main/startup
/// until the completed-visual -> production handoff has its own restart-safe
/// checkpoint; wiring it into the paid 30-task batch before that would risk an
/// accounted Provider response with no sealable production receipt.
pub struct E005FormalR3TaskCoordinator {
    r2: E005FormalR2TaskCoordinator,
    production: E005ProductionReviewCoordinatorV1,
}

impl E005FormalR3TaskCoordinator {
    pub fn new(
        r2: E005FormalR2TaskCoordinator,
        production: E005ProductionReviewCoordinatorV1,
    ) -> Self {
        Self { r2, production }
    }

    pub async fn execute_task(
        &self,
        request: E005FormalTaskRequest,
        visual_request: E005VisualReviewRequestV1,
        cancellation: CancellationToken,
    ) -> Result<E005FormalR3TaskExecution, E005ProviderRunnerFailure> {
        let started = Instant::now();
        let r2 = self
            .r2
            .execute_task(request, visual_request, cancellation.child_token())
            .await?;
        let production = self
            .production
            .execute(&r2.visual.review.final_source, cancellation)
            .await
            .map_err(|error| failure(&error.code, &error.message, false, None))?;
        Ok(E005FormalR3TaskExecution {
            r2,
            production,
            elapsed_ms: bounded_elapsed_ms(started),
        })
    }

    pub fn seal_receipt(
        &self,
        execution: E005FormalR3TaskExecution,
    ) -> Result<E005RunReceipt, E005ProviderRunnerFailure> {
        let E005FormalR3TaskExecution {
            r2,
            production,
            elapsed_ms,
        } = execution;
        let production_review = production.review.clone();
        let receipt = self.r2.seal_receipt(r2)?;
        upgrade_r3_formal_receipt(receipt, production_review, &production.geometry, elapsed_ms)
            .map_err(vp204_failure_after_provider)
    }
}

#[derive(Debug, Clone)]
pub struct E005FormalR3TaskExecution {
    pub r2: E005FormalR2TaskExecution,
    pub production: E005ProductionReviewResultV1,
    pub elapsed_ms: u64,
}

fn combined_r2_provider_usage(
    author: &ProviderUsage,
    visual: &ProviderUsage,
    geometry_build_count: u8,
) -> Result<VisualProgramUsageV2, E005ProviderRunnerFailure> {
    fn add(left: u64, right: u64) -> Result<u64, E005ProviderRunnerFailure> {
        left.checked_add(right)
            .ok_or_else(|| verifier_failure("E005_R2_PROVIDER_USAGE_OVERFLOW"))
    }
    Ok(VisualProgramUsageV2 {
        provider_requests: 2,
        product_tool_calls: 1_u16 + u16::from(geometry_build_count),
        input_tokens: add(author.input_tokens, visual.input_tokens)?,
        output_tokens: add(author.output_tokens, visual.output_tokens)?,
        prompt_cache_hit_tokens: add(
            author.prompt_cache_hit_tokens,
            visual.prompt_cache_hit_tokens,
        )?,
        prompt_cache_miss_tokens: add(
            author.prompt_cache_miss_tokens,
            visual.prompt_cache_miss_tokens,
        )?,
        estimated_cost_microusd: add(
            author.estimated_cost_microusd,
            visual.estimated_cost_microusd,
        )?,
    })
}

pub struct E005FormalTaskExecution {
    pub request: E005FormalTaskRequest,
    pub author: E005ProviderCallExecution,
    pub patch: Option<E005ProviderCallExecution>,
    pub vp204_result: Vp204RuntimeResult,
    pub elapsed_ms: u64,
}

impl E005FormalTaskExecution {
    pub fn network_provider_calls(&self) -> u8 {
        1 + u8::from(self.patch.is_some())
    }

    pub fn budget_evidence(&self) -> Vec<&E005ProviderBudgetEvidence> {
        std::iter::once(&self.author.budget_evidence)
            .chain(self.patch.as_ref().map(|patch| &patch.budget_evidence))
            .collect()
    }
}

/// One-task formal coordinator. It performs one author call and only if the
/// exact VP204 gate is repairable, one typed patch call. There is no retry or
/// fallback author path.
pub struct E005FormalTaskCoordinator {
    runner: E005FormalProviderRunner,
    verifier: Arc<E005Vp204OutputVerifier>,
}

impl E005FormalTaskCoordinator {
    pub fn bootstrap<F>(
        budget: Arc<dyn E005ProviderBudgetPort>,
        geometry: Arc<dyn RestrictedGeometryPort>,
        provider_factory: F,
    ) -> Result<Self, E005ProviderRunnerFailure>
    where
        F: FnOnce() -> Result<Arc<dyn ProviderClient>, ProviderError>,
    {
        let verifier = Arc::new(E005Vp204OutputVerifier::new(geometry));
        let runner = E005FormalProviderRunner::bootstrap(
            budget,
            verifier.clone() as Arc<dyn E005ProviderOutputVerifier>,
            provider_factory,
        )?;
        Ok(Self { runner, verifier })
    }

    pub fn startup_recovery(&self) -> &[E005ProviderBudgetEvidence] {
        self.runner.startup_recovery()
    }

    pub async fn execute_task(
        &self,
        request: E005FormalTaskRequest,
        cancellation: CancellationToken,
    ) -> Result<E005FormalTaskExecution, E005ProviderRunnerFailure> {
        let started = Instant::now();
        let author = match self
            .runner
            .execute_call(
                E005ProviderCallInput {
                    authorization_id: request.authorization_id.clone(),
                    task_id: request.task_id.clone(),
                    task_payload: request.task_payload.clone(),
                    call_kind: E005ProviderCallKind::Author,
                    patch_base_source: None,
                    failed_gate: None,
                },
                cancellation.child_token(),
            )
            .await
        {
            Ok(author) => author,
            Err(error) => {
                let _ = self.verifier.discard_task(&request.task_id);
                return Err(error);
            }
        };
        self.runner
            .verify_budget_evidence(&author.budget_evidence)?;

        let patch = if author.verification.verdict == E005ProviderVerificationVerdict::Repairable {
            if author
                .final_source
                .get("schema_version")
                .and_then(Value::as_str)
                == Some("ForgeVisualAuthorSource@1")
            {
                let _ = self.verifier.discard_task(&request.task_id);
                return Err(failure(
                    "E005_R1_VISUAL_PATCH_REQUIRED",
                    "The unified author reached a repairable visual gate, but R2 hash-bound visual patching is not active; no second Provider call was dispatched.",
                    true,
                    None,
                ));
            }
            let failed_gate =
                author.verification.failed_gate.clone().ok_or_else(|| {
                    verifier_failure("E005_FORMAL_REPAIRABLE_GATE_EVIDENCE_MISSING")
                })?;
            let patch = match self
                .runner
                .execute_call(
                    E005ProviderCallInput {
                        authorization_id: request.authorization_id.clone(),
                        task_id: request.task_id.clone(),
                        task_payload: request.task_payload.clone(),
                        call_kind: E005ProviderCallKind::Patch,
                        patch_base_source: Some(author.final_source.clone()),
                        failed_gate: Some(failed_gate),
                    },
                    cancellation.child_token(),
                )
                .await
            {
                Ok(patch) => patch,
                Err(error) => {
                    let _ = self.verifier.discard_task(&request.task_id);
                    return Err(error);
                }
            };
            self.runner.verify_budget_evidence(&patch.budget_evidence)?;
            if patch.verification.verdict == E005ProviderVerificationVerdict::Repairable {
                let _ = self.verifier.discard_task(&request.task_id);
                return Err(verifier_failure("E005_FORMAL_SECOND_PATCH_FORBIDDEN"));
            }
            Some(patch)
        } else {
            None
        };
        let vp204_result = self
            .verifier
            .take_completed_result(&request.task_id)?
            .ok_or_else(|| verifier_failure("E005_FORMAL_VP204_RESULT_MISSING"))?;
        Ok(E005FormalTaskExecution {
            request,
            author,
            patch,
            vp204_result,
            elapsed_ms: bounded_elapsed_ms(started),
        })
    }

    /// Seals one formal receipt only after every exported Provider evidence
    /// object has been reloaded and matched by the Rust-owned budget ledger.
    pub fn seal_receipt(
        &self,
        execution: E005FormalTaskExecution,
    ) -> Result<E005RunReceipt, E005ProviderRunnerFailure> {
        let network_provider_calls = execution.network_provider_calls();
        let (authorization, authorization_sha256) = self
            .runner
            .authorization_snapshot(&execution.request.authorization_id)?;
        if authorization.task_set_sha256 != E005_TASK_SET_SHA256
            || authorization.authorization_id != execution.request.authorization_id
        {
            return Err(verifier_failure("E005_FORMAL_AUTHORIZATION_SCOPE_MISMATCH"));
        }
        let evidence = execution
            .budget_evidence()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        for item in &evidence {
            self.runner.verify_budget_evidence(item)?;
        }
        let evidence_sha256 = semantic_sha256(&evidence).map_err(core_failure_after_provider)?;
        let task_payload_sha256 = semantic_sha256(&execution.request.task_payload)
            .map_err(core_failure_after_provider)?;
        let request_sha256 = semantic_sha256(&json!({
            "schema_version": "E005FormalTaskReceiptRequest@1",
            "authorization_sha256": &authorization_sha256,
            "task_set_sha256": E005_TASK_SET_SHA256,
            "task_id": &execution.request.task_id,
            "task_payload_sha256": &task_payload_sha256,
            "author_request_sha256": &execution.author.request_sha256,
            "patch_request_sha256": execution.patch.as_ref().map(|patch| patch.request_sha256.as_str()),
            "provider_call_evidence_sha256": &evidence_sha256,
        }))
        .map_err(core_failure_after_provider)?;
        let mut receipt = adapt_result(
            E005OfflineHarnessRequest {
                task_set_sha256: E005_TASK_SET_SHA256.into(),
                task_id: execution.request.task_id,
                task_payload: execution.request.task_payload,
                source: Some(execution.author.final_source),
                patch: execution
                    .patch
                    .as_ref()
                    .map(|patch| patch.authored_value.clone()),
            },
            task_payload_sha256,
            request_sha256,
            execution.vp204_result,
            execution.elapsed_ms,
        )
        .map_err(vp204_failure_after_provider)?;
        receipt.run_mode = "formal_provider".into();
        receipt.distribution_eligible = true;
        receipt.author_source_mode = "provider_authored_v2".into();
        receipt.provider_authorization_id = Some(authorization.authorization_id);
        receipt.provider_authorization_sha256 = Some(authorization_sha256);
        receipt.provider_call_evidence = Some(evidence);
        receipt.provider_call_evidence_sha256 = Some(evidence_sha256);
        receipt.network_provider_calls = network_provider_calls;
        receipt.billable_cost_microusd = receipt
            .usage
            .as_ref()
            .map(|usage| usage.estimated_cost_microusd)
            .unwrap_or_default();
        receipt.human_review_status = if matches!(
            receipt.status,
            E005RunStatus::PassedWithoutPatch | E005RunStatus::PassedAfterPatch
        ) {
            E005HumanReviewStatus::Pending
        } else {
            E005HumanReviewStatus::NotRun
        };
        receipt.validate().map_err(vp204_failure_after_provider)?;
        Ok(receipt)
    }
}

impl E005FormalProviderRunner {
    /// Recovery is deliberately completed before the Provider factory runs.
    /// A failed recovery therefore cannot read credentials, construct an HTTP
    /// client, run a health check, or expose a formal dispatch path.
    pub fn bootstrap<F>(
        budget: Arc<dyn E005ProviderBudgetPort>,
        verifier: Arc<dyn E005ProviderOutputVerifier>,
        provider_factory: F,
    ) -> Result<Self, E005ProviderRunnerFailure>
    where
        F: FnOnce() -> Result<Arc<dyn ProviderClient>, ProviderError>,
    {
        let startup_recovery = budget
            .recover_after_restart()
            .map_err(core_failure_without_evidence)?;
        let provider = provider_factory().map_err(provider_failure_without_evidence)?;
        Ok(Self {
            budget,
            provider,
            verifier,
            startup_recovery,
        })
    }

    pub fn startup_recovery(&self) -> &[E005ProviderBudgetEvidence] {
        &self.startup_recovery
    }

    pub fn verify_budget_evidence(
        &self,
        evidence: &E005ProviderBudgetEvidence,
    ) -> Result<(), E005ProviderRunnerFailure> {
        self.budget
            .verify_evidence(evidence)
            .map_err(core_failure_without_evidence)
    }

    pub fn authorization_snapshot(
        &self,
        authorization_id: &str,
    ) -> Result<(E005ProviderRunAuthorizationContract, String), E005ProviderRunnerFailure> {
        let ledger = self
            .budget
            .ledger(authorization_id)
            .map_err(core_failure_without_evidence)?
            .ok_or_else(|| {
                failure(
                    "E005_PROVIDER_AUTHORIZATION_MISSING",
                    "No explicit E005 Provider authorization is active.",
                    false,
                    None,
                )
            })?;
        ledger.validate().map_err(core_failure_without_evidence)?;
        let authorization_sha256 =
            semantic_sha256(&ledger.authorization).map_err(core_failure_without_evidence)?;
        Ok((ledger.authorization, authorization_sha256))
    }

    pub async fn execute_call(
        &self,
        input: E005ProviderCallInput,
        cancellation: CancellationToken,
    ) -> Result<E005ProviderCallExecution, E005ProviderRunnerFailure> {
        validate_call_input(&input)?;
        let ledger = self
            .budget
            .ledger(&input.authorization_id)
            .map_err(core_failure_without_evidence)?
            .ok_or_else(|| {
                failure(
                    "E005_PROVIDER_AUTHORIZATION_MISSING",
                    "No explicit E005 Provider authorization is active.",
                    false,
                    None,
                )
            })?;
        ledger.validate().map_err(core_failure_without_evidence)?;
        let authorization = &ledger.authorization;
        if ledger.status != "authorized" {
            return Err(failure(
                "E005_PROVIDER_AUTHORIZATION_INACTIVE",
                "The E005 Provider authorization is not active.",
                false,
                None,
            ));
        }
        let source_policy_sha256 = e005_provider_source_policy_sha256()?;
        if authorization.source_policy_sha256.as_deref() != Some(&source_policy_sha256) {
            return Err(failure(
                "E005_PROVIDER_SOURCE_POLICY_MISMATCH",
                "The authorized E005 source policy does not match the code-owned prompt and schemas.",
                false,
                None,
            ));
        }
        let provider_id = authorization.provider_id.clone().ok_or_else(|| {
            failure(
                "E005_PROVIDER_AUTHORIZATION_INVALID",
                "The authorized Provider is missing.",
                false,
                None,
            )
        })?;
        let model_id = authorization.model_id.clone().ok_or_else(|| {
            failure(
                "E005_PROVIDER_AUTHORIZATION_INVALID",
                "The authorized model is missing.",
                false,
                None,
            )
        })?;
        if cancellation.is_cancelled() {
            return Err(failure(
                "E005_PROVIDER_CANCELLED_BEFORE_PREPARE",
                "E005 Provider execution was cancelled before request preparation.",
                false,
                None,
            ));
        }

        let provider = match self.provider.turn_session() {
            Ok(Some(session)) => session,
            Ok(None) => self.provider.clone(),
            Err(error) => return Err(provider_failure_without_evidence(error)),
        };
        let request = build_provider_request(&input, &provider_id, &model_id)?;
        let max_output_tokens = request.max_output_tokens;
        let prepared = provider
            .prepare_request(request)
            .map_err(provider_failure_without_evidence)?;
        let commitment = prepared.commitment().clone();
        if authorization.pricing_snapshot_sha256.as_deref()
            != Some(&commitment.pricing_snapshot_sha256)
        {
            return Err(failure(
                "E005_PROVIDER_PRICING_MISMATCH",
                "The prepared Provider pricing snapshot does not match the explicit authorization.",
                false,
                None,
            ));
        }

        let reserved_cost_ceiling_microusd = commitment
            .budget_policy
            .input_cost_ceiling_microusd
            .checked_add(
                commitment
                    .budget_policy
                    .output_cost_ceiling_microusd(max_output_tokens),
            )
            .ok_or_else(|| {
                failure(
                    "E005_PROVIDER_COST_OVERFLOW",
                    "The Provider cost ceiling overflowed.",
                    false,
                    None,
                )
            })?;
        let task_payload_sha256 =
            semantic_sha256(&input.task_payload).map_err(core_failure_without_evidence)?;
        let (patch_base_source_sha256, failed_gate_sha256) = patch_lineage(&input)?;
        let reservation = self
            .budget
            .reserve(&E005ProviderCallReservationRequest {
                authorization_id: input.authorization_id.clone(),
                authorization_binding_sha256: authorization.authorization_binding_sha256.clone(),
                provider_id,
                model_id,
                task_id: input.task_id.clone(),
                task_payload_sha256,
                call_kind: input.call_kind.clone(),
                request_sha256: commitment.request_sha256.clone(),
                patch_base_source_sha256,
                failed_gate_sha256,
                reserved_input_tokens: commitment.budget_policy.input_tokens_upper_bound,
                reserved_output_tokens: max_output_tokens,
                reserved_cost_ceiling_microusd,
            })
            .map_err(core_failure_without_evidence)?;

        if cancellation.is_cancelled() {
            let evidence = self.settle(
                &reservation,
                E005ProviderCallOutcome::PreDispatchReleased,
                None,
                None,
            )?;
            return Err(failure(
                "E005_PROVIDER_CANCELLED_BEFORE_DISPATCH",
                "E005 Provider execution was cancelled before network dispatch.",
                false,
                Some(evidence),
            ));
        }

        if let Err(error) = self.budget.mark_dispatching(&reservation.reservation_id) {
            let evidence = self
                .settle(
                    &reservation,
                    E005ProviderCallOutcome::PreDispatchReleased,
                    None,
                    None,
                )
                .ok();
            return Err(failure(
                error.code(),
                "E005 Provider dispatch permit could not be acquired.",
                false,
                evidence,
            ));
        }

        let dispatch_cancellation = cancellation.child_token();
        let dispatch = prepared.dispatch(
            reservation.reservation_id.clone(),
            dispatch_cancellation.clone(),
            Box::new(|_| {}),
        );
        let remaining = reservation_remaining(&reservation);
        let response = match tokio::time::timeout(remaining, dispatch).await {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => {
                let outcome = provider_error_outcome(&error);
                let evidence = self.settle(&reservation, outcome, None, None)?;
                return Err(failure(&error.code, &error.message, true, Some(evidence)));
            }
            Err(_) => {
                dispatch_cancellation.cancel();
                let evidence = self.settle(
                    &reservation,
                    E005ProviderCallOutcome::ProviderTimeout,
                    None,
                    None,
                )?;
                return Err(failure(
                    "E005_PROVIDER_TIMEOUT",
                    "The prepared E005 Provider call reached its authorized deadline.",
                    true,
                    Some(evidence),
                ));
            }
        };
        let response = match response.validate() {
            Ok(response) => response,
            Err(error) => {
                let evidence = self.settle(
                    &reservation,
                    E005ProviderCallOutcome::ProviderCompletedFailed,
                    None,
                    None,
                )?;
                return Err(failure(&error.code, &error.message, true, Some(evidence)));
            }
        };
        if !response.network_call_made || usage_exceeds(&response.usage, &reservation) {
            let evidence = self.settle(
                &reservation,
                E005ProviderCallOutcome::ProviderCompletedFailed,
                None,
                None,
            )?;
            return Err(failure(
                "E005_PROVIDER_USAGE_BOUND_EXCEEDED",
                "Provider usage or network truth exceeded the prepared reservation.",
                true,
                Some(evidence),
            ));
        }

        let provider_usage = response.usage.clone();
        let authored = match validate_authored_output(&input, response) {
            Ok(output) => output,
            Err(error) => {
                let evidence = self.settle(
                    &reservation,
                    E005ProviderCallOutcome::ProviderCompletedFailed,
                    None,
                    None,
                )?;
                return Err(failure(&error.code, &error.message, true, Some(evidence)));
            }
        };
        if let Err(error) =
            validate_source_operation_allowlist(&input.task_payload, &authored.final_source)
        {
            let evidence = self.settle(
                &reservation,
                E005ProviderCallOutcome::ProviderCompletedFailed,
                None,
                None,
            )?;
            return Err(failure(&error.code, &error.message, true, Some(evidence)));
        }
        let expected_source_sha256 = authored.final_source_sha256.clone();
        let authored_value = authored.authored_value.clone();
        let final_source = authored.final_source.clone();
        let verification = match self
            .verifier
            .verify(authored, cancellation.child_token())
            .await
        {
            Ok(verification) => verification,
            Err(mut error) => {
                let evidence = self.settle(
                    &reservation,
                    E005ProviderCallOutcome::ProviderCompletedFailed,
                    None,
                    None,
                )?;
                error.network_call_made = true;
                error.budget_evidence = Some(evidence);
                return Err(error);
            }
        };
        if let Err(mut error) = validate_verification(&verification, &expected_source_sha256) {
            let evidence = self.settle(
                &reservation,
                E005ProviderCallOutcome::ProviderCompletedFailed,
                None,
                None,
            )?;
            error.network_call_made = true;
            error.budget_evidence = Some(evidence);
            return Err(error);
        }
        let outcome = match verification.verdict {
            E005ProviderVerificationVerdict::Passed => {
                E005ProviderCallOutcome::ProviderCompletedPassed
            }
            E005ProviderVerificationVerdict::Repairable => {
                E005ProviderCallOutcome::ProviderCompletedRepairable
            }
            E005ProviderVerificationVerdict::Failed => {
                E005ProviderCallOutcome::ProviderCompletedFailed
            }
        };
        let evidence = self.settle(
            &reservation,
            outcome,
            verification.source_program_sha256.clone(),
            verification.gate_sha256.clone(),
        )?;
        Ok(E005ProviderCallExecution {
            task_id: input.task_id,
            call_kind: input.call_kind,
            request_sha256: commitment.request_sha256,
            pricing_snapshot_sha256: commitment.pricing_snapshot_sha256,
            provider_usage,
            authored_value,
            final_source,
            verification,
            budget_evidence: evidence,
        })
    }

    /// Executes the only E005-R2 visual-model call under the existing 0045
    /// Patch allowance. The exact multimodal request is already immutable;
    /// this method only verifies authorization, reserves, dispatches once,
    /// completes Rust review/rebuild, and settles conservative evidence.
    pub async fn execute_prepared_visual_call(
        &self,
        input: E005FormalVisualCallInput,
        review_coordinator: &E005VisualReviewCoordinatorV1,
        prepared_review: E005PreparedVisualReviewV1,
        prepared_provider: PreparedE005VisualReviewProviderRequest,
        cancellation: CancellationToken,
    ) -> Result<E005FormalVisualCallExecution, E005ProviderRunnerFailure> {
        if input.authorization_id.is_empty()
            || input.task_id.is_empty()
            || input.task_payload.get("task_id").and_then(Value::as_str)
                != Some(input.task_id.as_str())
            || !valid_sha256(&input.initial_source_sha256)
            || !valid_sha256(&input.comparison_input_sha256)
            || prepared_review.initial_source_sha256() != input.initial_source_sha256
            || prepared_review
                .comparison_input_sha256()
                .map_err(core_failure_without_evidence)?
                != input.comparison_input_sha256
            || prepared_provider.comparison_input_sha256() != input.comparison_input_sha256
        {
            return Err(failure(
                "E005_R2_FORMAL_VISUAL_INPUT_INVALID",
                "Formal visual call is not bound to the exact task, source and comparison input.",
                false,
                None,
            ));
        }
        if cancellation.is_cancelled() {
            return Err(failure(
                "E005_R2_VISUAL_CANCELLED_BEFORE_RESERVATION",
                "Formal visual review was cancelled before budget reservation.",
                false,
                None,
            ));
        }

        let ledger = self
            .budget
            .ledger(&input.authorization_id)
            .map_err(core_failure_without_evidence)?
            .ok_or_else(|| {
                failure(
                    "E005_PROVIDER_AUTHORIZATION_MISSING",
                    "No explicit E005 Provider authorization is active.",
                    false,
                    None,
                )
            })?;
        ledger.validate().map_err(core_failure_without_evidence)?;
        if ledger.status != "authorized" {
            return Err(failure(
                "E005_PROVIDER_AUTHORIZATION_INACTIVE",
                "The E005 Provider authorization is not active.",
                false,
                None,
            ));
        }
        let authorization = &ledger.authorization;
        let source_policy_sha256 = e005_provider_source_policy_sha256()?;
        if authorization.source_policy_sha256.as_deref() != Some(source_policy_sha256.as_str()) {
            return Err(failure(
                "E005_PROVIDER_SOURCE_POLICY_MISMATCH",
                "The authorized E005 source policy does not match the code-owned visual contract.",
                false,
                None,
            ));
        }
        let provider_id = authorization.provider_id.as_deref().ok_or_else(|| {
            failure(
                "E005_PROVIDER_AUTHORIZATION_INVALID",
                "The authorized Provider is missing.",
                false,
                None,
            )
        })?;
        let model_id = authorization.model_id.as_deref().ok_or_else(|| {
            failure(
                "E005_PROVIDER_AUTHORIZATION_INVALID",
                "The authorized model is missing.",
                false,
                None,
            )
        })?;
        let commitment = prepared_provider.commitment().clone();
        if prepared_provider.provider_id() != provider_id
            || prepared_provider.model_id() != model_id
            || authorization.pricing_snapshot_sha256.as_deref()
                != Some(commitment.pricing_snapshot_sha256.as_str())
        {
            return Err(failure(
                "E005_R2_VISUAL_AUTHORIZATION_MISMATCH",
                "Prepared visual Provider, model or pricing does not match the explicit authorization.",
                false,
                None,
            ));
        }
        let reserved_cost_ceiling_microusd = commitment
            .budget_policy
            .input_cost_ceiling_microusd
            .checked_add(
                commitment
                    .budget_policy
                    .output_cost_ceiling_microusd(prepared_provider.max_output_tokens()),
            )
            .ok_or_else(|| {
                failure(
                    "E005_PROVIDER_COST_OVERFLOW",
                    "The visual Provider cost ceiling overflowed.",
                    false,
                    None,
                )
            })?;
        let task_payload_sha256 =
            semantic_sha256(&input.task_payload).map_err(core_failure_without_evidence)?;
        let reservation = self
            .budget
            .reserve(&E005ProviderCallReservationRequest {
                authorization_id: input.authorization_id.clone(),
                authorization_binding_sha256: authorization.authorization_binding_sha256.clone(),
                provider_id: provider_id.into(),
                model_id: model_id.into(),
                task_id: input.task_id.clone(),
                task_payload_sha256,
                call_kind: E005ProviderCallKind::Patch,
                request_sha256: commitment.request_sha256.clone(),
                patch_base_source_sha256: Some(input.initial_source_sha256.clone()),
                failed_gate_sha256: Some(input.comparison_input_sha256.clone()),
                reserved_input_tokens: commitment.budget_policy.input_tokens_upper_bound,
                reserved_output_tokens: prepared_provider.max_output_tokens(),
                reserved_cost_ceiling_microusd,
            })
            .map_err(core_failure_without_evidence)?;

        if cancellation.is_cancelled() {
            let evidence = self.settle(
                &reservation,
                E005ProviderCallOutcome::PreDispatchReleased,
                None,
                None,
            )?;
            return Err(failure(
                "E005_R2_VISUAL_CANCELLED_BEFORE_DISPATCH",
                "Formal visual review was cancelled before network dispatch.",
                false,
                Some(evidence),
            ));
        }
        if let Err(error) = self.budget.mark_dispatching(&reservation.reservation_id) {
            let evidence = self
                .settle(
                    &reservation,
                    E005ProviderCallOutcome::PreDispatchReleased,
                    None,
                    None,
                )
                .ok();
            return Err(failure(
                error.code(),
                "The formal visual dispatch permit could not be acquired.",
                false,
                evidence,
            ));
        }

        let dispatch_cancellation = cancellation.child_token();
        let dispatch = prepared_provider.dispatch(
            reservation.reservation_id.clone(),
            dispatch_cancellation.clone(),
        );
        let response =
            match tokio::time::timeout(reservation_remaining(&reservation), dispatch).await {
                Ok(Ok(response)) => response,
                Ok(Err(error)) => {
                    let outcome = if error.code.contains("CANCELLED") {
                        E005ProviderCallOutcome::ProviderCancelled
                    } else if error.code.contains("TIMEOUT") {
                        E005ProviderCallOutcome::ProviderTimeout
                    } else {
                        E005ProviderCallOutcome::ProviderTransportFailed
                    };
                    let evidence = self.settle(&reservation, outcome, None, None)?;
                    return Err(failure(error.code, &error.message, true, Some(evidence)));
                }
                Err(_) => {
                    dispatch_cancellation.cancel();
                    let evidence = self.settle(
                        &reservation,
                        E005ProviderCallOutcome::ProviderTimeout,
                        None,
                        None,
                    )?;
                    return Err(failure(
                        "E005_R2_VISUAL_PROVIDER_TIMEOUT",
                        "The prepared visual Provider call reached its authorized deadline.",
                        true,
                        Some(evidence),
                    ));
                }
            };
        if let Err(error) = response.usage.validate(true) {
            let evidence = self.settle(
                &reservation,
                E005ProviderCallOutcome::ProviderCompletedFailed,
                None,
                None,
            )?;
            return Err(failure(&error.code, &error.message, true, Some(evidence)));
        }
        if !response.output.network_call_made
            || response.output.budget_evidence.is_some()
            || response.output.provider_id != provider_id
            || response.output.model_id != model_id
            || usage_exceeds(&response.usage, &reservation)
        {
            let evidence = self.settle(
                &reservation,
                E005ProviderCallOutcome::ProviderCompletedFailed,
                None,
                None,
            )?;
            return Err(failure(
                "E005_R2_VISUAL_PROVIDER_OUTPUT_INVALID",
                "Visual Provider identity, usage, network truth or budget ownership is invalid.",
                true,
                Some(evidence),
            ));
        }
        if cancellation.is_cancelled() {
            let evidence = self.settle(
                &reservation,
                E005ProviderCallOutcome::ProviderCancelled,
                None,
                None,
            )?;
            return Err(failure(
                "E005_R2_VISUAL_CANCELLED_AFTER_DISPATCH",
                "Late visual Provider output was discarded after cancellation.",
                true,
                Some(evidence),
            ));
        }
        let provider_usage = response.usage;
        let review = match review_coordinator
            .complete_prepared_output(prepared_review, response.output, cancellation.child_token())
            .await
        {
            Ok(review) => review,
            Err(error) => {
                let evidence = self.settle(
                    &reservation,
                    E005ProviderCallOutcome::ProviderCompletedFailed,
                    None,
                    None,
                )?;
                return Err(failure(&error.code, &error.message, true, Some(evidence)));
            }
        };
        let evidence = self.settle(
            &reservation,
            E005ProviderCallOutcome::ProviderCompletedPassed,
            Some(review.final_source_sha256.clone()),
            Some(review.comparison_report.report_sha256.clone()),
        )?;
        Ok(E005FormalVisualCallExecution {
            task_id: input.task_id,
            request_sha256: commitment.request_sha256,
            pricing_snapshot_sha256: commitment.pricing_snapshot_sha256,
            provider_usage,
            review,
            budget_evidence: evidence,
        })
    }

    fn settle(
        &self,
        reservation: &E005ProviderCallReservation,
        outcome: E005ProviderCallOutcome,
        output_source_sha256: Option<String>,
        output_gate_sha256: Option<String>,
    ) -> Result<E005ProviderBudgetEvidence, E005ProviderRunnerFailure> {
        self.budget
            .settle(
                &reservation.reservation_id,
                &E005ProviderCallSettlement {
                    outcome,
                    output_source_sha256,
                    output_gate_sha256,
                },
            )
            .map_err(core_failure_without_evidence)
    }
}

pub fn e005_provider_source_policy_sha256() -> Result<String, E005ProviderRunnerFailure> {
    let author_schema = bundled_author_schema()?;
    let patch_schema: Value = serde_json::from_str(PATCH_SCHEMA_TEXT).map_err(|_| {
        failure(
            "E005_PROVIDER_SCHEMA_INVALID",
            "The code-owned E005 patch schema could not be parsed.",
            false,
            None,
        )
    })?;
    let visual_patch_proposal_schema: Value =
        serde_json::from_str(E005_VISUAL_PATCH_PROPOSAL_SCHEMA_TEXT).map_err(|_| {
            failure(
                "E005_PROVIDER_SCHEMA_INVALID",
                "The code-owned E005 visual patch proposal schema could not be parsed.",
                false,
                None,
            )
        })?;
    let visual_patch_schema: Value =
        serde_json::from_str(E005_VISUAL_PATCH_SCHEMA_TEXT).map_err(|_| {
            failure(
                "E005_PROVIDER_SCHEMA_INVALID",
                "The code-owned E005 sealed visual patch schema could not be parsed.",
                false,
                None,
            )
        })?;
    semantic_sha256(&json!({
        "schema_version": "E005ProviderSourcePolicy@1",
        "task_set_sha256": E005_FORMAL_TASK_SET_SHA256,
        "source_contract_id": "ForgeVisualAuthorSource@1",
        "compiler_profile_id": "forgecad-core-e005-r1.1",
        "id_algorithm_version": "author-instance-hash-v1",
        "author": {
            "system_prompt": E005_AUTHOR_SYSTEM_PROMPT,
            "tool_name": E005_AUTHOR_TOOL_NAME,
            "tool_schema": author_schema,
            "max_output_tokens": E005_AUTHOR_MAX_OUTPUT_TOKENS,
        },
        "patch": {
            "system_prompt": E005_PATCH_SYSTEM_PROMPT,
            "tool_name": E005_PATCH_TOOL_NAME,
            "tool_schema": patch_schema,
            "max_output_tokens": E005_PATCH_MAX_OUTPUT_TOKENS,
        },
        "visual_review": {
            "system_prompt": E005_VISUAL_REVIEW_SYSTEM_PROMPT,
            "proposal_schema": visual_patch_proposal_schema,
            "sealed_patch_schema": visual_patch_schema,
            "candidate_view_profile": "turntable_eight",
            "max_output_tokens": E005_VISUAL_REVIEW_MAX_OUTPUT_TOKENS,
            "budget_owner": "e005_0045_patch",
            "maximum_visual_calls_per_task": 1,
            "maximum_geometry_builds_per_task": 2,
            "second_visual_model_call_permitted": false,
            "patched_result_status": "patched_pending_visual_confirmation",
        },
        "whole_object_template_policy": "forbidden",
        "provider_calls": {"author_per_task": 1, "visual_review_per_task": 1, "maximum_total_per_task": 2},
    }))
    .map_err(core_failure_without_evidence)
}

fn bundled_author_schema() -> Result<Value, E005ProviderRunnerFailure> {
    let mut author_schema: Value = serde_json::from_str(AUTHOR_SCHEMA_TEXT).map_err(|_| {
        failure(
            "E005_PROVIDER_SCHEMA_INVALID",
            "The code-owned E005 author schema could not be parsed.",
            false,
            None,
        )
    })?;
    let geometry_schema: Value =
        serde_json::from_str(GEOMETRY_TEMPLATE_SCHEMA_TEXT).map_err(|_| {
            failure(
                "E005_PROVIDER_SCHEMA_INVALID",
                "The code-owned E005 geometry-template schema could not be parsed.",
                false,
                None,
            )
        })?;
    author_schema["properties"]["geometry_templates"] = geometry_schema;
    Ok(author_schema)
}

fn build_provider_request(
    input: &E005ProviderCallInput,
    provider_id: &str,
    model_id: &str,
) -> Result<ProviderRequest, E005ProviderRunnerFailure> {
    let (system_prompt, tool_name, description, max_output_tokens, payload) = match input.call_kind
    {
        E005ProviderCallKind::Author => (
            E005_AUTHOR_SYSTEM_PROMPT,
            E005_AUTHOR_TOOL_NAME,
            "Submit one complete compact ForgeVisualAuthorSource@1 source.",
            E005_AUTHOR_MAX_OUTPUT_TOKENS,
            json!({
                "schema_version": "E005AuthorRequest@1",
                "task_set_sha256": E005_FORMAL_TASK_SET_SHA256,
                "task": input.task_payload,
            }),
        ),
        E005ProviderCallKind::Patch => (
            E005_PATCH_SYSTEM_PROMPT,
            E005_PATCH_TOOL_NAME,
            "Submit one bounded ForgeVisualGeometryPatch@1 repair.",
            E005_PATCH_MAX_OUTPUT_TOKENS,
            json!({
                "schema_version": "E005PatchRequest@1",
                "task_set_sha256": E005_FORMAL_TASK_SET_SHA256,
                "task": input.task_payload,
                "source": input.patch_base_source,
                "failed_gate": input.failed_gate,
            }),
        ),
    };
    let schema = match input.call_kind {
        E005ProviderCallKind::Author => bundled_author_schema()?,
        E005ProviderCallKind::Patch => serde_json::from_str(PATCH_SCHEMA_TEXT).map_err(|_| {
            failure(
                "E005_PROVIDER_SCHEMA_INVALID",
                "The code-owned E005 patch schema could not be parsed.",
                false,
                None,
            )
        })?,
    };
    let context_digest = semantic_sha256(&json!({
        "policy": e005_provider_source_policy_sha256()?,
        "call_kind": input.call_kind,
        "payload": payload,
    }))
    .map_err(core_failure_without_evidence)?;
    let user_content =
        forgecad_core::canonical_json(&payload).map_err(core_failure_without_evidence)?;
    Ok(ProviderRequest {
        provider_id: provider_id.into(),
        model: model_id.into(),
        context_digest,
        messages: vec![
            ProviderMessage {
                role: ProviderRole::System,
                content: system_prompt.into(),
                tool_call_id: None,
                tool_calls: Vec::new(),
                ephemeral_reasoning: None,
            },
            ProviderMessage {
                role: ProviderRole::User,
                content: user_content,
                tool_call_id: None,
                tool_calls: Vec::new(),
                ephemeral_reasoning: None,
            },
        ],
        tools: vec![ProviderToolDefinition {
            name: tool_name.into(),
            description: description.into(),
            input_schema: schema,
        }],
        require_tool_call: true,
        max_output_tokens,
    })
}

fn validate_call_input(input: &E005ProviderCallInput) -> Result<(), E005ProviderRunnerFailure> {
    if input.authorization_id.is_empty()
        || input.task_id.is_empty()
        || input.task_payload.get("task_id").and_then(Value::as_str) != Some(input.task_id.as_str())
    {
        return Err(failure(
            "E005_PROVIDER_CALL_INPUT_INVALID",
            "E005 Provider call input is not bound to one frozen task.",
            false,
            None,
        ));
    }
    match input.call_kind {
        E005ProviderCallKind::Author
            if input.patch_base_source.is_none() && input.failed_gate.is_none() => {}
        E005ProviderCallKind::Patch
            if input.patch_base_source.is_some() && input.failed_gate.is_some() => {}
        _ => {
            return Err(failure(
                "E005_PROVIDER_PATCH_LINEAGE_INVALID",
                "E005 author and patch inputs do not match their required lineage.",
                false,
                None,
            ))
        }
    }
    Ok(())
}

fn patch_lineage(
    input: &E005ProviderCallInput,
) -> Result<(Option<String>, Option<String>), E005ProviderRunnerFailure> {
    match input.call_kind {
        E005ProviderCallKind::Author => Ok((None, None)),
        E005ProviderCallKind::Patch => Ok((
            Some(
                semantic_sha256(input.patch_base_source.as_ref().expect("validated"))
                    .map_err(core_failure_without_evidence)?,
            ),
            Some(
                semantic_sha256(input.failed_gate.as_ref().expect("validated"))
                    .map_err(core_failure_without_evidence)?,
            ),
        )),
    }
}

fn validate_authored_output(
    input: &E005ProviderCallInput,
    response: ProviderResponse,
) -> Result<E005ProviderAuthoredOutput, ProviderError> {
    if response.finish_reason != ProviderFinishReason::ToolCalls || response.tool_calls.len() != 1 {
        return Err(ProviderError::schema_mismatch(
            "E005 Provider must return exactly one code-owned tool call.",
            true,
        ));
    }
    let call = response
        .tool_calls
        .into_iter()
        .next()
        .expect("exactly one tool call");
    let expected_tool = match input.call_kind {
        E005ProviderCallKind::Author => E005_AUTHOR_TOOL_NAME,
        E005ProviderCallKind::Patch => E005_PATCH_TOOL_NAME,
    };
    if call.name != expected_tool {
        return Err(ProviderError::schema_mismatch(
            "E005 Provider selected a tool outside the code-owned call kind.",
            true,
        ));
    }
    let final_source = match input.call_kind {
        E005ProviderCallKind::Author => {
            let lowering = lower_forge_visual_author_source_v1(&call.arguments).map_err(|_| {
                ProviderError::schema_mismatch(
                    "E005 Provider author output failed local unified R1 lowering.",
                    true,
                )
            })?;
            (call.arguments.clone(), lowering.source_program_sha256)
        }
        E005ProviderCallKind::Patch => {
            let base = input.patch_base_source.as_ref().expect("validated");
            if base.get("schema_version").and_then(Value::as_str)
                == Some("ForgeVisualAuthorSource@1")
            {
                return Err(ProviderError::schema_mismatch(
                    "E005-R1 unified author patches are disabled until the R2 hash-bound visual patch contract is active.",
                    true,
                ));
            }
            let patched =
                apply_forge_visual_geometry_patch_v2(base, &call.arguments).map_err(|_| {
                    ProviderError::schema_mismatch(
                        "E005 Provider patch output failed local typed patch validation.",
                        true,
                    )
                })?;
            (
                patched.patched_program,
                patched.lowering.source_program_sha256,
            )
        }
    };
    let (final_source, final_source_sha256) = final_source;
    Ok(E005ProviderAuthoredOutput {
        task_id: input.task_id.clone(),
        call_kind: input.call_kind.clone(),
        tool_name: call.name,
        authored_value: call.arguments,
        final_source,
        final_source_sha256,
        provider_usage: response.usage,
    })
}

fn validate_verification(
    verification: &E005ProviderVerification,
    expected_source_sha256: &str,
) -> Result<(), E005ProviderRunnerFailure> {
    let evidence_valid = match verification.verdict {
        E005ProviderVerificationVerdict::Passed => {
            verification.source_program_sha256.as_deref() == Some(expected_source_sha256)
                && verification
                    .gate_sha256
                    .as_deref()
                    .is_some_and(valid_sha256)
                && verification.failed_gate.is_none()
        }
        E005ProviderVerificationVerdict::Repairable => {
            verification.source_program_sha256.as_deref() == Some(expected_source_sha256)
                && verification
                    .gate_sha256
                    .as_deref()
                    .is_some_and(valid_sha256)
                && verification
                    .failed_gate
                    .as_ref()
                    .and_then(|gate| semantic_sha256(gate).ok())
                    .as_deref()
                    == verification.gate_sha256.as_deref()
        }
        E005ProviderVerificationVerdict::Failed => {
            verification.source_program_sha256.is_none()
                && verification.gate_sha256.is_none()
                && verification.failed_gate.is_none()
        }
    };
    if !evidence_valid {
        return Err(failure(
            "E005_PROVIDER_VERIFICATION_LINEAGE_INVALID",
            "E005 verifier output is not bound to the exact authored source and gate.",
            true,
            None,
        ));
    }
    Ok(())
}

fn usage_exceeds(usage: &ProviderUsage, reservation: &E005ProviderCallReservation) -> bool {
    usage.input_tokens > reservation.reserved_input_tokens
        || usage.output_tokens > reservation.reserved_output_tokens
        || usage.estimated_cost_microusd > reservation.reserved_cost_ceiling_microusd
}

fn provider_error_outcome(error: &ProviderError) -> E005ProviderCallOutcome {
    match error.category {
        ProviderErrorCategory::Cancelled => E005ProviderCallOutcome::ProviderCancelled,
        ProviderErrorCategory::Timeout => E005ProviderCallOutcome::ProviderTimeout,
        ProviderErrorCategory::Transport
        | ProviderErrorCategory::RateLimited
        | ProviderErrorCategory::ServerUnavailable => {
            E005ProviderCallOutcome::ProviderTransportFailed
        }
        _ => E005ProviderCallOutcome::ProviderCompletedFailed,
    }
}

fn reservation_remaining(reservation: &E005ProviderCallReservation) -> Duration {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let now = i64::try_from(now).unwrap_or(i64::MAX);
    let remaining = reservation.deadline_unix_ms.saturating_sub(now);
    Duration::from_millis(u64::try_from(remaining.max(1)).unwrap_or(1))
}

fn bounded_elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis())
        .unwrap_or(u64::MAX)
        .min(3_600_000)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn provider_failure_without_evidence(error: ProviderError) -> E005ProviderRunnerFailure {
    failure(&error.code, &error.message, error.network_call_made, None)
}

fn core_failure_without_evidence(error: CoreError) -> E005ProviderRunnerFailure {
    failure(
        error.code(),
        "The Rust-owned E005 budget ledger rejected the operation.",
        false,
        None,
    )
}

fn failure(
    code: &str,
    message: &str,
    network_call_made: bool,
    budget_evidence: Option<E005ProviderBudgetEvidence>,
) -> E005ProviderRunnerFailure {
    E005ProviderRunnerFailure {
        code: code.into(),
        message: message.into(),
        network_call_made,
        budget_evidence,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Mutex,
        },
    };

    use forgecad_core::{
        E005ProviderRunAuthorizationContract, VisualClaimStatus, VisualReferenceClaimAssessment,
        VisualReferenceMatchOutcome, E005_MAXIMUM_AUTHOR_CALLS, E005_MAXIMUM_PATCH_CALLS,
        E005_MAXIMUM_TOTAL_CALLS, E005_PROVIDER_BUDGET_EVIDENCE_SCHEMA_VERSION,
        E005_PROVIDER_LEDGER_SCHEMA_VERSION, E005_PROVIDER_RESERVATION_SCHEMA_VERSION,
        E005_PROVIDER_RUN_AUTHORIZATION_SCHEMA_VERSION,
    };

    use crate::{
        e005_offline_harness::tests::RepairableGeometryPort,
        e005_visual_review::tests::{request_fixture, GeometryFixture, VisualFixture},
        E005PreparedVisualReviewProviderResponse, PreparedProviderRequest, ProviderEventSink,
        ProviderFuture, ProviderHealthCheck, ProviderPreflight, ProviderRequestBudgetPolicy,
        ProviderRequestCommitment, ProviderStreamEvent, ProviderToolCall,
        VisualReferenceComparisonCoordinator, VisualReferenceComparisonProviderOutput,
    };

    use super::*;

    const PRICING_SHA256: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    const REQUEST_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const GATE_SHA256: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[derive(Default)]
    struct FakeBudgetState {
        ledger: Option<E005ProviderBudgetLedger>,
        reservation: Option<E005ProviderCallReservation>,
        events: Vec<String>,
        fail_recovery: bool,
    }

    #[derive(Default)]
    struct FakeBudget {
        state: Mutex<FakeBudgetState>,
    }

    #[derive(Default)]
    struct FakeVisualCheckpoint {
        checkpoint: Mutex<Option<E005VisualReviewCheckpoint>>,
        events: Mutex<Vec<String>>,
    }

    impl FakeVisualCheckpoint {
        fn events(&self) -> Vec<String> {
            self.events.lock().unwrap().clone()
        }
    }

    impl E005VisualReviewCheckpointPort for FakeVisualCheckpoint {
        fn recover_after_provider_recovery(
            &self,
        ) -> Result<Vec<E005VisualReviewCheckpoint>, CoreError> {
            self.events.lock().unwrap().push("recover".into());
            Ok(self
                .checkpoint
                .lock()
                .unwrap()
                .clone()
                .into_iter()
                .collect())
        }

        fn checkpoint_author(
            &self,
            evidence: &E005ProviderBudgetEvidence,
            usage: &ProviderUsage,
            source: &Value,
        ) -> Result<E005VisualReviewCheckpoint, CoreError> {
            self.events.lock().unwrap().push("author".into());
            let source_sha256 = lower_forge_visual_author_source_v1(source)?.source_program_sha256;
            let provider_usage = E005ProviderUsageCheckpoint {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                prompt_cache_hit_tokens: usage.prompt_cache_hit_tokens,
                prompt_cache_miss_tokens: usage.prompt_cache_miss_tokens,
                estimated_cost_microusd: usage.estimated_cost_microusd,
            };
            let checkpoint = E005VisualReviewCheckpoint {
                schema_version: forgecad_core::E005_VISUAL_REVIEW_CHECKPOINT_SCHEMA_VERSION.into(),
                authorization_id: evidence.authorization_id.clone(),
                task_id: evidence.task_id.clone(),
                task_payload_sha256: evidence.task_payload_sha256.clone(),
                state: E005VisualReviewCheckpointState::AwaitingVisualReview,
                author_source: source.clone(),
                author_source_sha256: source_sha256,
                author_reservation_id: evidence.reservation_id.clone(),
                author_budget_evidence: evidence.clone(),
                author_budget_evidence_sha256: semantic_sha256(evidence)?,
                author_provider_usage_sha256: semantic_sha256(&provider_usage)?,
                author_provider_usage: provider_usage,
                visual_reservation_id: None,
                visual_budget_evidence_sha256: None,
                visual_review_evidence_sha256: None,
            };
            checkpoint.validate()?;
            *self.checkpoint.lock().unwrap() = Some(checkpoint.clone());
            Ok(checkpoint)
        }

        fn checkpoint(
            &self,
            authorization_id: &str,
            task_id: &str,
        ) -> Result<Option<E005VisualReviewCheckpoint>, CoreError> {
            self.events.lock().unwrap().push("load".into());
            Ok(self.checkpoint.lock().unwrap().clone().filter(|item| {
                item.authorization_id == authorization_id && item.task_id == task_id
            }))
        }

        fn complete_visual(
            &self,
            evidence: &E005ProviderBudgetEvidence,
            visual_review_evidence_sha256: &str,
        ) -> Result<E005VisualReviewCheckpoint, CoreError> {
            self.events.lock().unwrap().push("visual".into());
            let mut state = self.checkpoint.lock().unwrap();
            let checkpoint = state
                .as_mut()
                .ok_or_else(|| CoreError::not_found("test visual checkpoint"))?;
            checkpoint.state = E005VisualReviewCheckpointState::Completed;
            checkpoint.visual_reservation_id = Some(evidence.reservation_id.clone());
            checkpoint.visual_budget_evidence_sha256 = Some(semantic_sha256(evidence)?);
            checkpoint.visual_review_evidence_sha256 = Some(visual_review_evidence_sha256.into());
            checkpoint.validate()?;
            Ok(checkpoint.clone())
        }
    }

    impl FakeBudget {
        fn authorized(pricing_sha256: &str) -> Arc<Self> {
            let budget = Arc::new(Self::default());
            budget.state.lock().unwrap().ledger = Some(ledger(pricing_sha256));
            budget
        }

        fn events(&self) -> Vec<String> {
            self.state.lock().unwrap().events.clone()
        }
    }

    impl E005ProviderBudgetPort for FakeBudget {
        fn recover_after_restart(&self) -> Result<Vec<E005ProviderBudgetEvidence>, CoreError> {
            let mut state = self.state.lock().unwrap();
            state.events.push("recover".into());
            if state.fail_recovery {
                return Err(CoreError::conflict(
                    "E005_TEST_RECOVERY_FAILED",
                    "test recovery failed",
                ));
            }
            Ok(Vec::new())
        }

        fn ledger(
            &self,
            _authorization_id: &str,
        ) -> Result<Option<E005ProviderBudgetLedger>, CoreError> {
            self.state.lock().unwrap().events.push("ledger".into());
            Ok(self.state.lock().unwrap().ledger.clone())
        }

        fn reserve(
            &self,
            request: &E005ProviderCallReservationRequest,
        ) -> Result<E005ProviderCallReservation, CoreError> {
            let now = now_unix_ms();
            let ordinal = self
                .state
                .lock()
                .unwrap()
                .events
                .iter()
                .filter(|event| event.as_str() == "reserve")
                .count()
                + 1;
            let reservation = E005ProviderCallReservation {
                schema_version: E005_PROVIDER_RESERVATION_SCHEMA_VERSION.into(),
                reservation_id: format!("e005_reservation_test_{ordinal:03}"),
                authorization_id: request.authorization_id.clone(),
                authorization_binding_sha256: request.authorization_binding_sha256.clone(),
                task_id: request.task_id.clone(),
                task_payload_sha256: request.task_payload_sha256.clone(),
                call_kind: request.call_kind.clone(),
                call_number: ordinal as u8,
                kind_call_number: 1,
                reservation_ordinal: ordinal as u32,
                request_sha256: request.request_sha256.clone(),
                patch_base_source_sha256: request.patch_base_source_sha256.clone(),
                failed_gate_sha256: request.failed_gate_sha256.clone(),
                reserved_input_tokens: request.reserved_input_tokens,
                reserved_output_tokens: request.reserved_output_tokens,
                reserved_cost_ceiling_microusd: request.reserved_cost_ceiling_microusd,
                deadline_unix_ms: now + 5_000,
                created_at_unix_ms: now,
            };
            let mut state = self.state.lock().unwrap();
            state.events.push("reserve".into());
            state.reservation = Some(reservation.clone());
            Ok(reservation)
        }

        fn mark_dispatching(&self, _reservation_id: &str) -> Result<(), CoreError> {
            self.state.lock().unwrap().events.push("mark".into());
            Ok(())
        }

        fn settle(
            &self,
            _reservation_id: &str,
            settlement: &E005ProviderCallSettlement,
        ) -> Result<E005ProviderBudgetEvidence, CoreError> {
            let mut state = self.state.lock().unwrap();
            state
                .events
                .push(format!("settle:{:?}", settlement.outcome));
            let reservation = state.reservation.clone().expect("reservation exists");
            let is_patch = reservation.call_kind == E005ProviderCallKind::Patch;
            let call_number = reservation.call_number;
            Ok(E005ProviderBudgetEvidence {
                schema_version: E005_PROVIDER_BUDGET_EVIDENCE_SCHEMA_VERSION.into(),
                authorization_id: reservation.authorization_id,
                authorization_binding_sha256: reservation.authorization_binding_sha256,
                reservation_id: reservation.reservation_id,
                task_id: reservation.task_id,
                task_payload_sha256: reservation.task_payload_sha256,
                request_sha256: reservation.request_sha256,
                provider_id: "provider_test".into(),
                model_id: "model_test_v1".into(),
                call_kind: reservation.call_kind,
                call_number: reservation.call_number,
                kind_call_number: reservation.kind_call_number,
                settlement: "accounted".into(),
                network_call_made: settlement.outcome
                    != E005ProviderCallOutcome::PreDispatchReleased,
                outcome_code: serde_json::to_value(settlement.outcome)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .into(),
                output_source_sha256: settlement.output_source_sha256.clone(),
                output_gate_sha256: settlement.output_gate_sha256.clone(),
                reserved_input_tokens: reservation.reserved_input_tokens,
                reserved_output_tokens: reservation.reserved_output_tokens,
                reserved_cost_ceiling_microusd: reservation.reserved_cost_ceiling_microusd,
                author_calls_accounted_after: 1,
                patch_calls_accounted_after: u8::from(is_patch),
                calls_accounted_after: call_number,
                accounted_input_tokens_after: reservation.reserved_input_tokens,
                accounted_output_tokens_after: reservation.reserved_output_tokens,
                accounted_cost_ceiling_microusd_after: reservation.reserved_cost_ceiling_microusd,
                settled_at_unix_ms: now_unix_ms(),
            })
        }

        fn verify_evidence(&self, evidence: &E005ProviderBudgetEvidence) -> Result<(), CoreError> {
            if !evidence
                .reservation_id
                .starts_with("e005_reservation_test_")
            {
                return Err(CoreError::conflict(
                    "E005_TEST_EVIDENCE_MISMATCH",
                    "test evidence does not bind the reservation",
                ));
            }
            Ok(())
        }
    }

    struct FakePreparedProvider {
        commitment: ProviderRequestCommitment,
        responses: Mutex<VecDeque<ProviderResponse>>,
        prepare_count: AtomicUsize,
        dispatch_count: Arc<AtomicUsize>,
        captured_key: Arc<Mutex<Option<String>>>,
        captured_request: Arc<Mutex<Option<ProviderRequest>>>,
        cancel_on_prepare: Option<CancellationToken>,
    }

    impl FakePreparedProvider {
        fn new(response: ProviderResponse) -> Arc<Self> {
            Self::sequence(vec![response])
        }

        fn sequence(responses: Vec<ProviderResponse>) -> Arc<Self> {
            Arc::new(Self {
                commitment: ProviderRequestCommitment {
                    request_sha256: REQUEST_SHA256.into(),
                    pricing_snapshot_sha256: PRICING_SHA256.into(),
                    budget_policy: ProviderRequestBudgetPolicy {
                        input_tokens_upper_bound: 10_000,
                        input_cost_ceiling_microusd: 10_000,
                        output_microusd_per_million_tokens: 1_000_000,
                    },
                },
                responses: Mutex::new(responses.into()),
                prepare_count: AtomicUsize::new(0),
                dispatch_count: Arc::new(AtomicUsize::new(0)),
                captured_key: Arc::new(Mutex::new(None)),
                captured_request: Arc::new(Mutex::new(None)),
                cancel_on_prepare: None,
            })
        }

        fn with_cancel(response: ProviderResponse, cancellation: CancellationToken) -> Arc<Self> {
            let mut provider = Arc::try_unwrap(Self::new(response)).ok().unwrap();
            provider.cancel_on_prepare = Some(cancellation);
            Arc::new(provider)
        }
    }

    impl ProviderClient for FakePreparedProvider {
        fn preflight(&self, _cancellation: CancellationToken) -> ProviderFuture<ProviderPreflight> {
            Box::pin(async {
                Err(ProviderError::schema_mismatch(
                    "preflight is not used by E005 tests",
                    false,
                ))
            })
        }

        fn request_budget_policy(
            &self,
            _request: &ProviderRequest,
        ) -> Result<ProviderRequestBudgetPolicy, ProviderError> {
            Ok(self.commitment.budget_policy)
        }

        fn request_commitment(
            &self,
            _request: &ProviderRequest,
        ) -> Result<ProviderRequestCommitment, ProviderError> {
            Ok(self.commitment.clone())
        }

        fn prepare_request(
            &self,
            request: ProviderRequest,
        ) -> Result<PreparedProviderRequest, ProviderError> {
            self.prepare_count.fetch_add(1, Ordering::SeqCst);
            *self.captured_request.lock().unwrap() = Some(request);
            if let Some(cancellation) = &self.cancel_on_prepare {
                cancellation.cancel();
            }
            let response = self.responses.lock().unwrap().pop_front().ok_or_else(|| {
                ProviderError::schema_mismatch(
                    "E005 fake Provider response sequence is exhausted.",
                    false,
                )
            })?;
            let dispatch_count = self.dispatch_count.clone();
            let captured_key = self.captured_key.clone();
            PreparedProviderRequest::new(
                self.commitment.clone(),
                move |remote_key, _cancellation, mut events| {
                    dispatch_count.fetch_add(1, Ordering::SeqCst);
                    *captured_key.lock().unwrap() = Some(remote_key);
                    events(ProviderStreamEvent::NetworkRequestStarted);
                    Box::pin(async move { Ok(response) })
                },
            )
        }

        fn check(
            &self,
            _provider_id: String,
            _timeout_ms: u32,
            _cancellation: CancellationToken,
        ) -> ProviderFuture<ProviderHealthCheck> {
            Box::pin(async {
                Err(ProviderError::schema_mismatch(
                    "health check is not used by E005 tests",
                    false,
                ))
            })
        }

        fn stream(
            &self,
            _request: ProviderRequest,
            _cancellation: CancellationToken,
            _events: ProviderEventSink,
        ) -> ProviderFuture<ProviderResponse> {
            Box::pin(async {
                Err(ProviderError::schema_mismatch_with_code(
                    "E005_TEST_DIRECT_STREAM_FORBIDDEN",
                    "E005 tests must use prepare once",
                    false,
                ))
            })
        }

        fn cancel(
            &self,
            _cancellation_id: String,
            _cancellation_token: String,
        ) -> ProviderFuture<bool> {
            Box::pin(async { Ok(false) })
        }
    }

    struct PassVerifier;

    impl E005ProviderOutputVerifier for PassVerifier {
        fn verify(
            &self,
            output: E005ProviderAuthoredOutput,
            _cancellation: CancellationToken,
        ) -> E005VerificationFuture {
            Box::pin(async move {
                Ok(E005ProviderVerification {
                    verdict: E005ProviderVerificationVerdict::Passed,
                    source_program_sha256: Some(output.final_source_sha256),
                    gate_sha256: Some(GATE_SHA256.into()),
                    failed_gate: None,
                })
            })
        }
    }

    fn ledger(pricing_sha256: &str) -> E005ProviderBudgetLedger {
        let mut authorization = E005ProviderRunAuthorizationContract {
            schema_version: E005_PROVIDER_RUN_AUTHORIZATION_SCHEMA_VERSION.into(),
            authorization_id: "e005_auth_test".into(),
            task_set_sha256: E005_FORMAL_TASK_SET_SHA256.into(),
            status: "authorized".into(),
            grant_mode: "explicit_user_confirmation".into(),
            provider_id: Some("provider_test".into()),
            model_id: Some("model_test_v1".into()),
            source_policy_sha256: Some(e005_provider_source_policy_sha256().unwrap()),
            pricing_snapshot_sha256: Some(pricing_sha256.into()),
            disclosure_sha256: Some("3".repeat(64)),
            authorized_at: Some("2026-07-29T00:00:00Z".into()),
            expires_at: Some("2099-07-29T00:00:00Z".into()),
            maximum_author_calls: E005_MAXIMUM_AUTHOR_CALLS,
            maximum_patch_calls: E005_MAXIMUM_PATCH_CALLS,
            maximum_total_calls: E005_MAXIMUM_TOTAL_CALLS,
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
        E005ProviderBudgetLedger {
            schema_version: E005_PROVIDER_LEDGER_SCHEMA_VERSION.into(),
            authorization,
            status: "authorized".into(),
            reservations_created: 0,
            author_calls_accounted: 0,
            patch_calls_accounted: 0,
            calls_accounted: 0,
            reserved_input_tokens: 0,
            reserved_output_tokens: 0,
            reserved_cost_ceiling_microusd: 0,
            accounted_input_tokens: 0,
            accounted_output_tokens: 0,
            accounted_cost_ceiling_microusd: 0,
            authorized_at_unix_ms: 1_775_000_000_000,
            expires_at_unix_ms: 4_088_000_000_000,
            batch_deadline_unix_ms: 1_775_003_150_000,
            updated_at_unix_ms: 1_775_000_000_000,
        }
    }

    fn task() -> Value {
        let set: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/concept-spec/fixtures/e005-unseen-mechanical-hard-surface-task-set.json"
        )))
        .unwrap();
        set["tasks"][0].clone()
    }

    fn source() -> Value {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/concept-spec/fixtures/e005-r1-unified-service-console.json"
        )))
        .unwrap()
    }

    fn response(usage: ProviderUsage) -> ProviderResponse {
        ProviderResponse {
            content: None,
            tool_calls: vec![ProviderToolCall {
                call_id: "call_e005_author".into(),
                name: E005_AUTHOR_TOOL_NAME.into(),
                arguments: source(),
            }],
            ephemeral_reasoning: None,
            usage,
            finish_reason: ProviderFinishReason::ToolCalls,
            network_call_made: true,
        }
    }

    fn input() -> E005ProviderCallInput {
        E005ProviderCallInput {
            authorization_id: "e005_auth_test".into(),
            task_id: "e005_enclosure_sensor_pod".into(),
            task_payload: task(),
            call_kind: E005ProviderCallKind::Author,
            patch_base_source: None,
            failed_gate: None,
        }
    }

    fn usage() -> ProviderUsage {
        ProviderUsage {
            input_tokens: 1_000,
            output_tokens: 1_000,
            prompt_cache_hit_tokens: 0,
            prompt_cache_miss_tokens: 1_000,
            estimated_cost_microusd: 2_000,
        }
    }

    fn formal_visual_output(
        request: &crate::VisualReferenceComparisonProviderRequest,
        patch: bool,
    ) -> VisualReferenceComparisonProviderOutput {
        let assessments = request
            .graph
            .claims
            .iter()
            .filter(|claim| claim.status != VisualClaimStatus::Unknown)
            .map(|claim| {
                let mismatch = patch && claim.claim_id == "vclaim_macro";
                VisualReferenceClaimAssessment {
                    claim_id: claim.claim_id.clone(),
                    outcome: if mismatch {
                        VisualReferenceMatchOutcome::Partial
                    } else {
                        VisualReferenceMatchOutcome::Matched
                    },
                    similarity_bps: if mismatch { 5_000 } else { 8_500 },
                    confidence_bps: 9_000,
                    source_evidence_ids: claim.source_evidence_ids.clone(),
                    candidate_view_ids: vec!["turntable_000".into()],
                    reason: if mismatch {
                        "The macro silhouette requires one bounded parameter repair".into()
                    } else {
                        "The visible reference claim matches the candidate".into()
                    },
                }
            })
            .collect();
        let proposal = if patch {
            json!({
                "schema_version":"E005VisualPatchProposal@1",
                "patch_id":"visualpatch_e005_r2_formal_patch",
                "decision":"typed_visual_patch",
                "expected_source_sha256":request.input.source_program_sha256,
                "comparison_input_sha256":semantic_sha256(&request.input).unwrap(),
                "repair_claim_ids":["vclaim_macro"],
                "operations":[{
                    "op":"set_parameter_default",
                    "parameter_id":"param_fastener_count",
                    "value":8
                }]
            })
        } else {
            json!({
                "schema_version":"E005VisualPatchProposal@1",
                "patch_id":"visualpatch_e005_r2_formal_accept",
                "decision":"accept",
                "expected_source_sha256":request.input.source_program_sha256,
                "comparison_input_sha256":semantic_sha256(&request.input).unwrap(),
                "repair_claim_ids":[],
                "operations":[]
            })
        };
        VisualReferenceComparisonProviderOutput {
            provider_id: "provider_test".into(),
            model_id: "model_test_v1".into(),
            provider_response_sha256: "f".repeat(64),
            analyzed_at: "2026-07-29T13:00:00Z".into(),
            assessments,
            network_call_made: true,
            budget_evidence: None,
            e005_visual_patch_proposal: Some(proposal),
        }
    }

    fn prepared_visual_provider(
        request: &crate::VisualReferenceComparisonProviderRequest,
        provider_id: &str,
        patch: bool,
        dispatch_count: Arc<AtomicUsize>,
    ) -> PreparedE005VisualReviewProviderRequest {
        let comparison_input_sha256 = semantic_sha256(&request.input).unwrap();
        let output = formal_visual_output(request, patch);
        PreparedE005VisualReviewProviderRequest::new(
            provider_id.into(),
            "model_test_v1".into(),
            comparison_input_sha256,
            8_192,
            ProviderRequestCommitment {
                request_sha256: REQUEST_SHA256.into(),
                pricing_snapshot_sha256: PRICING_SHA256.into(),
                budget_policy: ProviderRequestBudgetPolicy {
                    input_tokens_upper_bound: 10_000,
                    input_cost_ceiling_microusd: 10_000,
                    output_microusd_per_million_tokens: 1_000_000,
                },
            },
            move |_remote_idempotency_key, _cancellation| {
                dispatch_count.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move {
                    Ok(E005PreparedVisualReviewProviderResponse {
                        output,
                        usage: usage(),
                    })
                })
            },
        )
        .unwrap()
    }

    struct FakePreparedVisualPort {
        patch: bool,
        dispatch_count: Arc<AtomicUsize>,
    }

    impl E005PreparedVisualReviewProviderPort for FakePreparedVisualPort {
        fn prepare_e005_visual_review(
            &self,
            request: crate::VisualReferenceComparisonProviderRequest,
        ) -> Result<
            PreparedE005VisualReviewProviderRequest,
            crate::VisualReferenceComparisonProviderError,
        > {
            Ok(prepared_visual_provider(
                &request,
                "provider_test",
                self.patch,
                self.dispatch_count.clone(),
            ))
        }
    }

    struct FailPreparedVisualPort;

    impl E005PreparedVisualReviewProviderPort for FailPreparedVisualPort {
        fn prepare_e005_visual_review(
            &self,
            _request: crate::VisualReferenceComparisonProviderRequest,
        ) -> Result<
            PreparedE005VisualReviewProviderRequest,
            crate::VisualReferenceComparisonProviderError,
        > {
            Err(crate::VisualReferenceComparisonProviderError::new(
                "E005_TEST_VISUAL_PREPARE_INTERRUPTED",
                "simulated restart after the durable Author handoff",
                false,
                false,
            ))
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
    }

    fn now_unix_ms() -> i64 {
        i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap()
    }

    #[test]
    fn recovery_precedes_provider_factory_and_failure_prevents_factory() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let budget = FakeBudget::authorized(PRICING_SHA256);
        let shared = order.clone();
        let wrapped_budget = Arc::new(OrderBudget {
            inner: budget,
            order: order.clone(),
        });
        let provider = FakePreparedProvider::new(response(usage()));
        E005FormalProviderRunner::bootstrap(wrapped_budget, Arc::new(PassVerifier), move || {
            shared.lock().unwrap().push("factory".into());
            Ok(provider)
        })
        .unwrap();
        assert_eq!(*order.lock().unwrap(), vec!["recover", "factory"]);

        let failed = Arc::new(FakeBudget::default());
        failed.state.lock().unwrap().fail_recovery = true;
        let factory_calls = Arc::new(AtomicUsize::new(0));
        let captured = factory_calls.clone();
        let result =
            E005FormalProviderRunner::bootstrap(failed, Arc::new(PassVerifier), move || {
                captured.fetch_add(1, Ordering::SeqCst);
                Ok(FakePreparedProvider::new(response(usage())))
            });
        assert_eq!(result.unwrap_err().code, "E005_TEST_RECOVERY_FAILED");
        assert_eq!(factory_calls.load(Ordering::SeqCst), 0);
    }

    struct OrderBudget {
        inner: Arc<FakeBudget>,
        order: Arc<Mutex<Vec<String>>>,
    }

    impl E005ProviderBudgetPort for OrderBudget {
        fn recover_after_restart(&self) -> Result<Vec<E005ProviderBudgetEvidence>, CoreError> {
            self.order.lock().unwrap().push("recover".into());
            self.inner.recover_after_restart()
        }
        fn ledger(&self, id: &str) -> Result<Option<E005ProviderBudgetLedger>, CoreError> {
            self.inner.ledger(id)
        }
        fn reserve(
            &self,
            request: &E005ProviderCallReservationRequest,
        ) -> Result<E005ProviderCallReservation, CoreError> {
            self.inner.reserve(request)
        }
        fn mark_dispatching(&self, id: &str) -> Result<(), CoreError> {
            self.inner.mark_dispatching(id)
        }
        fn settle(
            &self,
            id: &str,
            settlement: &E005ProviderCallSettlement,
        ) -> Result<E005ProviderBudgetEvidence, CoreError> {
            self.inner.settle(id, settlement)
        }
        fn verify_evidence(&self, evidence: &E005ProviderBudgetEvidence) -> Result<(), CoreError> {
            self.inner.verify_evidence(evidence)
        }
    }

    #[test]
    fn missing_authorization_and_pricing_mismatch_stop_before_dispatch() {
        let missing = Arc::new(FakeBudget::default());
        let provider = FakePreparedProvider::new(response(usage()));
        let runner = E005FormalProviderRunner::bootstrap(missing, Arc::new(PassVerifier), {
            let provider = provider.clone();
            move || Ok(provider)
        })
        .unwrap();
        let error = runtime()
            .block_on(runner.execute_call(input(), CancellationToken::new()))
            .unwrap_err();
        assert_eq!(error.code, "E005_PROVIDER_AUTHORIZATION_MISSING");
        assert_eq!(provider.prepare_count.load(Ordering::SeqCst), 0);
        assert_eq!(provider.dispatch_count.load(Ordering::SeqCst), 0);

        let budget = FakeBudget::authorized(&"4".repeat(64));
        let provider = FakePreparedProvider::new(response(usage()));
        let runner = E005FormalProviderRunner::bootstrap(budget, Arc::new(PassVerifier), {
            let provider = provider.clone();
            move || Ok(provider)
        })
        .unwrap();
        let error = runtime()
            .block_on(runner.execute_call(input(), CancellationToken::new()))
            .unwrap_err();
        assert_eq!(error.code, "E005_PROVIDER_PRICING_MISMATCH");
        assert_eq!(provider.prepare_count.load(Ordering::SeqCst), 1);
        assert_eq!(provider.dispatch_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn e005_r1_successful_call_uses_bundled_unified_schema_and_one_dispatch() {
        let budget = FakeBudget::authorized(PRICING_SHA256);
        let provider = FakePreparedProvider::new(response(usage()));
        let runner = E005FormalProviderRunner::bootstrap(budget.clone(), Arc::new(PassVerifier), {
            let provider = provider.clone();
            move || Ok(provider)
        })
        .unwrap();

        let result = runtime()
            .block_on(runner.execute_call(input(), CancellationToken::new()))
            .unwrap();

        assert_eq!(result.request_sha256, REQUEST_SHA256);
        assert_eq!(result.pricing_snapshot_sha256, PRICING_SHA256);
        assert_eq!(provider.prepare_count.load(Ordering::SeqCst), 1);
        assert_eq!(provider.dispatch_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            provider.captured_key.lock().unwrap().as_deref(),
            Some("e005_reservation_test_001")
        );
        let request = provider.captured_request.lock().unwrap();
        let request = request.as_ref().unwrap();
        assert_eq!(request.tools.len(), 1);
        assert_eq!(request.tools[0].name, E005_AUTHOR_TOOL_NAME);
        assert_eq!(
            request.tools[0].input_schema["properties"]["schema_version"]["const"],
            "ForgeVisualAuthorSource@1"
        );
        assert_eq!(
            request.tools[0].input_schema["properties"]["geometry_templates"]["properties"]
                ["schema_version"]["const"],
            "ForgeVisualGeometryProgram@2"
        );
        assert!(request.require_tool_call);
        assert_eq!(request.max_output_tokens, E005_AUTHOR_MAX_OUTPUT_TOKENS);
        assert_eq!(
            budget.events(),
            vec![
                "recover",
                "ledger",
                "reserve",
                "mark",
                "settle:ProviderCompletedPassed"
            ]
        );
    }

    #[test]
    fn cancellation_after_prepare_releases_reservation_without_dispatch() {
        let cancellation = CancellationToken::new();
        let budget = FakeBudget::authorized(PRICING_SHA256);
        let provider = FakePreparedProvider::with_cancel(response(usage()), cancellation.clone());
        let runner = E005FormalProviderRunner::bootstrap(budget.clone(), Arc::new(PassVerifier), {
            let provider = provider.clone();
            move || Ok(provider)
        })
        .unwrap();

        let error = runtime()
            .block_on(runner.execute_call(input(), cancellation))
            .unwrap_err();

        assert_eq!(error.code, "E005_PROVIDER_CANCELLED_BEFORE_DISPATCH");
        assert!(!error.network_call_made);
        assert_eq!(provider.dispatch_count.load(Ordering::SeqCst), 0);
        assert!(budget
            .events()
            .contains(&"settle:PreDispatchReleased".into()));
    }

    #[test]
    fn provider_usage_over_reservation_is_accounted_failure_without_verifier_success() {
        let budget = FakeBudget::authorized(PRICING_SHA256);
        let mut excessive = usage();
        excessive.input_tokens = 10_001;
        let provider = FakePreparedProvider::new(response(excessive));
        let runner = E005FormalProviderRunner::bootstrap(budget.clone(), Arc::new(PassVerifier), {
            let provider = provider.clone();
            move || Ok(provider)
        })
        .unwrap();

        let error = runtime()
            .block_on(runner.execute_call(input(), CancellationToken::new()))
            .unwrap_err();

        assert_eq!(error.code, "E005_PROVIDER_USAGE_BOUND_EXCEEDED");
        assert!(error.network_call_made);
        assert_eq!(provider.dispatch_count.load(Ordering::SeqCst), 1);
        assert!(budget
            .events()
            .contains(&"settle:ProviderCompletedFailed".into()));
    }

    #[test]
    fn e005_r2_formal_visual_accept_reuses_0045_patch_budget_and_one_geometry_build() {
        let budget = FakeBudget::authorized(PRICING_SHA256);
        let author_provider = FakePreparedProvider::new(response(usage()));
        let runner = E005FormalProviderRunner::bootstrap(
            budget.clone(),
            Arc::new(PassVerifier),
            move || Ok(author_provider),
        )
        .unwrap();
        let geometry = Arc::new(GeometryFixture::default());
        let comparison = VisualReferenceComparisonCoordinator::new(
            Arc::new(VisualFixture::new(false)),
            Duration::from_secs(2),
        )
        .unwrap();
        let review_coordinator = E005VisualReviewCoordinatorV1::new(geometry.clone(), comparison);
        let (review_request, _) = request_fixture();
        let prepared_review = runtime()
            .block_on(review_coordinator.prepare(review_request, CancellationToken::new()))
            .unwrap();
        let initial_source_sha256 = prepared_review.initial_source_sha256().to_string();
        let comparison_input_sha256 = prepared_review.comparison_input_sha256().unwrap();
        let dispatch_count = Arc::new(AtomicUsize::new(0));
        let prepared_provider = prepared_visual_provider(
            prepared_review.provider_request(),
            "provider_test",
            false,
            dispatch_count.clone(),
        );
        let result = runtime()
            .block_on(runner.execute_prepared_visual_call(
                E005FormalVisualCallInput {
                    authorization_id: "e005_auth_test".into(),
                    task_id: "e005_enclosure_sensor_pod".into(),
                    task_payload: task(),
                    initial_source_sha256,
                    comparison_input_sha256,
                },
                &review_coordinator,
                prepared_review,
                prepared_provider,
                CancellationToken::new(),
            ))
            .unwrap();

        assert_eq!(dispatch_count.load(Ordering::SeqCst), 1);
        assert_eq!(geometry.call_count(), 1);
        assert_eq!(result.review.geometry_build_count, 1);
        assert_eq!(result.review.visual_provider_call_count, 1);
        assert_eq!(
            result.budget_evidence.call_kind,
            E005ProviderCallKind::Patch
        );
        assert_eq!(
            result.budget_evidence.outcome_code,
            "PROVIDER_COMPLETED_PASSED"
        );
        assert_eq!(
            result.budget_evidence.output_source_sha256.as_deref(),
            Some(result.review.final_source_sha256.as_str())
        );
        assert_eq!(
            result.budget_evidence.output_gate_sha256.as_deref(),
            Some(result.review.comparison_report.report_sha256.as_str())
        );
        assert_eq!(
            budget.events(),
            vec![
                "recover",
                "ledger",
                "reserve",
                "mark",
                "settle:ProviderCompletedPassed"
            ]
        );
    }

    #[test]
    fn e005_r2_formal_visual_patch_uses_one_dispatch_two_builds_and_no_fake_recheck() {
        let budget = FakeBudget::authorized(PRICING_SHA256);
        let author_provider = FakePreparedProvider::new(response(usage()));
        let runner = E005FormalProviderRunner::bootstrap(
            budget.clone(),
            Arc::new(PassVerifier),
            move || Ok(author_provider),
        )
        .unwrap();
        let geometry = Arc::new(GeometryFixture::default());
        let comparison = VisualReferenceComparisonCoordinator::new(
            Arc::new(VisualFixture::new(false)),
            Duration::from_secs(2),
        )
        .unwrap();
        let review_coordinator = E005VisualReviewCoordinatorV1::new(geometry.clone(), comparison);
        let (review_request, _) = request_fixture();
        let prepared_review = runtime()
            .block_on(review_coordinator.prepare(review_request, CancellationToken::new()))
            .unwrap();
        let initial_source_sha256 = prepared_review.initial_source_sha256().to_string();
        let comparison_input_sha256 = prepared_review.comparison_input_sha256().unwrap();
        let dispatch_count = Arc::new(AtomicUsize::new(0));
        let prepared_provider = prepared_visual_provider(
            prepared_review.provider_request(),
            "provider_test",
            true,
            dispatch_count.clone(),
        );
        let result = runtime()
            .block_on(runner.execute_prepared_visual_call(
                E005FormalVisualCallInput {
                    authorization_id: "e005_auth_test".into(),
                    task_id: "e005_enclosure_sensor_pod".into(),
                    task_payload: task(),
                    initial_source_sha256: initial_source_sha256.clone(),
                    comparison_input_sha256,
                },
                &review_coordinator,
                prepared_review,
                prepared_provider,
                CancellationToken::new(),
            ))
            .unwrap();

        assert_eq!(dispatch_count.load(Ordering::SeqCst), 1);
        assert_eq!(geometry.call_count(), 2);
        assert_eq!(result.review.geometry_build_count, 2);
        assert_eq!(result.review.visual_provider_call_count, 1);
        assert_ne!(result.review.final_source_sha256, initial_source_sha256);
        assert!(!result.review.final_visual_model_recheck_performed);
        assert_eq!(
            result.review.status,
            crate::E005VisualReviewStatusV1::PatchedPendingVisualConfirmation
        );
        assert_eq!(
            result.budget_evidence.call_kind,
            E005ProviderCallKind::Patch
        );
        assert_eq!(
            result.budget_evidence.outcome_code,
            "PROVIDER_COMPLETED_PASSED"
        );
        assert_eq!(
            budget.events(),
            vec![
                "recover",
                "ledger",
                "reserve",
                "mark",
                "settle:ProviderCompletedPassed"
            ]
        );
    }

    #[test]
    fn e005_r2_formal_visual_identity_mismatch_stops_before_reservation_and_dispatch() {
        let budget = FakeBudget::authorized(PRICING_SHA256);
        let author_provider = FakePreparedProvider::new(response(usage()));
        let runner = E005FormalProviderRunner::bootstrap(
            budget.clone(),
            Arc::new(PassVerifier),
            move || Ok(author_provider),
        )
        .unwrap();
        let geometry = Arc::new(GeometryFixture::default());
        let comparison = VisualReferenceComparisonCoordinator::new(
            Arc::new(VisualFixture::new(false)),
            Duration::from_secs(2),
        )
        .unwrap();
        let review_coordinator = E005VisualReviewCoordinatorV1::new(geometry.clone(), comparison);
        let (review_request, _) = request_fixture();
        let prepared_review = runtime()
            .block_on(review_coordinator.prepare(review_request, CancellationToken::new()))
            .unwrap();
        let initial_source_sha256 = prepared_review.initial_source_sha256().to_string();
        let comparison_input_sha256 = prepared_review.comparison_input_sha256().unwrap();
        let dispatch_count = Arc::new(AtomicUsize::new(0));
        let prepared_provider = prepared_visual_provider(
            prepared_review.provider_request(),
            "different_provider",
            false,
            dispatch_count.clone(),
        );
        let error = runtime()
            .block_on(runner.execute_prepared_visual_call(
                E005FormalVisualCallInput {
                    authorization_id: "e005_auth_test".into(),
                    task_id: "e005_enclosure_sensor_pod".into(),
                    task_payload: task(),
                    initial_source_sha256,
                    comparison_input_sha256,
                },
                &review_coordinator,
                prepared_review,
                prepared_provider,
                CancellationToken::new(),
            ))
            .unwrap_err();

        assert_eq!(error.code, "E005_R2_VISUAL_AUTHORIZATION_MISMATCH");
        assert!(!error.network_call_made);
        assert_eq!(dispatch_count.load(Ordering::SeqCst), 0);
        assert_eq!(geometry.call_count(), 1);
        assert_eq!(budget.events(), vec!["recover", "ledger"]);
    }

    #[test]
    fn e005_r2_full_task_uses_one_author_one_visual_and_no_duplicate_geometry_build() {
        let budget = FakeBudget::authorized(PRICING_SHA256);
        let checkpoints = Arc::new(FakeVisualCheckpoint::default());
        let author_provider = FakePreparedProvider::new(response(usage()));
        let author_dispatch_count = author_provider.dispatch_count.clone();
        let geometry = Arc::new(GeometryFixture::default());
        let comparison = VisualReferenceComparisonCoordinator::new(
            Arc::new(VisualFixture::new(false)),
            Duration::from_secs(2),
        )
        .unwrap();
        let review = E005VisualReviewCoordinatorV1::new(geometry.clone(), comparison);
        let visual_dispatch_count = Arc::new(AtomicUsize::new(0));
        let visual_provider = Arc::new(FakePreparedVisualPort {
            patch: false,
            dispatch_count: visual_dispatch_count.clone(),
        });
        let coordinator = E005FormalR2TaskCoordinator::bootstrap(
            budget.clone(),
            checkpoints.clone(),
            review,
            visual_provider,
            move || Ok(author_provider),
        )
        .unwrap();
        let (visual_request, _) = request_fixture();
        let result = runtime()
            .block_on(coordinator.execute_task(
                E005FormalTaskRequest {
                    authorization_id: "e005_auth_test".into(),
                    task_id: "e005_enclosure_sensor_pod".into(),
                    task_payload: task(),
                },
                visual_request,
                CancellationToken::new(),
            ))
            .unwrap();

        assert_eq!(result.network_provider_calls(), 2);
        assert_eq!(result.geometry_build_count(), 1);
        assert_eq!(result.budget_evidence().len(), 2);
        assert_eq!(author_dispatch_count.load(Ordering::SeqCst), 1);
        assert_eq!(visual_dispatch_count.load(Ordering::SeqCst), 1);
        assert_eq!(geometry.call_count(), 1);
        assert_eq!(
            checkpoints.events(),
            vec!["recover", "load", "author", "visual"]
        );
        assert_eq!(
            result.author.verification.verdict,
            E005ProviderVerificationVerdict::Repairable
        );
        assert_eq!(
            result.visual.review.status,
            crate::E005VisualReviewStatusV1::AcceptedByVisualReview
        );
        assert_eq!(
            budget.events(),
            vec![
                "recover",
                "ledger",
                "reserve",
                "mark",
                "settle:ProviderCompletedRepairable",
                "ledger",
                "reserve",
                "mark",
                "settle:ProviderCompletedPassed"
            ]
        );
        let receipt = coordinator.seal_receipt(result).unwrap();
        assert_eq!(receipt.status, E005RunStatus::PassedWithoutPatch);
        assert_eq!(receipt.network_provider_calls, 2);
        assert!(receipt.visual_review_evidence.is_some());
        assert!(receipt.visual_session_sha256.is_some());
        assert!(receipt.visual_session_receipt_sha256.is_some());
        assert!(receipt.vp204_session_sha256.is_none());
        assert!(receipt.vp204_receipt_sha256.is_none());
        assert_eq!(receipt.fixed_views.as_ref().unwrap().len(), 8);
    }

    #[test]
    fn e005_r2_restart_resumes_visual_stage_without_a_second_author_dispatch() {
        let budget = FakeBudget::authorized(PRICING_SHA256);
        let checkpoints = Arc::new(FakeVisualCheckpoint::default());
        let geometry = Arc::new(GeometryFixture::default());
        let comparison = VisualReferenceComparisonCoordinator::new(
            Arc::new(VisualFixture::new(false)),
            Duration::from_secs(2),
        )
        .unwrap();
        let first_author = FakePreparedProvider::new(response(usage()));
        let first_author_dispatch_count = first_author.dispatch_count.clone();
        let first = E005FormalR2TaskCoordinator::bootstrap(
            budget.clone(),
            checkpoints.clone(),
            E005VisualReviewCoordinatorV1::new(geometry.clone(), comparison.clone()),
            Arc::new(FailPreparedVisualPort),
            move || Ok(first_author),
        )
        .unwrap();
        let (visual_request, _) = request_fixture();
        let error = runtime()
            .block_on(first.execute_task(
                E005FormalTaskRequest {
                    authorization_id: "e005_auth_test".into(),
                    task_id: "e005_enclosure_sensor_pod".into(),
                    task_payload: task(),
                },
                visual_request,
                CancellationToken::new(),
            ))
            .unwrap_err();
        assert_eq!(error.code, "E005_TEST_VISUAL_PREPARE_INTERRUPTED");
        assert_eq!(first_author_dispatch_count.load(Ordering::SeqCst), 1);
        assert_eq!(geometry.call_count(), 1);

        let second_author = FakePreparedProvider::new(response(usage()));
        let second_author_dispatch_count = second_author.dispatch_count.clone();
        let visual_dispatch_count = Arc::new(AtomicUsize::new(0));
        let second = E005FormalR2TaskCoordinator::bootstrap(
            budget,
            checkpoints.clone(),
            E005VisualReviewCoordinatorV1::new(geometry.clone(), comparison),
            Arc::new(FakePreparedVisualPort {
                patch: false,
                dispatch_count: visual_dispatch_count.clone(),
            }),
            move || Ok(second_author),
        )
        .unwrap();
        assert_eq!(second.startup_checkpoint_recovery().len(), 1);
        let (visual_request, _) = request_fixture();
        let result = runtime()
            .block_on(second.execute_task(
                E005FormalTaskRequest {
                    authorization_id: "e005_auth_test".into(),
                    task_id: "e005_enclosure_sensor_pod".into(),
                    task_payload: task(),
                },
                visual_request,
                CancellationToken::new(),
            ))
            .unwrap();

        assert_eq!(result.network_provider_calls(), 2);
        assert_eq!(second_author_dispatch_count.load(Ordering::SeqCst), 0);
        assert_eq!(visual_dispatch_count.load(Ordering::SeqCst), 1);
        assert_eq!(geometry.call_count(), 2);
        assert_eq!(
            checkpoints
                .checkpoint
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .state,
            E005VisualReviewCheckpointState::Completed
        );
    }

    #[test]
    fn e005_r2_full_patch_receipt_reports_two_builds_and_pending_visual_confirmation() {
        let budget = FakeBudget::authorized(PRICING_SHA256);
        let checkpoints = Arc::new(FakeVisualCheckpoint::default());
        let author_provider = FakePreparedProvider::new(response(usage()));
        let geometry = Arc::new(GeometryFixture::default());
        let comparison = VisualReferenceComparisonCoordinator::new(
            Arc::new(VisualFixture::new(false)),
            Duration::from_secs(2),
        )
        .unwrap();
        let visual_dispatch_count = Arc::new(AtomicUsize::new(0));
        let coordinator = E005FormalR2TaskCoordinator::bootstrap(
            budget,
            checkpoints,
            E005VisualReviewCoordinatorV1::new(geometry.clone(), comparison),
            Arc::new(FakePreparedVisualPort {
                patch: true,
                dispatch_count: visual_dispatch_count.clone(),
            }),
            move || Ok(author_provider),
        )
        .unwrap();
        let (visual_request, _) = request_fixture();
        let execution = runtime()
            .block_on(coordinator.execute_task(
                E005FormalTaskRequest {
                    authorization_id: "e005_auth_test".into(),
                    task_id: "e005_enclosure_sensor_pod".into(),
                    task_payload: task(),
                },
                visual_request,
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(execution.geometry_build_count(), 2);
        assert_eq!(visual_dispatch_count.load(Ordering::SeqCst), 1);
        let receipt = coordinator.seal_receipt(execution).unwrap();
        assert_eq!(receipt.status, E005RunStatus::PassedAfterPatch);
        assert_eq!(receipt.patch_count, 1);
        assert_eq!(receipt.network_provider_calls, 2);
        assert_eq!(receipt.phase_receipts.as_ref().unwrap().len(), 13);
        assert_eq!(
            receipt.visual_review_evidence.as_ref().unwrap().status,
            crate::E005VisualReviewStatusV1::PatchedPendingVisualConfirmation
        );
        assert!(
            !receipt
                .visual_review_evidence
                .as_ref()
                .unwrap()
                .final_visual_model_recheck_performed
        );
    }

    #[test]
    fn e005_r3_formal_receipt_upgrades_same_source_to_one_production_pbr_build() {
        let budget = FakeBudget::authorized(PRICING_SHA256);
        let checkpoints = Arc::new(FakeVisualCheckpoint::default());
        let author_provider = FakePreparedProvider::new(response(usage()));
        let author_dispatch_count = author_provider.dispatch_count.clone();
        let geometry = Arc::new(GeometryFixture::default());
        let comparison = VisualReferenceComparisonCoordinator::new(
            Arc::new(VisualFixture::new(false)),
            Duration::from_secs(2),
        )
        .unwrap();
        let visual_dispatch_count = Arc::new(AtomicUsize::new(0));
        let r2 = E005FormalR2TaskCoordinator::bootstrap(
            budget,
            checkpoints,
            E005VisualReviewCoordinatorV1::new(geometry.clone(), comparison),
            Arc::new(FakePreparedVisualPort {
                patch: false,
                dispatch_count: visual_dispatch_count.clone(),
            }),
            move || Ok(author_provider),
        )
        .unwrap();
        let coordinator = E005FormalR3TaskCoordinator::new(
            r2,
            E005ProductionReviewCoordinatorV1::new(geometry.clone()),
        );
        let (visual_request, _) = request_fixture();
        let execution = runtime()
            .block_on(coordinator.execute_task(
                E005FormalTaskRequest {
                    authorization_id: "e005_auth_test".into(),
                    task_id: "e005_enclosure_sensor_pod".into(),
                    task_payload: task(),
                },
                visual_request,
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(author_dispatch_count.load(Ordering::SeqCst), 1);
        assert_eq!(visual_dispatch_count.load(Ordering::SeqCst), 1);
        assert_eq!(geometry.call_count(), 2);
        assert_ne!(
            execution.r2.visual.review.final_geometry.glb_sha256,
            execution.production.geometry.glb_sha256
        );
        let receipt = coordinator.seal_receipt(execution).unwrap();
        assert_eq!(receipt.status, E005RunStatus::PassedWithoutPatch);
        assert_eq!(receipt.network_provider_calls, 2);
        assert_eq!(
            receipt.artifact_profile_id.as_deref(),
            Some("production_concept")
        );
        let production = receipt.production_review_evidence.as_ref().unwrap();
        assert_eq!(production.surface_adornment_count, 11);
        assert_eq!(production.visual_texture_set_count, 11);
        assert_eq!(production.visual_texture_map_count, 55);
        assert_eq!(receipt.glb_sha256.as_ref(), Some(&production.glb_sha256));
        assert!(receipt.production_review_evidence_sha256.is_some());
    }

    #[test]
    fn vp204_verifier_consumes_one_continuation_without_repeating_initial_geometry() {
        let source: Value = serde_json::from_str(include_str!(
            "../../../../../../packages/concept-spec/fixtures/e005-harness-sensor-pod-source.json"
        ))
        .unwrap();
        let source_sha256 = semantic_sha256(&source).unwrap();
        let patch = json!({
            "schema_version": "ForgeVisualGeometryPatch@1",
            "patch_id": "patch_e005_formal_verifier",
            "expected_source_sha256": source_sha256,
            "operations": [{
                "op": "set_node_position",
                "node_id": "node_upper_shell",
                "position": [0.0, 0.0, 300.0]
            }]
        });
        let patched = apply_forge_visual_geometry_patch_v2(&source, &patch)
            .unwrap()
            .patched_program;
        let patched_sha256 = semantic_sha256(&patched).unwrap();
        let geometry = Arc::new(RepairableGeometryPort::default());
        let verifier = E005Vp204OutputVerifier::new(geometry.clone());
        let author_usage = ProviderUsage {
            input_tokens: 100,
            output_tokens: 60,
            prompt_cache_hit_tokens: 0,
            prompt_cache_miss_tokens: 100,
            estimated_cost_microusd: 40,
        };
        let author = E005ProviderAuthoredOutput {
            task_id: "e005_enclosure_sensor_pod".into(),
            call_kind: E005ProviderCallKind::Author,
            tool_name: E005_AUTHOR_TOOL_NAME.into(),
            authored_value: source.clone(),
            final_source: source,
            final_source_sha256: source_sha256,
            provider_usage: author_usage,
        };
        let author_verification = runtime()
            .block_on(verifier.verify(author, CancellationToken::new()))
            .unwrap();
        assert_eq!(
            author_verification.verdict,
            E005ProviderVerificationVerdict::Repairable
        );
        assert_eq!(geometry.call_count(), 1);
        assert!(verifier
            .has_pending_patch("e005_enclosure_sensor_pod")
            .unwrap());

        let patch_usage = ProviderUsage {
            input_tokens: 30,
            output_tokens: 20,
            prompt_cache_hit_tokens: 10,
            prompt_cache_miss_tokens: 20,
            estimated_cost_microusd: 15,
        };
        let patch_verification = runtime()
            .block_on(verifier.verify(
                E005ProviderAuthoredOutput {
                    task_id: "e005_enclosure_sensor_pod".into(),
                    call_kind: E005ProviderCallKind::Patch,
                    tool_name: E005_PATCH_TOOL_NAME.into(),
                    authored_value: patch,
                    final_source: patched,
                    final_source_sha256: patched_sha256,
                    provider_usage: patch_usage,
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(
            patch_verification.verdict,
            E005ProviderVerificationVerdict::Passed
        );
        assert_eq!(geometry.call_count(), 2);
        assert!(!verifier
            .has_pending_patch("e005_enclosure_sensor_pod")
            .unwrap());
        let result = verifier
            .take_completed_result("e005_enclosure_sensor_pod")
            .unwrap()
            .unwrap();
        assert_eq!(result.session.authoring_count, 1);
        assert_eq!(result.session.patch_count, 1);
        assert_eq!(result.session.receipt.usage.provider_requests, 2);
        assert_eq!(result.session.receipt.usage.input_tokens, 130);
        assert_eq!(result.session.receipt.usage.output_tokens, 80);
        assert_eq!(result.session.receipt.usage.estimated_cost_microusd, 55);
        assert_eq!(
            result
                .session
                .receipt
                .phases
                .iter()
                .filter(|phase| phase.phase == VisualProgramPhaseV2::CompileReadback)
                .count(),
            2
        );
    }

    #[test]
    fn formal_task_coordinator_runs_one_author_and_returns_ledger_verified_vp204_result() {
        let budget = FakeBudget::authorized(PRICING_SHA256);
        let provider = FakePreparedProvider::new(response(usage()));
        let geometry = Arc::new(RepairableGeometryPort::passing());
        let coordinator = E005FormalTaskCoordinator::bootstrap(budget.clone(), geometry.clone(), {
            let provider = provider.clone();
            move || Ok(provider)
        })
        .unwrap();
        let result = runtime()
            .block_on(coordinator.execute_task(
                E005FormalTaskRequest {
                    authorization_id: "e005_auth_test".into(),
                    task_id: "e005_enclosure_sensor_pod".into(),
                    task_payload: task(),
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(result.network_provider_calls(), 1);
        assert!(result.patch.is_none());
        assert_eq!(result.budget_evidence().len(), 1);
        assert_eq!(result.vp204_result.session.authoring_count, 1);
        assert_eq!(result.vp204_result.session.patch_count, 0);
        assert_eq!(
            result.vp204_result.session.state,
            VisualProgramAuthoringStateV2::ReadyForPreview
        );
        assert_eq!(geometry.call_count(), 2);
        assert_eq!(provider.dispatch_count.load(Ordering::SeqCst), 1);
        assert!(budget
            .events()
            .contains(&"settle:ProviderCompletedPassed".into()));
        let receipt = coordinator.seal_receipt(result).unwrap();
        assert_eq!(receipt.run_mode, "formal_provider");
        assert!(receipt.distribution_eligible);
        assert_eq!(receipt.status, E005RunStatus::PassedWithoutPatch);
        assert_eq!(receipt.network_provider_calls, 1);
        assert_eq!(receipt.human_review_status, E005HumanReviewStatus::Pending);
        assert_eq!(receipt.provider_call_evidence.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn e005_r1_repairable_unified_author_fails_before_a_second_provider_dispatch() {
        let budget = FakeBudget::authorized(PRICING_SHA256);
        let provider = FakePreparedProvider::new(response(usage()));
        let geometry = Arc::new(RepairableGeometryPort::default());
        let coordinator = E005FormalTaskCoordinator::bootstrap(budget, geometry.clone(), {
            let provider = provider.clone();
            move || Ok(provider)
        })
        .unwrap();
        let error = match runtime().block_on(coordinator.execute_task(
            E005FormalTaskRequest {
                authorization_id: "e005_auth_test".into(),
                task_id: "e005_enclosure_sensor_pod".into(),
                task_payload: task(),
            },
            CancellationToken::new(),
        )) {
            Ok(_) => panic!("E005-R1 must not dispatch an unversioned visual patch"),
            Err(error) => error,
        };
        assert_eq!(error.code, "E005_R1_VISUAL_PATCH_REQUIRED");
        assert!(error.network_call_made);
        assert_eq!(geometry.call_count(), 1);
        assert_eq!(provider.prepare_count.load(Ordering::SeqCst), 1);
        assert_eq!(provider.dispatch_count.load(Ordering::SeqCst), 1);
    }
}
