//! Production DeepSeek Provider boundary for the Rust-owned K002 Agent.
//!
//! Credentials are injected by the desktop owner (normally macOS Keychain),
//! the HTTP transport is independently replaceable, and hidden reasoning is
//! represented only by `EphemeralReasoning`.  Tests use a fake transport and
//! never open a socket.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    future::{poll_fn, Future},
    sync::{Arc, Mutex},
    task::Poll,
    time::Duration,
};

use forgecad_app_server::{
    CancellationToken, EphemeralReasoning, ProviderClient, ProviderError, ProviderEventSink,
    ProviderFinishReason, ProviderFuture, ProviderHealthCheck, ProviderMessage, ProviderPreflight,
    ProviderRequest, ProviderRequestBudgetPolicy, ProviderResponse, ProviderRole,
    ProviderStreamEvent, ProviderToolCall, ProviderUsage,
};
use reqwest::{header, redirect, Client, Url};
use serde_json::{json, Map, Value};
use zeroize::Zeroizing;

const MAX_BASE_URL_BYTES: usize = 2_048;
const MAX_MODEL_BYTES: usize = 160;
const MAX_API_KEY_BYTES: usize = 4_096;
const MAX_MESSAGES: usize = 128;
const MAX_MESSAGE_BYTES: usize = 200_000;
const MAX_TOOLS: usize = 13;
const MAX_TOOL_CALLS: usize = 12;
const MAX_TOOL_NAME_BYTES: usize = 64;
const MAX_TOOL_DESCRIPTION_BYTES: usize = 500;
const MAX_TOOL_SCHEMA_BYTES: usize = 512_000;
const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;
const MAX_OUTPUT_TOKENS: u64 = 100_000;
const PROVIDER_INPUT_FRAMING_OVERHEAD_BYTES: u64 = 4_096;
const DEFAULT_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_MAX_SSE_EVENT_BYTES: usize = 256 * 1024;
const DEFAULT_MAX_CONTENT_BYTES: usize = 1024 * 1024;
const DEFAULT_MAX_REASONING_BYTES: usize = 1024 * 1024;
const DEFAULT_MAX_TOOL_ARGUMENT_BYTES: usize = 200_000;

#[derive(Clone)]
struct SecretText(Arc<Zeroizing<String>>);

impl SecretText {
    fn new(value: impl Into<String>) -> Self {
        Self::from_zeroizing(Zeroizing::new(value.into()))
    }

    fn from_zeroizing(value: Zeroizing<String>) -> Self {
        Self(Arc::new(value))
    }

    fn expose(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for SecretText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

/// Values loaded from a desktop-owned credential store.
///
/// Debug intentionally reveals neither endpoint, model nor key.  The endpoint
/// and model are metadata rather than passwords, but keeping all three out of
/// generic logs prevents accidental configuration disclosure.
#[derive(Clone)]
pub struct DeepSeekCredentials {
    base_url: SecretText,
    model: SecretText,
    api_key: SecretText,
}

impl DeepSeekCredentials {
    #[cfg(test)]
    pub fn new(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        Self::from_zeroizing(
            base_url.into(),
            model.into(),
            Zeroizing::new(api_key.into()),
        )
    }

    pub(crate) fn from_zeroizing(
        base_url: String,
        model: String,
        api_key: Zeroizing<String>,
    ) -> Self {
        Self {
            base_url: SecretText::new(base_url),
            model: SecretText::new(model),
            api_key: SecretText::from_zeroizing(api_key),
        }
    }
}

impl fmt::Debug for DeepSeekCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeepSeekCredentials")
            .field("base_url", &"[REDACTED]")
            .field("model", &"[REDACTED]")
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DeepSeekCredentialSourceError;

impl fmt::Debug for DeepSeekCredentialSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DeepSeekCredentialSourceError([REDACTED])")
    }
}

impl fmt::Display for DeepSeekCredentialSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("The desktop credential store could not be read.")
    }
}

impl Error for DeepSeekCredentialSourceError {}

pub trait DeepSeekCredentialSource: Send + Sync + 'static {
    fn load(&self) -> Result<Option<DeepSeekCredentials>, DeepSeekCredentialSourceError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeepSeekPricing {
    pub input_microusd_per_million_tokens: u64,
    pub output_microusd_per_million_tokens: u64,
}

impl DeepSeekPricing {
    pub fn new(
        input_microusd_per_million_tokens: u64,
        output_microusd_per_million_tokens: u64,
    ) -> Result<Self, ProviderError> {
        if input_microusd_per_million_tokens == 0
            || output_microusd_per_million_tokens == 0
            || input_microusd_per_million_tokens > 100_000_000
            || output_microusd_per_million_tokens > 100_000_000
        {
            return Err(local_schema_error(
                "Provider pricing policy is missing or outside the reviewed bound.",
            ));
        }
        Ok(Self {
            input_microusd_per_million_tokens,
            output_microusd_per_million_tokens,
        })
    }

    fn estimate(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        cost_for_tokens(input_tokens, self.input_microusd_per_million_tokens).saturating_add(
            cost_for_tokens(output_tokens, self.output_microusd_per_million_tokens),
        )
    }
}

fn cost_for_tokens(tokens: u64, rate_per_million: u64) -> u64 {
    tokens
        .saturating_mul(rate_per_million)
        .saturating_add(999_999)
        / 1_000_000
}

#[derive(Debug, Clone)]
pub struct DeepSeekProviderConfig {
    pub request_timeout: Duration,
    pub max_response_bytes: usize,
    pub max_sse_event_bytes: usize,
    pub max_content_bytes: usize,
    pub max_reasoning_bytes: usize,
    pub max_tool_argument_bytes: usize,
    pub pricing: DeepSeekPricing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepSeekThinkingMode {
    Enabled,
    Disabled,
}

impl DeepSeekThinkingMode {
    fn as_api_value(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepSeekReasoningEffort {
    High,
    Max,
}

impl DeepSeekReasoningEffort {
    fn as_api_value(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Max => "max",
        }
    }
}

fn thinking_policy_for_model(
    model: &str,
) -> (DeepSeekThinkingMode, Option<DeepSeekReasoningEffort>) {
    match model {
        // Preserve the Provider's documented compatibility behavior until
        // these aliases are removed. New ForgeCAD configuration defaults to a
        // current V4 model and therefore takes the Agent-oriented max path.
        "deepseek-chat" => (DeepSeekThinkingMode::Disabled, None),
        "deepseek-reasoner" => (
            DeepSeekThinkingMode::Enabled,
            Some(DeepSeekReasoningEffort::High),
        ),
        _ => (
            DeepSeekThinkingMode::Enabled,
            Some(DeepSeekReasoningEffort::Max),
        ),
    }
}

impl DeepSeekProviderConfig {
    pub fn bounded(pricing: DeepSeekPricing) -> Self {
        Self {
            request_timeout: Duration::from_secs(60),
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            max_sse_event_bytes: DEFAULT_MAX_SSE_EVENT_BYTES,
            max_content_bytes: DEFAULT_MAX_CONTENT_BYTES,
            max_reasoning_bytes: DEFAULT_MAX_REASONING_BYTES,
            max_tool_argument_bytes: DEFAULT_MAX_TOOL_ARGUMENT_BYTES,
            pricing,
        }
    }

    fn validate(&self) -> Result<(), ProviderError> {
        if self.request_timeout.is_zero()
            || self.request_timeout > Duration::from_secs(300)
            || !(1024..=16 * 1024 * 1024).contains(&self.max_response_bytes)
            || !(1024..=1024 * 1024).contains(&self.max_sse_event_bytes)
            || self.max_sse_event_bytes > self.max_response_bytes
            || !(1024..=4 * 1024 * 1024).contains(&self.max_content_bytes)
            || !(1024..=4 * 1024 * 1024).contains(&self.max_reasoning_bytes)
            || !(1024..=1024 * 1024).contains(&self.max_tool_argument_bytes)
        {
            return Err(local_schema_error(
                "Provider response limits are outside the reviewed bounds.",
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct DeepSeekHttpRequest {
    endpoint: Url,
    authorization: SecretText,
    body: Arc<[u8]>,
}

impl DeepSeekHttpRequest {
    #[cfg(test)]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    #[cfg(test)]
    pub fn endpoint_path(&self) -> &str {
        self.endpoint.path()
    }
}

impl fmt::Debug for DeepSeekHttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeepSeekHttpRequest")
            .field("method", &"POST")
            .field("endpoint", &"[REDACTED]")
            .field("authorization", &"[REDACTED]")
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepSeekHttpResponseMeta {
    pub status: u16,
    pub retry_after_ms: Option<u64>,
    pub content_type: Option<String>,
    pub network_call_made: bool,
}

pub type DeepSeekHttpChunkSink =
    Box<dyn FnMut(&[u8]) -> Result<(), ProviderError> + Send + 'static>;

pub trait DeepSeekHttpTransport: Send + Sync + 'static {
    fn post_sse(
        &self,
        request: DeepSeekHttpRequest,
        cancellation: CancellationToken,
        chunks: DeepSeekHttpChunkSink,
    ) -> ProviderFuture<DeepSeekHttpResponseMeta>;
}

/// HTTPS-only reqwest transport. Redirects are disabled so credentials cannot
/// cross an endpoint boundary.
#[derive(Clone)]
pub struct ReqwestDeepSeekTransport {
    client: Client,
    max_response_bytes: usize,
}

impl ReqwestDeepSeekTransport {
    pub fn production(max_response_bytes: usize) -> Result<Self, ProviderError> {
        if !(1024..=16 * 1024 * 1024).contains(&max_response_bytes) {
            return Err(local_schema_error(
                "Provider response byte limit is outside the reviewed bound.",
            ));
        }
        let client = Client::builder()
            .https_only(true)
            .redirect(redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| ProviderError::transport(false))?;
        Ok(Self {
            client,
            max_response_bytes,
        })
    }
}

impl fmt::Debug for ReqwestDeepSeekTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReqwestDeepSeekTransport")
            .field("client", &"[HTTPS_ONLY]")
            .field("max_response_bytes", &self.max_response_bytes)
            .finish()
    }
}

impl DeepSeekHttpTransport for ReqwestDeepSeekTransport {
    fn post_sse(
        &self,
        request: DeepSeekHttpRequest,
        cancellation: CancellationToken,
        mut chunks: DeepSeekHttpChunkSink,
    ) -> ProviderFuture<DeepSeekHttpResponseMeta> {
        let client = self.client.clone();
        let max_response_bytes = self.max_response_bytes;
        Box::pin(async move {
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled(false));
            }
            let mut response = client
                .post(request.endpoint)
                .header(header::AUTHORIZATION, request.authorization.expose())
                .header(header::ACCEPT, "text/event-stream")
                .header(header::CONTENT_TYPE, "application/json")
                .body(request.body.to_vec())
                .send()
                .await
                .map_err(map_reqwest_error)?;
            let status = response.status().as_u16();
            let retry_after_ms = parse_retry_after_ms(response.headers());
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .filter(|value| value.len() <= 160)
                .map(str::to_owned);

            if !(200..300).contains(&status) {
                return Ok(DeepSeekHttpResponseMeta {
                    status,
                    retry_after_ms,
                    content_type,
                    network_call_made: true,
                });
            }
            // Reject a successful non-SSE response before any body bytes can
            // reach the incremental parser/event sink. The client repeats the
            // check because injected test transports do not share this HTTP
            // implementation.
            if !content_type
                .as_deref()
                .is_some_and(is_event_stream_content_type)
            {
                return Ok(DeepSeekHttpResponseMeta {
                    status,
                    retry_after_ms,
                    content_type,
                    network_call_made: true,
                });
            }
            if response
                .content_length()
                .is_some_and(|length| length > max_response_bytes as u64)
            {
                return Err(remote_schema_error(
                    "Provider response exceeded the reviewed byte limit.",
                ));
            }

            let mut received = 0usize;
            loop {
                if cancellation.is_cancelled() {
                    return Err(ProviderError::cancelled(true));
                }
                let chunk = response.chunk().await.map_err(map_reqwest_error)?;
                let Some(chunk) = chunk else { break };
                received = received.saturating_add(chunk.len());
                if received > max_response_bytes {
                    return Err(remote_schema_error(
                        "Provider response exceeded the reviewed byte limit.",
                    ));
                }
                chunks(&chunk)?;
            }
            Ok(DeepSeekHttpResponseMeta {
                status,
                retry_after_ms,
                content_type,
                network_call_made: true,
            })
        })
    }
}

fn map_reqwest_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::timeout(true)
    } else {
        ProviderError::transport(true)
    }
}

fn parse_retry_after_ms(headers: &header::HeaderMap) -> Option<u64> {
    headers
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|seconds| seconds.saturating_mul(1000).min(86_400_000))
}

pub struct DeepSeekProviderClient {
    credentials: CredentialMode,
    transport: Arc<dyn DeepSeekHttpTransport>,
    config: DeepSeekProviderConfig,
}

/// Credential ownership is deliberately either dynamic or one-Turn. The
/// long-lived desktop client holds only the source; a Turn client contains one
/// validated snapshot and is dropped by NativeAgentRuntime at Turn completion.
/// There is no process-global cache and no shared mutable session map.
#[derive(Clone)]
enum CredentialMode {
    Dynamic(Arc<dyn DeepSeekCredentialSource>),
    TurnSnapshot(ValidatedCredentials),
}

impl DeepSeekProviderClient {
    pub fn new(
        credential_source: Arc<dyn DeepSeekCredentialSource>,
        transport: Arc<dyn DeepSeekHttpTransport>,
        config: DeepSeekProviderConfig,
    ) -> Result<Self, ProviderError> {
        config.validate()?;
        Ok(Self {
            credentials: CredentialMode::Dynamic(credential_source),
            transport,
            config,
        })
    }

    fn load_credentials(&self) -> Result<ValidatedCredentials, ProviderError> {
        match &self.credentials {
            CredentialMode::Dynamic(credential_source) => {
                let credentials = credential_source
                    .load()
                    .map_err(|_| ProviderError::transport(false))?
                    .ok_or_else(ProviderError::unconfigured)?;
                ValidatedCredentials::try_from(credentials)
            }
            CredentialMode::TurnSnapshot(credentials) => Ok(credentials.clone()),
        }
    }

    fn from_turn_snapshot(
        credentials: ValidatedCredentials,
        transport: Arc<dyn DeepSeekHttpTransport>,
        config: DeepSeekProviderConfig,
    ) -> Self {
        Self {
            credentials: CredentialMode::TurnSnapshot(credentials),
            transport,
            config,
        }
    }
}

impl fmt::Debug for DeepSeekProviderClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeepSeekProviderClient")
            .field("credentials", &"[REDACTED]")
            .field("transport", &"[INJECTED]")
            .field("config", &self.config)
            .finish()
    }
}

impl ProviderClient for DeepSeekProviderClient {
    fn turn_session(&self) -> Result<Option<Arc<dyn ProviderClient>>, ProviderError> {
        match &self.credentials {
            CredentialMode::Dynamic(_) => {
                let credentials = self.load_credentials()?;
                Ok(Some(Arc::new(Self::from_turn_snapshot(
                    credentials,
                    self.transport.clone(),
                    self.config.clone(),
                ))))
            }
            CredentialMode::TurnSnapshot(_) => Err(local_schema_error(
                "Provider credential session cannot be nested.",
            )),
        }
    }

    fn preflight(&self, cancellation: CancellationToken) -> ProviderFuture<ProviderPreflight> {
        let credentials = self.load_credentials();
        Box::pin(async move {
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled(false));
            }
            let credentials = credentials?;
            Ok(ProviderPreflight {
                provider_id: "deepseek".into(),
                model: credentials.model,
                configured: true,
                streaming: true,
                tool_calls: true,
                network_call_made: false,
            })
        })
    }

    fn request_budget_policy(
        &self,
        request: &ProviderRequest,
    ) -> Result<ProviderRequestBudgetPolicy, ProviderError> {
        let credentials = self.load_credentials()?;
        let http_request = build_http_request(request, &credentials, &self.config)?;
        // One input token per serialized byte plus a reviewed Provider-side
        // framing allowance is deliberately conservative. This path performs
        // credential/request validation only and never polls the transport.
        let body_bytes = u64::try_from(http_request.body.len()).map_err(|_| {
            local_schema_error("Provider request size exceeded the reviewed accounting bound.")
        })?;
        let input_tokens_upper_bound = body_bytes
            .checked_add(PROVIDER_INPUT_FRAMING_OVERHEAD_BYTES)
            .ok_or_else(|| {
                local_schema_error("Provider request size exceeded the reviewed accounting bound.")
            })?;
        ProviderRequestBudgetPolicy {
            input_tokens_upper_bound,
            input_cost_ceiling_microusd: cost_for_tokens(
                input_tokens_upper_bound,
                self.config.pricing.input_microusd_per_million_tokens,
            ),
            output_microusd_per_million_tokens: self
                .config
                .pricing
                .output_microusd_per_million_tokens,
        }
        .validate()
    }

    fn stream(
        &self,
        request: ProviderRequest,
        cancellation: CancellationToken,
        events: ProviderEventSink,
    ) -> ProviderFuture<ProviderResponse> {
        if cancellation.is_cancelled() {
            return Box::pin(async { Err(ProviderError::cancelled(false)) });
        }
        let credentials = match self.load_credentials() {
            Ok(credentials) => credentials,
            Err(error) => return Box::pin(async move { Err(error) }),
        };
        let http_request = match build_http_request(&request, &credentials, &self.config) {
            Ok(request) => request,
            Err(error) => return Box::pin(async move { Err(error) }),
        };
        let transport = self.transport.clone();
        let timeout = self.config.request_timeout;
        let thinking_enabled =
            thinking_policy_for_model(&credentials.model).0 == DeepSeekThinkingMode::Enabled;
        let parser = Arc::new(Mutex::new(DeepSeekSseParser::new(
            &self.config,
            thinking_enabled,
        )));
        let parser_for_chunks = parser.clone();
        let event_sink = Arc::new(Mutex::new(events));
        let event_sink_for_chunks = event_sink.clone();
        let cancellation_for_transport = cancellation.clone();

        Box::pin(async move {
            // This event is emitted only after credentials and the complete
            // request body passed local validation. It deliberately means
            // "network attempt started", not "the Provider responded"; the
            // Action Loop uses it to preserve conservative network truth when
            // an outer cancellation or wall-time limit drops this future.
            event_sink
                .lock()
                .expect("DeepSeek event sink mutex poisoned")(
                ProviderStreamEvent::NetworkRequestStarted,
            );
            let transport_future = transport.post_sse(
                http_request,
                cancellation_for_transport,
                Box::new(move |chunk| {
                    let emitted = parser_for_chunks
                        .lock()
                        .expect("DeepSeek SSE parser mutex poisoned")
                        .feed(chunk)?;
                    let mut sink = event_sink_for_chunks
                        .lock()
                        .expect("DeepSeek event sink mutex poisoned");
                    for event in emitted {
                        sink(event);
                    }
                    Ok(())
                }),
            );
            let meta = await_transport(transport_future, cancellation, timeout).await?;
            if !(200..300).contains(&meta.status) {
                return Err(ProviderError::from_http_status(
                    meta.status,
                    meta.retry_after_ms,
                ));
            }
            if !meta
                .content_type
                .as_deref()
                .is_some_and(is_event_stream_content_type)
            {
                return Err(remote_schema_error(
                    "Provider response was not a bounded SSE stream.",
                ));
            }

            let (mut response, final_events) = parser
                .lock()
                .expect("DeepSeek SSE parser mutex poisoned")
                .finish()?;
            let mut sink = event_sink
                .lock()
                .expect("DeepSeek event sink mutex poisoned");
            for event in final_events {
                sink(event);
            }
            response.network_call_made = meta.network_call_made;
            response.validate()
        })
    }

    fn check(
        &self,
        provider_id: String,
        timeout_ms: u32,
        cancellation: CancellationToken,
    ) -> ProviderFuture<ProviderHealthCheck> {
        if provider_id != "deepseek" || timeout_ms == 0 || timeout_ms > 120_000 {
            return Box::pin(async {
                Err(local_schema_error(
                    "Provider connectivity check is outside the reviewed contract.",
                ))
            });
        }
        if cancellation.is_cancelled() {
            return Box::pin(async { Err(ProviderError::cancelled(false)) });
        }
        let credentials = match self.load_credentials() {
            Ok(credentials) => credentials,
            Err(error) => return Box::pin(async move { Err(error) }),
        };
        let mut config = self.config.clone();
        config.request_timeout = Duration::from_millis(u64::from(timeout_ms));
        // The explicit connectivity check owns one local snapshot too. It
        // must not re-open Keychain after its initial validation read.
        let probe = Self::from_turn_snapshot(credentials.clone(), self.transport.clone(), config);
        let future = probe.stream(
            ProviderRequest {
                provider_id: "deepseek".into(),
                model: credentials.model,
                context_digest: "0".repeat(64),
                messages: vec![ProviderMessage {
                    role: ProviderRole::User,
                    content: "Return OK.".into(),
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    ephemeral_reasoning: None,
                }],
                tools: Vec::new(),
                max_output_tokens: 8,
            },
            cancellation,
            Box::new(|_| {}),
        );
        Box::pin(async move {
            let response = future.await?;
            Ok(ProviderHealthCheck {
                provider_id,
                network_call_made: response.network_call_made,
                usage: Some(response.usage),
            })
        })
    }

    fn cancel(&self, cancellation_id: String, cancellation_token: String) -> ProviderFuture<bool> {
        Box::pin(async move {
            if !valid_cancellation_value(&cancellation_id)
                || !valid_cancellation_value(&cancellation_token)
            {
                return Err(local_schema_error(
                    "Provider cancellation identity is outside the reviewed contract.",
                ));
            }
            // Active operations are cancelled by the Rust-owned Turn
            // CancellationToken. The Provider client does not keep a second
            // cancellation registry keyed by user-controlled text.
            Ok(false)
        })
    }
}

fn valid_cancellation_value(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn is_event_stream_content_type(value: &str) -> bool {
    value.len() <= 160
        && value
            .split(';')
            .next()
            .map(str::trim)
            .is_some_and(|media_type| media_type.eq_ignore_ascii_case("text/event-stream"))
}

async fn await_transport(
    mut operation: ProviderFuture<DeepSeekHttpResponseMeta>,
    cancellation: CancellationToken,
    timeout: Duration,
) -> Result<DeepSeekHttpResponseMeta, ProviderError> {
    let mut cancelled = Box::pin(cancellation.cancelled());
    let mut deadline = Box::pin(tokio::time::sleep(timeout));
    poll_fn(move |context| {
        if cancelled.as_mut().poll(context).is_ready() {
            return Poll::Ready(Err(ProviderError::cancelled(true)));
        }
        if deadline.as_mut().poll(context).is_ready() {
            return Poll::Ready(Err(ProviderError::timeout(true)));
        }
        operation.as_mut().poll(context)
    })
    .await
}

#[derive(Clone)]
struct ValidatedCredentials {
    endpoint: Url,
    model: String,
    api_key: SecretText,
}

impl fmt::Debug for ValidatedCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedCredentials")
            .field("endpoint", &"[REDACTED]")
            .field("model", &"[REDACTED]")
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

impl TryFrom<DeepSeekCredentials> for ValidatedCredentials {
    type Error = ProviderError;

    fn try_from(credentials: DeepSeekCredentials) -> Result<Self, Self::Error> {
        let base_url = credentials.base_url.expose();
        let model = credentials.model.expose();
        let api_key = credentials.api_key.expose();
        if base_url.is_empty()
            || base_url.len() > MAX_BASE_URL_BYTES
            || model.is_empty()
            || model.len() > MAX_MODEL_BYTES
            || api_key.is_empty()
            || api_key.len() > MAX_API_KEY_BYTES
            || model.bytes().any(|byte| byte.is_ascii_control())
            || api_key.bytes().any(|byte| !byte.is_ascii_graphic())
        {
            return Err(local_schema_error(
                "Provider credentials are missing or outside the reviewed bounds.",
            ));
        }

        let mut endpoint = Url::parse(base_url).map_err(|_| {
            local_schema_error("Provider endpoint is not a valid production HTTPS base URL.")
        })?;
        if endpoint.scheme() != "https"
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(local_schema_error(
                "Provider endpoint is not a valid production HTTPS base URL.",
            ));
        }
        let base_path = endpoint.path().trim_end_matches('/');
        let path = if base_path.is_empty() {
            "/chat/completions".to_string()
        } else {
            format!("{base_path}/chat/completions")
        };
        endpoint.set_path(&path);

        Ok(Self {
            endpoint,
            model: model.to_owned(),
            api_key: credentials.api_key,
        })
    }
}

fn build_http_request(
    request: &ProviderRequest,
    credentials: &ValidatedCredentials,
    config: &DeepSeekProviderConfig,
) -> Result<DeepSeekHttpRequest, ProviderError> {
    if request.provider_id != "deepseek"
        || request.model != credentials.model
        || request.context_digest.len() != 64
        || !request
            .context_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || request.messages.is_empty()
        || request.messages.len() > MAX_MESSAGES
        || request.tools.len() > MAX_TOOLS
        || !(1..=MAX_OUTPUT_TOKENS).contains(&request.max_output_tokens)
    {
        return Err(local_schema_error(
            "Provider request metadata is outside the reviewed contract.",
        ));
    }

    let (thinking_mode, reasoning_effort) = thinking_policy_for_model(&credentials.model);
    let messages = request
        .messages
        .iter()
        .map(|message| {
            provider_message_json(
                message,
                config.max_tool_argument_bytes,
                thinking_mode == DeepSeekThinkingMode::Enabled,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let tools = request
        .tools
        .iter()
        .map(|tool| {
            if tool.name.is_empty()
                || tool.name.len() > MAX_TOOL_NAME_BYTES
                || tool.description.is_empty()
                || tool.description.len() > MAX_TOOL_DESCRIPTION_BYTES
                || serde_json::to_vec(&tool.input_schema)
                    .map_err(|_| local_schema_error("Product Tool Schema could not be encoded."))?
                    .len()
                    > MAX_TOOL_SCHEMA_BYTES
            {
                return Err(local_schema_error(
                    "Product Tool definition is outside the reviewed bounds.",
                ));
            }
            Ok(json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema,
                }
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut body_value = Map::from_iter([
        ("model".into(), json!(credentials.model)),
        ("messages".into(), Value::Array(messages)),
        ("stream".into(), Value::Bool(true)),
        ("stream_options".into(), json!({"include_usage": true})),
        ("max_tokens".into(), json!(request.max_output_tokens)),
        (
            "thinking".into(),
            json!({"type": thinking_mode.as_api_value()}),
        ),
    ]);
    if let Some(reasoning_effort) = reasoning_effort {
        body_value.insert(
            "reasoning_effort".into(),
            Value::String(reasoning_effort.as_api_value().into()),
        );
    }
    // DeepSeek/OpenAI-compatible endpoints define tools and tool_choice as
    // optional. In particular, the connectivity probe intentionally has no
    // tools, so sending tools=[] plus tool_choice=auto can be rejected by
    // otherwise compatible endpoints.
    if !tools.is_empty() {
        body_value.insert("tools".into(), Value::Array(tools));
        body_value.insert("tool_choice".into(), Value::String("auto".into()));
    }
    let body_value = Value::Object(body_value);
    let body = serde_json::to_vec(&body_value)
        .map_err(|_| local_schema_error("Provider request could not be encoded."))?;
    if body.len() > MAX_REQUEST_BYTES {
        return Err(local_schema_error(
            "Provider request exceeded the reviewed byte limit.",
        ));
    }
    Ok(DeepSeekHttpRequest {
        endpoint: credentials.endpoint.clone(),
        authorization: SecretText::new(format!("Bearer {}", credentials.api_key.expose())),
        body: Arc::from(body),
    })
}

fn provider_message_json(
    message: &ProviderMessage,
    max_tool_argument_bytes: usize,
    thinking_enabled: bool,
) -> Result<Value, ProviderError> {
    if message.content.len() > MAX_MESSAGE_BYTES || message.tool_calls.len() > MAX_TOOL_CALLS {
        return Err(local_schema_error(
            "Provider message exceeded the reviewed bounds.",
        ));
    }
    match message.role {
        ProviderRole::System | ProviderRole::User => {
            if message.tool_call_id.is_some()
                || !message.tool_calls.is_empty()
                || message.ephemeral_reasoning.is_some()
            {
                return Err(local_schema_error(
                    "Provider message role contained forbidden fields.",
                ));
            }
            Ok(json!({
                "role": if matches!(message.role, ProviderRole::System) { "system" } else { "user" },
                "content": message.content,
            }))
        }
        ProviderRole::Assistant => {
            if message.tool_call_id.is_some() {
                return Err(local_schema_error(
                    "Assistant Provider message contained a tool result ID.",
                ));
            }
            let tool_calls = message
                .tool_calls
                .iter()
                .map(|call| provider_tool_call_json(call, max_tool_argument_bytes))
                .collect::<Result<Vec<_>, _>>()?;
            if thinking_enabled && !tool_calls.is_empty() && message.ephemeral_reasoning.is_none() {
                return Err(local_schema_error(
                    "Thinking-mode assistant Tool Calls require their ephemeral reasoning continuation.",
                ));
            }
            let mut value = Map::new();
            value.insert("role".into(), Value::String("assistant".into()));
            value.insert("content".into(), Value::String(message.content.clone()));
            if !tool_calls.is_empty() {
                value.insert("tool_calls".into(), Value::Array(tool_calls));
            }
            if let Some(reasoning) = &message.ephemeral_reasoning {
                if reasoning.as_str().len() > MAX_MESSAGE_BYTES {
                    return Err(local_schema_error(
                        "Ephemeral Provider reasoning exceeded the reviewed bound.",
                    ));
                }
                value.insert(
                    "reasoning_content".into(),
                    Value::String(reasoning.as_str().to_owned()),
                );
            }
            Ok(Value::Object(value))
        }
        ProviderRole::Tool => {
            let call_id = message.tool_call_id.as_deref().ok_or_else(|| {
                local_schema_error("Tool Provider message is missing its call ID.")
            })?;
            if call_id.is_empty()
                || call_id.len() > 160
                || !message.tool_calls.is_empty()
                || message.ephemeral_reasoning.is_some()
            {
                return Err(local_schema_error(
                    "Tool Provider message contained forbidden fields.",
                ));
            }
            Ok(json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": message.content,
            }))
        }
    }
}

fn provider_tool_call_json(
    call: &ProviderToolCall,
    max_tool_argument_bytes: usize,
) -> Result<Value, ProviderError> {
    if call.call_id.is_empty()
        || call.call_id.len() > 160
        || call.name.is_empty()
        || call.name.len() > MAX_TOOL_NAME_BYTES
        || !call.arguments.is_object()
    {
        return Err(local_schema_error(
            "Provider Tool Call is outside the reviewed contract.",
        ));
    }
    let arguments = serde_json::to_string(&call.arguments)
        .map_err(|_| local_schema_error("Provider Tool Call arguments could not be encoded."))?;
    if arguments.len() > max_tool_argument_bytes {
        return Err(local_schema_error(
            "Provider Tool Call arguments exceeded the reviewed bound.",
        ));
    }
    Ok(json!({
        "id": call.call_id,
        "type": "function",
        "function": {"name": call.name, "arguments": arguments},
    }))
}

struct DeepSeekSseParser {
    line_buffer: Vec<u8>,
    event_data: Vec<u8>,
    total_bytes: usize,
    content: String,
    reasoning_fragments: Vec<EphemeralReasoning>,
    reasoning_bytes: usize,
    tool_calls: BTreeMap<u32, PartialToolCall>,
    usage: Option<(u64, u64, u64, u64)>,
    finish_reason: Option<ProviderFinishReason>,
    saw_json_event: bool,
    done: bool,
    max_response_bytes: usize,
    max_event_bytes: usize,
    max_content_bytes: usize,
    max_reasoning_bytes: usize,
    max_tool_argument_bytes: usize,
    thinking_enabled: bool,
    pricing: DeepSeekPricing,
}

#[derive(Default)]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

impl DeepSeekSseParser {
    fn new(config: &DeepSeekProviderConfig, thinking_enabled: bool) -> Self {
        Self {
            line_buffer: Vec::new(),
            event_data: Vec::new(),
            total_bytes: 0,
            content: String::new(),
            reasoning_fragments: Vec::new(),
            reasoning_bytes: 0,
            tool_calls: BTreeMap::new(),
            usage: None,
            finish_reason: None,
            saw_json_event: false,
            done: false,
            max_response_bytes: config.max_response_bytes,
            max_event_bytes: config.max_sse_event_bytes,
            max_content_bytes: config.max_content_bytes,
            max_reasoning_bytes: config.max_reasoning_bytes,
            max_tool_argument_bytes: config.max_tool_argument_bytes,
            thinking_enabled,
            pricing: config.pricing,
        }
    }

    fn feed(&mut self, bytes: &[u8]) -> Result<Vec<ProviderStreamEvent>, ProviderError> {
        self.total_bytes = self.total_bytes.saturating_add(bytes.len());
        if self.total_bytes > self.max_response_bytes {
            return Err(remote_schema_error(
                "Provider response exceeded the reviewed byte limit.",
            ));
        }
        self.line_buffer.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(newline) = self.line_buffer.iter().position(|byte| *byte == b'\n') {
            let mut line = self.line_buffer.drain(..=newline).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            self.process_line(&line, &mut events)?;
        }
        if self.line_buffer.len() > self.max_event_bytes {
            return Err(remote_schema_error(
                "Provider SSE line exceeded the reviewed bound.",
            ));
        }
        Ok(events)
    }

    fn process_line(
        &mut self,
        line: &[u8],
        events: &mut Vec<ProviderStreamEvent>,
    ) -> Result<(), ProviderError> {
        if line.is_empty() {
            return self.dispatch_event(events);
        }
        if line.starts_with(b":") {
            return Ok(());
        }
        if let Some(data) = line.strip_prefix(b"data:") {
            let data = data.strip_prefix(b" ").unwrap_or(data);
            if !self.event_data.is_empty() {
                self.event_data.push(b'\n');
            }
            self.event_data.extend_from_slice(data);
            if self.event_data.len() > self.max_event_bytes {
                return Err(remote_schema_error(
                    "Provider SSE event exceeded the reviewed bound.",
                ));
            }
            return Ok(());
        }
        if line.starts_with(b"event:") || line.starts_with(b"id:") || line.starts_with(b"retry:") {
            return Ok(());
        }
        Err(remote_schema_error(
            "Provider returned a malformed SSE field.",
        ))
    }

    fn dispatch_event(
        &mut self,
        events: &mut Vec<ProviderStreamEvent>,
    ) -> Result<(), ProviderError> {
        if self.event_data.is_empty() {
            return Ok(());
        }
        let data = std::mem::take(&mut self.event_data);
        if data == b"[DONE]" {
            if self.done {
                return Err(remote_schema_error(
                    "Provider emitted more than one SSE completion marker.",
                ));
            }
            self.done = true;
            return Ok(());
        }
        if self.done {
            return Err(remote_schema_error(
                "Provider emitted data after the SSE completion marker.",
            ));
        }
        let value: Value =
            serde_json::from_slice(&data).map_err(|_| ProviderError::invalid_json(true))?;
        self.saw_json_event = true;
        self.apply_chunk(value, events)
    }

    fn apply_chunk(
        &mut self,
        value: Value,
        events: &mut Vec<ProviderStreamEvent>,
    ) -> Result<(), ProviderError> {
        let object = value
            .as_object()
            .ok_or_else(|| remote_schema_error("Provider SSE data was not a JSON object."))?;
        let choices = object
            .get("choices")
            .and_then(Value::as_array)
            .ok_or_else(|| remote_schema_error("Provider SSE chunk is missing choices."))?;
        if choices.len() > 1 {
            return Err(remote_schema_error(
                "Provider returned more than one completion choice.",
            ));
        }
        // DeepSeek may attach `usage` to the same terminal chunk that carries
        // the final choice.  Keep accepting the stricter empty-choice usage
        // event too, but only commit usage after the choice has established a
        // terminal finish reason.  This preserves the no-data-after-usage
        // invariant while matching the documented stream_options contract.
        let final_usage = object.get("usage").filter(|usage| !usage.is_null());
        if self.usage.is_some() && !choices.is_empty() {
            return Err(remote_schema_error(
                "Provider emitted completion data after its final usage block.",
            ));
        }
        let Some(choice) = choices.first() else {
            if let Some(usage) = final_usage {
                if self.finish_reason.is_none() || self.usage.is_some() {
                    return Err(remote_schema_error(
                        "Provider final usage block was out of order or duplicated.",
                    ));
                }
                self.usage = Some(parse_usage(usage)?);
            }
            return Ok(());
        };
        let choice = choice
            .as_object()
            .ok_or_else(|| remote_schema_error("Provider completion choice was invalid."))?;
        if let Some(reason) = choice.get("finish_reason") {
            if !reason.is_null() {
                let reason = reason
                    .as_str()
                    .ok_or_else(|| remote_schema_error("Provider finish reason was not text."))?;
                let parsed = match reason {
                    "stop" => ProviderFinishReason::Stop,
                    "tool_calls" => ProviderFinishReason::ToolCalls,
                    "length" => return Err(ProviderError::output_truncated(true)),
                    "content_filter" => return Err(ProviderError::content_filtered(true)),
                    "insufficient_system_resource" => {
                        return Err(ProviderError::insufficient_system_resource(true))
                    }
                    _ => {
                        return Err(remote_schema_error(
                            "Provider completion ended with an unsupported finish reason.",
                        ))
                    }
                };
                if self
                    .finish_reason
                    .as_ref()
                    .is_some_and(|existing| existing != &parsed)
                {
                    return Err(remote_schema_error(
                        "Provider completion returned conflicting finish reasons.",
                    ));
                }
                self.finish_reason = Some(parsed);
            }
        }
        let delta = choice
            .get("delta")
            .and_then(Value::as_object)
            .ok_or_else(|| remote_schema_error("Provider completion choice is missing delta."))?;

        if let Some(content) = optional_text(delta, "content")? {
            if self.content.len().saturating_add(content.len()) > self.max_content_bytes {
                return Err(remote_schema_error(
                    "Provider content exceeded the reviewed bound.",
                ));
            }
            self.content.push_str(content);
            if !content.is_empty() {
                events.push(ProviderStreamEvent::ContentDelta(content.to_owned()));
            }
        }
        if let Some(reasoning) = optional_text(delta, "reasoning_content")? {
            self.reasoning_bytes = self.reasoning_bytes.saturating_add(reasoning.len());
            if self.reasoning_bytes > self.max_reasoning_bytes {
                return Err(remote_schema_error(
                    "Provider reasoning exceeded the reviewed in-memory bound.",
                ));
            }
            if !reasoning.is_empty() {
                let reasoning = EphemeralReasoning::new(reasoning);
                self.reasoning_fragments.push(reasoning.clone());
                events.push(ProviderStreamEvent::ReasoningDelta(reasoning));
            }
        }
        if let Some(calls) = delta.get("tool_calls") {
            let calls = calls
                .as_array()
                .ok_or_else(|| remote_schema_error("Provider Tool Call delta was not an array."))?;
            if calls.len() > MAX_TOOL_CALLS {
                return Err(remote_schema_error(
                    "Provider returned too many Tool Calls in one delta.",
                ));
            }
            for call in calls {
                self.apply_tool_call_delta(call)?;
            }
        }
        if let Some(usage) = final_usage {
            if self.finish_reason.is_none() || self.usage.is_some() {
                return Err(remote_schema_error(
                    "Provider final usage block was out of order or duplicated.",
                ));
            }
            self.usage = Some(parse_usage(usage)?);
        }
        Ok(())
    }

    fn apply_tool_call_delta(&mut self, value: &Value) -> Result<(), ProviderError> {
        let object = value
            .as_object()
            .ok_or_else(|| remote_schema_error("Provider Tool Call delta was invalid."))?;
        let index = object
            .get("index")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|index| (*index as usize) < MAX_TOOL_CALLS)
            .ok_or_else(|| remote_schema_error("Provider Tool Call index was invalid."))?;
        if object
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind != "function")
        {
            return Err(remote_schema_error(
                "Provider returned a non-function Tool Call.",
            ));
        }
        let partial = self.tool_calls.entry(index).or_default();
        if let Some(id) = optional_text(object, "id")? {
            partial.id.push_str(id);
            if partial.id.len() > 160 {
                return Err(remote_schema_error(
                    "Provider Tool Call ID exceeded the reviewed bound.",
                ));
            }
        }
        if let Some(function) = object.get("function") {
            let function = function
                .as_object()
                .ok_or_else(|| remote_schema_error("Provider Tool Call function was invalid."))?;
            if let Some(name) = optional_text(function, "name")? {
                partial.name.push_str(name);
                if partial.name.len() > MAX_TOOL_NAME_BYTES {
                    return Err(remote_schema_error(
                        "Provider Tool Call name exceeded the reviewed bound.",
                    ));
                }
            }
            if let Some(arguments) = optional_text(function, "arguments")? {
                partial.arguments.push_str(arguments);
                if partial.arguments.len() > self.max_tool_argument_bytes {
                    return Err(remote_schema_error(
                        "Provider Tool Call arguments exceeded the reviewed bound.",
                    ));
                }
            }
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(ProviderResponse, Vec<ProviderStreamEvent>), ProviderError> {
        let mut events = Vec::new();
        if !self.line_buffer.is_empty() {
            let mut line = std::mem::take(&mut self.line_buffer);
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            self.process_line(&line, &mut events)?;
        }
        self.dispatch_event(&mut events)?;
        if !self.done {
            return Err(remote_schema_error(
                "Provider SSE stream ended before the completion marker.",
            ));
        }
        if !self.saw_json_event {
            return Err(ProviderError::empty_json(true));
        }
        let (input_tokens, output_tokens, prompt_cache_hit_tokens, prompt_cache_miss_tokens) =
            self.usage.ok_or_else(|| {
                remote_schema_error("Provider SSE stream did not include final usage.")
            })?;
        let finish_reason = self.finish_reason.clone().ok_or_else(|| {
            remote_schema_error("Provider SSE stream did not include a finish reason.")
        })?;
        let mut tool_calls = Vec::with_capacity(self.tool_calls.len());
        for (_index, partial) in std::mem::take(&mut self.tool_calls) {
            if partial.id.is_empty() || partial.name.is_empty() || partial.arguments.is_empty() {
                return Err(remote_schema_error(
                    "Provider Tool Call was missing a required field.",
                ));
            }
            let arguments: Value = serde_json::from_str(&partial.arguments).map_err(|_| {
                remote_schema_error("Provider Tool Call arguments were not valid JSON.")
            })?;
            if !arguments.is_object() {
                return Err(remote_schema_error(
                    "Provider Tool Call arguments were not a JSON object.",
                ));
            }
            let call = ProviderToolCall {
                call_id: partial.id,
                name: partial.name,
                arguments,
            };
            events.push(ProviderStreamEvent::ToolCallReady(call.clone()));
            tool_calls.push(call);
        }
        if tool_calls.len() > MAX_TOOL_CALLS {
            return Err(remote_schema_error(
                "Provider returned too many Tool Calls.",
            ));
        }
        let ephemeral_reasoning = if self.reasoning_fragments.is_empty() {
            None
        } else {
            let mut joined = String::with_capacity(self.reasoning_bytes);
            for fragment in &self.reasoning_fragments {
                joined.push_str(fragment.as_str());
            }
            Some(EphemeralReasoning::new(joined))
        };
        if self.thinking_enabled
            && finish_reason == ProviderFinishReason::ToolCalls
            && ephemeral_reasoning.is_none()
        {
            return Err(remote_schema_error(
                "Thinking-mode Provider Tool Calls omitted the required reasoning continuation.",
            ));
        }
        Ok((
            ProviderResponse {
                content: (!self.content.is_empty()).then(|| std::mem::take(&mut self.content)),
                tool_calls,
                ephemeral_reasoning,
                usage: ProviderUsage {
                    input_tokens,
                    output_tokens,
                    prompt_cache_hit_tokens,
                    prompt_cache_miss_tokens,
                    estimated_cost_microusd: self.pricing.estimate(input_tokens, output_tokens),
                },
                finish_reason,
                network_call_made: true,
            },
            events,
        ))
    }
}

fn optional_text<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<Option<&'a str>, ProviderError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(remote_schema_error(
            "Provider SSE text field had an invalid type.",
        )),
    }
}

fn parse_usage(value: &Value) -> Result<(u64, u64, u64, u64), ProviderError> {
    let usage = value
        .as_object()
        .ok_or_else(|| remote_schema_error("Provider usage was not a JSON object."))?;
    let input = usage_u64(usage, "prompt_tokens")?
        .ok_or_else(|| remote_schema_error("Provider usage is missing prompt tokens."))?;
    let output = usage_u64(usage, "completion_tokens")?
        .ok_or_else(|| remote_schema_error("Provider usage is missing completion tokens."))?;
    let prompt_cache_hit_tokens = usage_u64(usage, "prompt_cache_hit_tokens")?.unwrap_or(0);
    let prompt_cache_miss_tokens = usage_u64(usage, "prompt_cache_miss_tokens")?.unwrap_or(0);
    if input > 10_000_000
        || output > 10_000_000
        || prompt_cache_hit_tokens > 10_000_000
        || prompt_cache_miss_tokens > 10_000_000
    {
        return Err(remote_schema_error(
            "Provider usage exceeded the reviewed token bound.",
        ));
    }
    if usage_u64(usage, "total_tokens")?.is_some_and(|total| total != input.saturating_add(output))
    {
        return Err(remote_schema_error(
            "Provider usage totals were internally inconsistent.",
        ));
    }
    if prompt_cache_hit_tokens
        .checked_add(prompt_cache_miss_tokens)
        .is_none_or(|cache_total| cache_total > input)
    {
        return Err(remote_schema_error(
            "Provider cache usage exceeded total prompt tokens.",
        ));
    }
    Ok((
        input,
        output,
        prompt_cache_hit_tokens,
        prompt_cache_miss_tokens,
    ))
}

fn usage_u64(usage: &Map<String, Value>, key: &str) -> Result<Option<u64>, ProviderError> {
    match usage.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| remote_schema_error("Provider usage token field had an invalid type.")),
    }
}

fn local_schema_error(message: &str) -> ProviderError {
    ProviderError::schema_mismatch(message, false)
}

fn remote_schema_error(message: &str) -> ProviderError {
    let code = match message {
        "Provider response was not a bounded SSE stream." => "PROVIDER_SCHEMA_RESPONSE_NOT_SSE",
        "Provider response exceeded the reviewed byte limit." => {
            "PROVIDER_SCHEMA_RESPONSE_TOO_LARGE"
        }
        "Provider SSE line exceeded the reviewed bound." => "PROVIDER_SCHEMA_SSE_LINE_TOO_LARGE",
        "Provider SSE event exceeded the reviewed bound." => "PROVIDER_SCHEMA_SSE_EVENT_TOO_LARGE",
        "Provider returned a malformed SSE field." => "PROVIDER_SCHEMA_SSE_FIELD_INVALID",
        "Provider emitted more than one SSE completion marker." => {
            "PROVIDER_SCHEMA_SSE_DUPLICATE_DONE"
        }
        "Provider emitted data after the SSE completion marker." => {
            "PROVIDER_SCHEMA_SSE_DATA_AFTER_DONE"
        }
        "Provider SSE data was not a JSON object." => "PROVIDER_SCHEMA_SSE_OBJECT_INVALID",
        "Provider SSE chunk is missing choices." => "PROVIDER_SCHEMA_MISSING_CHOICES",
        "Provider returned more than one completion choice." => "PROVIDER_SCHEMA_MULTI_CHOICE",
        "Provider final usage block was out of order or duplicated." => {
            "PROVIDER_SCHEMA_USAGE_ORDER"
        }
        "Provider emitted completion data after its final usage block." => {
            "PROVIDER_SCHEMA_DATA_AFTER_USAGE"
        }
        "Provider completion choice was invalid." => "PROVIDER_SCHEMA_CHOICE_INVALID",
        "Provider finish reason was not text." => "PROVIDER_SCHEMA_FINISH_TYPE",
        "Provider completion ended with an unsupported finish reason." => {
            "PROVIDER_SCHEMA_FINISH_UNSUPPORTED"
        }
        "Provider completion returned conflicting finish reasons." => {
            "PROVIDER_SCHEMA_FINISH_CONFLICT"
        }
        "Provider completion choice is missing delta." => "PROVIDER_SCHEMA_MISSING_DELTA",
        "Provider content exceeded the reviewed bound." => "PROVIDER_SCHEMA_CONTENT_TOO_LARGE",
        "Provider reasoning exceeded the reviewed in-memory bound." => {
            "PROVIDER_SCHEMA_REASONING_TOO_LARGE"
        }
        "Provider Tool Call delta was not an array." => "PROVIDER_SCHEMA_TOOL_DELTA_ARRAY",
        "Provider returned too many Tool Calls in one delta." => {
            "PROVIDER_SCHEMA_TOOL_DELTA_TOO_MANY"
        }
        "Provider Tool Call delta was invalid." => "PROVIDER_SCHEMA_TOOL_DELTA_INVALID",
        "Provider Tool Call index was invalid." => "PROVIDER_SCHEMA_TOOL_INDEX_INVALID",
        "Provider returned a non-function Tool Call." => "PROVIDER_SCHEMA_TOOL_TYPE",
        "Provider Tool Call ID exceeded the reviewed bound." => "PROVIDER_SCHEMA_TOOL_ID_TOO_LARGE",
        "Provider Tool Call function was invalid." => "PROVIDER_SCHEMA_TOOL_FUNCTION_INVALID",
        "Provider Tool Call name exceeded the reviewed bound." => {
            "PROVIDER_SCHEMA_TOOL_NAME_TOO_LARGE"
        }
        "Provider Tool Call arguments exceeded the reviewed bound." => {
            "PROVIDER_SCHEMA_TOOL_ARGUMENTS_TOO_LARGE"
        }
        "Provider Tool Call was missing a required field." => "PROVIDER_SCHEMA_TOOL_REQUIRED_FIELD",
        "Provider Tool Call arguments were not a JSON object." => {
            "PROVIDER_SCHEMA_TOOL_ARGUMENTS_OBJECT"
        }
        "Provider Tool Call arguments were not valid JSON." => {
            "PROVIDER_SCHEMA_TOOL_ARGUMENTS_INVALID_JSON"
        }
        "Provider returned too many Tool Calls." => "PROVIDER_SCHEMA_TOOL_TOO_MANY",
        "Thinking-mode Provider Tool Calls omitted the required reasoning continuation." => {
            "PROVIDER_SCHEMA_REASONING_MISSING"
        }
        "Provider SSE stream ended before the completion marker." => "PROVIDER_SCHEMA_DONE_MISSING",
        "Provider SSE stream did not include final usage." => "PROVIDER_SCHEMA_USAGE_MISSING",
        "Provider SSE stream did not include a finish reason." => "PROVIDER_SCHEMA_FINISH_MISSING",
        "Provider usage was not a JSON object." => "PROVIDER_SCHEMA_USAGE_OBJECT",
        "Provider usage is missing prompt tokens." => "PROVIDER_SCHEMA_USAGE_PROMPT_MISSING",
        "Provider usage is missing completion tokens." => {
            "PROVIDER_SCHEMA_USAGE_COMPLETION_MISSING"
        }
        "Provider usage exceeded the reviewed token bound." => "PROVIDER_SCHEMA_USAGE_TOO_LARGE",
        "Provider usage totals were internally inconsistent." => {
            "PROVIDER_SCHEMA_USAGE_TOTAL_MISMATCH"
        }
        "Provider cache usage exceeded total prompt tokens." => {
            "PROVIDER_SCHEMA_USAGE_CACHE_MISMATCH"
        }
        "Provider usage token field had an invalid type." => "PROVIDER_SCHEMA_USAGE_TYPE",
        _ => "PROVIDER_SCHEMA_MISMATCH",
    };
    ProviderError::schema_mismatch_with_code(code, message, true)
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

    use forgecad_app_server::{ProviderErrorCategory, ProviderToolDefinition};

    use super::*;

    #[derive(Clone)]
    struct FakeCredentialSource {
        value: Result<Option<DeepSeekCredentials>, DeepSeekCredentialSourceError>,
        loads: Arc<AtomicUsize>,
    }

    impl DeepSeekCredentialSource for FakeCredentialSource {
        fn load(&self) -> Result<Option<DeepSeekCredentials>, DeepSeekCredentialSourceError> {
            self.loads.fetch_add(1, Ordering::SeqCst);
            self.value.clone()
        }
    }

    struct QueuedCredentialSource {
        values: Mutex<VecDeque<Option<DeepSeekCredentials>>>,
        loads: Arc<AtomicUsize>,
    }

    impl DeepSeekCredentialSource for QueuedCredentialSource {
        fn load(&self) -> Result<Option<DeepSeekCredentials>, DeepSeekCredentialSourceError> {
            self.loads.fetch_add(1, Ordering::SeqCst);
            Ok(self.values.lock().unwrap().pop_front().unwrap_or(None))
        }
    }

    struct FakeHttpScript {
        status: u16,
        content_type: Option<String>,
        retry_after_ms: Option<u64>,
        chunks: Vec<Vec<u8>>,
        delay: Duration,
        network_call_made: bool,
    }

    struct FakeHttpTransport {
        scripts: Mutex<VecDeque<FakeHttpScript>>,
        requests: Arc<Mutex<Vec<DeepSeekHttpRequest>>>,
        calls: Arc<AtomicUsize>,
    }

    impl FakeHttpTransport {
        fn new(scripts: Vec<FakeHttpScript>) -> Self {
            Self {
                scripts: Mutex::new(scripts.into()),
                requests: Arc::new(Mutex::new(Vec::new())),
                calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl DeepSeekHttpTransport for FakeHttpTransport {
        fn post_sse(
            &self,
            request: DeepSeekHttpRequest,
            _cancellation: CancellationToken,
            mut chunks: DeepSeekHttpChunkSink,
        ) -> ProviderFuture<DeepSeekHttpResponseMeta> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.requests.lock().unwrap().push(request);
            let script = self.scripts.lock().unwrap().pop_front().unwrap();
            Box::pin(async move {
                if !script.delay.is_zero() {
                    tokio::time::sleep(script.delay).await;
                }
                if (200..300).contains(&script.status) {
                    for chunk in script.chunks {
                        chunks(&chunk)?;
                    }
                }
                Ok(DeepSeekHttpResponseMeta {
                    status: script.status,
                    retry_after_ms: script.retry_after_ms,
                    content_type: script.content_type,
                    network_call_made: script.network_call_made,
                })
            })
        }
    }

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    fn credentials() -> DeepSeekCredentials {
        DeepSeekCredentials::new(
            "https://api.deepseek.invalid/v1",
            "deepseek-test-model",
            "unit-test-credential-material-123",
        )
    }

    fn source(value: Option<DeepSeekCredentials>) -> Arc<dyn DeepSeekCredentialSource> {
        Arc::new(FakeCredentialSource {
            value: Ok(value),
            loads: Arc::new(AtomicUsize::new(0)),
        })
    }

    fn counting_source(
        value: Option<DeepSeekCredentials>,
    ) -> (Arc<dyn DeepSeekCredentialSource>, Arc<AtomicUsize>) {
        let loads = Arc::new(AtomicUsize::new(0));
        (
            Arc::new(FakeCredentialSource {
                value: Ok(value),
                loads: loads.clone(),
            }),
            loads,
        )
    }

    fn queued_source(
        values: Vec<Option<DeepSeekCredentials>>,
    ) -> (Arc<dyn DeepSeekCredentialSource>, Arc<AtomicUsize>) {
        let loads = Arc::new(AtomicUsize::new(0));
        (
            Arc::new(QueuedCredentialSource {
                values: Mutex::new(values.into()),
                loads: loads.clone(),
            }),
            loads,
        )
    }

    fn config() -> DeepSeekProviderConfig {
        let mut config =
            DeepSeekProviderConfig::bounded(DeepSeekPricing::new(1_000_000, 2_000_000).unwrap());
        config.request_timeout = Duration::from_secs(1);
        config
    }

    fn request(tools: usize) -> ProviderRequest {
        ProviderRequest {
            provider_id: "deepseek".into(),
            model: "deepseek-test-model".into(),
            context_digest: "a".repeat(64),
            messages: vec![ProviderMessage {
                role: ProviderRole::User,
                content: "设计一个虚构的机械概念道具外观".into(),
                tool_call_id: None,
                tool_calls: Vec::new(),
                ephemeral_reasoning: None,
            }],
            tools: (0..tools)
                .map(|index| ProviderToolDefinition {
                    name: format!("tool_{index}"),
                    description: "Bounded test tool.".into(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {},
                        "additionalProperties": false,
                    }),
                })
                .collect(),
            max_output_tokens: 512,
        }
    }

    fn sse_script(body: &str) -> FakeHttpScript {
        FakeHttpScript {
            status: 200,
            content_type: Some("text/event-stream; charset=utf-8".into()),
            retry_after_ms: None,
            chunks: vec![body.as_bytes().to_vec()],
            delay: Duration::ZERO,
            network_call_made: true,
        }
    }

    fn stop_stream(content: &str) -> String {
        format!(
            "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
            json!({"choices":[{"delta":{"content":content,"reasoning_content":"private"},"finish_reason":"stop"}]}),
            json!({"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":4,"total_tokens":14,"prompt_cache_hit_tokens":6,"prompt_cache_miss_tokens":4}}),
        )
    }

    fn tool_call_stream() -> String {
        format!(
            "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
            json!({"choices":[{"delta":{"reasoning_content":"short-lived","tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"tool_0","arguments":"{}"}}]},"finish_reason":"tool_calls"}]}),
            json!({"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":4,"total_tokens":14,"prompt_cache_hit_tokens":6,"prompt_cache_miss_tokens":4}}),
        )
    }

    #[test]
    fn credentials_and_http_request_debug_are_redacted_and_http_is_rejected() {
        let credential = credentials();
        let debug = format!("{credential:?}");
        assert!(!debug.contains("api.deepseek.invalid"));
        assert!(!debug.contains("deepseek-test-model"));
        assert!(!debug.contains("unit-test-credential-material"));

        let invalid = DeepSeekCredentials::new(
            "http://api.deepseek.invalid",
            "private-model",
            "private-key",
        );
        let error = ValidatedCredentials::try_from(invalid).unwrap_err();
        let error_text = format!("{error:?}");
        assert!(!error_text.contains("api.deepseek.invalid"));
        assert!(!error_text.contains("private-model"));
        assert!(!error_text.contains("private-key"));
        assert!(!error.network_call_made);
    }

    #[test]
    fn content_type_requires_the_exact_sse_media_type() {
        assert!(is_event_stream_content_type("text/event-stream"));
        assert!(is_event_stream_content_type(
            "Text/Event-Stream; charset=utf-8"
        ));
        assert!(is_event_stream_content_type(
            " text/event-stream ; charset=utf-8"
        ));
        assert!(!is_event_stream_content_type("text/event-stream-evil"));
        assert!(!is_event_stream_content_type("text/event-streaming"));
        assert!(!is_event_stream_content_type("application/json"));
    }

    #[test]
    fn current_v4_models_use_explicit_max_thinking_and_legacy_aliases_stay_compatible() {
        for model in ["deepseek-v4-flash", "deepseek-v4-pro"] {
            assert_eq!(
                thinking_policy_for_model(model),
                (
                    DeepSeekThinkingMode::Enabled,
                    Some(DeepSeekReasoningEffort::Max)
                )
            );
        }
        assert_eq!(
            thinking_policy_for_model("deepseek-chat"),
            (DeepSeekThinkingMode::Disabled, None)
        );
        assert_eq!(
            thinking_policy_for_model("deepseek-reasoner"),
            (
                DeepSeekThinkingMode::Enabled,
                Some(DeepSeekReasoningEffort::High)
            )
        );
    }

    #[test]
    fn request_is_bounded_and_contains_no_endpoint_key_path_or_database_fields() {
        block_on(async {
            let transport = Arc::new(FakeHttpTransport::new(vec![sse_script(&stop_stream(
                "完成",
            ))]));
            let client = DeepSeekProviderClient::new(
                source(Some(credentials())),
                transport.clone(),
                config(),
            )
            .unwrap();
            client
                .stream(request(1), CancellationToken::new(), Box::new(|_| {}))
                .await
                .unwrap();
            let requests = transport.requests.lock().unwrap();
            let captured = requests.last().unwrap();
            assert_eq!(captured.endpoint.scheme(), "https");
            assert_eq!(captured.endpoint.path(), "/v1/chat/completions");
            assert_eq!(captured.endpoint_path(), "/v1/chat/completions");
            let debug = format!("{captured:?}");
            assert!(!debug.contains("api.deepseek.invalid"));
            assert!(!debug.contains("unit-test-credential-material"));
            let body: Value = serde_json::from_slice(captured.body()).unwrap();
            assert_eq!(
                body.as_object()
                    .unwrap()
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>(),
                vec![
                    "max_tokens",
                    "messages",
                    "model",
                    "reasoning_effort",
                    "stream",
                    "stream_options",
                    "thinking",
                    "tool_choice",
                    "tools",
                ]
            );
            assert_eq!(body["thinking"], json!({"type": "enabled"}));
            assert_eq!(body["reasoning_effort"], "max");
            let encoded = String::from_utf8(captured.body().to_vec()).unwrap();
            assert!(!encoded.contains("base_url"));
            assert!(!encoded.contains("api_key"));
            assert!(!encoded.contains("context_digest"));
            assert!(!encoded.contains("database"));
            assert!(!encoded.contains("file_path"));
            assert!(!encoded.contains("unit-test-credential-material"));
        });
    }

    #[test]
    fn configured_tool_argument_limit_applies_to_follow_up_assistant_messages() {
        block_on(async {
            let transport = Arc::new(FakeHttpTransport::new(Vec::new()));
            let mut bounded_config = config();
            bounded_config.max_tool_argument_bytes = 1024;
            let client = DeepSeekProviderClient::new(
                source(Some(credentials())),
                transport.clone(),
                bounded_config,
            )
            .unwrap();
            let mut follow_up = request(1);
            follow_up.messages = vec![ProviderMessage {
                role: ProviderRole::Assistant,
                content: String::new(),
                tool_call_id: None,
                tool_calls: vec![ProviderToolCall {
                    call_id: "call_oversized".into(),
                    name: "tool_0".into(),
                    arguments: json!({"payload": "x".repeat(1100)}),
                }],
                ephemeral_reasoning: Some(EphemeralReasoning::new("short-lived")),
            }];
            let events = Arc::new(Mutex::new(Vec::new()));
            let captured = events.clone();
            let error = client
                .stream(
                    follow_up,
                    CancellationToken::new(),
                    Box::new(move |event| captured.lock().unwrap().push(event)),
                )
                .await
                .unwrap_err();
            assert_eq!(error.category, ProviderErrorCategory::SchemaMismatch);
            assert!(!error.network_call_made);
            assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
            assert!(events.lock().unwrap().is_empty());
        });
    }

    #[test]
    fn fragmented_sse_streams_content_ephemeral_reasoning_tool_calls_and_usage() {
        block_on(async {
            let stream = format!(
                "data: {}\n\ndata: {}\n\ndata: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
                json!({"choices":[{"delta":{"reasoning_content":"hidden ","tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"tool_0","arguments":"{\"value\":"}}]},"finish_reason":null}]}),
                json!({"choices":[{"delta":{"reasoning_content":"chain","tool_calls":[{"index":0,"function":{"arguments":"1}"}}]},"finish_reason":"tool_calls"}]}),
                json!({"choices":[],"usage":{"prompt_tokens":21,"completion_tokens":7,"total_tokens":28,"prompt_cache_hit_tokens":13,"prompt_cache_miss_tokens":8}}),
                json!({"choices":[]}),
            );
            let bytes = stream.into_bytes();
            let split = bytes.len() / 3;
            let script = FakeHttpScript {
                status: 200,
                content_type: Some("text/event-stream".into()),
                retry_after_ms: None,
                chunks: vec![
                    bytes[..split].to_vec(),
                    bytes[split..split * 2].to_vec(),
                    bytes[split * 2..].to_vec(),
                ],
                delay: Duration::ZERO,
                network_call_made: true,
            };
            let client = DeepSeekProviderClient::new(
                source(Some(credentials())),
                Arc::new(FakeHttpTransport::new(vec![script])),
                config(),
            )
            .unwrap();
            let captured = Arc::new(Mutex::new(Vec::new()));
            let event_capture = captured.clone();
            let response = client
                .stream(
                    request(1),
                    CancellationToken::new(),
                    Box::new(move |event| event_capture.lock().unwrap().push(event)),
                )
                .await
                .unwrap();
            assert_eq!(response.finish_reason, ProviderFinishReason::ToolCalls);
            assert_eq!(response.tool_calls.len(), 1);
            assert_eq!(response.tool_calls[0].arguments, json!({"value": 1}));
            assert_eq!(
                response.ephemeral_reasoning.as_ref().unwrap().as_str(),
                "hidden chain"
            );
            assert_eq!(
                format!("{:?}", response.ephemeral_reasoning.as_ref().unwrap()),
                "EphemeralReasoning([REDACTED])"
            );
            assert_eq!(response.usage.input_tokens, 21);
            assert_eq!(response.usage.output_tokens, 7);
            assert_eq!(response.usage.prompt_cache_hit_tokens, 13);
            assert_eq!(response.usage.prompt_cache_miss_tokens, 8);
            assert_eq!(response.usage.estimated_cost_microusd, 35);
            let events = captured.lock().unwrap();
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(event, ProviderStreamEvent::ReasoningDelta(_)))
                    .count(),
                2
            );
            assert!(events
                .iter()
                .any(|event| matches!(event, ProviderStreamEvent::ToolCallReady(_))));
        });
    }

    #[test]
    fn terminal_choice_may_carry_usage_in_the_same_sse_event() {
        block_on(async {
            let stream = format!(
                "data: {}\n\ndata: [DONE]\n\n",
                json!({
                    "choices": [{
                        "delta": {"content": "OK", "reasoning_content": ""},
                        "finish_reason": "stop"
                    }],
                    "usage": {
                        "prompt_tokens": 10,
                        "completion_tokens": 2,
                        "total_tokens": 12,
                        "prompt_cache_hit_tokens": 6,
                        "prompt_cache_miss_tokens": 4
                    }
                })
            );
            let client = DeepSeekProviderClient::new(
                source(Some(credentials())),
                Arc::new(FakeHttpTransport::new(vec![sse_script(&stream)])),
                config(),
            )
            .unwrap();
            let response = client
                .stream(request(0), CancellationToken::new(), Box::new(|_| {}))
                .await
                .unwrap();
            assert_eq!(response.finish_reason, ProviderFinishReason::Stop);
            assert_eq!(response.content.as_deref(), Some("OK"));
            assert_eq!(response.usage.input_tokens, 10);
            assert_eq!(response.usage.output_tokens, 2);
            assert_eq!(response.usage.prompt_cache_hit_tokens, 6);
            assert_eq!(response.usage.prompt_cache_miss_tokens, 4);
        });
    }

    #[test]
    fn thinking_tool_call_without_reasoning_continuation_fails_before_follow_up() {
        block_on(async {
            let stream = format!(
                "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
                json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"tool_0","arguments":"{}"}}]},"finish_reason":"tool_calls"}]}),
                json!({"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":4,"total_tokens":14}}),
            );
            let transport = Arc::new(FakeHttpTransport::new(vec![sse_script(&stream)]));
            let client = DeepSeekProviderClient::new(
                source(Some(credentials())),
                transport.clone(),
                config(),
            )
            .unwrap();
            let error = client
                .stream(request(1), CancellationToken::new(), Box::new(|_| {}))
                .await
                .unwrap_err();
            assert_eq!(error.category, ProviderErrorCategory::SchemaMismatch);
            assert!(error.network_call_made);
            assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn http_status_empty_invalid_json_and_schema_failures_are_stable() {
        block_on(async {
            for (status, category) in [
                (401, ProviderErrorCategory::Authentication),
                (403, ProviderErrorCategory::Authentication),
                (402, ProviderErrorCategory::Balance),
                (429, ProviderErrorCategory::RateLimited),
                (503, ProviderErrorCategory::ServerUnavailable),
            ] {
                let script = FakeHttpScript {
                    status,
                    content_type: Some("application/json".into()),
                    retry_after_ms: Some(1000),
                    chunks: Vec::new(),
                    delay: Duration::ZERO,
                    network_call_made: true,
                };
                let client = DeepSeekProviderClient::new(
                    source(Some(credentials())),
                    Arc::new(FakeHttpTransport::new(vec![script])),
                    config(),
                )
                .unwrap();
                let error = client
                    .stream(request(0), CancellationToken::new(), Box::new(|_| {}))
                    .await
                    .unwrap_err();
                assert_eq!(error.category, category);
                assert!(error.network_call_made);
            }

            for (body, category) in [
                ("data: [DONE]\n\n", ProviderErrorCategory::EmptyJson),
                ("data: {bad json}\n\n", ProviderErrorCategory::InvalidJson),
                (
                    "data: {\"choices\":\"invalid\"}\n\ndata: [DONE]\n\n",
                    ProviderErrorCategory::SchemaMismatch,
                ),
            ] {
                let client = DeepSeekProviderClient::new(
                    source(Some(credentials())),
                    Arc::new(FakeHttpTransport::new(vec![sse_script(body)])),
                    config(),
                )
                .unwrap();
                let error = client
                    .stream(request(0), CancellationToken::new(), Box::new(|_| {}))
                    .await
                    .unwrap_err();
                assert_eq!(error.category, category);
            }
        });
    }

    #[test]
    fn official_non_success_finish_reasons_map_to_stable_safe_failures() {
        block_on(async {
            for (finish_reason, code, category, recoverable) in [
                (
                    "length",
                    "PROVIDER_OUTPUT_TRUNCATED",
                    ProviderErrorCategory::InvalidRequest,
                    true,
                ),
                (
                    "content_filter",
                    "PROVIDER_CONTENT_FILTERED",
                    ProviderErrorCategory::InvalidRequest,
                    false,
                ),
                (
                    "insufficient_system_resource",
                    "PROVIDER_SYSTEM_RESOURCE_UNAVAILABLE",
                    ProviderErrorCategory::ServerUnavailable,
                    true,
                ),
            ] {
                let body = format!(
                    "data: {}\n\ndata: [DONE]\n\n",
                    json!({"choices":[{"delta":{"content":"partial private output"},"finish_reason":finish_reason}]})
                );
                let client = DeepSeekProviderClient::new(
                    source(Some(credentials())),
                    Arc::new(FakeHttpTransport::new(vec![sse_script(&body)])),
                    config(),
                )
                .unwrap();
                let error = client
                    .stream(request(0), CancellationToken::new(), Box::new(|_| {}))
                    .await
                    .unwrap_err();
                assert_eq!(error.code, code);
                assert_eq!(error.category, category);
                assert_eq!(error.recoverable, recoverable);
                assert!(error.network_call_made);
                assert!(!error.message.contains("partial private output"));
            }
        });
    }

    #[test]
    fn usage_accepts_missing_cache_fields_as_zero_and_rejects_invalid_boundaries() {
        assert_eq!(
            parse_usage(&json!({
                "prompt_tokens": 10,
                "completion_tokens": 4,
                "total_tokens": 14
            }))
            .unwrap(),
            (10, 4, 0, 0)
        );
        assert!(parse_usage(&json!({
            "prompt_tokens": 10,
            "completion_tokens": 4,
            "total_tokens": 15
        }))
        .is_err());
        assert!(parse_usage(&json!({
            "prompt_tokens": 10,
            "completion_tokens": 4,
            "total_tokens": 14,
            "prompt_cache_hit_tokens": 8,
            "prompt_cache_miss_tokens": 3
        }))
        .is_err());
        assert!(parse_usage(&json!({
            "prompt_tokens": 10,
            "completion_tokens": 4,
            "total_tokens": "14"
        }))
        .is_err());
    }

    #[test]
    fn final_usage_and_done_markers_are_ordered_and_unique() {
        block_on(async {
            let usage = json!({
                "prompt_tokens": 10,
                "completion_tokens": 4,
                "total_tokens": 14
            });
            let finish = json!({
                "choices": [{"delta": {"content": "done"}, "finish_reason": "stop"}]
            });
            let invalid_streams = [
                format!(
                    "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
                    json!({"choices": [], "usage": usage}),
                    finish
                ),
                format!(
                    "data: {}\n\ndata: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
                    finish,
                    json!({"choices": [], "usage": usage}),
                    json!({"choices": [], "usage": usage})
                ),
                format!(
                    "data: {}\n\ndata: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
                    finish,
                    json!({"choices": [], "usage": usage}),
                    json!({"choices": [{"delta": {"content": "late"}, "finish_reason": null}]})
                ),
                format!(
                    "data: {}\n\ndata: {}\n\ndata: [DONE]\n\ndata: [DONE]\n\n",
                    finish,
                    json!({"choices": [], "usage": usage})
                ),
            ];

            for body in invalid_streams {
                let client = DeepSeekProviderClient::new(
                    source(Some(credentials())),
                    Arc::new(FakeHttpTransport::new(vec![sse_script(&body)])),
                    config(),
                )
                .unwrap();
                let error = client
                    .stream(request(0), CancellationToken::new(), Box::new(|_| {}))
                    .await
                    .unwrap_err();
                assert_eq!(error.category, ProviderErrorCategory::SchemaMismatch);
                assert!(error.network_call_made);
            }
        });
    }

    #[test]
    fn request_budget_policy_is_deterministic_conservative_and_transport_free() {
        let provider_request = request(1);
        let provider_config = config();
        let validated_credentials = ValidatedCredentials::try_from(credentials()).unwrap();
        let serialized_request =
            build_http_request(&provider_request, &validated_credentials, &provider_config)
                .unwrap();
        let expected_input_upper_bound = u64::try_from(serialized_request.body().len())
            .unwrap()
            .checked_add(PROVIDER_INPUT_FRAMING_OVERHEAD_BYTES)
            .unwrap();
        let transport = Arc::new(FakeHttpTransport::new(Vec::new()));
        let client = DeepSeekProviderClient::new(
            source(Some(credentials())),
            transport.clone(),
            provider_config,
        )
        .unwrap();

        let first = client.request_budget_policy(&provider_request).unwrap();
        let second = client.request_budget_policy(&provider_request).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.input_tokens_upper_bound, expected_input_upper_bound);
        assert_eq!(
            first.input_cost_ceiling_microusd,
            expected_input_upper_bound
        );
        assert_eq!(first.output_microusd_per_million_tokens, 2_000_000);
        assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
        assert!(transport.requests.lock().unwrap().is_empty());
    }

    #[test]
    fn request_budget_policy_fails_closed_without_credentials_and_never_uses_transport() {
        let transport = Arc::new(FakeHttpTransport::new(Vec::new()));
        let client =
            DeepSeekProviderClient::new(source(None), transport.clone(), config()).unwrap();

        let error = client.request_budget_policy(&request(0)).unwrap_err();

        assert_eq!(error.category, ProviderErrorCategory::Unconfigured);
        assert!(!error.network_call_made);
        assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
        assert!(transport.requests.lock().unwrap().is_empty());
    }

    #[test]
    fn cancellation_timeout_unconfigured_and_tool_bounds_never_fall_through() {
        block_on(async {
            let transport = Arc::new(FakeHttpTransport::new(Vec::new()));
            let client =
                DeepSeekProviderClient::new(source(None), transport.clone(), config()).unwrap();
            let error = client
                .preflight(CancellationToken::new())
                .await
                .unwrap_err();
            assert_eq!(error.category, ProviderErrorCategory::Unconfigured);
            assert!(!error.network_call_made);
            assert_eq!(transport.calls.load(Ordering::SeqCst), 0);

            let delayed = FakeHttpScript {
                delay: Duration::from_millis(100),
                ..sse_script(&stop_stream("late"))
            };
            let mut timeout_config = config();
            timeout_config.request_timeout = Duration::from_millis(5);
            let client = DeepSeekProviderClient::new(
                source(Some(credentials())),
                Arc::new(FakeHttpTransport::new(vec![delayed])),
                timeout_config,
            )
            .unwrap();
            let error = client
                .stream(request(0), CancellationToken::new(), Box::new(|_| {}))
                .await
                .unwrap_err();
            assert_eq!(error.category, ProviderErrorCategory::Timeout);
            assert!(error.network_call_made);

            let delayed = FakeHttpScript {
                delay: Duration::from_millis(100),
                ..sse_script(&stop_stream("late"))
            };
            let client = DeepSeekProviderClient::new(
                source(Some(credentials())),
                Arc::new(FakeHttpTransport::new(vec![delayed])),
                config(),
            )
            .unwrap();
            let cancellation = CancellationToken::new();
            let trigger = cancellation.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(5)).await;
                trigger.cancel();
            });
            let error = client
                .stream(request(0), cancellation, Box::new(|_| {}))
                .await
                .unwrap_err();
            assert_eq!(error.category, ProviderErrorCategory::Cancelled);
            assert!(error.network_call_made);

            let transport = Arc::new(FakeHttpTransport::new(Vec::new()));
            let client = DeepSeekProviderClient::new(
                source(Some(credentials())),
                transport.clone(),
                config(),
            )
            .unwrap();
            let error = client
                .stream(request(14), CancellationToken::new(), Box::new(|_| {}))
                .await
                .unwrap_err();
            assert_eq!(error.category, ProviderErrorCategory::SchemaMismatch);
            assert!(!error.network_call_made);
            assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
        });
    }

    #[test]
    fn preflight_is_local_and_explicit_check_uses_the_injected_transport_once() {
        block_on(async {
            let transport = Arc::new(FakeHttpTransport::new(vec![sse_script(&stop_stream("OK"))]));
            let client = DeepSeekProviderClient::new(
                source(Some(credentials())),
                transport.clone(),
                config(),
            )
            .unwrap();
            let preflight = client.preflight(CancellationToken::new()).await.unwrap();
            assert!(preflight.configured);
            assert!(!preflight.network_call_made);
            assert_eq!(transport.calls.load(Ordering::SeqCst), 0);

            let check = client
                .check("deepseek".into(), 30_000, CancellationToken::new())
                .await
                .unwrap();
            assert!(check.network_call_made);
            let usage = check.usage.unwrap();
            assert_eq!(usage.total_tokens(), 14);
            assert_eq!(usage.prompt_cache_hit_tokens, 6);
            assert_eq!(usage.prompt_cache_miss_tokens, 4);
            assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
            let requests = transport.requests.lock().unwrap();
            let body: Value = serde_json::from_slice(requests[0].body()).unwrap();
            let body = body.as_object().unwrap();
            assert!(!body.contains_key("tools"));
            assert!(!body.contains_key("tool_choice"));
            assert!(!client
                .cancel("cancel_1".into(), "token_1".into())
                .await
                .unwrap());
        });
    }

    #[test]
    fn explicit_check_reads_one_credential_snapshot() {
        block_on(async {
            let (credential_source, loads) = counting_source(Some(credentials()));
            let transport = Arc::new(FakeHttpTransport::new(vec![sse_script(&stop_stream("OK"))]));
            let client =
                DeepSeekProviderClient::new(credential_source, transport.clone(), config())
                    .unwrap();

            let check = client
                .check("deepseek".into(), 30_000, CancellationToken::new())
                .await
                .unwrap();

            assert!(check.network_call_made);
            assert_eq!(loads.load(Ordering::SeqCst), 1);
            assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn network_attempt_event_is_once_and_never_emitted_for_local_rejection() {
        block_on(async {
            let transport = Arc::new(FakeHttpTransport::new(Vec::new()));
            let client =
                DeepSeekProviderClient::new(source(None), transport.clone(), config()).unwrap();
            let events = Arc::new(Mutex::new(Vec::new()));
            let captured = events.clone();
            let error = client
                .stream(
                    request(0),
                    CancellationToken::new(),
                    Box::new(move |event| captured.lock().unwrap().push(event)),
                )
                .await
                .unwrap_err();
            assert_eq!(error.category, ProviderErrorCategory::Unconfigured);
            assert!(events.lock().unwrap().is_empty());
            assert_eq!(transport.calls.load(Ordering::SeqCst), 0);

            let transport = Arc::new(FakeHttpTransport::new(Vec::new()));
            let client = DeepSeekProviderClient::new(
                source(Some(credentials())),
                transport.clone(),
                config(),
            )
            .unwrap();
            let events = Arc::new(Mutex::new(Vec::new()));
            let captured = events.clone();
            let error = client
                .stream(
                    request(MAX_TOOLS + 1),
                    CancellationToken::new(),
                    Box::new(move |event| captured.lock().unwrap().push(event)),
                )
                .await
                .unwrap_err();
            assert_eq!(error.category, ProviderErrorCategory::SchemaMismatch);
            assert!(events.lock().unwrap().is_empty());
            assert_eq!(transport.calls.load(Ordering::SeqCst), 0);

            let transport = Arc::new(FakeHttpTransport::new(vec![sse_script(&stop_stream("OK"))]));
            let client = DeepSeekProviderClient::new(
                source(Some(credentials())),
                transport.clone(),
                config(),
            )
            .unwrap();
            let events = Arc::new(Mutex::new(Vec::new()));
            let captured = events.clone();
            client
                .stream(
                    request(0),
                    CancellationToken::new(),
                    Box::new(move |event| captured.lock().unwrap().push(event)),
                )
                .await
                .unwrap();
            let events = events.lock().unwrap();
            assert!(matches!(
                events.first(),
                Some(ProviderStreamEvent::NetworkRequestStarted)
            ));
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(event, ProviderStreamEvent::NetworkRequestStarted))
                    .count(),
                1
            );
            assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn turn_session_reads_credentials_once_for_preflight_budget_and_tool_follow_up() {
        block_on(async {
            let (credential_source, loads) = counting_source(Some(credentials()));
            let transport = Arc::new(FakeHttpTransport::new(vec![
                sse_script(&tool_call_stream()),
                sse_script(&stop_stream("完成")),
            ]));
            let client =
                DeepSeekProviderClient::new(credential_source, transport.clone(), config())
                    .unwrap();

            let session = client.turn_session().unwrap().unwrap();
            assert_eq!(loads.load(Ordering::SeqCst), 1);
            let preflight = session.preflight(CancellationToken::new()).await.unwrap();
            assert_eq!(preflight.model, "deepseek-test-model");
            assert!(session.request_budget_policy(&request(1)).is_ok());
            let first = session
                .stream(request(1), CancellationToken::new(), Box::new(|_| {}))
                .await
                .unwrap();
            assert_eq!(first.finish_reason, ProviderFinishReason::ToolCalls);
            assert!(session.request_budget_policy(&request(1)).is_ok());
            let second = session
                .stream(request(1), CancellationToken::new(), Box::new(|_| {}))
                .await
                .unwrap();
            assert_eq!(second.finish_reason, ProviderFinishReason::Stop);
            assert_eq!(loads.load(Ordering::SeqCst), 1);
            assert_eq!(transport.calls.load(Ordering::SeqCst), 2);

            // Dropping the scoped client releases its zeroizing snapshot. A
            // subsequent explicit Turn must read a fresh source snapshot.
            drop(session);
            let next = client.turn_session().unwrap().unwrap();
            assert_eq!(loads.load(Ordering::SeqCst), 2);
            drop(next);
        });
    }

    #[test]
    fn failed_or_cancelled_turn_session_does_not_retain_credentials() {
        block_on(async {
            let (credential_source, loads) = counting_source(Some(credentials()));
            let transport = Arc::new(FakeHttpTransport::new(vec![FakeHttpScript {
                status: 503,
                content_type: Some("application/json".into()),
                retry_after_ms: None,
                chunks: Vec::new(),
                delay: Duration::ZERO,
                network_call_made: true,
            }]));
            let client =
                DeepSeekProviderClient::new(credential_source, transport, config()).unwrap();

            let failed = client.turn_session().unwrap().unwrap();
            let error = failed
                .stream(request(0), CancellationToken::new(), Box::new(|_| {}))
                .await
                .unwrap_err();
            assert_eq!(error.category, ProviderErrorCategory::ServerUnavailable);
            drop(failed);

            let cancelled = client.turn_session().unwrap().unwrap();
            let cancellation = CancellationToken::new();
            cancellation.cancel();
            let error = cancelled.preflight(cancellation).await.unwrap_err();
            assert_eq!(error.category, ProviderErrorCategory::Cancelled);
            drop(cancelled);
            assert_eq!(loads.load(Ordering::SeqCst), 2);

            let fresh = client.turn_session().unwrap().unwrap();
            assert_eq!(loads.load(Ordering::SeqCst), 3);
            drop(fresh);
        });
    }

    #[test]
    fn parallel_turn_sessions_do_not_mix_credential_snapshots() {
        block_on(async {
            let first_credentials = DeepSeekCredentials::new(
                "https://one.example.test/v1",
                "model-one",
                "credential-one",
            );
            let second_credentials = DeepSeekCredentials::new(
                "https://two.example.test/v1",
                "model-two",
                "credential-two",
            );
            let (credential_source, loads) =
                queued_source(vec![Some(first_credentials), Some(second_credentials)]);
            let transport = Arc::new(FakeHttpTransport::new(vec![
                sse_script(&stop_stream("one")),
                sse_script(&stop_stream("two")),
            ]));
            let client = Arc::new(
                DeepSeekProviderClient::new(credential_source, transport.clone(), config())
                    .unwrap(),
            );

            let first = client.turn_session().unwrap().unwrap();
            let second = client.turn_session().unwrap().unwrap();
            let first_preflight_session = first.clone();
            let second_preflight_session = second.clone();
            let first_preflight = tokio::spawn(async move {
                first_preflight_session
                    .preflight(CancellationToken::new())
                    .await
            });
            let second_preflight = tokio::spawn(async move {
                second_preflight_session
                    .preflight(CancellationToken::new())
                    .await
            });
            assert_eq!(first_preflight.await.unwrap().unwrap().model, "model-one");
            assert_eq!(second_preflight.await.unwrap().unwrap().model, "model-two");

            let first_request = ProviderRequest {
                model: "model-one".into(),
                ..request(0)
            };
            let second_request = ProviderRequest {
                model: "model-two".into(),
                ..request(0)
            };
            let first_response = tokio::spawn(async move {
                first
                    .stream(first_request, CancellationToken::new(), Box::new(|_| {}))
                    .await
            });
            let second_response = tokio::spawn(async move {
                second
                    .stream(second_request, CancellationToken::new(), Box::new(|_| {}))
                    .await
            });
            first_response.await.unwrap().unwrap();
            second_response.await.unwrap().unwrap();
            assert_eq!(loads.load(Ordering::SeqCst), 2);
            let request_models = transport
                .requests
                .lock()
                .unwrap()
                .iter()
                .map(|captured| {
                    serde_json::from_slice::<Value>(captured.body()).unwrap()["model"].clone()
                })
                .collect::<Vec<_>>();
            assert!(request_models.contains(&json!("model-one")));
            assert!(request_models.contains(&json!("model-two")));
        });
    }
}
