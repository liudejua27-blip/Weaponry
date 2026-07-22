//! Bounded Rust-owned Agent Action Loop.
//!
//! The loop is sequential and fail-closed: at most twelve code-owned Product
//! Tool calls, explicit wall/token/cost budgets, hierarchical cancellation,
//! no permanent writes, and no acceptance of results that arrive after a
//! cancelled execution scope.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use forgecad_app_server_protocol::{
    ProductToolApprovalPolicy, ProductToolExecutionRequest, ProductToolExecutionResult,
    ProductToolExecutionStatus, ProductToolFailureCategory,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::Instant;

use crate::{
    canonical::canonical_json, AgentContext, CancellationToken, ContextRole,
    ProductToolExecutorPort, ProductToolPortError, ProductToolRegistry, ProviderClient,
    ProviderError, ProviderFinishReason, ProviderMessage, ProviderRequest, ProviderRole,
    ProviderStreamEvent, ProviderUsage, RedactedExecutionTrace, RedactedTraceEntry, TraceEventKind,
    TracePhase, MAX_PRODUCT_TOOL_CALLS,
};

const MAX_ACTION_LOOP_WALL_TIME_MS: u64 = 300_000;
const MAX_ACTION_LOOP_TOTAL_TOKENS: u64 = 1_000_000;
const MAX_ACTION_LOOP_COST_MICROUSD: u64 = 100_000_000;
const MAX_ACTION_LOOP_OUTPUT_TOKENS_PER_REQUEST: u64 = 100_000;
const MAX_PROVIDER_SCHEMA_REPAIR_ATTEMPTS: u8 = 1;
const MAX_PRODUCT_TOOL_RECOVERY_ATTEMPTS: u8 = 2;
const PROVIDER_SCHEMA_REPAIR_MESSAGE: &str =
    "上一轮结构化工具调用未通过 JSON 校验。请重新调用一个受限 Product Tool：arguments 必须是单个 JSON object，使用双引号和有效 JSON，不要输出 Markdown、注释或额外文本。严格遵守当前工具 schema。";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionLoopConfig {
    pub max_tool_calls: u32,
    pub max_wall_time_ms: u64,
    pub max_total_tokens: u64,
    pub max_estimated_cost_microusd: u64,
    pub max_output_tokens_per_request: u64,
}

impl Default for ActionLoopConfig {
    fn default() -> Self {
        Self {
            max_tool_calls: MAX_PRODUCT_TOOL_CALLS,
            // A production concept turn can contain one bounded 120-second
            // geometry compile plus the fixed multi-view render and Rust
            // persistence. This remains well below the reviewed five-minute
            // absolute ceiling and is still cancelled as one Turn scope.
            max_wall_time_ms: 280_000,
            // DeepSeek thinking/tool-call turns replay the bounded registry
            // and the prior tool envelopes on every request.  The compact
            // Provider projection keeps ordinary turns small, but a model
            // that emits the full reviewed arm intent plus bounded repair
            // still needs headroom for the complete synthesis chain.  This
            // is a finite per-Turn ceiling, not an unlimited conversation.
            max_total_tokens: 256_000,
            max_estimated_cost_microusd: 100_000,
            // DeepSeek thinking/tool-call turns can spend several thousand
            // tokens on private reasoning before emitting a compact plan.
            // Reserve 16K per request so a valid plan is not truncated while
            // the finite total-token and cost ceilings still bound a Turn.
            max_output_tokens_per_request: 16_384,
        }
    }
}

impl ActionLoopConfig {
    /// Explicit live-provider acceptance profile.  It removes the ordinary
    /// 256K/100K per-Turn ceilings by moving to the already reviewed hard
    /// maximums; wall time, Product Tool count, request output bound, and
    /// cancellation remain finite.  The desktop bridge exposes this profile
    /// only behind its opt-in acceptance environment contract.
    pub fn for_explicit_live_acceptance() -> Self {
        let mut config = Self::default();
        config.max_total_tokens = MAX_ACTION_LOOP_TOTAL_TOKENS;
        config.max_estimated_cost_microusd = MAX_ACTION_LOOP_COST_MICROUSD;
        config.max_output_tokens_per_request = MAX_ACTION_LOOP_OUTPUT_TOKENS_PER_REQUEST;
        config
    }

    pub fn validate(&self) -> Result<(), ActionLoopConfigError> {
        if self.max_tool_calls == 0 || self.max_tool_calls > MAX_PRODUCT_TOOL_CALLS {
            return Err(ActionLoopConfigError {
                code: "ACTION_LOOP_TOOL_LIMIT_INVALID".into(),
                message: format!("max_tool_calls must be between 1 and {MAX_PRODUCT_TOOL_CALLS}."),
            });
        }
        if self.max_wall_time_ms == 0
            || self.max_total_tokens == 0
            || self.max_estimated_cost_microusd == 0
            || self.max_output_tokens_per_request == 0
        {
            return Err(ActionLoopConfigError {
                code: "ACTION_LOOP_BUDGET_INVALID".into(),
                message: "Action Loop budgets must be positive.".into(),
            });
        }
        if self.max_wall_time_ms > MAX_ACTION_LOOP_WALL_TIME_MS
            || self.max_total_tokens > MAX_ACTION_LOOP_TOTAL_TOKENS
            || self.max_estimated_cost_microusd > MAX_ACTION_LOOP_COST_MICROUSD
            || self.max_output_tokens_per_request > MAX_ACTION_LOOP_OUTPUT_TOKENS_PER_REQUEST
        {
            return Err(ActionLoopConfigError {
                code: "ACTION_LOOP_BUDGET_OUT_OF_RANGE".into(),
                message: "Action Loop budgets exceed the reviewed hard bounds.".into(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionLoopConfigError {
    pub code: String,
    pub message: String,
}

#[derive(Clone)]
pub struct ActionLoopInput {
    pub execution_id: String,
    pub turn_id: String,
    pub cancellation_id: String,
    pub cancellation_token: String,
    pub provider_id: String,
    /// A runtime may persist the safe Provider-gateway preflight fact before
    /// entering the Action Loop, then pass the exact resolved metadata here
    /// so preflight is not repeated and the selected model cannot drift.
    pub provider_preflight: Option<crate::ProviderPreflight>,
    pub context: AgentContext,
}

impl fmt::Debug for ActionLoopInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActionLoopInput")
            .field("execution_id", &self.execution_id)
            .field("turn_id", &self.turn_id)
            .field("cancellation_id", &"[REDACTED]")
            .field("cancellation_token", &"[REDACTED]")
            .field("provider_id", &self.provider_id)
            .field("provider_preflight", &self.provider_preflight)
            .field("context", &self.context)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ActionLoopUsage {
    pub provider_requests: u32,
    pub product_tool_calls: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub prompt_cache_hit_tokens: u64,
    pub prompt_cache_miss_tokens: u64,
    pub estimated_cost_microusd: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionLoopItemEventKind {
    ToolCall,
    ToolResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionLoopItemStatus {
    Pending,
    Completed,
    Failed,
    Cancelled,
    Rejected,
}

/// Schema-validated, bounded Item material that the lifecycle handler can map
/// to alternating A004 ToolCall/ToolResult Items. Provider reasoning and
/// credentials are structurally absent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionLoopItemEvent {
    pub sequence: u32,
    pub event_kind: ActionLoopItemEventKind,
    pub call_id: String,
    pub tool_id: String,
    pub tool_name: String,
    pub status: ActionLoopItemStatus,
    pub idempotency_key: String,
    pub approval_policy: ProductToolApprovalPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<BTreeMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<BTreeMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_category: Option<ProductToolFailureCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub type ActionLoopItemEventSinkFuture =
    Pin<Box<dyn Future<Output = Result<(), ActionLoopItemEventSinkError>> + Send + 'static>>;

/// Transport-neutral incremental Item boundary. Implementations must finish
/// durable append and publication before returning success. This keeps a Tool
/// Call observable before execution begins and prevents a completed Tool
/// Result from waiting for the whole Action Loop to finish.
pub trait ActionLoopItemEventSink: Send + Sync + 'static {
    fn emit(
        &self,
        event: ActionLoopItemEvent,
        cancellation: CancellationToken,
    ) -> ActionLoopItemEventSinkFuture;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionLoopItemEventSinkError {
    pub code: String,
    pub message: String,
    pub recoverable: bool,
}

#[derive(Debug, Default)]
pub struct NoopActionLoopItemEventSink;

impl ActionLoopItemEventSink for NoopActionLoopItemEventSink {
    fn emit(
        &self,
        _event: ActionLoopItemEvent,
        _cancellation: CancellationToken,
    ) -> ActionLoopItemEventSinkFuture {
        Box::pin(async { Ok(()) })
    }
}

impl ActionLoopUsage {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens.saturating_add(self.output_tokens)
    }

    fn add_provider(&mut self, usage: &ProviderUsage) {
        self.provider_requests = self.provider_requests.saturating_add(1);
        self.input_tokens = self.input_tokens.saturating_add(usage.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(usage.output_tokens);
        self.prompt_cache_hit_tokens = self
            .prompt_cache_hit_tokens
            .saturating_add(usage.prompt_cache_hit_tokens);
        self.prompt_cache_miss_tokens = self
            .prompt_cache_miss_tokens
            .saturating_add(usage.prompt_cache_miss_tokens);
        self.estimated_cost_microusd = self
            .estimated_cost_microusd
            .saturating_add(usage.estimated_cost_microusd);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionLoopResult {
    pub execution_id: String,
    pub turn_id: String,
    pub final_content: String,
    pub usage: ActionLoopUsage,
    pub network_call_made: bool,
    pub item_events: Vec<ActionLoopItemEvent>,
    pub trace: RedactedExecutionTrace,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionLoopFailureKind {
    Provider,
    ProductTool,
    ProductToolSchema,
    ProductToolBudget,
    TokenBudget,
    CostBudget,
    WallTimeBudget,
    Cancelled,
    DuplicateToolCall,
    PermanentWriteRejected,
    ItemEventPersistence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionLoopFailure {
    pub code: String,
    pub kind: ActionLoopFailureKind,
    pub message: String,
    pub recoverable: bool,
    pub network_call_made: bool,
    /// Usage observed before the failure. This is authoritative accounting
    /// evidence for terminal persistence; it never contains Provider content.
    pub usage: ActionLoopUsage,
    pub item_events: Vec<ActionLoopItemEvent>,
    pub trace: RedactedExecutionTrace,
}

#[derive(Clone)]
pub struct ActionLoop {
    provider: Arc<dyn ProviderClient>,
    executor: Arc<dyn ProductToolExecutorPort>,
    registry: ProductToolRegistry,
    config: ActionLoopConfig,
}

impl ActionLoop {
    pub fn new(
        provider: Arc<dyn ProviderClient>,
        executor: Arc<dyn ProductToolExecutorPort>,
        registry: ProductToolRegistry,
        config: ActionLoopConfig,
    ) -> Result<Self, ActionLoopConfigError> {
        config.validate()?;
        Ok(Self {
            provider,
            executor,
            registry,
            config,
        })
    }

    /// Keeps the immutable loop configuration, executor, and code-owned Tool
    /// registry while replacing only the Provider for one Turn. Native runtime
    /// uses this for a short-lived credential session; no session is retained
    /// by the long-lived runtime or by another Turn.
    pub fn with_provider(&self, provider: Arc<dyn ProviderClient>) -> Self {
        Self {
            provider,
            executor: self.executor.clone(),
            registry: self.registry.clone(),
            config: self.config.clone(),
        }
    }

    pub async fn run(
        &self,
        input: ActionLoopInput,
        cancellation: CancellationToken,
    ) -> Result<ActionLoopResult, ActionLoopFailure> {
        self.run_with_item_event_sink(input, cancellation, Arc::new(NoopActionLoopItemEventSink))
            .await
    }

    pub async fn run_with_item_event_sink(
        &self,
        input: ActionLoopInput,
        cancellation: CancellationToken,
        item_event_sink: Arc<dyn ActionLoopItemEventSink>,
    ) -> Result<ActionLoopResult, ActionLoopFailure> {
        let started = Instant::now();
        let deadline = started + Duration::from_millis(self.config.max_wall_time_ms);
        let mut trace = RedactedExecutionTrace::new(
            input.execution_id.clone(),
            input.context.context_digest.clone(),
        );
        let mut item_events = Vec::new();
        trace.push(RedactedTraceEntry::new(
            TracePhase::Context,
            TraceEventKind::Completed,
            0,
        ));
        let mut usage = ActionLoopUsage::default();
        let mut network_call_made = false;
        macro_rules! emit_item_event_or_fail {
            ($event:expr) => {{
                if let Err(error) = emit_item_event(
                    &mut item_events,
                    $event,
                    item_event_sink.as_ref(),
                    &cancellation,
                    deadline,
                )
                .await
                {
                    return Err(item_event_failure(
                        error,
                        network_call_made,
                        &usage,
                        &item_events,
                        &mut trace,
                        started,
                    ));
                }
            }};
        }

        let provider_preflight = if let Some(preflight) = input.provider_preflight.clone() {
            preflight
        } else {
            let preflight_scope = cancellation.child_token();
            match guarded(
                self.provider.preflight(preflight_scope.clone()),
                preflight_scope,
                deadline,
            )
            .await
            {
                Ok(preflight) => preflight,
                Err(GuardedError::Cancelled) => {
                    return Err(failure(
                        "ACTION_LOOP_CANCELLED",
                        ActionLoopFailureKind::Cancelled,
                        "Action Loop was cancelled before Provider execution.",
                        true,
                        false,
                        &usage,
                        &item_events,
                        &mut trace,
                        started,
                        TracePhase::Cancellation,
                        TraceEventKind::Cancelled,
                    ));
                }
                Err(GuardedError::Timeout) => {
                    return Err(failure(
                        "ACTION_LOOP_WALL_TIME_EXCEEDED",
                        ActionLoopFailureKind::WallTimeBudget,
                        "Action Loop exceeded its wall-time budget.",
                        true,
                        false,
                        &usage,
                        &item_events,
                        &mut trace,
                        started,
                        TracePhase::Budget,
                        TraceEventKind::BudgetExceeded,
                    ));
                }
                Err(GuardedError::Inner(error)) => {
                    return Err(provider_failure(
                        error,
                        &usage,
                        &item_events,
                        &mut trace,
                        started,
                    ));
                }
            }
        };
        if provider_preflight.provider_id != input.provider_id {
            return Err(failure(
                "ACTION_LOOP_PROVIDER_IDENTITY_MISMATCH",
                ActionLoopFailureKind::Provider,
                "Provider preflight identity does not match the Turn-selected Provider.",
                false,
                false,
                &usage,
                &item_events,
                &mut trace,
                started,
                TracePhase::Provider,
                TraceEventKind::Rejected,
            ));
        }
        if !provider_preflight.configured
            || !provider_preflight.streaming
            || !provider_preflight.tool_calls
        {
            return Err(failure(
                "ACTION_LOOP_PROVIDER_CAPABILITY_MISMATCH",
                ActionLoopFailureKind::Provider,
                "Provider preflight did not confirm streaming and Product Tool capabilities.",
                false,
                false,
                &usage,
                &item_events,
                &mut trace,
                started,
                TracePhase::Provider,
                TraceEventKind::Rejected,
            ));
        }

        let mut messages = context_messages(&input.context);
        // An existing ActiveDesignSnapshot plus an explicit continuation verb
        // is an edit Turn, not a new research/synthesis Turn.  Giving the
        // Provider the whole discovery registry here made DeepSeek spend the
        // bounded Turn on `infer_product_domain`/reference research before it
        // ever emitted the required AssemblyDelta.  Restrict the advertised
        // tools to the single plan contract; Rust still validates the full
        // Product Tool schema after the call and the ChangeSet path remains
        // the only write route.
        let provider_tools = provider_definitions_for_context(&self.registry, &input.context);
        let mut seen_call_ids = BTreeSet::new();
        let mut provider_schema_repair_attempts = 0u8;
        let mut product_tool_recovery_attempts = 0u8;
        let mut product_tool_attempts = 0u32;

        loop {
            if cancellation.is_cancelled() {
                return Err(failure(
                    "ACTION_LOOP_CANCELLED",
                    ActionLoopFailureKind::Cancelled,
                    "Action Loop was cancelled.",
                    true,
                    network_call_made,
                    &usage,
                    &item_events,
                    &mut trace,
                    started,
                    TracePhase::Cancellation,
                    TraceEventKind::Cancelled,
                ));
            }
            if Instant::now() >= deadline {
                return Err(failure(
                    "ACTION_LOOP_WALL_TIME_EXCEEDED",
                    ActionLoopFailureKind::WallTimeBudget,
                    "Action Loop exceeded its wall-time budget.",
                    true,
                    network_call_made,
                    &usage,
                    &item_events,
                    &mut trace,
                    started,
                    TracePhase::Budget,
                    TraceEventKind::BudgetExceeded,
                ));
            }

            let remaining_tokens = self
                .config
                .max_total_tokens
                .saturating_sub(usage.total_tokens());
            if remaining_tokens == 0 {
                return Err(failure(
                    "ACTION_LOOP_TOKEN_BUDGET_EXHAUSTED",
                    ActionLoopFailureKind::TokenBudget,
                    "No token budget remains for another Provider request.",
                    false,
                    network_call_made,
                    &usage,
                    &item_events,
                    &mut trace,
                    started,
                    TracePhase::Budget,
                    TraceEventKind::BudgetExceeded,
                ));
            }
            let remaining_cost = self
                .config
                .max_estimated_cost_microusd
                .saturating_sub(usage.estimated_cost_microusd);
            if remaining_cost == 0 {
                return Err(failure(
                    "ACTION_LOOP_COST_BUDGET_EXHAUSTED",
                    ActionLoopFailureKind::CostBudget,
                    "No estimated cost budget remains for another Provider request.",
                    false,
                    network_call_made,
                    &usage,
                    &item_events,
                    &mut trace,
                    started,
                    TracePhase::Budget,
                    TraceEventKind::BudgetExceeded,
                ));
            }

            let mut provider_request = ProviderRequest {
                provider_id: provider_preflight.provider_id.clone(),
                // Credential metadata may change while the desktop app remains
                // running. Preflight is the per-execution source of truth;
                // static runtime defaults must never override the currently
                // selected Keychain model.
                model: provider_preflight.model.clone(),
                context_digest: input.context.context_digest.clone(),
                messages: messages.clone(),
                tools: provider_tools.clone(),
                max_output_tokens: self
                    .config
                    .max_output_tokens_per_request
                    .min(remaining_tokens),
            };
            let request_budget = match self
                .provider
                .request_budget_policy(&provider_request)
                .and_then(|policy| policy.validate())
            {
                Ok(policy) => policy,
                Err(error) => {
                    return Err(provider_failure(
                        error,
                        &usage,
                        &item_events,
                        &mut trace,
                        started,
                    ));
                }
            };
            let output_tokens_by_total =
                remaining_tokens.saturating_sub(request_budget.input_tokens_upper_bound);
            if output_tokens_by_total == 0 {
                return Err(failure(
                    "ACTION_LOOP_TOKEN_BUDGET_RESERVATION_FAILED",
                    ActionLoopFailureKind::TokenBudget,
                    "The remaining token budget cannot safely reserve the next Provider input.",
                    false,
                    network_call_made,
                    &usage,
                    &item_events,
                    &mut trace,
                    started,
                    TracePhase::Budget,
                    TraceEventKind::BudgetExceeded,
                ));
            }
            let output_tokens_by_cost = request_budget.max_output_tokens_for_cost(remaining_cost);
            if output_tokens_by_cost == 0 {
                return Err(failure(
                    "ACTION_LOOP_COST_BUDGET_RESERVATION_FAILED",
                    ActionLoopFailureKind::CostBudget,
                    "The remaining estimated cost budget cannot safely reserve another Provider request.",
                    false,
                    network_call_made,
                    &usage,
                    &item_events,
                    &mut trace,
                    started,
                    TracePhase::Budget,
                    TraceEventKind::BudgetExceeded,
                ));
            }
            provider_request.max_output_tokens = provider_request
                .max_output_tokens
                .min(output_tokens_by_total)
                .min(output_tokens_by_cost);
            let request_cost_ceiling = request_budget.input_cost_ceiling_microusd.saturating_add(
                request_budget.output_cost_ceiling_microusd(provider_request.max_output_tokens),
            );
            if request_cost_ceiling > remaining_cost {
                return Err(failure(
                    "ACTION_LOOP_COST_BUDGET_RESERVATION_FAILED",
                    ActionLoopFailureKind::CostBudget,
                    "The next Provider request could not be bounded within the remaining estimated cost budget.",
                    false,
                    network_call_made,
                    &usage,
                    &item_events,
                    &mut trace,
                    started,
                    TracePhase::Budget,
                    TraceEventKind::BudgetExceeded,
                ));
            }

            let mut provider_started = RedactedTraceEntry::new(
                TracePhase::Provider,
                TraceEventKind::Started,
                elapsed_ms(started),
            );
            provider_started.input_sha256 = Some(input.context.context_digest.clone());
            provider_started.input_tokens = request_budget.input_tokens_upper_bound;
            provider_started.output_tokens = provider_request.max_output_tokens;
            provider_started.estimated_cost_microusd = request_cost_ceiling;
            trace.push(provider_started);
            let provider_scope = cancellation.child_token();
            let provider_attempted = Arc::new(AtomicBool::new(false));
            let provider_attempt_latch = provider_attempted.clone();
            let provider_result = guarded(
                self.provider.stream(
                    provider_request.clone(),
                    provider_scope.clone(),
                    Box::new(move |event| {
                        if matches!(event, ProviderStreamEvent::NetworkRequestStarted) {
                            provider_attempt_latch.store(true, Ordering::Release);
                        }
                    }),
                ),
                provider_scope,
                deadline,
            )
            .await;
            let response = match provider_result {
                Ok(response) => response,
                Err(GuardedError::Cancelled) => {
                    network_call_made |= provider_attempted.load(Ordering::Acquire);
                    let mut ignored = RedactedTraceEntry::new(
                        TracePhase::Provider,
                        TraceEventKind::LateResultIgnored,
                        elapsed_ms(started),
                    );
                    ignored.network_call_made = network_call_made;
                    trace.push(ignored);
                    return Err(failure(
                        "ACTION_LOOP_CANCELLED",
                        ActionLoopFailureKind::Cancelled,
                        "Action Loop cancelled Provider work; late output is rejected.",
                        true,
                        network_call_made,
                        &usage,
                        &item_events,
                        &mut trace,
                        started,
                        TracePhase::Cancellation,
                        TraceEventKind::Cancelled,
                    ));
                }
                Err(GuardedError::Timeout) => {
                    network_call_made |= provider_attempted.load(Ordering::Acquire);
                    return Err(failure(
                        "ACTION_LOOP_WALL_TIME_EXCEEDED",
                        ActionLoopFailureKind::WallTimeBudget,
                        "Action Loop exceeded its wall-time budget during Provider execution.",
                        true,
                        network_call_made,
                        &usage,
                        &item_events,
                        &mut trace,
                        started,
                        TracePhase::Budget,
                        TraceEventKind::BudgetExceeded,
                    ));
                }
                Err(GuardedError::Inner(mut error)) => {
                    error.network_call_made |= provider_attempted.load(Ordering::Acquire);
                    if provider_schema_repair_attempts < MAX_PROVIDER_SCHEMA_REPAIR_ATTEMPTS
                        && provider_error_supports_schema_repair(&error)
                        && !cancellation.is_cancelled()
                    {
                        provider_schema_repair_attempts =
                            provider_schema_repair_attempts.saturating_add(1);
                        network_call_made |= error.network_call_made;
                        let mut repair = RedactedTraceEntry::new(
                            TracePhase::Provider,
                            TraceEventKind::Rejected,
                            elapsed_ms(started),
                        );
                        repair.error_code = Some("PROVIDER_SCHEMA_REPAIR_REQUESTED".into());
                        repair.provider_failure_category = Some(error.category.clone());
                        repair.network_call_made = error.network_call_made;
                        trace.push(repair);
                        // The malformed response is deliberately not inserted
                        // into the next request. This keeps untrusted bytes
                        // out of the conversation and asks the Provider to
                        // re-emit only the bounded call shape.
                        messages.push(ProviderMessage {
                            role: ProviderRole::User,
                            content: PROVIDER_SCHEMA_REPAIR_MESSAGE.into(),
                            tool_call_id: None,
                            tool_calls: Vec::new(),
                            ephemeral_reasoning: None,
                        });
                        continue;
                    }
                    return Err(provider_failure(
                        error,
                        &usage,
                        &item_events,
                        &mut trace,
                        started,
                    ));
                }
            };
            let provider_attempted = provider_attempted.load(Ordering::Acquire);
            network_call_made |= response.network_call_made || provider_attempted;
            let response_exceeded_reservation = response.usage.input_tokens
                > request_budget.input_tokens_upper_bound
                || response.usage.output_tokens > provider_request.max_output_tokens
                || response.usage.estimated_cost_microusd > request_cost_ceiling;
            usage.add_provider(&response.usage);
            let mut provider_completed = RedactedTraceEntry::new(
                TracePhase::Provider,
                TraceEventKind::Completed,
                elapsed_ms(started),
            );
            provider_completed.input_tokens = response.usage.input_tokens;
            provider_completed.output_tokens = response.usage.output_tokens;
            provider_completed.estimated_cost_microusd = response.usage.estimated_cost_microusd;
            provider_completed.network_call_made = response.network_call_made || provider_attempted;
            provider_completed.output_sha256 = Some(RedactedExecutionTrace::digest_value(&json!({
                "content": response.content,
                "tool_calls": response.tool_calls,
            })));
            trace.push(provider_completed);

            if response_exceeded_reservation {
                return Err(provider_failure(
                    ProviderError::schema_mismatch(
                        "Provider usage exceeded the pre-request budget reservation.",
                        network_call_made,
                    ),
                    &usage,
                    &item_events,
                    &mut trace,
                    started,
                ));
            }

            if usage.total_tokens() > self.config.max_total_tokens {
                return Err(failure(
                    "ACTION_LOOP_TOKEN_BUDGET_EXCEEDED",
                    ActionLoopFailureKind::TokenBudget,
                    "Provider token budget was exceeded.",
                    false,
                    network_call_made,
                    &usage,
                    &item_events,
                    &mut trace,
                    started,
                    TracePhase::Budget,
                    TraceEventKind::BudgetExceeded,
                ));
            }
            if usage.estimated_cost_microusd > self.config.max_estimated_cost_microusd {
                return Err(failure(
                    "ACTION_LOOP_COST_BUDGET_EXCEEDED",
                    ActionLoopFailureKind::CostBudget,
                    "Estimated Provider cost budget was exceeded.",
                    false,
                    network_call_made,
                    &usage,
                    &item_events,
                    &mut trace,
                    started,
                    TracePhase::Budget,
                    TraceEventKind::BudgetExceeded,
                ));
            }

            match response.finish_reason {
                ProviderFinishReason::Stop => {
                    let final_content = response
                        .content
                        .expect("validated stop response has content");
                    trace.push(RedactedTraceEntry::new(
                        TracePhase::Final,
                        TraceEventKind::Completed,
                        elapsed_ms(started),
                    ));
                    return Ok(ActionLoopResult {
                        execution_id: input.execution_id,
                        turn_id: input.turn_id,
                        final_content,
                        usage,
                        network_call_made,
                        item_events,
                        trace,
                    });
                }
                ProviderFinishReason::ToolCalls => {
                    messages.push(ProviderMessage {
                        role: ProviderRole::Assistant,
                        content: response.content.unwrap_or_default(),
                        tool_call_id: None,
                        tool_calls: response.tool_calls.clone(),
                        ephemeral_reasoning: response.ephemeral_reasoning,
                    });
                    for call in response.tool_calls {
                        if product_tool_attempts >= self.config.max_tool_calls {
                            return Err(failure(
                                "ACTION_LOOP_TOOL_CALL_BUDGET_EXCEEDED",
                                ActionLoopFailureKind::ProductToolBudget,
                                "Product Tool call budget was exceeded.",
                                false,
                                network_call_made,
                                &usage,
                                &item_events,
                                &mut trace,
                                started,
                                TracePhase::Budget,
                                TraceEventKind::BudgetExceeded,
                            ));
                        }
                        if !seen_call_ids.insert(call.call_id.clone()) {
                            return Err(failure(
                                "ACTION_LOOP_DUPLICATE_TOOL_CALL",
                                ActionLoopFailureKind::DuplicateToolCall,
                                "Provider reused a Product Tool call ID.",
                                false,
                                network_call_made,
                                &usage,
                                &item_events,
                                &mut trace,
                                started,
                                TracePhase::ProductTool,
                                TraceEventKind::Rejected,
                            ));
                        }
                        product_tool_attempts = product_tool_attempts.saturating_add(1);
                        let call_number = product_tool_attempts;
                        usage.product_tool_calls = call_number;
                        let request = match self.registry.build_execution_request(
                            &input.turn_id,
                            &call,
                            &input.execution_id,
                            &input.cancellation_id,
                            &input.cancellation_token,
                        ) {
                            Ok(request) => request,
                            Err(error) => {
                                // A provider can understand the intent but still miss one
                                // of the Rust-owned enum/required-field constraints.  Give it
                                // one bounded, fixed repair envelope.  The original arguments
                                // and validator text never enter the Provider messages, item
                                // log, or redacted trace.
                                if product_tool_recovery_attempts
                                    < MAX_PRODUCT_TOOL_RECOVERY_ATTEMPTS
                                {
                                    if let Some(recovery_message) =
                                        product_tool_schema_recovery_message(
                                            &call.name,
                                            &error.code,
                                        )
                                    {
                                        product_tool_recovery_attempts =
                                            product_tool_recovery_attempts.saturating_add(1);
                                        let mut repair = RedactedTraceEntry::new(
                                            TracePhase::ProductTool,
                                            TraceEventKind::Rejected,
                                            elapsed_ms(started),
                                        );
                                        repair.call_id = Some(call.call_id.clone());
                                        repair.tool_name = Some(call.name.clone());
                                        repair.error_code =
                                            Some("PRODUCT_TOOL_SCHEMA_REPAIR_REQUESTED".into());
                                        repair.network_call_made = network_call_made;
                                        trace.push(repair);
                                        messages.push(ProviderMessage {
                                            role: ProviderRole::Tool,
                                            content: serde_json::to_string(&json!({
                                                "error_code": error.code,
                                                "message": recovery_message
                                            }))
                                            .expect("fixed Product Tool schema repair serializes"),
                                            tool_call_id: Some(call.call_id),
                                            tool_calls: Vec::new(),
                                            ephemeral_reasoning: None,
                                        });
                                        continue;
                                    }
                                }
                                return Err(failure(
                                    &error.code,
                                    ActionLoopFailureKind::ProductToolSchema,
                                    &error.message,
                                    false,
                                    network_call_made,
                                    &usage,
                                    &item_events,
                                    &mut trace,
                                    started,
                                    TracePhase::ProductTool,
                                    TraceEventKind::Rejected,
                                ));
                            }
                        };
                        emit_item_event_or_fail!(ActionLoopItemEvent::tool_call(&request));
                        let mut tool_started = RedactedTraceEntry::new(
                            TracePhase::ProductTool,
                            TraceEventKind::Started,
                            elapsed_ms(started),
                        );
                        tool_started.call_id = Some(call.call_id.clone());
                        tool_started.tool_name = Some(call.name.clone());
                        tool_started.input_sha256 =
                            Some(RedactedExecutionTrace::digest_value(&call.arguments));
                        trace.push(tool_started);

                        let tool_scope = cancellation.child_token();
                        let result = match guarded(
                            self.executor.execute(request.clone(), tool_scope.clone()),
                            tool_scope,
                            deadline,
                        )
                        .await
                        {
                            Ok(result) => result,
                            Err(GuardedError::Cancelled) => {
                                let mut ignored = RedactedTraceEntry::new(
                                    TracePhase::ProductTool,
                                    TraceEventKind::LateResultIgnored,
                                    elapsed_ms(started),
                                );
                                ignored.call_id = Some(call.call_id);
                                ignored.tool_name = Some(call.name);
                                trace.push(ignored);
                                return Err(failure(
                                    "ACTION_LOOP_CANCELLED",
                                    ActionLoopFailureKind::Cancelled,
                                    "Action Loop cancelled Product Tool work; late output is rejected.",
                                    true,
                                    network_call_made,
                                    &usage,
                                    &item_events,
                                    &mut trace,
                                    started,
                                    TracePhase::Cancellation,
                                    TraceEventKind::Cancelled,
                                ));
                            }
                            Err(GuardedError::Timeout) => {
                                return Err(failure(
                                    "ACTION_LOOP_WALL_TIME_EXCEEDED",
                                    ActionLoopFailureKind::WallTimeBudget,
                                    "Action Loop exceeded its wall-time budget during Product Tool execution.",
                                    true,
                                    network_call_made,
                                    &usage,
                                    &item_events,
                                    &mut trace,
                                    started,
                                    TracePhase::Budget,
                                    TraceEventKind::BudgetExceeded,
                                ));
                            }
                            Err(GuardedError::Inner(error)) => {
                                let status =
                                    if error.kind == crate::ProductToolPortErrorKind::Cancelled {
                                        ActionLoopItemStatus::Cancelled
                                    } else {
                                        ActionLoopItemStatus::Failed
                                    };
                                let category = match error.kind {
                                    crate::ProductToolPortErrorKind::Cancelled => {
                                        ProductToolFailureCategory::Cancelled
                                    }
                                    crate::ProductToolPortErrorKind::Timeout => {
                                        ProductToolFailureCategory::Timeout
                                    }
                                    crate::ProductToolPortErrorKind::Unavailable
                                    | crate::ProductToolPortErrorKind::InvalidResponse => {
                                        ProductToolFailureCategory::Execution
                                    }
                                };
                                emit_item_event_or_fail!(ActionLoopItemEvent::synthetic_failure(
                                    &request,
                                    status,
                                    category,
                                    error.code.clone(),
                                    error.message.clone(),
                                ));
                                return Err(tool_port_failure(
                                    error,
                                    network_call_made,
                                    &usage,
                                    &item_events,
                                    &mut trace,
                                    started,
                                ));
                            }
                        };
                        if result.permanent_side_effects != 0 {
                            emit_item_event_or_fail!(ActionLoopItemEvent::synthetic_failure(
                                &request,
                                ActionLoopItemStatus::Rejected,
                                ProductToolFailureCategory::Permission,
                                "PRODUCT_TOOL_PERMANENT_WRITE_REJECTED",
                                "Product Tool reported a permanent side effect before approval.",
                            ));
                            return Err(failure(
                                "PRODUCT_TOOL_PERMANENT_WRITE_REJECTED",
                                ActionLoopFailureKind::PermanentWriteRejected,
                                "Product Tool reported a permanent side effect before approval.",
                                false,
                                network_call_made,
                                &usage,
                                &item_events,
                                &mut trace,
                                started,
                                TracePhase::ProductTool,
                                TraceEventKind::Rejected,
                            ));
                        }
                        if let Err(error) = self.registry.validate_result(&request, &result) {
                            emit_item_event_or_fail!(ActionLoopItemEvent::synthetic_failure(
                                &request,
                                ActionLoopItemStatus::Rejected,
                                ProductToolFailureCategory::Schema,
                                error.code.clone(),
                                error.message.clone(),
                            ));
                            return Err(failure(
                                &error.code,
                                ActionLoopFailureKind::ProductToolSchema,
                                &error.message,
                                false,
                                network_call_made,
                                &usage,
                                &item_events,
                                &mut trace,
                                started,
                                TracePhase::ProductTool,
                                TraceEventKind::Rejected,
                            ));
                        }
                        emit_item_event_or_fail!(ActionLoopItemEvent::tool_result(
                            &request, &result
                        ));
                        if result.status != ProductToolExecutionStatus::Completed {
                            if product_tool_recovery_attempts < MAX_PRODUCT_TOOL_RECOVERY_ATTEMPTS {
                                if let Some(recovery_message) =
                                    product_tool_recovery_message(&call.name, &result)
                                {
                                    product_tool_recovery_attempts =
                                        product_tool_recovery_attempts.saturating_add(1);
                                    messages.push(ProviderMessage {
                                        role: ProviderRole::Tool,
                                        content: serde_json::to_string(&json!({
                                            "error_code": result.error_code,
                                            "message": recovery_message
                                        }))
                                        .expect("fixed Product Tool recovery message serializes"),
                                        tool_call_id: Some(call.call_id),
                                        tool_calls: Vec::new(),
                                        ephemeral_reasoning: None,
                                    });
                                    continue;
                                }
                            }
                            return Err(non_completed_tool_failure(
                                &result,
                                network_call_made,
                                &usage,
                                &item_events,
                                &mut trace,
                                started,
                            ));
                        }
                        let output = result
                            .validated_output
                            .expect("validated completed result has output");
                        let output_value = Value::Object(output.value.into_iter().collect());
                        let mut tool_completed = RedactedTraceEntry::new(
                            TracePhase::ProductTool,
                            TraceEventKind::Completed,
                            elapsed_ms(started),
                        );
                        tool_completed.call_id = Some(call.call_id.clone());
                        tool_completed.tool_name = Some(call.name.clone());
                        tool_completed.output_sha256 =
                            Some(RedactedExecutionTrace::digest_value(&output_value));
                        trace.push(tool_completed);

                        // A continuation request is a plan-only transaction.  The
                        // plan tool has already been validated by Rust, including
                        // the ActiveDesignSnapshot base version and the reviewed
                        // AssemblyDelta allow-list.  Do not let the Provider fall
                        // through into the expensive six-tool synthesis chain:
                        // that would build a second model before the user has
                        // previewed the requested edit, and can make a packaged
                        // edit turn time out.  The desktop bridge maps the plan
                        // into a normal preview ChangeSet; confirmation remains
                        // the only permanent-write boundary.
                        if is_plan_only_assembly_delta(&call.name, &output_value) {
                            trace.push(RedactedTraceEntry::new(
                                TracePhase::Final,
                                TraceEventKind::Completed,
                                elapsed_ms(started),
                            ));
                            return Ok(ActionLoopResult {
                                execution_id: input.execution_id,
                                turn_id: input.turn_id,
                                final_content:
                                    "已验证当前机械臂的增量设计方案，可在工作台预览后确认。".into(),
                                usage,
                                network_call_made,
                                item_events,
                                trace,
                            });
                        }
                        messages.push(ProviderMessage {
                            role: ProviderRole::Tool,
                            content: serde_json::to_string(&output_value).map_err(|_| {
                                failure(
                                    "PRODUCT_TOOL_OUTPUT_SERIALIZATION_FAILED",
                                    ActionLoopFailureKind::ProductToolSchema,
                                    "Validated Product Tool output could not be serialized.",
                                    false,
                                    network_call_made,
                                    &usage,
                                    &item_events,
                                    &mut trace,
                                    started,
                                    TracePhase::ProductTool,
                                    TraceEventKind::Failed,
                                )
                            })?,
                            tool_call_id: Some(call.call_id),
                            tool_calls: Vec::new(),
                            ephemeral_reasoning: None,
                        });
                    }
                }
            }
        }
    }
}

fn provider_definitions_for_context(
    registry: &ProductToolRegistry,
    context: &AgentContext,
) -> Vec<crate::ProviderToolDefinition> {
    let definitions = registry.provider_definitions();
    if !is_plan_only_continuation(context) {
        return definitions;
    }
    definitions
        .into_iter()
        .filter(|definition| definition.name == "plan_complete_concept")
        .collect()
}

fn is_plan_only_continuation(context: &AgentContext) -> bool {
    if context.active_snapshot.is_none() {
        return false;
    }
    let Some(message) = context
        .messages
        .iter()
        .rev()
        .find(|message| message.role == ContextRole::User)
    else {
        return false;
    };
    let normalized = message.content.to_ascii_lowercase();
    [
        "当前",
        "继续",
        "增加",
        "添加",
        "替换",
        "修改",
        "保留",
        "装配",
        "组装",
        "在现有",
        "in the current",
        "continue",
        "add ",
        "replace ",
        "modify ",
        "keep ",
        "assemble",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn is_plan_only_assembly_delta(tool_name: &str, output: &Value) -> bool {
    tool_name == "plan_complete_concept"
        && output
            .get("plan")
            .and_then(Value::as_object)
            .and_then(|plan| plan.get("assembly_delta"))
            .is_some_and(|delta| delta.is_object())
}

/// A malformed initial-synthesis delta is recoverable exactly once.  The
/// recovery is deliberately narrow: other Product Tool failures remain
/// terminal so a model cannot use retries to bypass the Rust-owned contract.
fn product_tool_recovery_message(
    tool_name: &str,
    result: &forgecad_app_server_protocol::ProductToolExecutionResult,
) -> Option<&'static str> {
    if tool_name != "plan_complete_concept" {
        return None;
    }
    match result.error_code.as_deref() {
        Some("ASSEMBLY_DELTA_NOT_ALLOWED_ON_INITIAL_SYNTHESIS") => Some(
            "This is an initial synthesis with no active asset. Remove assembly_delta or set it to null, then provide the complete ArmDesignIntent@1 object.",
        ),
        Some("ARM_DESIGN_INTENT_INVALID") => Some(
            "Rust rejected the ArmDesignIntent. Retry plan_complete_concept with exactly the current schema fields, only allowed enum values, source=agent_inferred, visual_only=true, and no unknown fields; architecture must be serial_chain or parallel_link for the current reviewed families.",
        ),
        Some("ASSEMBLY_DELTA_INVALID") => Some(
            "Rust rejected the AssemblyDelta. Retry with exactly AssemblyDeltaProgram@1: use the current active asset_version_id, visual_only=true, 1-8 operations, and only the reviewed recipe IDs, attachment slots, Part/Connector IDs, bounded transforms, Joint poses, or connector snaps shown in the tool schema. Do not add dimensions, ShapeProgram operations, code, or unknown fields.",
        ),
        Some("ASSEMBLY_DELTA_BASE_STALE") => Some(
            "The AssemblyDelta targeted an old version. Retry using the asset_version_id from the current Rust-owned ActiveDesignSnapshot as base_asset_version_id; do not invent or reuse a previous version ID.",
        ),
        _ => None,
    }
}

/// Repair only the argument envelope for the current plan tool.  This path is
/// intentionally separate from execution-result recovery: no geometry has
/// run, and the Provider receives a stable instruction rather than the raw
/// JSON-schema validator message.
fn product_tool_schema_recovery_message(tool_name: &str, error_code: &str) -> Option<&'static str> {
    if tool_name != "plan_complete_concept" {
        return None;
    }
    match error_code {
        "PRODUCT_TOOL_ARGUMENTS_NOT_OBJECT" | "PRODUCT_TOOL_ARGUMENT_SCHEMA_INVALID" => Some(
            "Rust rejected the plan_complete_concept argument envelope. Retry exactly one time with a single JSON object matching the current tool schema: plan must contain one direction, spec must be an object, and arm_design_intent must be a complete ArmDesignIntent@1 object for a visual-only robotic-arm concept. Do not add unknown fields, Markdown, research tools, dimensions, or executable code. If editing an existing asset, include only a valid AssemblyDeltaProgram@1 using the current ActiveDesignSnapshot asset_version_id.",
        ),
        _ => None,
    }
}

impl ActionLoopItemEvent {
    fn tool_call(request: &ProductToolExecutionRequest) -> Self {
        Self {
            sequence: 0,
            event_kind: ActionLoopItemEventKind::ToolCall,
            call_id: request.call_id.clone(),
            tool_id: request.tool_id.clone(),
            tool_name: request.tool_name.clone(),
            status: ActionLoopItemStatus::Pending,
            idempotency_key: request.idempotency_key.clone(),
            approval_policy: request.approval_policy,
            arguments: Some(request.validated_arguments.value.clone()),
            result: None,
            failure_category: None,
            error_code: None,
            message: None,
        }
    }

    fn tool_result(
        request: &ProductToolExecutionRequest,
        result: &ProductToolExecutionResult,
    ) -> Self {
        let status = match result.status {
            ProductToolExecutionStatus::Completed => ActionLoopItemStatus::Completed,
            ProductToolExecutionStatus::Failed => ActionLoopItemStatus::Failed,
            ProductToolExecutionStatus::Cancelled => ActionLoopItemStatus::Cancelled,
            ProductToolExecutionStatus::Rejected => ActionLoopItemStatus::Rejected,
        };
        Self {
            sequence: 0,
            event_kind: ActionLoopItemEventKind::ToolResult,
            call_id: request.call_id.clone(),
            tool_id: request.tool_id.clone(),
            tool_name: request.tool_name.clone(),
            status,
            idempotency_key: request.idempotency_key.clone(),
            approval_policy: request.approval_policy,
            arguments: None,
            result: result
                .validated_output
                .as_ref()
                .map(|payload| payload.value.clone()),
            failure_category: result.failure_category,
            error_code: result.error_code.clone(),
            message: result.message.clone(),
        }
    }

    fn synthetic_failure(
        request: &ProductToolExecutionRequest,
        status: ActionLoopItemStatus,
        failure_category: ProductToolFailureCategory,
        error_code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            sequence: 0,
            event_kind: ActionLoopItemEventKind::ToolResult,
            call_id: request.call_id.clone(),
            tool_id: request.tool_id.clone(),
            tool_name: request.tool_name.clone(),
            status,
            idempotency_key: request.idempotency_key.clone(),
            approval_policy: request.approval_policy,
            arguments: None,
            result: None,
            failure_category: Some(failure_category),
            error_code: Some(error_code.into()),
            message: Some(message.into()),
        }
    }
}

enum ItemEventEmitError {
    Cancelled,
    Timeout,
    Sink(ActionLoopItemEventSinkError),
}

async fn emit_item_event(
    events: &mut Vec<ActionLoopItemEvent>,
    mut event: ActionLoopItemEvent,
    sink: &dyn ActionLoopItemEventSink,
    cancellation: &CancellationToken,
    deadline: Instant,
) -> Result<(), ItemEventEmitError> {
    event.sequence = events.len() as u32 + 1;
    let sink_scope = cancellation.child_token();
    match guarded(
        sink.emit(event.clone(), sink_scope.clone()),
        sink_scope,
        deadline,
    )
    .await
    {
        Ok(()) => {}
        Err(GuardedError::Cancelled) => return Err(ItemEventEmitError::Cancelled),
        Err(GuardedError::Timeout) => return Err(ItemEventEmitError::Timeout),
        Err(GuardedError::Inner(error)) => {
            // A failed durable append/publication is a hard boundary. Cancelling
            // the root scope guarantees no subsequent Provider or Product Tool
            // work can begin after the sink failed.
            cancellation.cancel();
            return Err(ItemEventEmitError::Sink(error));
        }
    }
    if cancellation.is_cancelled() {
        return Err(ItemEventEmitError::Cancelled);
    }
    events.push(event);
    Ok(())
}

fn item_event_failure(
    error: ItemEventEmitError,
    network_call_made: bool,
    usage: &ActionLoopUsage,
    item_events: &[ActionLoopItemEvent],
    trace: &mut RedactedExecutionTrace,
    started: Instant,
) -> ActionLoopFailure {
    match error {
        ItemEventEmitError::Cancelled => failure(
            "ACTION_LOOP_CANCELLED",
            ActionLoopFailureKind::Cancelled,
            "Action Loop Item publication was cancelled; late output is rejected.",
            true,
            network_call_made,
            usage,
            item_events,
            trace,
            started,
            TracePhase::Cancellation,
            TraceEventKind::Cancelled,
        ),
        ItemEventEmitError::Timeout => failure(
            "ACTION_LOOP_WALL_TIME_EXCEEDED",
            ActionLoopFailureKind::WallTimeBudget,
            "Action Loop exceeded its wall-time budget during Item publication.",
            true,
            network_call_made,
            usage,
            item_events,
            trace,
            started,
            TracePhase::Budget,
            TraceEventKind::BudgetExceeded,
        ),
        ItemEventEmitError::Sink(error) => failure(
            "ACTION_LOOP_ITEM_EVENT_PERSISTENCE_FAILED",
            ActionLoopFailureKind::ItemEventPersistence,
            "Action Loop stopped because an incremental Item could not be persisted and published.",
            error.recoverable,
            network_call_made,
            usage,
            item_events,
            trace,
            started,
            TracePhase::ProductTool,
            TraceEventKind::Failed,
        ),
    }
}

fn provider_error_supports_schema_repair(error: &ProviderError) -> bool {
    matches!(
        error.code.as_str(),
        "PROVIDER_INVALID_JSON"
            | "PROVIDER_SCHEMA_TOOL_ARGUMENTS_INVALID_JSON"
            | "PROVIDER_SCHEMA_TOOL_ARGUMENTS_OBJECT"
            | "PROVIDER_SCHEMA_TOOL_REQUIRED_FIELD"
    )
}

fn context_messages(context: &AgentContext) -> Vec<ProviderMessage> {
    let mut messages: Vec<ProviderMessage> = context
        .messages
        .iter()
        .map(|message| ProviderMessage {
            role: match message.role {
                ContextRole::System => ProviderRole::System,
                ContextRole::User => ProviderRole::User,
                ContextRole::Assistant => ProviderRole::Assistant,
                ContextRole::Tool => ProviderRole::Tool,
            },
            content: message.content.clone(),
            tool_call_id: message.tool_call_id.clone(),
            tool_calls: Vec::new(),
            ephemeral_reasoning: None,
        })
        .collect();
    if let Some(snapshot) = &context.active_snapshot {
        // Snapshot is a read-only design projection. Keep it as an explicit
        // system message so a Provider can produce an AssemblyDelta relative
        // to the current asset instead of silently starting a new design.
        // ContextBuilder has already rejected secrets, paths and unbounded
        // values before this conversion.
        let snapshot_message = ProviderMessage {
            role: ProviderRole::System,
            content: format!(
                "当前 Rust-owned ActiveDesignSnapshot（只读编辑上下文）：{}",
                canonical_json(snapshot)
            ),
            tool_call_id: None,
            tool_calls: Vec::new(),
            ephemeral_reasoning: None,
        };
        let insert_at = messages
            .iter()
            .take_while(|message| message.role == ProviderRole::System)
            .count()
            .min(messages.len());
        messages.insert(insert_at, snapshot_message);
    }
    messages
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u64::MAX as u128) as u64
}

enum GuardedError<E> {
    Cancelled,
    Timeout,
    Inner(E),
}

async fn guarded<T, E, F>(
    future: F,
    cancellation: CancellationToken,
    deadline: Instant,
) -> Result<T, GuardedError<E>>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        cancellation.cancel();
        return Err(GuardedError::Timeout);
    }
    let mut future = Box::pin(future);
    let timeout_cancellation = cancellation.clone();
    let mut cancelled = Box::pin(cancellation.cancelled_owned());
    let mut timeout = Box::pin(tokio::time::sleep(remaining));
    std::future::poll_fn(move |context| {
        if cancelled.as_mut().poll(context).is_ready() {
            return std::task::Poll::Ready(Err(GuardedError::Cancelled));
        }
        if let std::task::Poll::Ready(result) = future.as_mut().poll(context) {
            return std::task::Poll::Ready(result.map_err(GuardedError::Inner));
        }
        if timeout.as_mut().poll(context).is_ready() {
            timeout_cancellation.cancel();
            return std::task::Poll::Ready(Err(GuardedError::Timeout));
        }
        std::task::Poll::Pending
    })
    .await
}

#[allow(clippy::too_many_arguments)]
fn failure(
    code: &str,
    kind: ActionLoopFailureKind,
    message: &str,
    recoverable: bool,
    network_call_made: bool,
    usage: &ActionLoopUsage,
    item_events: &[ActionLoopItemEvent],
    trace: &mut RedactedExecutionTrace,
    started: Instant,
    phase: TracePhase,
    event: TraceEventKind,
) -> ActionLoopFailure {
    let mut entry = RedactedTraceEntry::new(phase, event, elapsed_ms(started));
    entry.error_code = Some(code.into());
    entry.network_call_made = network_call_made;
    trace.push(entry);
    ActionLoopFailure {
        code: code.into(),
        kind,
        message: message.into(),
        recoverable,
        network_call_made,
        usage: usage.clone(),
        item_events: item_events.to_vec(),
        trace: trace.clone(),
    }
}

fn provider_failure(
    error: ProviderError,
    usage: &ActionLoopUsage,
    item_events: &[ActionLoopItemEvent],
    trace: &mut RedactedExecutionTrace,
    started: Instant,
) -> ActionLoopFailure {
    let mut entry = RedactedTraceEntry::new(
        TracePhase::Provider,
        TraceEventKind::Failed,
        elapsed_ms(started),
    );
    entry.error_code = Some(error.code.clone());
    entry.provider_failure_category = Some(error.category.clone());
    entry.network_call_made = error.network_call_made;
    trace.push(entry);
    ActionLoopFailure {
        code: error.code,
        kind: ActionLoopFailureKind::Provider,
        message: error.message,
        recoverable: error.recoverable,
        network_call_made: error.network_call_made,
        usage: usage.clone(),
        item_events: item_events.to_vec(),
        trace: trace.clone(),
    }
}

fn tool_port_failure(
    error: ProductToolPortError,
    network_call_made: bool,
    usage: &ActionLoopUsage,
    item_events: &[ActionLoopItemEvent],
    trace: &mut RedactedExecutionTrace,
    started: Instant,
) -> ActionLoopFailure {
    failure(
        &error.code,
        ActionLoopFailureKind::ProductTool,
        &error.message,
        error.recoverable,
        network_call_made,
        usage,
        item_events,
        trace,
        started,
        TracePhase::ProductTool,
        TraceEventKind::Failed,
    )
}

fn non_completed_tool_failure(
    result: &forgecad_app_server_protocol::ProductToolExecutionResult,
    network_call_made: bool,
    usage: &ActionLoopUsage,
    item_events: &[ActionLoopItemEvent],
    trace: &mut RedactedExecutionTrace,
    started: Instant,
) -> ActionLoopFailure {
    let code = result
        .error_code
        .as_deref()
        .unwrap_or("PRODUCT_TOOL_EXECUTION_FAILED");
    let message = result
        .message
        .as_deref()
        .unwrap_or("Product Tool execution did not complete.");
    let kind = if result.failure_category == Some(ProductToolFailureCategory::Permission) {
        ActionLoopFailureKind::PermanentWriteRejected
    } else {
        ActionLoopFailureKind::ProductTool
    };
    failure(
        code,
        kind,
        message,
        result.status == ProductToolExecutionStatus::Cancelled,
        network_call_made,
        usage,
        item_events,
        trace,
        started,
        TracePhase::ProductTool,
        TraceEventKind::Failed,
    )
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, VecDeque},
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Mutex,
        },
    };

    use forgecad_app_server_protocol::{
        ProductToolExecutionRequest, ProductToolExecutionResult, ValidatedProductToolPayload,
        PRODUCT_TOOL_EXECUTION_RESULT_SCHEMA_VERSION,
    };

    use crate::{
        ContextBuildInput, ContextBuilder, ContextMessage, EphemeralReasoning, FakeDeepSeekClient,
        ProductToolPortFuture, ProviderError, ProviderFuture, ProviderHealthCheck,
        ProviderPreflight, ProviderResponse, ProviderToolCall,
    };

    use super::*;

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[derive(Clone)]
    struct FakeExecutor {
        output_schema_sha256: String,
        delay_ms: u64,
        permanent_side_effects: u32,
        calls: Arc<AtomicUsize>,
        completed: Arc<AtomicUsize>,
        captured: Arc<Mutex<VecDeque<ProductToolExecutionRequest>>>,
    }

    impl FakeExecutor {
        fn new(registry: &ProductToolRegistry) -> Self {
            Self {
                output_schema_sha256: registry
                    .definition("compile_readback_candidate")
                    .unwrap()
                    .output_schema_sha256
                    .clone(),
                delay_ms: 0,
                permanent_side_effects: 0,
                calls: Arc::new(AtomicUsize::new(0)),
                completed: Arc::new(AtomicUsize::new(0)),
                captured: Arc::new(Mutex::new(VecDeque::new())),
            }
        }
    }

    impl ProductToolExecutorPort for FakeExecutor {
        fn execute(
            &self,
            request: ProductToolExecutionRequest,
            _cancellation: CancellationToken,
        ) -> ProductToolPortFuture {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.captured.lock().unwrap().push_back(request.clone());
            let delay_ms = self.delay_ms;
            let completed = self.completed.clone();
            let output_schema_sha256 = self.output_schema_sha256.clone();
            let permanent_side_effects = self.permanent_side_effects;
            Box::pin(async move {
                if delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
                completed.fetch_add(1, Ordering::SeqCst);
                Ok(ProductToolExecutionResult {
                    schema_version: PRODUCT_TOOL_EXECUTION_RESULT_SCHEMA_VERSION.into(),
                    execution_id: request.execution_id,
                    turn_id: request.turn_id,
                    call_id: request.call_id,
                    tool_id: request.tool_id.clone(),
                    cancellation_id: request.cancellation_id,
                    status: ProductToolExecutionStatus::Completed,
                    validated_output: Some(ValidatedProductToolPayload {
                        schema_id: format!("{}:output", request.tool_id),
                        schema_sha256: output_schema_sha256,
                        value: BTreeMap::from([
                            ("triangle_count".into(), json!(1200)),
                            ("bounds_mm".into(), json!([100, 40, 30])),
                            ("mesh_count".into(), json!(2)),
                            ("primitive_count".into(), json!(3)),
                            ("material_count".into(), json!(2)),
                            (
                                "evidence_source".into(),
                                json!("geometry_compile_glb_readback"),
                            ),
                        ]),
                    }),
                    failure_category: None,
                    error_code: None,
                    message: None,
                    duration_ms: delay_ms,
                    permanent_side_effects,
                })
            })
        }
    }

    #[derive(Default)]
    struct FailingItemEventSink {
        emissions: AtomicUsize,
    }

    impl ActionLoopItemEventSink for FailingItemEventSink {
        fn emit(
            &self,
            _event: ActionLoopItemEvent,
            _cancellation: CancellationToken,
        ) -> ActionLoopItemEventSinkFuture {
            self.emissions.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Err(ActionLoopItemEventSinkError {
                    code: "TEST_ITEM_SINK_FAILED".into(),
                    message: "Injected incremental Item sink failure.".into(),
                    recoverable: true,
                })
            })
        }
    }

    #[derive(Clone)]
    struct StatefulChainExecutor {
        output_schema_sha256: Arc<BTreeMap<String, String>>,
        expected_tool_ids: Arc<Vec<String>>,
        next: Arc<AtomicUsize>,
        captured: Arc<Mutex<Vec<ProductToolExecutionRequest>>>,
    }

    impl StatefulChainExecutor {
        fn new(registry: &ProductToolRegistry, expected_names: &[&str]) -> Self {
            let output_schema_sha256 = registry
                .definitions()
                .map(|definition| {
                    (
                        definition.tool_id.clone(),
                        definition.output_schema_sha256.clone(),
                    )
                })
                .collect();
            let expected_tool_ids = expected_names
                .iter()
                .map(|name| registry.definition(name).unwrap().tool_id.clone())
                .collect();
            Self {
                output_schema_sha256: Arc::new(output_schema_sha256),
                expected_tool_ids: Arc::new(expected_tool_ids),
                next: Arc::new(AtomicUsize::new(0)),
                captured: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl ProductToolExecutorPort for StatefulChainExecutor {
        fn execute(
            &self,
            request: ProductToolExecutionRequest,
            _cancellation: CancellationToken,
        ) -> ProductToolPortFuture {
            let index = self.next.fetch_add(1, Ordering::SeqCst);
            let expected = self.expected_tool_ids.get(index).cloned();
            self.captured.lock().unwrap().push(request.clone());
            let schema_sha256 = self.output_schema_sha256.get(&request.tool_id).cloned();
            Box::pin(async move {
                if expected.as_deref() != Some(request.tool_id.as_str()) {
                    return Err(ProductToolPortError::invalid_response(
                        "Stateful executor received Product Tools out of order.",
                    ));
                }
                let schema_sha256 = schema_sha256.ok_or_else(|| {
                    ProductToolPortError::invalid_response(
                        "Stateful executor received an unknown Product Tool ID.",
                    )
                })?;
                let value = stateful_output(&request.tool_id);
                Ok(ProductToolExecutionResult {
                    schema_version: PRODUCT_TOOL_EXECUTION_RESULT_SCHEMA_VERSION.into(),
                    execution_id: request.execution_id,
                    turn_id: request.turn_id,
                    call_id: request.call_id,
                    tool_id: request.tool_id.clone(),
                    cancellation_id: request.cancellation_id,
                    status: ProductToolExecutionStatus::Completed,
                    validated_output: Some(ValidatedProductToolPayload {
                        schema_id: format!("{}:output", request.tool_id),
                        schema_sha256,
                        value,
                    }),
                    failure_category: None,
                    error_code: None,
                    message: None,
                    duration_ms: 1,
                    permanent_side_effects: 0,
                })
            })
        }
    }

    fn stateful_output(tool_id: &str) -> BTreeMap<String, Value> {
        let value = match tool_id {
            "forgecad.plan.complete_concept.v1" => {
                json!({"plan": {"plan_id": "plan_primary"}, "accepted": true})
            }
            "forgecad.geometry.build.v1" => json!({
                "direction_id": "direction_primary",
                "topology_hash": "a".repeat(64),
                "triangle_count": 1200,
                "bounds_mm": [100, 40, 30],
                "candidate_only": true
            }),
            "forgecad.geometry.compile_readback.v1" => json!({
                "triangle_count": 1200,
                "bounds_mm": [100, 40, 30],
                "mesh_count": 2,
                "primitive_count": 3,
                "material_count": 2,
                "evidence_source": "geometry_compile_glb_readback"
            }),
            "forgecad.render.concept.v1" => json!({
                "view_ids": ["front", "iso", "side", "top"],
                "view_sha256": {},
                "renderer_id": "forgecad-agent-software-raster@1"
            }),
            "forgecad.candidate.evaluate.v1" => json!({
                "hard_gate_passed": true,
                "checks": {},
                "evidence_source": "geometry_compile_glb_readback+concept_render_readback"
            }),
            "forgecad.preview.prepare.v1" => json!({
                "preview_id": "preview_1",
                "topology_hash": "a".repeat(64),
                "view_sha256": {},
                "requires_user_confirmation": true,
                "permanent_side_effects": 0
            }),
            _ => json!({}),
        };
        value
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }

    fn context() -> AgentContext {
        ContextBuilder
            .build(ContextBuildInput {
                system_prompt: "只生成非功能性的生产级概念资产。".into(),
                thread_summary: String::new(),
                recent_messages: vec![ContextMessage {
                    role: ContextRole::User,
                    content: "创建唯一最佳候选。".into(),
                    name: None,
                    tool_call_id: None,
                }],
                active_snapshot: Some(json!({"snapshot_id": "snapshot_1"})),
                allowed_component_ids: Vec::new(),
                allowed_material_ids: Vec::new(),
                tools: Vec::new(),
            })
            .unwrap()
    }

    fn input() -> ActionLoopInput {
        ActionLoopInput {
            execution_id: "execution_1".into(),
            turn_id: "turn_1".into(),
            cancellation_id: "cancel_1".into(),
            cancellation_token: "cancel_token_1".into(),
            provider_id: "deepseek".into(),
            provider_preflight: None,
            context: context(),
        }
    }

    #[test]
    fn active_arm_edit_context_advertises_only_plan_tool() {
        let mut edit_context = context();
        edit_context.messages.push(ContextMessage {
            role: ContextRole::User,
            content: "在当前已确认的机械臂上继续设计，增加一个传感器舱。".into(),
            name: None,
            tool_call_id: None,
        });
        let definitions =
            provider_definitions_for_context(&ProductToolRegistry::default(), &edit_context);
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].name, "plan_complete_concept");
    }

    #[test]
    fn active_initial_context_keeps_full_discovery_tool_projection() {
        let definitions =
            provider_definitions_for_context(&ProductToolRegistry::default(), &context());
        assert!(definitions.len() > 1);
        assert!(definitions
            .iter()
            .any(|definition| definition.name == "infer_product_domain"));
        assert!(definitions
            .iter()
            .any(|definition| definition.name == "plan_complete_concept"));
    }

    fn tool_response(call_id: &str) -> ProviderResponse {
        named_tool_response(call_id, "compile_readback_candidate", json!({}))
    }

    fn named_tool_response(call_id: &str, name: &str, arguments: Value) -> ProviderResponse {
        ProviderResponse {
            content: None,
            tool_calls: vec![ProviderToolCall {
                call_id: call_id.into(),
                name: name.into(),
                arguments,
            }],
            ephemeral_reasoning: Some(EphemeralReasoning::new("private chain of thought")),
            usage: ProviderUsage {
                input_tokens: 10,
                output_tokens: 3,
                prompt_cache_hit_tokens: 6,
                prompt_cache_miss_tokens: 4,
                estimated_cost_microusd: 2,
            },
            finish_reason: ProviderFinishReason::ToolCalls,
            network_call_made: true,
        }
    }

    fn complete_plan_arguments() -> Value {
        let direction = |id: &str, silhouette: &str| {
            json!({
                "direction_id": id,
                "title": "候选方向",
                "summary": "完整的非功能机械概念外观。",
                "silhouette": silhouette,
                "primary_part_roles": ["body_shell", "control_panel"],
                "material_direction": "深色阳极金属与聚合物"
            })
        };
        json!({
            "plan": {
                "plan_id": "plan_primary",
                "domain_pack_id": "pack_future_prop",
                "brief": "生成一个非功能性的未来机械概念道具。",
                "spec": {},
                "provider_id": "deepseek",
                "directions": [
                    direction("direction_primary", "compact")
                ]
            }
        })
    }

    #[test]
    fn assembly_delta_plan_is_plan_only_and_other_plans_keep_the_synthesis_chain() {
        let mut continuation = complete_plan_arguments();
        continuation["plan"]["assembly_delta"] = json!({
            "schema_version": "AssemblyDeltaProgram@1",
            "domain_pack_id": "pack_robotic_arm_concept",
            "base_asset_version_id": "assetver_current",
            "summary": "增加腕部视觉护盖",
            "operations": [],
            "visual_only": true
        });
        assert!(is_plan_only_assembly_delta(
            "plan_complete_concept",
            &json!({"plan": continuation["plan"].clone()})
        ));
        assert!(!is_plan_only_assembly_delta(
            "plan_complete_concept",
            &json!({"plan": complete_plan_arguments()["plan"].clone()})
        ));
        assert!(!is_plan_only_assembly_delta(
            "build_candidate_geometry",
            &json!({"plan": continuation["plan"].clone()})
        ));
    }

    fn final_response() -> ProviderResponse {
        ProviderResponse {
            content: Some("唯一生产概念候选已准备完成。".into()),
            tool_calls: Vec::new(),
            ephemeral_reasoning: None,
            usage: ProviderUsage {
                input_tokens: 12,
                output_tokens: 5,
                prompt_cache_hit_tokens: 7,
                prompt_cache_miss_tokens: 5,
                estimated_cost_microusd: 2,
            },
            finish_reason: ProviderFinishReason::Stop,
            network_call_made: true,
        }
    }

    #[derive(Clone, Default)]
    struct AttemptingBlockingProvider {
        started: Arc<AtomicBool>,
        stream_cancellation: Arc<Mutex<Option<CancellationToken>>>,
    }

    impl ProviderClient for AttemptingBlockingProvider {
        fn preflight(&self, _cancellation: CancellationToken) -> ProviderFuture<ProviderPreflight> {
            Box::pin(async {
                Ok(ProviderPreflight {
                    provider_id: "deepseek".into(),
                    model: "deepseek-chat".into(),
                    configured: true,
                    streaming: true,
                    tool_calls: true,
                    network_call_made: false,
                })
            })
        }

        fn check(
            &self,
            provider_id: String,
            _timeout_ms: u32,
            _cancellation: CancellationToken,
        ) -> ProviderFuture<ProviderHealthCheck> {
            Box::pin(async move {
                Ok(ProviderHealthCheck {
                    provider_id,
                    network_call_made: true,
                    usage: None,
                })
            })
        }

        fn request_budget_policy(
            &self,
            _request: &ProviderRequest,
        ) -> Result<crate::ProviderRequestBudgetPolicy, ProviderError> {
            Ok(crate::ProviderRequestBudgetPolicy {
                input_tokens_upper_bound: 1,
                input_cost_ceiling_microusd: 1,
                output_microusd_per_million_tokens: 1,
            })
        }

        fn stream(
            &self,
            _request: ProviderRequest,
            cancellation: CancellationToken,
            mut events: crate::ProviderEventSink,
        ) -> ProviderFuture<ProviderResponse> {
            let started = self.started.clone();
            let stream_cancellation = self.stream_cancellation.clone();
            Box::pin(async move {
                *stream_cancellation.lock().unwrap() = Some(cancellation);
                events(ProviderStreamEvent::NetworkRequestStarted);
                started.store(true, Ordering::SeqCst);
                std::future::pending::<Result<ProviderResponse, ProviderError>>().await
            })
        }

        fn cancel(
            &self,
            _cancellation_id: String,
            _cancellation_token: String,
        ) -> ProviderFuture<bool> {
            Box::pin(async { Ok(true) })
        }
    }

    #[test]
    fn active_snapshot_is_forwarded_as_read_only_provider_context() {
        let messages = context_messages(&context());
        assert_eq!(messages[0].role, ProviderRole::System);
        assert!(messages[1]
            .content
            .contains("当前 Rust-owned ActiveDesignSnapshot"));
        assert!(messages[1].content.contains("snapshot_1"));
        assert_eq!(messages[2].role, ProviderRole::User);
        assert!(messages[0]
            .content
            .contains("只生成非功能性的生产级概念资产"));
    }

    #[test]
    fn tool_loop_forwards_reasoning_ephemerally_and_returns_only_redacted_evidence() {
        block_on(async {
            let registry = ProductToolRegistry::default();
            let executor = FakeExecutor::new(&registry);
            let provider = FakeDeepSeekClient::scripted(
                "deepseek-chat",
                true,
                true,
                vec![Ok(tool_response("call_1")), Ok(final_response())],
            );
            let records = provider.clone();
            let loop_ = ActionLoop::new(
                Arc::new(provider),
                Arc::new(executor),
                registry,
                ActionLoopConfig::default(),
            )
            .unwrap();
            let result = loop_.run(input(), CancellationToken::new()).await.unwrap();
            assert_eq!(result.usage.product_tool_calls, 1);
            assert_eq!(records.records()[1].prior_reasoning_count, 1);
            let serialized = serde_json::to_string(&result).unwrap();
            for forbidden in ["private chain of thought", "reasoning_content", "api_key"] {
                assert!(!serialized.contains(forbidden));
            }
            assert!(serialized.contains("geometry_compile_glb_readback"));
            let trace = serde_json::to_string(&result.trace).unwrap();
            assert!(!trace.contains("geometry_compile_glb_readback"));
        });
    }

    #[test]
    fn malformed_provider_json_gets_one_bounded_repair_before_any_tool_execution() {
        block_on(async {
            let registry = ProductToolRegistry::default();
            let executor = FakeExecutor::new(&registry);
            let executor_calls = executor.calls.clone();
            let provider = FakeDeepSeekClient::scripted(
                "deepseek-chat",
                true,
                true,
                vec![Err(ProviderError::invalid_json(true)), Ok(final_response())],
            );
            let records = provider.clone();
            let result = ActionLoop::new(
                Arc::new(provider),
                Arc::new(executor),
                registry,
                ActionLoopConfig::default(),
            )
            .unwrap()
            .run(input(), CancellationToken::new())
            .await
            .unwrap();

            assert_eq!(records.records().len(), 2);
            assert_eq!(executor_calls.load(Ordering::SeqCst), 0);
            assert_eq!(result.usage.provider_requests, 1);
            assert!(result.trace.entries.iter().any(|entry| {
                entry.error_code.as_deref() == Some("PROVIDER_SCHEMA_REPAIR_REQUESTED")
            }));
        });
    }

    #[test]
    fn malformed_provider_json_repair_is_hard_bounded() {
        block_on(async {
            let registry = ProductToolRegistry::default();
            let provider = FakeDeepSeekClient::scripted(
                "deepseek-chat",
                true,
                true,
                vec![
                    Err(ProviderError::schema_mismatch_with_code(
                        "PROVIDER_SCHEMA_TOOL_ARGUMENTS_INVALID_JSON",
                        "invalid tool arguments",
                        true,
                    )),
                    Err(ProviderError::invalid_json(true)),
                ],
            );
            let records = provider.clone();
            let failure = ActionLoop::new(
                Arc::new(provider),
                Arc::new(FakeExecutor::new(&registry)),
                registry,
                ActionLoopConfig::default(),
            )
            .unwrap()
            .run(input(), CancellationToken::new())
            .await
            .unwrap_err();

            assert_eq!(failure.code, "PROVIDER_INVALID_JSON");
            assert_eq!(records.records().len(), 2);
            assert_eq!(
                failure
                    .trace
                    .entries
                    .iter()
                    .filter(|entry| {
                        entry.error_code.as_deref() == Some("PROVIDER_SCHEMA_REPAIR_REQUESTED")
                    })
                    .count(),
                1
            );
        });
    }

    #[test]
    fn invalid_plan_arguments_get_one_redacted_schema_repair_before_execution() {
        block_on(async {
            let registry = ProductToolRegistry::default();
            let executor = StatefulChainExecutor::new(&registry, &["plan_complete_concept"]);
            let executor_calls = executor.next.clone();
            let provider = FakeDeepSeekClient::scripted(
                "deepseek-chat",
                true,
                true,
                vec![
                    Ok(named_tool_response(
                        "bad_plan",
                        "plan_complete_concept",
                        json!({"plan": {}}),
                    )),
                    Ok(named_tool_response(
                        "good_plan",
                        "plan_complete_concept",
                        complete_plan_arguments(),
                    )),
                    Ok(final_response()),
                ],
            );
            let records = provider.clone();
            let result = ActionLoop::new(
                Arc::new(provider),
                Arc::new(executor),
                registry,
                ActionLoopConfig::default(),
            )
            .unwrap()
            .run(input(), CancellationToken::new())
            .await
            .unwrap();

            assert_eq!(records.records().len(), 3);
            assert_eq!(result.usage.product_tool_calls, 2);
            assert_eq!(executor_calls.load(Ordering::SeqCst), 1);
            assert!(result.trace.entries.iter().any(|entry| {
                entry.error_code.as_deref() == Some("PRODUCT_TOOL_SCHEMA_REPAIR_REQUESTED")
            }));
            let serialized = serde_json::to_string(&result).unwrap();
            assert!(!serialized.contains("Product Tool arguments must be a JSON object."));
        });
    }

    #[test]
    fn item_sink_failure_cancels_before_tool_execution_or_followup_provider_work() {
        block_on(async {
            let registry = ProductToolRegistry::default();
            let executor = FakeExecutor::new(&registry);
            let executor_calls = executor.calls.clone();
            let provider = FakeDeepSeekClient::scripted(
                "deepseek-chat",
                true,
                true,
                vec![Ok(tool_response("call_1")), Ok(final_response())],
            );
            let provider_records = provider.clone();
            let sink = Arc::new(FailingItemEventSink::default());
            let cancellation = CancellationToken::new();
            let failure = ActionLoop::new(
                Arc::new(provider),
                Arc::new(executor),
                registry,
                ActionLoopConfig::default(),
            )
            .unwrap()
            .run_with_item_event_sink(input(), cancellation.clone(), sink.clone())
            .await
            .unwrap_err();

            assert_eq!(failure.kind, ActionLoopFailureKind::ItemEventPersistence);
            assert_eq!(failure.code, "ACTION_LOOP_ITEM_EVENT_PERSISTENCE_FAILED");
            assert!(cancellation.is_cancelled());
            assert_eq!(sink.emissions.load(Ordering::SeqCst), 1);
            assert_eq!(executor_calls.load(Ordering::SeqCst), 0);
            assert_eq!(provider_records.records().len(), 1);
            assert!(failure.item_events.is_empty());
        });
    }

    #[test]
    fn offline_fake_chain_is_supported_without_claiming_network() {
        block_on(async {
            let registry = ProductToolRegistry::default();
            let executor = FakeExecutor::new(&registry);
            let provider = FakeDeepSeekClient::scripted(
                "offline-planner",
                true,
                false,
                vec![Ok(final_response())],
            );
            let records = provider.clone();
            let result = ActionLoop::new(
                Arc::new(provider),
                Arc::new(executor),
                registry,
                ActionLoopConfig::default(),
            )
            .unwrap()
            .run(input(), CancellationToken::new())
            .await
            .unwrap();
            assert!(!result.network_call_made);
            assert_eq!(records.records()[0].model, "offline-planner");
        });
    }

    #[test]
    fn provider_attempt_latch_survives_cancellation_and_cancels_child_scope() {
        block_on(async {
            let registry = ProductToolRegistry::default();
            let provider = AttemptingBlockingProvider::default();
            let observed = provider.clone();
            let loop_ = ActionLoop::new(
                Arc::new(provider),
                Arc::new(FakeExecutor::new(&registry)),
                registry,
                ActionLoopConfig::default(),
            )
            .unwrap();
            let cancellation = CancellationToken::new();
            let task_cancellation = cancellation.clone();
            let task = tokio::spawn(async move { loop_.run(input(), task_cancellation).await });
            tokio::time::timeout(Duration::from_secs(1), async {
                while !observed.started.load(Ordering::SeqCst) {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            cancellation.cancel();
            let failure = task.await.unwrap().unwrap_err();
            assert_eq!(failure.kind, ActionLoopFailureKind::Cancelled);
            assert!(failure.network_call_made);
            assert!(observed
                .stream_cancellation
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(CancellationToken::is_cancelled));
        });
    }

    #[test]
    fn provider_timeout_preserves_attempt_truth_and_cancels_child_scope() {
        block_on(async {
            let registry = ProductToolRegistry::default();
            let provider = AttemptingBlockingProvider::default();
            let observed = provider.clone();
            let mut config = ActionLoopConfig::default();
            config.max_wall_time_ms = 20;
            let failure = ActionLoop::new(
                Arc::new(provider),
                Arc::new(FakeExecutor::new(&registry)),
                registry,
                config,
            )
            .unwrap()
            .run(input(), CancellationToken::new())
            .await
            .unwrap_err();
            assert_eq!(failure.kind, ActionLoopFailureKind::WallTimeBudget);
            assert!(failure.network_call_made);
            assert!(observed.started.load(Ordering::SeqCst));
            assert!(observed
                .stream_cancellation
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(CancellationToken::is_cancelled));
        });
    }

    #[test]
    fn model_switch_uses_each_execution_preflight_metadata_without_runtime_restart() {
        block_on(async {
            let registry = ProductToolRegistry::default();
            let first = FakeDeepSeekClient::scripted(
                "deepseek-chat-v1",
                true,
                false,
                vec![Ok(final_response())],
            );
            let first_records = first.clone();
            ActionLoop::new(
                Arc::new(first),
                Arc::new(FakeExecutor::new(&registry)),
                registry.clone(),
                ActionLoopConfig::default(),
            )
            .unwrap()
            .run(input(), CancellationToken::new())
            .await
            .unwrap();
            assert_eq!(first_records.records()[0].model, "deepseek-chat-v1");

            let switched = FakeDeepSeekClient::scripted(
                "deepseek-reasoner-v2",
                true,
                false,
                vec![Ok(final_response())],
            );
            let switched_records = switched.clone();
            ActionLoop::new(
                Arc::new(switched),
                Arc::new(FakeExecutor::new(&registry)),
                registry,
                ActionLoopConfig::default(),
            )
            .unwrap()
            .run(input(), CancellationToken::new())
            .await
            .unwrap();
            assert_eq!(switched_records.records()[0].model, "deepseek-reasoner-v2");
        });
    }

    #[test]
    fn six_tool_stateful_chain_reuses_executor_identity_and_emits_alternating_items() {
        block_on(async {
            let registry = ProductToolRegistry::default();
            let names = [
                "plan_complete_concept",
                "build_candidate_geometry",
                "compile_readback_candidate",
                "render_candidate_views",
                "evaluate_candidate",
                "prepare_candidate_preview",
            ];
            let executor = StatefulChainExecutor::new(&registry, &names);
            let captured = executor.captured.clone();
            let completed = executor.next.clone();
            let provider = FakeDeepSeekClient::scripted(
                "deepseek-chat",
                true,
                true,
                vec![
                    Ok(named_tool_response(
                        "call_plan",
                        "plan_complete_concept",
                        complete_plan_arguments(),
                    )),
                    Ok(named_tool_response(
                        "call_build",
                        "build_candidate_geometry",
                        json!({
                            "direction_id": "direction_primary",
                            "presentation_profile": "showcase"
                        }),
                    )),
                    Ok(named_tool_response(
                        "call_compile",
                        "compile_readback_candidate",
                        json!({}),
                    )),
                    Ok(named_tool_response(
                        "call_render",
                        "render_candidate_views",
                        json!({}),
                    )),
                    Ok(named_tool_response(
                        "call_evaluate",
                        "evaluate_candidate",
                        json!({}),
                    )),
                    Ok(named_tool_response(
                        "call_preview",
                        "prepare_candidate_preview",
                        json!({}),
                    )),
                    Ok(final_response()),
                ],
            );
            let result = ActionLoop::new(
                Arc::new(provider),
                Arc::new(executor),
                registry,
                ActionLoopConfig::default(),
            )
            .unwrap()
            .run(input(), CancellationToken::new())
            .await
            .unwrap();

            assert_eq!(completed.load(Ordering::SeqCst), 6);
            assert_eq!(result.item_events.len(), 12);
            for (index, pair) in result.item_events.chunks_exact(2).enumerate() {
                assert_eq!(pair[0].sequence, (index * 2 + 1) as u32);
                assert_eq!(pair[1].sequence, (index * 2 + 2) as u32);
                assert_eq!(pair[0].event_kind, ActionLoopItemEventKind::ToolCall);
                assert_eq!(pair[0].status, ActionLoopItemStatus::Pending);
                assert_eq!(pair[1].event_kind, ActionLoopItemEventKind::ToolResult);
                assert_eq!(pair[1].status, ActionLoopItemStatus::Completed);
                assert_eq!(pair[0].call_id, pair[1].call_id);
                assert_eq!(pair[0].tool_name, names[index]);
                assert_eq!(pair[1].tool_name, names[index]);
            }
            let requests = captured.lock().unwrap();
            assert_eq!(requests.len(), 6);
            assert!(requests.iter().all(|request| {
                request.execution_id == "execution_1"
                    && request.cancellation_id == "cancel_1"
                    && request.cancellation_token == "cancel_token_1"
            }));
        });
    }

    #[test]
    fn thirteenth_tool_call_is_rejected_before_executor_invocation() {
        block_on(async {
            let registry = ProductToolRegistry::default();
            let executor = FakeExecutor::new(&registry);
            let call_counter = executor.calls.clone();
            let scripts = (1..=13)
                .map(|index| Ok(tool_response(&format!("call_{index}"))))
                .collect();
            let provider = FakeDeepSeekClient::scripted("deepseek-chat", true, true, scripts);
            let failure = ActionLoop::new(
                Arc::new(provider),
                Arc::new(executor),
                registry,
                ActionLoopConfig::default(),
            )
            .unwrap()
            .run(input(), CancellationToken::new())
            .await
            .unwrap_err();
            assert_eq!(failure.kind, ActionLoopFailureKind::ProductToolBudget);
            assert_eq!(call_counter.load(Ordering::SeqCst), 12);
        });
    }

    #[test]
    fn token_cost_and_wall_budgets_fail_closed() {
        block_on(async {
            for (config, expected) in [
                (
                    ActionLoopConfig {
                        max_total_tokens: 1,
                        ..ActionLoopConfig::default()
                    },
                    ActionLoopFailureKind::TokenBudget,
                ),
                (
                    ActionLoopConfig {
                        max_estimated_cost_microusd: 1,
                        ..ActionLoopConfig::default()
                    },
                    ActionLoopFailureKind::CostBudget,
                ),
            ] {
                let registry = ProductToolRegistry::default();
                let provider = FakeDeepSeekClient::scripted(
                    "deepseek-chat",
                    true,
                    true,
                    vec![Ok(final_response())],
                );
                let failure = ActionLoop::new(
                    Arc::new(provider),
                    Arc::new(FakeExecutor::new(&registry)),
                    registry,
                    config,
                )
                .unwrap()
                .run(input(), CancellationToken::new())
                .await
                .unwrap_err();
                assert_eq!(failure.kind, expected);
            }

            let registry = ProductToolRegistry::default();
            let mut executor = FakeExecutor::new(&registry);
            executor.delay_ms = 50;
            let provider = FakeDeepSeekClient::scripted(
                "deepseek-chat",
                true,
                true,
                vec![Ok(tool_response("call_1"))],
            );
            let failure = ActionLoop::new(
                Arc::new(provider),
                Arc::new(executor),
                registry,
                ActionLoopConfig {
                    max_wall_time_ms: 5,
                    ..ActionLoopConfig::default()
                },
            )
            .unwrap()
            .run(input(), CancellationToken::new())
            .await
            .unwrap_err();
            assert_eq!(failure.kind, ActionLoopFailureKind::WallTimeBudget);
        });
    }

    #[test]
    fn each_provider_request_reserves_remaining_token_and_cost_budget_before_network() {
        block_on(async {
            let registry = ProductToolRegistry::default();
            let provider = FakeDeepSeekClient::scripted(
                "deepseek-chat",
                true,
                true,
                vec![Ok(tool_response("call_1")), Ok(final_response())],
            );
            let records = provider.clone();
            let result = ActionLoop::new(
                Arc::new(provider),
                Arc::new(FakeExecutor::new(&registry)),
                registry,
                ActionLoopConfig {
                    max_total_tokens: 30,
                    ..ActionLoopConfig::default()
                },
            )
            .unwrap()
            .run(input(), CancellationToken::new())
            .await
            .unwrap();
            assert_eq!(result.usage.total_tokens(), 30);
            assert_eq!(
                records
                    .records()
                    .iter()
                    .map(|record| record.max_output_tokens)
                    .collect::<Vec<_>>(),
                vec![20, 5],
                "the second request must be narrowed by already-consumed tokens",
            );

            let registry = ProductToolRegistry::default();
            let provider = FakeDeepSeekClient::scripted(
                "deepseek-chat",
                true,
                true,
                vec![Ok(tool_response("call_1")), Ok(final_response())],
            );
            let records = provider.clone();
            let failure = ActionLoop::new(
                Arc::new(provider),
                Arc::new(FakeExecutor::new(&registry)),
                registry,
                ActionLoopConfig {
                    max_total_tokens: 24,
                    ..ActionLoopConfig::default()
                },
            )
            .unwrap()
            .run(input(), CancellationToken::new())
            .await
            .unwrap_err();
            assert_eq!(failure.kind, ActionLoopFailureKind::TokenBudget);
            assert_eq!(failure.usage.total_tokens(), 13);
            assert_eq!(records.records().len(), 1, "second request must not start");

            let registry = ProductToolRegistry::default();
            let provider = FakeDeepSeekClient::scripted(
                "deepseek-chat",
                true,
                true,
                vec![Ok(tool_response("call_1")), Ok(final_response())],
            );
            let records = provider.clone();
            let failure = ActionLoop::new(
                Arc::new(provider),
                Arc::new(FakeExecutor::new(&registry)),
                registry,
                ActionLoopConfig {
                    max_estimated_cost_microusd: 4,
                    ..ActionLoopConfig::default()
                },
            )
            .unwrap()
            .run(input(), CancellationToken::new())
            .await
            .unwrap_err();
            assert_eq!(failure.kind, ActionLoopFailureKind::CostBudget);
            assert_eq!(failure.usage.estimated_cost_microusd, 2);
            assert_eq!(records.records().len(), 1, "second request must not start");
        });
    }

    #[test]
    fn action_loop_config_rejects_every_budget_above_its_hard_bound() {
        for config in [
            ActionLoopConfig {
                max_wall_time_ms: MAX_ACTION_LOOP_WALL_TIME_MS + 1,
                ..ActionLoopConfig::default()
            },
            ActionLoopConfig {
                max_total_tokens: MAX_ACTION_LOOP_TOTAL_TOKENS + 1,
                ..ActionLoopConfig::default()
            },
            ActionLoopConfig {
                max_estimated_cost_microusd: MAX_ACTION_LOOP_COST_MICROUSD + 1,
                ..ActionLoopConfig::default()
            },
            ActionLoopConfig {
                max_output_tokens_per_request: MAX_ACTION_LOOP_OUTPUT_TOKENS_PER_REQUEST + 1,
                ..ActionLoopConfig::default()
            },
        ] {
            assert_eq!(
                config.validate().unwrap_err().code,
                "ACTION_LOOP_BUDGET_OUT_OF_RANGE"
            );
        }
    }

    #[test]
    fn action_loop_input_debug_redacts_cancellation_and_context_content() {
        let forbidden = "FORBIDDEN_ACTION_LOOP_DEBUG_SENTINEL";
        let context = ContextBuilder
            .build(ContextBuildInput {
                system_prompt: "bounded system".into(),
                thread_summary: forbidden.into(),
                recent_messages: vec![ContextMessage {
                    role: ContextRole::User,
                    content: forbidden.into(),
                    name: None,
                    tool_call_id: None,
                }],
                active_snapshot: Some(json!({"private": forbidden})),
                allowed_component_ids: Vec::new(),
                allowed_material_ids: Vec::new(),
                tools: Vec::new(),
            })
            .unwrap();
        let input = ActionLoopInput {
            execution_id: "execution_safe".into(),
            turn_id: "turn_safe".into(),
            cancellation_id: forbidden.into(),
            cancellation_token: forbidden.into(),
            provider_id: "deepseek".into(),
            provider_preflight: Some(ProviderPreflight {
                provider_id: "deepseek".into(),
                model: forbidden.into(),
                configured: true,
                streaming: true,
                tool_calls: true,
                network_call_made: false,
            }),
            context,
        };
        let debug = format!("{input:?}");
        assert!(!debug.contains(forbidden), "unsafe Debug output: {debug}");
    }

    #[test]
    fn cancellation_rejects_late_executor_result() {
        block_on(async {
            let registry = ProductToolRegistry::default();
            let mut executor = FakeExecutor::new(&registry);
            executor.delay_ms = 50;
            let calls = executor.calls.clone();
            let completed = executor.completed.clone();
            let provider = FakeDeepSeekClient::scripted(
                "deepseek-chat",
                true,
                true,
                vec![Ok(tool_response("call_1"))],
            );
            let loop_ = ActionLoop::new(
                Arc::new(provider),
                Arc::new(executor),
                registry,
                ActionLoopConfig::default(),
            )
            .unwrap();
            let cancellation = CancellationToken::new();
            let task_cancellation = cancellation.clone();
            let task = tokio::spawn(async move { loop_.run(input(), task_cancellation).await });
            while calls.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
            cancellation.cancel();
            let failure = task.await.unwrap().unwrap_err();
            assert_eq!(failure.kind, ActionLoopFailureKind::Cancelled);
            assert!(failure
                .trace
                .entries
                .iter()
                .any(|entry| entry.event == TraceEventKind::LateResultIgnored));
            tokio::time::sleep(Duration::from_millis(65)).await;
            // The adapter may ignore cancellation internally, but its late value
            // cannot re-enter the already terminal Rust Action Loop.
            assert!(completed.load(Ordering::SeqCst) <= 1);
        });
    }

    #[test]
    fn permanent_side_effect_report_is_rejected_before_followup_provider_call() {
        block_on(async {
            let registry = ProductToolRegistry::default();
            let mut executor = FakeExecutor::new(&registry);
            executor.permanent_side_effects = 1;
            let provider = FakeDeepSeekClient::scripted(
                "deepseek-chat",
                true,
                true,
                vec![Ok(tool_response("call_1")), Ok(final_response())],
            );
            let records = provider.clone();
            let failure = ActionLoop::new(
                Arc::new(provider),
                Arc::new(executor),
                registry,
                ActionLoopConfig::default(),
            )
            .unwrap()
            .run(input(), CancellationToken::new())
            .await
            .unwrap_err();
            assert_eq!(failure.kind, ActionLoopFailureKind::PermanentWriteRejected);
            assert_eq!(failure.item_events.len(), 2);
            assert_eq!(
                failure.item_events[1].status,
                ActionLoopItemStatus::Rejected
            );
            assert_eq!(records.records().len(), 1);
        });
    }
}
