//! Provider boundary and deterministic fake DeepSeek stream.
//!
//! The production credential and endpoint are owned outside this contract.
//! Requests contain only bounded model context and tool schemas. Provider
//! reasoning is an explicitly non-serializable in-memory value.

use std::{
    collections::VecDeque,
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::CancellationToken;

pub type ProviderFuture<T> =
    Pin<Box<dyn Future<Output = Result<T, ProviderError>> + Send + 'static>>;
pub type ProviderEventSink = Box<dyn FnMut(ProviderStreamEvent) + Send + 'static>;

#[derive(Clone, PartialEq, Eq)]
pub struct EphemeralReasoning(String);

impl EphemeralReasoning {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for EphemeralReasoning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EphemeralReasoning([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, PartialEq)]
pub struct ProviderMessage {
    pub role: ProviderRole,
    pub content: String,
    pub tool_call_id: Option<String>,
    pub tool_calls: Vec<ProviderToolCall>,
    /// DeepSeek requires the previous assistant reasoning on the next
    /// subrequest. It remains memory-only and is dropped with the Turn.
    pub ephemeral_reasoning: Option<EphemeralReasoning>,
}

impl fmt::Debug for ProviderMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderMessage")
            .field("role", &self.role)
            .field("content", &"[REDACTED]")
            .field("content_bytes", &self.content.len())
            .field(
                "tool_call_id",
                &self.tool_call_id.as_ref().map(|_| "[REDACTED]"),
            )
            .field("tool_call_count", &self.tool_calls.len())
            .field(
                "has_ephemeral_reasoning",
                &self.ephemeral_reasoning.is_some(),
            )
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProviderToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProviderToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments: Value,
}

impl fmt::Debug for ProviderToolCall {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderToolCall")
            .field("call_id", &"[REDACTED]")
            .field("name", &self.name)
            .field("arguments", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone)]
pub struct ProviderRequest {
    pub provider_id: String,
    pub model: String,
    pub context_digest: String,
    pub messages: Vec<ProviderMessage>,
    pub tools: Vec<ProviderToolDefinition>,
    pub max_output_tokens: u64,
}

impl fmt::Debug for ProviderRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderRequest")
            .field("provider_id", &self.provider_id)
            .field("model", &"[REDACTED]")
            .field("context_digest", &self.context_digest)
            .field("message_count", &self.messages.len())
            .field("tool_count", &self.tools.len())
            .field("max_output_tokens", &self.max_output_tokens)
            .finish()
    }
}

/// Provider-owned, request-specific accounting policy used by the Action Loop
/// before any transport is polled. The input bound includes Provider-side
/// framing overhead; the output rate is the reviewed conservative rate, not a
/// promise about the external invoice.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderRequestBudgetPolicy {
    pub input_tokens_upper_bound: u64,
    pub input_cost_ceiling_microusd: u64,
    pub output_microusd_per_million_tokens: u64,
}

impl ProviderRequestBudgetPolicy {
    pub fn validate(self) -> Result<Self, ProviderError> {
        if self.input_tokens_upper_bound == 0
            || self.input_cost_ceiling_microusd == 0
            || self.output_microusd_per_million_tokens == 0
            || self.output_microusd_per_million_tokens > 100_000_000
        {
            return Err(ProviderError::schema_mismatch(
                "Provider request budget policy is outside the reviewed bounds.",
                false,
            ));
        }
        Ok(self)
    }

    pub fn output_cost_ceiling_microusd(&self, output_tokens: u64) -> u64 {
        let numerator = u128::from(output_tokens)
            .saturating_mul(u128::from(self.output_microusd_per_million_tokens));
        let rounded = numerator.saturating_add(999_999) / 1_000_000;
        u64::try_from(rounded).unwrap_or(u64::MAX)
    }

    pub fn max_output_tokens_for_cost(&self, remaining_cost_microusd: u64) -> u64 {
        let available = remaining_cost_microusd.saturating_sub(self.input_cost_ceiling_microusd);
        let tokens = u128::from(available).saturating_mul(1_000_000)
            / u128::from(self.output_microusd_per_million_tokens);
        u64::try_from(tokens).unwrap_or(u64::MAX)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ProviderUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Provider-reported prompt tokens served from cache. Zero means the
    /// Provider did not report a hit (or reported no hit); it is never inferred
    /// from aggregate prompt usage.
    #[serde(default)]
    pub prompt_cache_hit_tokens: u64,
    /// Provider-reported prompt tokens that missed cache. Zero means the
    /// Provider did not report a miss (or reported no miss); it is never
    /// fabricated as `input_tokens - prompt_cache_hit_tokens`.
    #[serde(default)]
    pub prompt_cache_miss_tokens: u64,
    /// Estimated cost in millionths of one US dollar. A deterministic integer
    /// is used so budget comparisons never depend on floating point.
    pub estimated_cost_microusd: u64,
}

impl ProviderUsage {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens.saturating_add(self.output_tokens)
    }

    pub fn validate(&self, network_call_made: bool) -> Result<(), ProviderError> {
        let cache_prompt_tokens = self
            .prompt_cache_hit_tokens
            .checked_add(self.prompt_cache_miss_tokens)
            .ok_or_else(|| {
                ProviderError::schema_mismatch(
                    "Provider cache usage exceeded the reviewed token boundary.",
                    network_call_made,
                )
            })?;
        if cache_prompt_tokens > self.input_tokens {
            return Err(ProviderError::schema_mismatch(
                "Provider cache usage exceeded total prompt tokens.",
                network_call_made,
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFinishReason {
    Stop,
    ToolCalls,
}

#[derive(Clone)]
pub struct ProviderResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ProviderToolCall>,
    pub ephemeral_reasoning: Option<EphemeralReasoning>,
    pub usage: ProviderUsage,
    pub finish_reason: ProviderFinishReason,
    pub network_call_made: bool,
}

impl fmt::Debug for ProviderResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderResponse")
            .field("content", &self.content.as_ref().map(|_| "[REDACTED]"))
            .field(
                "content_bytes",
                &self.content.as_ref().map_or(0, String::len),
            )
            .field("tool_call_count", &self.tool_calls.len())
            .field("ephemeral_reasoning", &self.ephemeral_reasoning)
            .field("usage", &self.usage)
            .field("finish_reason", &self.finish_reason)
            .field("network_call_made", &self.network_call_made)
            .finish()
    }
}

impl ProviderResponse {
    pub fn validate(self) -> Result<Self, ProviderError> {
        self.usage.validate(self.network_call_made)?;
        match self.finish_reason {
            ProviderFinishReason::Stop => {
                if self
                    .content
                    .as_deref()
                    .is_none_or(|content| content.trim().is_empty())
                {
                    return Err(ProviderError::empty_content(self.network_call_made));
                }
                if !self.tool_calls.is_empty() {
                    return Err(ProviderError::schema_mismatch(
                        "Provider stop response unexpectedly included tool calls.",
                        self.network_call_made,
                    ));
                }
            }
            ProviderFinishReason::ToolCalls => {
                if self.tool_calls.is_empty() {
                    return Err(ProviderError::empty_json(self.network_call_made));
                }
            }
        }
        Ok(self)
    }
}

#[derive(Clone)]
pub enum ProviderStreamEvent {
    /// The production Provider accepted a locally validated request and is
    /// about to poll its HTTP transport. This is a conservative attempt latch,
    /// not evidence that a response or billable token was received.
    NetworkRequestStarted,
    ContentDelta(String),
    ReasoningDelta(EphemeralReasoning),
    ToolCallReady(ProviderToolCall),
}

impl fmt::Debug for ProviderStreamEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NetworkRequestStarted => formatter.write_str("NetworkRequestStarted"),
            Self::ContentDelta(content) => formatter
                .debug_struct("ContentDelta")
                .field("content", &"[REDACTED]")
                .field("content_bytes", &content.len())
                .finish(),
            Self::ReasoningDelta(_) => formatter
                .debug_tuple("ReasoningDelta")
                .field(&"[REDACTED]")
                .finish(),
            Self::ToolCallReady(call) => {
                formatter.debug_tuple("ToolCallReady").field(call).finish()
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderPreflight {
    pub provider_id: String,
    pub model: String,
    pub configured: bool,
    pub streaming: bool,
    pub tool_calls: bool,
    pub network_call_made: bool,
}

impl fmt::Debug for ProviderPreflight {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderPreflight")
            .field("provider_id", &self.provider_id)
            .field("model", &"[REDACTED]")
            .field("configured", &self.configured)
            .field("streaming", &self.streaming)
            .field("tool_calls", &self.tool_calls)
            .field("network_call_made", &self.network_call_made)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderHealthCheck {
    pub provider_id: String,
    pub network_call_made: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<ProviderUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorCategory {
    Unconfigured,
    InvalidRequest,
    Authentication,
    Balance,
    RateLimited,
    ServerUnavailable,
    Timeout,
    Transport,
    EmptyContent,
    EmptyJson,
    InvalidJson,
    SchemaMismatch,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderError {
    pub code: String,
    pub category: ProviderErrorCategory,
    /// User-safe, bounded text only. Raw Provider bodies are never retained.
    pub message: String,
    pub recoverable: bool,
    pub network_call_made: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

impl ProviderError {
    pub fn unconfigured() -> Self {
        Self::new(
            "PROVIDER_NOT_CONFIGURED",
            ProviderErrorCategory::Unconfigured,
            "The selected Provider is not configured in the desktop credential store.",
            false,
            false,
        )
    }

    pub fn cancelled(network_call_made: bool) -> Self {
        Self::new(
            "PROVIDER_CANCELLED",
            ProviderErrorCategory::Cancelled,
            "Provider execution was cancelled.",
            true,
            network_call_made,
        )
    }

    pub fn timeout(network_call_made: bool) -> Self {
        Self::new(
            "PROVIDER_TIMEOUT",
            ProviderErrorCategory::Timeout,
            "Provider execution exceeded its time limit.",
            true,
            network_call_made,
        )
    }

    pub fn transport(network_call_made: bool) -> Self {
        Self::new(
            "PROVIDER_TRANSPORT_FAILED",
            ProviderErrorCategory::Transport,
            "Provider transport failed before a valid response was received.",
            true,
            network_call_made,
        )
    }

    pub fn empty_content(network_call_made: bool) -> Self {
        Self::new(
            "PROVIDER_EMPTY_CONTENT",
            ProviderErrorCategory::EmptyContent,
            "Provider returned no final content.",
            true,
            network_call_made,
        )
    }

    pub fn empty_json(network_call_made: bool) -> Self {
        Self::new(
            "PROVIDER_EMPTY_JSON",
            ProviderErrorCategory::EmptyJson,
            "Provider returned no tool-call object.",
            true,
            network_call_made,
        )
    }

    pub fn invalid_json(network_call_made: bool) -> Self {
        Self::new(
            "PROVIDER_INVALID_JSON",
            ProviderErrorCategory::InvalidJson,
            "Provider returned malformed structured output.",
            true,
            network_call_made,
        )
    }

    pub fn schema_mismatch(message: &str, network_call_made: bool) -> Self {
        Self::new(
            "PROVIDER_SCHEMA_MISMATCH",
            ProviderErrorCategory::SchemaMismatch,
            message,
            true,
            network_call_made,
        )
    }

    /// Construct a fail-closed schema error with a fixed, reviewed subtype.
    /// Callers must pass a code that is safe to expose at the protocol
    /// boundary; raw Provider bodies are still never retained.
    pub fn schema_mismatch_with_code(code: &str, message: &str, network_call_made: bool) -> Self {
        Self::new(
            code,
            ProviderErrorCategory::SchemaMismatch,
            message,
            true,
            network_call_made,
        )
    }

    pub fn output_truncated(network_call_made: bool) -> Self {
        Self::new(
            "PROVIDER_OUTPUT_TRUNCATED",
            ProviderErrorCategory::InvalidRequest,
            "Provider output reached its reviewed output-token limit before completion.",
            true,
            network_call_made,
        )
    }

    pub fn content_filtered(network_call_made: bool) -> Self {
        Self::new(
            "PROVIDER_CONTENT_FILTERED",
            ProviderErrorCategory::InvalidRequest,
            "Provider declined the request under its safety policy.",
            false,
            network_call_made,
        )
    }

    pub fn insufficient_system_resource(network_call_made: bool) -> Self {
        Self::new(
            "PROVIDER_SYSTEM_RESOURCE_UNAVAILABLE",
            ProviderErrorCategory::ServerUnavailable,
            "Provider service temporarily lacks capacity to complete the request.",
            true,
            network_call_made,
        )
    }

    pub fn from_http_status(status: u16, retry_after_ms: Option<u64>) -> Self {
        let (code, category, message, recoverable) = match status {
            400 | 422 => (
                "PROVIDER_INVALID_REQUEST",
                ProviderErrorCategory::InvalidRequest,
                "Provider rejected the bounded request.",
                false,
            ),
            401 | 403 => (
                "PROVIDER_AUTHENTICATION_FAILED",
                ProviderErrorCategory::Authentication,
                "Provider authentication failed.",
                false,
            ),
            402 => (
                "PROVIDER_BALANCE_REQUIRED",
                ProviderErrorCategory::Balance,
                "Provider account balance is insufficient.",
                false,
            ),
            429 => (
                "PROVIDER_RATE_LIMITED",
                ProviderErrorCategory::RateLimited,
                "Provider rate limit was reached.",
                true,
            ),
            500..=599 => (
                "PROVIDER_SERVER_UNAVAILABLE",
                ProviderErrorCategory::ServerUnavailable,
                "Provider service is temporarily unavailable.",
                true,
            ),
            _ => (
                "PROVIDER_TRANSPORT_FAILED",
                ProviderErrorCategory::Transport,
                "Provider returned an unsupported transport status.",
                true,
            ),
        };
        let mut error = Self::new(code, category, message, recoverable, true);
        error.retry_after_ms = retry_after_ms;
        error
    }

    fn new(
        code: &str,
        category: ProviderErrorCategory,
        message: &str,
        recoverable: bool,
        network_call_made: bool,
    ) -> Self {
        Self {
            code: code.into(),
            category,
            message: message.into(),
            recoverable,
            network_call_made,
            retry_after_ms: None,
        }
    }
}

pub trait ProviderClient: Send + Sync + 'static {
    /// Creates an isolated Provider client for one explicit Agent Turn.
    ///
    /// Most Providers have no per-Turn secret state and return `None`. A
    /// credential-backed Provider may return a short-lived client holding one
    /// validated credential snapshot. The runtime holds that client from
    /// preflight through every Action Loop subrequest, then drops it on every
    /// success, failure, and cancellation path. This deliberately avoids a
    /// process-global credential cache and prevents concurrent Turns sharing a
    /// snapshot.
    fn turn_session(&self) -> Result<Option<Arc<dyn ProviderClient>>, ProviderError> {
        Ok(None)
    }

    fn preflight(&self, cancellation: CancellationToken) -> ProviderFuture<ProviderPreflight>;

    /// Returns a conservative request ceiling without opening a socket. A
    /// Provider implementation that cannot supply this fact must fail closed.
    fn request_budget_policy(
        &self,
        _request: &ProviderRequest,
    ) -> Result<ProviderRequestBudgetPolicy, ProviderError> {
        Err(ProviderError::schema_mismatch(
            "Provider request budget policy is unavailable.",
            false,
        ))
    }

    /// Performs the explicit user-requested connectivity check. Unlike
    /// preflight, a Ready result must represent a real network call in a
    /// production implementation. The fake is scripted and never opens a
    /// socket.
    fn check(
        &self,
        provider_id: String,
        timeout_ms: u32,
        cancellation: CancellationToken,
    ) -> ProviderFuture<ProviderHealthCheck>;

    fn stream(
        &self,
        request: ProviderRequest,
        cancellation: CancellationToken,
        events: ProviderEventSink,
    ) -> ProviderFuture<ProviderResponse>;

    fn cancel(&self, cancellation_id: String, cancellation_token: String) -> ProviderFuture<bool>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SafeProviderRequestRecord {
    pub provider_id: String,
    pub model: String,
    pub context_digest: String,
    pub message_count: usize,
    pub prior_reasoning_count: usize,
    pub tool_names: Vec<String>,
    pub max_output_tokens: u64,
}

#[derive(Clone)]
pub struct FakeDeepSeekClient {
    inner: Arc<FakeDeepSeekInner>,
}

struct FakeDeepSeekInner {
    configured: bool,
    model: String,
    network_call_made: bool,
    scripted: Mutex<VecDeque<Result<ProviderResponse, ProviderError>>>,
    records: Mutex<Vec<SafeProviderRequestRecord>>,
}

impl FakeDeepSeekClient {
    pub fn scripted(
        model: impl Into<String>,
        configured: bool,
        network_call_made: bool,
        scripted: Vec<Result<ProviderResponse, ProviderError>>,
    ) -> Self {
        Self {
            inner: Arc::new(FakeDeepSeekInner {
                configured,
                model: model.into(),
                network_call_made,
                scripted: Mutex::new(scripted.into()),
                records: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn records(&self) -> Vec<SafeProviderRequestRecord> {
        self.inner
            .records
            .lock()
            .expect("fake Provider record mutex poisoned")
            .clone()
    }
}

impl ProviderClient for FakeDeepSeekClient {
    fn preflight(&self, cancellation: CancellationToken) -> ProviderFuture<ProviderPreflight> {
        let inner = self.inner.clone();
        Box::pin(async move {
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled(false));
            }
            if !inner.configured {
                return Err(ProviderError::unconfigured());
            }
            Ok(ProviderPreflight {
                provider_id: "deepseek".into(),
                model: inner.model.clone(),
                configured: true,
                streaming: true,
                tool_calls: true,
                network_call_made: false,
            })
        })
    }

    fn request_budget_policy(
        &self,
        _request: &ProviderRequest,
    ) -> Result<ProviderRequestBudgetPolicy, ProviderError> {
        let scripted = self
            .inner
            .scripted
            .lock()
            .expect("fake Provider script mutex poisoned");
        let (input_tokens_upper_bound, input_cost_ceiling_microusd) = scripted
            .front()
            .and_then(|result| result.as_ref().ok())
            .map(|response| {
                (
                    response.usage.input_tokens.max(1),
                    response.usage.estimated_cost_microusd.max(1),
                )
            })
            .unwrap_or((1, 1));
        ProviderRequestBudgetPolicy {
            input_tokens_upper_bound,
            input_cost_ceiling_microusd,
            // The scripted response cost is already conservatively reserved
            // as input cost above; retain a positive output ceiling so the
            // same fail-closed arithmetic is exercised. Production clients
            // override this with their reviewed rate.
            output_microusd_per_million_tokens: 1,
        }
        .validate()
    }

    fn stream(
        &self,
        request: ProviderRequest,
        cancellation: CancellationToken,
        mut events: ProviderEventSink,
    ) -> ProviderFuture<ProviderResponse> {
        let inner = self.inner.clone();
        Box::pin(async move {
            if !inner.configured {
                return Err(ProviderError::unconfigured());
            }
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled(false));
            }
            inner
                .records
                .lock()
                .expect("fake Provider record mutex poisoned")
                .push(SafeProviderRequestRecord {
                    provider_id: request.provider_id,
                    model: request.model,
                    context_digest: request.context_digest,
                    message_count: request.messages.len(),
                    prior_reasoning_count: request
                        .messages
                        .iter()
                        .filter(|message| message.ephemeral_reasoning.is_some())
                        .count(),
                    tool_names: request.tools.into_iter().map(|tool| tool.name).collect(),
                    max_output_tokens: request.max_output_tokens,
                });
            let scripted = inner
                .scripted
                .lock()
                .expect("fake Provider script mutex poisoned")
                .pop_front()
                .unwrap_or_else(|| Err(ProviderError::empty_content(inner.network_call_made)));
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled(inner.network_call_made));
            }
            let mut response = scripted?;
            response.network_call_made = inner.network_call_made;
            if let Some(reasoning) = response.ephemeral_reasoning.clone() {
                events(ProviderStreamEvent::ReasoningDelta(reasoning));
            }
            if let Some(content) = response.content.clone() {
                events(ProviderStreamEvent::ContentDelta(content));
            }
            for call in response.tool_calls.iter().cloned() {
                events(ProviderStreamEvent::ToolCallReady(call));
            }
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled(inner.network_call_made));
            }
            response.validate()
        })
    }

    fn check(
        &self,
        provider_id: String,
        _timeout_ms: u32,
        cancellation: CancellationToken,
    ) -> ProviderFuture<ProviderHealthCheck> {
        let inner = self.inner.clone();
        Box::pin(async move {
            if !inner.configured {
                return Err(ProviderError::unconfigured());
            }
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled(false));
            }
            if !inner.network_call_made {
                return Err(ProviderError::transport(false));
            }
            Ok(ProviderHealthCheck {
                provider_id,
                network_call_made: true,
                usage: Some(ProviderUsage::default()),
            })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    fn response() -> ProviderResponse {
        ProviderResponse {
            content: Some("完成唯一候选。".into()),
            tool_calls: Vec::new(),
            ephemeral_reasoning: Some(EphemeralReasoning::new("private chain")),
            usage: ProviderUsage {
                input_tokens: 20,
                output_tokens: 8,
                prompt_cache_hit_tokens: 12,
                prompt_cache_miss_tokens: 8,
                estimated_cost_microusd: 3,
            },
            finish_reason: ProviderFinishReason::Stop,
            network_call_made: true,
        }
    }

    #[test]
    fn fake_stream_records_only_safe_facts_and_emits_ephemeral_reasoning() {
        block_on(async {
            let client =
                FakeDeepSeekClient::scripted("deepseek-chat", true, true, vec![Ok(response())]);
            let events = Arc::new(Mutex::new(Vec::new()));
            let captured = events.clone();
            let result = client
                .stream(
                    ProviderRequest {
                        provider_id: "deepseek".into(),
                        model: "deepseek-chat".into(),
                        context_digest: "a".repeat(64),
                        messages: vec![ProviderMessage {
                            role: ProviderRole::User,
                            content: "secret user prompt is deliberately not recorded".into(),
                            tool_call_id: None,
                            tool_calls: Vec::new(),
                            ephemeral_reasoning: None,
                        }],
                        tools: Vec::new(),
                        max_output_tokens: 256,
                    },
                    CancellationToken::new(),
                    Box::new(move |event| captured.lock().unwrap().push(event)),
                )
                .await
                .unwrap();
            assert!(result.ephemeral_reasoning.is_some());
            assert!(matches!(
                events.lock().unwrap()[0],
                ProviderStreamEvent::ReasoningDelta(_)
            ));
            let serialized = serde_json::to_string(&client.records()).unwrap();
            assert!(!serialized.contains("secret user prompt"));
            assert!(!serialized.contains("private chain"));
            assert!(!serialized.contains("api_key"));
        });
    }

    #[test]
    fn error_taxonomy_is_stable_for_required_http_statuses_and_parse_failures() {
        for (status, category) in [
            (401, ProviderErrorCategory::Authentication),
            (403, ProviderErrorCategory::Authentication),
            (402, ProviderErrorCategory::Balance),
            (429, ProviderErrorCategory::RateLimited),
            (503, ProviderErrorCategory::ServerUnavailable),
        ] {
            assert_eq!(
                ProviderError::from_http_status(status, None).category,
                category
            );
        }
        assert_eq!(
            ProviderError::timeout(true).category,
            ProviderErrorCategory::Timeout
        );
        assert_eq!(
            ProviderError::empty_content(true).category,
            ProviderErrorCategory::EmptyContent
        );
        assert_eq!(
            ProviderError::empty_json(true).category,
            ProviderErrorCategory::EmptyJson
        );
        assert_eq!(
            ProviderError::invalid_json(true).category,
            ProviderErrorCategory::InvalidJson
        );
        assert_eq!(
            ProviderError::schema_mismatch("invalid", true).category,
            ProviderErrorCategory::SchemaMismatch
        );
        assert_eq!(
            ProviderError::output_truncated(true).code,
            "PROVIDER_OUTPUT_TRUNCATED"
        );
        assert_eq!(
            ProviderError::content_filtered(true).code,
            "PROVIDER_CONTENT_FILTERED"
        );
        assert_eq!(
            ProviderError::insufficient_system_resource(true).category,
            ProviderErrorCategory::ServerUnavailable
        );
    }

    #[test]
    fn usage_rejects_cache_counts_beyond_prompt_boundary() {
        let error = ProviderUsage {
            input_tokens: 4,
            output_tokens: 1,
            prompt_cache_hit_tokens: 3,
            prompt_cache_miss_tokens: 2,
            estimated_cost_microusd: 0,
        }
        .validate(true)
        .unwrap_err();
        assert_eq!(error.category, ProviderErrorCategory::SchemaMismatch);
        assert!(error.network_call_made);
    }

    #[test]
    fn unconfigured_and_cancelled_paths_do_not_make_network_calls() {
        block_on(async {
            let unconfigured =
                FakeDeepSeekClient::scripted("deepseek-chat", false, true, Vec::new());
            let error = unconfigured
                .preflight(CancellationToken::new())
                .await
                .unwrap_err();
            assert_eq!(error.category, ProviderErrorCategory::Unconfigured);
            assert!(!error.network_call_made);

            let configured =
                FakeDeepSeekClient::scripted("deepseek-chat", true, true, vec![Ok(response())]);
            let cancellation = CancellationToken::new();
            cancellation.cancel();
            let error = configured.preflight(cancellation).await.unwrap_err();
            assert_eq!(error.category, ProviderErrorCategory::Cancelled);
            assert!(!error.network_call_made);
        });
    }

    #[test]
    fn provider_debug_views_are_structurally_redacted() {
        let forbidden = "FORBIDDEN_PROVIDER_DEBUG_SENTINEL";
        let call = ProviderToolCall {
            call_id: forbidden.into(),
            name: "compile_readback_candidate".into(),
            arguments: serde_json::json!({"prompt": forbidden}),
        };
        let message = ProviderMessage {
            role: ProviderRole::Assistant,
            content: forbidden.into(),
            tool_call_id: Some(forbidden.into()),
            tool_calls: vec![call.clone()],
            ephemeral_reasoning: Some(EphemeralReasoning::new(forbidden)),
        };
        let request = ProviderRequest {
            provider_id: "deepseek".into(),
            model: forbidden.into(),
            context_digest: "a".repeat(64),
            messages: vec![message.clone()],
            tools: Vec::new(),
            max_output_tokens: 8,
        };
        let response = ProviderResponse {
            content: Some(forbidden.into()),
            tool_calls: vec![call.clone()],
            ephemeral_reasoning: Some(EphemeralReasoning::new(forbidden)),
            usage: ProviderUsage::default(),
            finish_reason: ProviderFinishReason::ToolCalls,
            network_call_made: true,
        };
        let preflight = ProviderPreflight {
            provider_id: "deepseek".into(),
            model: forbidden.into(),
            configured: true,
            streaming: true,
            tool_calls: true,
            network_call_made: false,
        };
        for debug in [
            format!("{message:?}"),
            format!("{call:?}"),
            format!("{request:?}"),
            format!("{response:?}"),
            format!("{:?}", ProviderStreamEvent::ContentDelta(forbidden.into())),
            format!(
                "{:?}",
                ProviderStreamEvent::ReasoningDelta(EphemeralReasoning::new(forbidden))
            ),
            format!("{:?}", ProviderStreamEvent::ToolCallReady(call)),
            format!("{preflight:?}"),
        ] {
            assert!(!debug.contains(forbidden), "unsafe Debug output: {debug}");
        }
    }
}
