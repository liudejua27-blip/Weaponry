//! OpenAI-compatible visual-evidence adapter and prompt-free credential store.
//!
//! This is not a mesh generator. It sends only the current authorized image
//! evidence to a vision model and returns bounded `VisualEvidenceClaim` values
//! to the Rust coordinator. Provider payloads, image bytes and credentials are
//! never persisted in product state or emitted through Debug.

use std::{
    fmt,
    fs::{self, OpenOptions},
    future::Future,
    io::{Read, Write},
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use forgecad_app_server::{
    CancellationToken, E005PreparedVisualReviewProviderPort,
    E005PreparedVisualReviewProviderResponse, PreparedE005VisualReviewProviderRequest,
    ProviderRequestBudgetPolicy, ProviderRequestCommitment, ProviderUsage,
    VisionEvidenceProviderError, VisionEvidenceProviderFuture, VisionEvidenceProviderOutput,
    VisionEvidenceProviderPort, VisionEvidenceProviderRequest,
    VisualReferenceComparisonProviderError, VisualReferenceComparisonProviderFuture,
    VisualReferenceComparisonProviderOutput, VisualReferenceComparisonProviderPort,
    VisualReferenceComparisonProviderRequest, E005_VISUAL_REVIEW_MAX_OUTPUT_TOKENS,
    E005_VISUAL_REVIEW_SYSTEM_PROMPT, MAX_VISION_EVIDENCE_RESPONSE_BYTES,
};
use forgecad_core::{
    VisualEvidenceClaim, VisualReferenceClaimAssessment, VisualReferenceMatchOutcome,
};
use reqwest::{
    header::{self, HeaderValue},
    redirect, Client, Url,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

const VISION_CONFIG_SCHEMA_VERSION: &str = "VisionEvidenceProviderConfig@1";
const ALLOWED_VISION_HOST_SUFFIX: &str = ".aliyuncs.com";
const ALLOWED_VISION_MODEL_PREFIX: &str = "qwen";
const MAX_VISION_REQUEST_BYTES: usize = 64 * 1024 * 1024;
const MAX_VISION_MODEL_OUTPUT_TOKENS: u64 = 8_192;
const FORMAL_VISION_INPUT_FRAMING_OVERHEAD_BYTES: u64 = 4_096;
const E005_VISUAL_PATCH_PROPOSAL_SCHEMA_TEXT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../packages/concept-spec/schemas/e005-visual-patch-proposal-v1.schema.json"
));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisionEvidenceFormalPricing {
    pub input_microusd_per_million_tokens: u64,
    pub output_microusd_per_million_tokens: u64,
}

impl VisionEvidenceFormalPricing {
    pub fn new(
        input_microusd_per_million_tokens: u64,
        output_microusd_per_million_tokens: u64,
    ) -> Result<Self, VisionEvidenceProviderError> {
        if input_microusd_per_million_tokens == 0
            || output_microusd_per_million_tokens == 0
            || input_microusd_per_million_tokens > 100_000_000
            || output_microusd_per_million_tokens > 100_000_000
        {
            return Err(protocol_error(
                "Formal vision pricing is outside the reviewed bound.",
                false,
            ));
        }
        Ok(Self {
            input_microusd_per_million_tokens,
            output_microusd_per_million_tokens,
        })
    }

    fn snapshot_sha256(&self, provider_id: &str, model: &str) -> String {
        sha256_hex(
            format!(
                "OpenAiCompatibleVisionPricingSnapshot@1\n{provider_id}\n{model}\n{}\n{}",
                self.input_microusd_per_million_tokens, self.output_microusd_per_million_tokens,
            )
            .as_bytes(),
        )
    }
}

pub type VisionEvidenceHttpFuture = Pin<
    Box<
        dyn Future<Output = Result<VisionEvidenceHttpResponse, VisionEvidenceProviderError>>
            + Send
            + 'static,
    >,
>;

#[derive(Clone)]
pub struct VisionEvidenceSecret(Arc<Zeroizing<String>>);

impl VisionEvidenceSecret {
    fn new(value: String) -> Result<Self, VisionEvidenceProviderError> {
        if value.trim().is_empty() || value.len() > 4_096 || value.contains('\0') {
            return Err(error(
                "VISION_EVIDENCE_CREDENTIAL_INVALID",
                "Vision evidence credential is empty or outside the reviewed bound.",
                false,
                false,
            ));
        }
        Ok(Self(Arc::new(Zeroizing::new(value))))
    }

    fn bearer_value(&self) -> Zeroizing<String> {
        Zeroizing::new(format!("Bearer {}", self.0.as_str()))
    }
}

impl fmt::Debug for VisionEvidenceSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("VisionEvidenceSecret([REDACTED])")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct VisionEvidenceConfigFile {
    schema_version: String,
    base_url: String,
    model: String,
    credential_id: String,
}

#[derive(Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VisionEvidenceConfigMetadata {
    pub base_url: String,
    pub model: String,
    pub configured: bool,
    pub storage: String,
    pub requires_os_prompt: bool,
}

impl fmt::Debug for VisionEvidenceConfigMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VisionEvidenceConfigMetadata")
            .field("base_url", &"[REDACTED]")
            .field("model", &"[REDACTED]")
            .field("configured", &self.configured)
            .field("storage", &self.storage)
            .field("requires_os_prompt", &self.requires_os_prompt)
            .finish()
    }
}

#[derive(Clone)]
pub struct VisionEvidenceCredentialSnapshot {
    base_url: Url,
    model: String,
    secret: VisionEvidenceSecret,
}

impl fmt::Debug for VisionEvidenceCredentialSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VisionEvidenceCredentialSnapshot")
            .field("base_url", &"[REDACTED]")
            .field("model", &"[REDACTED]")
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

pub trait VisionEvidenceCredentialSource: Send + Sync + 'static {
    fn load_snapshot(
        &self,
    ) -> Result<Option<VisionEvidenceCredentialSnapshot>, VisionEvidenceProviderError>;
}

/// Two-file, generation-bound credential storage for the unsigned Alpha.
/// Metadata inspection never opens the secret file, and the store never uses
/// Keychain, avoiding repeated macOS ACL prompts during local rebuilds.
#[derive(Clone)]
pub struct PrivateFileVisionEvidenceCredentialStore {
    root: PathBuf,
}

impl PrivateFileVisionEvidenceCredentialStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn metadata_path(&self) -> PathBuf {
        self.root.join("config.json")
    }

    fn secret_path(&self, credential_id: &str) -> PathBuf {
        self.root.join(format!("credential-{credential_id}.key"))
    }

    pub fn inspect_metadata(
        &self,
    ) -> Result<VisionEvidenceConfigMetadata, VisionEvidenceProviderError> {
        let Some(config) = self.read_config_if_present()? else {
            return Ok(VisionEvidenceConfigMetadata {
                base_url: String::new(),
                model: String::new(),
                configured: false,
                storage: "private_secret_file".into(),
                requires_os_prompt: false,
            });
        };
        Ok(VisionEvidenceConfigMetadata {
            base_url: config.base_url,
            model: config.model,
            configured: self.secret_path(&config.credential_id).exists(),
            storage: "private_secret_file".into(),
            requires_os_prompt: false,
        })
    }

    pub fn save(
        &self,
        base_url: String,
        model: String,
        api_key: String,
    ) -> Result<VisionEvidenceConfigMetadata, VisionEvidenceProviderError> {
        let base_url = validate_base_url(&base_url)?;
        validate_model(&model)?;
        let secret = VisionEvidenceSecret::new(api_key)?;
        ensure_private_directory(&self.root)?;
        let old = self.read_config_if_present()?;
        let credential_id = random_credential_id()?;
        let secret_path = self.secret_path(&credential_id);
        write_private_file(&secret_path, secret.0.as_bytes())?;
        let config = VisionEvidenceConfigFile {
            schema_version: VISION_CONFIG_SCHEMA_VERSION.into(),
            base_url: base_url.as_str().trim_end_matches('/').to_string(),
            model,
            credential_id: credential_id.clone(),
        };
        let encoded = serde_json::to_vec(&config)
            .map_err(|_| credential_error("Vision evidence metadata could not be serialized."))?;
        if let Err(error) = atomic_replace_private_file(&self.metadata_path(), &encoded) {
            let _ = fs::remove_file(&secret_path);
            return Err(error);
        }
        if let Some(old) = old.filter(|old| old.credential_id != credential_id) {
            let old_path = self.secret_path(&old.credential_id);
            if old_path.exists() {
                validate_private_file(&old_path)?;
                fs::remove_file(old_path).map_err(|_| {
                    credential_error("Previous vision credential could not be deleted.")
                })?;
            }
        }
        sync_directory(&self.root)?;
        self.inspect_metadata()
    }

    pub fn clear(&self) -> Result<VisionEvidenceConfigMetadata, VisionEvidenceProviderError> {
        if let Some(config) = self.read_config_if_present()? {
            let secret_path = self.secret_path(&config.credential_id);
            if secret_path.exists() {
                validate_private_file(&secret_path)?;
                fs::remove_file(secret_path)
                    .map_err(|_| credential_error("Vision credential could not be deleted."))?;
            }
        }
        let metadata_path = self.metadata_path();
        if metadata_path.exists() {
            validate_private_file(&metadata_path)?;
            fs::remove_file(metadata_path)
                .map_err(|_| credential_error("Vision metadata could not be deleted."))?;
        }
        if self.root.exists() {
            sync_directory(&self.root)?;
        }
        self.inspect_metadata()
    }

    fn read_config_if_present(
        &self,
    ) -> Result<Option<VisionEvidenceConfigFile>, VisionEvidenceProviderError> {
        let path = self.metadata_path();
        if !path.exists() {
            return Ok(None);
        }
        validate_private_directory(&self.root)?;
        validate_private_file(&path)?;
        let mut bytes = Vec::new();
        OpenOptions::new()
            .read(true)
            .open(path)
            .and_then(|file| file.take(64 * 1024).read_to_end(&mut bytes))
            .map_err(|_| credential_error("Vision metadata could not be read."))?;
        let config: VisionEvidenceConfigFile = serde_json::from_slice(&bytes)
            .map_err(|_| credential_error("Vision metadata is invalid."))?;
        if config.schema_version != VISION_CONFIG_SCHEMA_VERSION {
            return Err(credential_error("Vision metadata schema is invalid."));
        }
        validate_base_url(&config.base_url)?;
        validate_model(&config.model)?;
        validate_credential_id(&config.credential_id)?;
        Ok(Some(config))
    }
}

impl fmt::Debug for PrivateFileVisionEvidenceCredentialStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateFileVisionEvidenceCredentialStore([REDACTED_PATH])")
    }
}

impl VisionEvidenceCredentialSource for PrivateFileVisionEvidenceCredentialStore {
    fn load_snapshot(
        &self,
    ) -> Result<Option<VisionEvidenceCredentialSnapshot>, VisionEvidenceProviderError> {
        let Some(config) = self.read_config_if_present()? else {
            return Ok(None);
        };
        let path = self.secret_path(&config.credential_id);
        validate_private_file(&path)?;
        let metadata = fs::metadata(&path)
            .map_err(|_| credential_error("Vision credential metadata could not be read."))?;
        if metadata.len() == 0 || metadata.len() > 4_096 {
            return Err(credential_error(
                "Vision credential file is empty or outside the reviewed bound.",
            ));
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        OpenOptions::new()
            .read(true)
            .open(path)
            .and_then(|mut file| file.read_to_end(&mut bytes))
            .map_err(|_| credential_error("Vision credential could not be read."))?;
        let text = String::from_utf8(bytes)
            .map_err(|_| credential_error("Vision credential encoding is invalid."))?;
        Ok(Some(VisionEvidenceCredentialSnapshot {
            base_url: validate_base_url(&config.base_url)?,
            model: config.model,
            secret: VisionEvidenceSecret::new(text)?,
        }))
    }
}

#[derive(Clone)]
pub struct VisionEvidenceHttpRequest {
    pub endpoint: Url,
    pub authorization: VisionEvidenceSecret,
    pub body: Arc<[u8]>,
    pub remote_idempotency_key: Option<String>,
    pub timeout: Duration,
    pub max_response_bytes: usize,
}

impl fmt::Debug for VisionEvidenceHttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VisionEvidenceHttpRequest")
            .field(
                "endpoint_origin",
                &self.endpoint.origin().ascii_serialization(),
            )
            .field("endpoint_path", &self.endpoint.path())
            .field("authorization", &"[REDACTED]")
            .field("body", &"[REDACTED]")
            .field("body_bytes", &self.body.len())
            .field(
                "has_remote_idempotency_key",
                &self.remote_idempotency_key.is_some(),
            )
            .field("timeout", &self.timeout)
            .field("max_response_bytes", &self.max_response_bytes)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionEvidenceHttpResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
    pub network_call_made: bool,
}

pub trait VisionEvidenceHttpTransport: Send + Sync + 'static {
    fn execute(&self, request: VisionEvidenceHttpRequest) -> VisionEvidenceHttpFuture;
}

#[derive(Clone)]
pub struct ReqwestVisionEvidenceTransport {
    client: Client,
}

impl ReqwestVisionEvidenceTransport {
    pub fn new() -> Result<Self, VisionEvidenceProviderError> {
        let client = Client::builder()
            .https_only(true)
            .redirect(redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| transport_error("Vision HTTPS client could not be initialized."))?;
        Ok(Self { client })
    }
}

impl VisionEvidenceHttpTransport for ReqwestVisionEvidenceTransport {
    fn execute(&self, request: VisionEvidenceHttpRequest) -> VisionEvidenceHttpFuture {
        let client = self.client.clone();
        Box::pin(async move {
            if request.endpoint.scheme() != "https"
                || !request.endpoint.username().is_empty()
                || request.endpoint.password().is_some()
                || request.endpoint.fragment().is_some()
                || request.body.is_empty()
                || request.body.len() > MAX_VISION_REQUEST_BYTES
                || request
                    .remote_idempotency_key
                    .as_deref()
                    .is_some_and(|key| !valid_remote_idempotency_key(key))
                || request.timeout.is_zero()
                || request.timeout > Duration::from_secs(120)
                || request.max_response_bytes == 0
                || request.max_response_bytes > MAX_VISION_EVIDENCE_RESPONSE_BYTES
            {
                return Err(transport_error(
                    "Vision HTTPS request is outside the reviewed transport bounds.",
                ));
            }
            let bearer = request.authorization.bearer_value();
            let mut header_value = HeaderValue::from_bytes(bearer.as_bytes()).map_err(|_| {
                transport_error("Vision credential cannot form an Authorization header.")
            })?;
            header_value.set_sensitive(true);
            let mut builder = client
                .post(request.endpoint)
                .header(header::AUTHORIZATION, header_value)
                .header(header::CONTENT_TYPE, "application/json")
                .body(request.body.as_ref().to_vec())
                .timeout(request.timeout);
            if let Some(key) = request.remote_idempotency_key {
                builder = builder.header("Idempotency-Key", key);
            }
            let mut response = builder.send().await.map_err(|_| {
                VisionEvidenceProviderError::new(
                    "VISION_EVIDENCE_HTTP_FAILED",
                    "Vision evidence HTTPS request failed.",
                    true,
                    true,
                )
            })?;
            if response
                .content_length()
                .is_some_and(|length| length > request.max_response_bytes as u64)
            {
                return Err(VisionEvidenceProviderError::new(
                    "VISION_EVIDENCE_RESPONSE_TOO_LARGE",
                    "Vision evidence response exceeds the reviewed byte limit.",
                    true,
                    false,
                ));
            }
            let status = response.status().as_u16();
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .filter(|value| value.len() <= 256)
                .map(str::to_owned);
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| {
                VisionEvidenceProviderError::new(
                    "VISION_EVIDENCE_HTTP_FAILED",
                    "Vision evidence response stream failed.",
                    true,
                    true,
                )
            })? {
                if body
                    .len()
                    .checked_add(chunk.len())
                    .is_none_or(|length| length > request.max_response_bytes)
                {
                    return Err(VisionEvidenceProviderError::new(
                        "VISION_EVIDENCE_RESPONSE_TOO_LARGE",
                        "Vision evidence response exceeds the reviewed byte limit.",
                        true,
                        false,
                    ));
                }
                body.extend_from_slice(&chunk);
            }
            Ok(VisionEvidenceHttpResponse {
                status,
                content_type,
                body,
                network_call_made: true,
            })
        })
    }
}

#[derive(Clone)]
pub struct OpenAiCompatibleVisionEvidenceAdapter {
    credentials: Arc<dyn VisionEvidenceCredentialSource>,
    transport: Arc<dyn VisionEvidenceHttpTransport>,
    timeout: Duration,
    formal_pricing: Option<VisionEvidenceFormalPricing>,
}

impl OpenAiCompatibleVisionEvidenceAdapter {
    pub fn new(
        credentials: Arc<dyn VisionEvidenceCredentialSource>,
        transport: Arc<dyn VisionEvidenceHttpTransport>,
        timeout: Duration,
    ) -> Result<Self, VisionEvidenceProviderError> {
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(error(
                "VISION_EVIDENCE_TIMEOUT_INVALID",
                "Vision adapter timeout must be between one millisecond and two minutes.",
                false,
                false,
            ));
        }
        Ok(Self {
            credentials,
            transport,
            timeout,
            formal_pricing: None,
        })
    }

    pub fn with_formal_pricing(mut self, pricing: VisionEvidenceFormalPricing) -> Self {
        self.formal_pricing = Some(pricing);
        self
    }

    async fn analyze_inner(
        &self,
        request: VisionEvidenceProviderRequest,
        cancellation: CancellationToken,
    ) -> Result<VisionEvidenceProviderOutput, VisionEvidenceProviderError> {
        if cancellation.is_cancelled() {
            return Err(error(
                "VISION_EVIDENCE_CANCELLED",
                "Vision analysis was cancelled before credential access.",
                false,
                false,
            ));
        }
        let snapshot = self.credentials.load_snapshot()?.ok_or_else(|| {
            error(
                "VISION_EVIDENCE_NOT_CONFIGURED",
                "Configure a vision evidence Provider before analyzing images.",
                false,
                false,
            )
        })?;
        let endpoint = chat_completions_endpoint(&snapshot.base_url)?;
        let body = build_openai_compatible_body(&snapshot.model, &request)?;
        if cancellation.is_cancelled() {
            return Err(error(
                "VISION_EVIDENCE_CANCELLED",
                "Vision analysis was cancelled before network execution.",
                false,
                false,
            ));
        }
        let response = self
            .transport
            .execute(VisionEvidenceHttpRequest {
                endpoint,
                authorization: snapshot.secret,
                body: Arc::from(body),
                remote_idempotency_key: None,
                timeout: self.timeout,
                max_response_bytes: MAX_VISION_EVIDENCE_RESPONSE_BYTES,
            })
            .await?;
        if cancellation.is_cancelled() {
            return Err(error(
                "VISION_EVIDENCE_CANCELLED",
                "Late vision analysis output was discarded after cancellation.",
                response.network_call_made,
                false,
            ));
        }
        parse_openai_compatible_response(&snapshot.model, response)
    }
}

impl VisionEvidenceProviderPort for OpenAiCompatibleVisionEvidenceAdapter {
    fn analyze(
        &self,
        request: VisionEvidenceProviderRequest,
        cancellation: CancellationToken,
    ) -> VisionEvidenceProviderFuture {
        let adapter = self.clone();
        Box::pin(async move { adapter.analyze_inner(request, cancellation).await })
    }
}

impl VisualReferenceComparisonProviderPort for OpenAiCompatibleVisionEvidenceAdapter {
    fn compare(
        &self,
        request: VisualReferenceComparisonProviderRequest,
        cancellation: CancellationToken,
    ) -> VisualReferenceComparisonProviderFuture {
        let adapter = self.clone();
        Box::pin(async move { adapter.compare_inner(request, cancellation).await })
    }
}

impl E005PreparedVisualReviewProviderPort for OpenAiCompatibleVisionEvidenceAdapter {
    fn prepare_e005_visual_review(
        &self,
        request: VisualReferenceComparisonProviderRequest,
    ) -> Result<PreparedE005VisualReviewProviderRequest, VisualReferenceComparisonProviderError>
    {
        if request.e005_source.is_none() {
            return Err(comparison_protocol_error(
                "Formal E005 visual preparation requires the exact unified source.",
                false,
            ));
        }
        let source = request.e005_source.as_ref().expect("checked");
        let lowering =
            forgecad_core::lower_forge_visual_author_source_v1(source).map_err(|_| {
                comparison_protocol_error("Formal E005 visual source failed local lowering.", false)
            })?;
        if lowering.source_program_sha256 != request.input.source_program_sha256
            || request.input.candidate_view_profile
                != Some(forgecad_core::VisualReferenceCandidateViewProfile::TurntableEight)
        {
            return Err(comparison_protocol_error(
                "Formal E005 visual source or candidate profile has stale lineage.",
                false,
            ));
        }
        let pricing = self.formal_pricing.ok_or_else(|| {
            comparison_error(
                "E005_R2_FORMAL_VISION_PRICING_REQUIRED",
                "Formal visual preparation is disabled until reviewed pricing is configured.",
                false,
                false,
            )
        })?;
        let snapshot = self
            .credentials
            .load_snapshot()
            .map_err(map_vision_comparison_error)?
            .ok_or_else(|| {
                comparison_error(
                    "VISUAL_REFERENCE_COMPARISON_NOT_CONFIGURED",
                    "Configure a vision evidence Provider before preparing formal review.",
                    false,
                    false,
                )
            })?;
        let endpoint =
            chat_completions_endpoint(&snapshot.base_url).map_err(map_vision_comparison_error)?;
        let body = build_reference_comparison_body(&snapshot.model, &request)?;
        let input_tokens_upper_bound = u64::try_from(body.len())
            .ok()
            .and_then(|bytes| bytes.checked_add(FORMAL_VISION_INPUT_FRAMING_OVERHEAD_BYTES))
            .ok_or_else(|| {
                comparison_protocol_error(
                    "Formal visual request exceeded the accounting bound.",
                    false,
                )
            })?;
        let budget_policy = ProviderRequestBudgetPolicy {
            input_tokens_upper_bound,
            input_cost_ceiling_microusd: cost_for_tokens(
                input_tokens_upper_bound,
                pricing.input_microusd_per_million_tokens,
            ),
            output_microusd_per_million_tokens: pricing.output_microusd_per_million_tokens,
        }
        .validate()
        .map_err(|_| comparison_protocol_error("Formal visual budget policy is invalid.", false))?;
        let provider_id = "openai_compatible_vision";
        let model = snapshot.model.clone();
        let comparison_input_sha256 =
            forgecad_core::semantic_sha256(&request.input).map_err(|_| {
                comparison_protocol_error("Comparison input could not be hashed.", false)
            })?;
        let commitment = ProviderRequestCommitment {
            request_sha256: sha256_hex(&body),
            pricing_snapshot_sha256: pricing.snapshot_sha256(provider_id, &model),
            budget_policy,
        };
        let transport = Arc::clone(&self.transport);
        let timeout = self.timeout;
        PreparedE005VisualReviewProviderRequest::new(
            provider_id.into(),
            model.clone(),
            comparison_input_sha256,
            E005_VISUAL_REVIEW_MAX_OUTPUT_TOKENS,
            commitment,
            move |remote_idempotency_key, cancellation| {
                Box::pin(async move {
                    if cancellation.is_cancelled() {
                        return Err(comparison_error(
                            "VISUAL_REFERENCE_COMPARISON_CANCELLED",
                            "Formal visual comparison was cancelled before network execution.",
                            false,
                            false,
                        ));
                    }
                    let response = transport
                        .execute(VisionEvidenceHttpRequest {
                            endpoint,
                            authorization: snapshot.secret,
                            body: Arc::from(body),
                            remote_idempotency_key: Some(remote_idempotency_key),
                            timeout,
                            max_response_bytes: MAX_VISION_EVIDENCE_RESPONSE_BYTES,
                        })
                        .await
                        .map_err(map_vision_comparison_error)?;
                    if cancellation.is_cancelled() {
                        return Err(comparison_error(
                            "VISUAL_REFERENCE_COMPARISON_CANCELLED",
                            "Late formal visual comparison output was discarded after cancellation.",
                            response.network_call_made,
                            false,
                        ));
                    }
                    let usage = parse_formal_provider_usage(&response, pricing)?;
                    let output = parse_reference_comparison_response(&model, response, true)?;
                    Ok(E005PreparedVisualReviewProviderResponse { output, usage })
                })
            },
        )
    }
}

impl OpenAiCompatibleVisionEvidenceAdapter {
    async fn compare_inner(
        &self,
        request: VisualReferenceComparisonProviderRequest,
        cancellation: CancellationToken,
    ) -> Result<VisualReferenceComparisonProviderOutput, VisualReferenceComparisonProviderError>
    {
        if cancellation.is_cancelled() {
            return Err(comparison_error(
                "VISUAL_REFERENCE_COMPARISON_CANCELLED",
                "Reference comparison was cancelled before credential access.",
                false,
                false,
            ));
        }
        let snapshot = self
            .credentials
            .load_snapshot()
            .map_err(map_vision_comparison_error)?
            .ok_or_else(|| {
                comparison_error(
                    "VISUAL_REFERENCE_COMPARISON_NOT_CONFIGURED",
                    "Configure a vision evidence Provider before comparing a candidate.",
                    false,
                    false,
                )
            })?;
        let endpoint =
            chat_completions_endpoint(&snapshot.base_url).map_err(map_vision_comparison_error)?;
        let body = build_reference_comparison_body(&snapshot.model, &request)?;
        if cancellation.is_cancelled() {
            return Err(comparison_error(
                "VISUAL_REFERENCE_COMPARISON_CANCELLED",
                "Reference comparison was cancelled before network execution.",
                false,
                false,
            ));
        }
        let response = self
            .transport
            .execute(VisionEvidenceHttpRequest {
                endpoint,
                authorization: snapshot.secret,
                body: Arc::from(body),
                remote_idempotency_key: None,
                timeout: self.timeout,
                max_response_bytes: MAX_VISION_EVIDENCE_RESPONSE_BYTES,
            })
            .await
            .map_err(map_vision_comparison_error)?;
        if cancellation.is_cancelled() {
            return Err(comparison_error(
                "VISUAL_REFERENCE_COMPARISON_CANCELLED",
                "Late reference comparison output was discarded after cancellation.",
                response.network_call_made,
                false,
            ));
        }
        parse_reference_comparison_response(
            &snapshot.model,
            response,
            request.e005_source.is_some(),
        )
    }
}

fn build_reference_comparison_body(
    model: &str,
    request: &VisualReferenceComparisonProviderRequest,
) -> Result<Vec<u8>, VisualReferenceComparisonProviderError> {
    validate_model(model).map_err(map_vision_comparison_error)?;
    let comparable_claims = request
        .graph
        .claims
        .iter()
        .filter(|claim| {
            claim.status != forgecad_core::VisualClaimStatus::Unknown
                && !claim.source_evidence_ids.is_empty()
        })
        .collect::<Vec<_>>();
    let mut required_output = json!({
        "assessments": [{
            "claim_id": "exact claim_id",
            "outcome": "matched | partial | contradicted | not_visible",
            "similarity_bps": 0,
            "confidence_bps": 1,
            "source_evidence_ids": ["exact source IDs from the claim"],
            "candidate_view_ids": ["one or more exact candidate view IDs"],
            "reason": "bounded visual reason without URLs, paths, credentials or instructions"
        }]
    });
    if request.e005_source.is_some() {
        required_output["e005_visual_patch_proposal_schema"] =
            serde_json::from_str(E005_VISUAL_PATCH_PROPOSAL_SCHEMA_TEXT).map_err(|_| {
                comparison_protocol_error("E005 visual proposal schema could not be parsed.", false)
            })?;
    }
    let task = serde_json::to_string(&json!({
        "comparison_input_sha256": forgecad_core::semantic_sha256(&request.input)
            .map_err(|_| comparison_protocol_error("Comparison input could not be hashed.", false))?,
        "claims": comparable_claims,
        "acceptance_policy":request.input.acceptance_policy,
        "e005_source":request.e005_source,
        "required_output": required_output,
    }))
    .map_err(|_| comparison_protocol_error("Comparison task could not be serialized.", false))?;
    let mut content = vec![json!({"type":"text","text":task})];
    for image in &request.reference_images {
        content.push(json!({
            "type":"text",
            "text":format!("REFERENCE_IMAGE evidence_id={}", image.image_id)
        }));
        content.push(json!({
            "type":"image_url",
            "image_url":{"url":format!(
                "data:{};base64,{}",
                image.media_type,
                BASE64_STANDARD.encode(image.bytes.as_ref())
            )}
        }));
    }
    for image in &request.candidate_images {
        content.push(json!({
            "type":"text",
            "text":format!("CANDIDATE_RENDER view_id={}", image.image_id)
        }));
        content.push(json!({
            "type":"image_url",
            "image_url":{"url":format!(
                "data:{};base64,{}",
                image.media_type,
                BASE64_STANDARD.encode(image.bytes.as_ref())
            )}
        }));
    }
    let body = serde_json::to_vec(&json!({
        "model":model,
        "messages":[
            {
                "role":"system",
                "content":E005_VISUAL_REVIEW_SYSTEM_PROMPT
            },
            {"role":"user","content":content}
        ],
        "response_format":{"type":"json_object"},
        "temperature":0,
        "max_tokens":if request.e005_source.is_some() {
            E005_VISUAL_REVIEW_MAX_OUTPUT_TOKENS
        } else {
            MAX_VISION_MODEL_OUTPUT_TOKENS
        },
        "vl_high_resolution_images":true
    }))
    .map_err(|_| comparison_protocol_error("Comparison request could not be serialized.", false))?;
    if body.is_empty() || body.len() > MAX_VISION_REQUEST_BYTES {
        return Err(comparison_protocol_error(
            "Comparison request exceeds the reviewed byte limit.",
            false,
        ));
    }
    Ok(body)
}

fn parse_reference_comparison_response(
    model: &str,
    response: VisionEvidenceHttpResponse,
    require_e005_proposal: bool,
) -> Result<VisualReferenceComparisonProviderOutput, VisualReferenceComparisonProviderError> {
    if !(200..300).contains(&response.status) {
        return Err(comparison_error(
            match response.status {
                401 | 403 => "VISUAL_REFERENCE_COMPARISON_AUTH_FAILED",
                408 | 429 | 500..=599 => "VISUAL_REFERENCE_COMPARISON_HTTP_RETRYABLE",
                _ => "VISUAL_REFERENCE_COMPARISON_HTTP_FAILED",
            },
            "Vision Provider returned an unsuccessful comparison HTTP status.",
            response.network_call_made,
            matches!(response.status, 408 | 429 | 500..=599),
        ));
    }
    if !response
        .content_type
        .as_deref()
        .is_some_and(|value| value.starts_with("application/json"))
        || response.body.is_empty()
        || response.body.len() > MAX_VISION_EVIDENCE_RESPONSE_BYTES
    {
        return Err(comparison_protocol_error(
            "Reference comparison response must be bounded JSON.",
            response.network_call_made,
        ));
    }
    let response_sha256 = format!("{:x}", Sha256::digest(&response.body));
    let envelope: Value = serde_json::from_slice(&response.body).map_err(|_| {
        comparison_protocol_error(
            "Reference comparison response envelope is invalid JSON.",
            response.network_call_made,
        )
    })?;
    let content = envelope
        .get("choices")
        .and_then(Value::as_array)
        .filter(|choices| !choices.is_empty() && choices.len() <= 8)
        .and_then(|choices| choices[0].pointer("/message/content"))
        .and_then(Value::as_str)
        .filter(|content| {
            !content.is_empty() && content.len() <= MAX_VISION_EVIDENCE_RESPONSE_BYTES
        })
        .ok_or_else(|| {
            comparison_protocol_error(
                "Reference comparison response has no bounded assistant JSON content.",
                response.network_call_made,
            )
        })?;
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct AssessmentPayload {
        assessments: Vec<VisualReferenceClaimAssessment>,
        #[serde(default)]
        e005_visual_patch_proposal: Option<Value>,
    }
    let mut payload: AssessmentPayload = serde_json::from_str(content).map_err(|_| {
        comparison_protocol_error(
            "Reference comparison assistant content does not match the strict assessment contract.",
            response.network_call_made,
        )
    })?;
    normalize_reference_comparison_assessments(&mut payload.assessments);
    if require_e005_proposal != payload.e005_visual_patch_proposal.is_some() {
        return Err(comparison_protocol_error(
            "Reference comparison response does not match the requested E005 decision mode.",
            response.network_call_made,
        ));
    }
    Ok(VisualReferenceComparisonProviderOutput {
        provider_id: "openai_compatible_vision".into(),
        model_id: model.into(),
        provider_response_sha256: response_sha256,
        analyzed_at: format!(
            "unix:{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ),
        assessments: payload.assessments,
        network_call_made: response.network_call_made,
        budget_evidence: None,
        e005_visual_patch_proposal: payload.e005_visual_patch_proposal,
    })
}

fn parse_formal_provider_usage(
    response: &VisionEvidenceHttpResponse,
    pricing: VisionEvidenceFormalPricing,
) -> Result<ProviderUsage, VisualReferenceComparisonProviderError> {
    let envelope: Value = serde_json::from_slice(&response.body).map_err(|_| {
        comparison_protocol_error(
            "Formal visual response envelope is invalid JSON.",
            response.network_call_made,
        )
    })?;
    let usage = envelope.get("usage").ok_or_else(|| {
        comparison_protocol_error(
            "Formal visual response is missing Provider usage.",
            response.network_call_made,
        )
    })?;
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            comparison_protocol_error(
                "Formal visual prompt usage is invalid.",
                response.network_call_made,
            )
        })?;
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            comparison_protocol_error(
                "Formal visual completion usage is invalid.",
                response.network_call_made,
            )
        })?;
    if input_tokens == 0 || output_tokens == 0 {
        return Err(comparison_protocol_error(
            "Formal visual Provider usage must be non-zero.",
            response.network_call_made,
        ));
    }
    let prompt_cache_hit_tokens = usage
        .pointer("/prompt_tokens_details/cached_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if prompt_cache_hit_tokens > input_tokens {
        return Err(comparison_protocol_error(
            "Formal visual cache usage exceeds prompt usage.",
            response.network_call_made,
        ));
    }
    let estimated_cost_microusd =
        cost_for_tokens(input_tokens, pricing.input_microusd_per_million_tokens)
            .checked_add(cost_for_tokens(
                output_tokens,
                pricing.output_microusd_per_million_tokens,
            ))
            .ok_or_else(|| {
                comparison_protocol_error(
                    "Formal visual Provider cost overflowed.",
                    response.network_call_made,
                )
            })?;
    Ok(ProviderUsage {
        input_tokens,
        output_tokens,
        prompt_cache_hit_tokens,
        prompt_cache_miss_tokens: 0,
        estimated_cost_microusd,
    })
}

fn cost_for_tokens(tokens: u64, microusd_per_million_tokens: u64) -> u64 {
    let numerator = u128::from(tokens).saturating_mul(u128::from(microusd_per_million_tokens));
    let rounded = numerator.saturating_add(999_999) / 1_000_000;
    u64::try_from(rounded).unwrap_or(u64::MAX)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn valid_remote_idempotency_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

/// The vision Provider emits both a numeric similarity and a categorical
/// outcome. They encode the same observation, so accepting disagreements as
/// two independent truths makes otherwise valid comparisons nondeterministic.
///
/// Keep the bounded numeric score as the Provider observation and derive the
/// redundant category before the Rust-owned report derives pass/fail, repair
/// targets and quality scores. This does not raise a score or relax any Gate.
fn normalize_reference_comparison_assessments(assessments: &mut [VisualReferenceClaimAssessment]) {
    for assessment in assessments {
        assessment.outcome = match assessment.similarity_bps {
            7_000..=u16::MAX => VisualReferenceMatchOutcome::Matched,
            3_000..=6_999 => VisualReferenceMatchOutcome::Partial,
            1..=2_999 => VisualReferenceMatchOutcome::Contradicted,
            0 => VisualReferenceMatchOutcome::NotVisible,
        };
        assessment.reason = bounded_safe_comparison_reason(&assessment.reason);
    }
}

/// Provider prose is explanatory evidence rather than product-state truth. It
/// must never make an otherwise valid numeric assessment fail merely because a
/// multilingual model counted characters while the Rust contract counts UTF-8
/// bytes. Unsafe markers are replaced instead of persisted; ordinary text is
/// truncated only at a character boundary to the reviewed 320-byte limit.
fn bounded_safe_comparison_reason(reason: &str) -> String {
    const MAX_BYTES: usize = 320;
    const FALLBACK: &str = "The configured Provider returned a visual similarity assessment.";
    let trimmed = reason.trim();
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.is_empty()
        || trimmed.contains('\0')
        || trimmed.contains("://")
        || lower.contains("data:image/")
        || lower.contains("bearer ")
        || lower.contains("api_key")
        || lower.contains("/users/")
        || lower.contains("../")
        || lower.contains("sk-")
    {
        return FALLBACK.into();
    }
    if trimmed.len() <= MAX_BYTES {
        return trimmed.into();
    }
    let mut end = MAX_BYTES;
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    trimmed[..end].trim_end().into()
}

fn map_vision_comparison_error(
    error: VisionEvidenceProviderError,
) -> VisualReferenceComparisonProviderError {
    comparison_error(
        "VISUAL_REFERENCE_COMPARISON_PROVIDER_FAILED",
        error.message,
        error.network_call_made,
        error.retryable,
    )
}

fn comparison_protocol_error(
    message: &'static str,
    network_call_made: bool,
) -> VisualReferenceComparisonProviderError {
    comparison_error(
        "VISUAL_REFERENCE_COMPARISON_PROTOCOL_INVALID",
        message,
        network_call_made,
        false,
    )
}

fn comparison_error(
    code: &'static str,
    message: impl Into<String>,
    network_call_made: bool,
    retryable: bool,
) -> VisualReferenceComparisonProviderError {
    VisualReferenceComparisonProviderError::new(code, message, network_call_made, retryable)
}

fn build_openai_compatible_body(
    model: &str,
    request: &VisionEvidenceProviderRequest,
) -> Result<Vec<u8>, VisionEvidenceProviderError> {
    validate_model(model)?;
    let reference_summary = request
        .request
        .reference_inputs
        .iter()
        .map(|reference| {
            json!({
                "evidence_id": reference.evidence_id,
                "role": reference.role,
                "view_id": reference.view_id,
                "region": reference.region,
            })
        })
        .collect::<Vec<_>>();
    let text = serde_json::to_string(&json!({
        "instruction": request.request.instruction,
        "domain_pack_id": request.request.domain_pack_id,
        "references": reference_summary,
        "required_output": {
            "status_rules": {
                "observed": "confidence_bps must be 1..10000 and source_evidence_ids must contain the visible sealed reference",
                "inferred": "confidence_bps must be 1..10000; use only for a positive visual inference supported by the supplied reference",
                "unknown": "confidence_bps must be 0, source_evidence_ids must be [], source_view_id must be null, and source_region must be null"
            },
            "claims": [{
                "claim_id": "vclaim_stable_identifier",
                "level": "macro | meso | micro",
                "status": "observed",
                "target": "geometry | assembly | material | surface | style | evaluation_only",
                "description": "bounded visual claim without URLs, paths or credentials",
                "critical": true,
                "confidence_bps": 8500,
                "source_evidence_ids": ["refevid_identifier"],
                "source_view_id": null,
                "source_region": null
            }, {
                "claim_id": "vclaim_unknown_example",
                "level": "micro",
                "status": "unknown",
                "target": "surface",
                "description": "a hidden or unsupported visual property",
                "critical": false,
                "confidence_bps": 0,
                "source_evidence_ids": [],
                "source_view_id": null,
                "source_region": null
            }]
        }
    }))
    .map_err(|_| protocol_error("Vision request summary could not be serialized.", false))?;
    let mut content = vec![json!({"type":"text","text":text})];
    for image in &request.images {
        let encoded = BASE64_STANDARD.encode(image.bytes.as_ref());
        content.push(json!({
            "type":"image_url",
            "image_url":{"url":format!("data:{};base64,{}", image.media_type, encoded)}
        }));
    }
    let body = serde_json::to_vec(&json!({
        "model": model,
        "messages": [
            {
                "role":"system",
                "content":"Analyze only visible design evidence. Return JSON only: one object with exactly one top-level field, claims. claims must be an array, and every item must use only the requested claim fields and enum values. Do not add analysis, summary, markdown fences, or extra fields. Never output URLs, paths, credentials, executable code, dimensions, hidden structure, manufacturing or functional weapon guidance. Observed and inferred claims require confidence_bps from 1 to 10000. A claim with zero confidence must be unknown, with empty source_evidence_ids and null source_view_id/source_region. Use inferred only for a positive visual inference; otherwise use unknown. Cover macro, meso and micro evidence."
            },
            {"role":"user","content":content}
        ],
        "response_format":{"type":"json_object"},
        "temperature":0,
        "max_tokens":MAX_VISION_MODEL_OUTPUT_TOKENS,
        "vl_high_resolution_images":true
    }))
    .map_err(|_| protocol_error("Vision request body could not be serialized.", false))?;
    if body.is_empty() || body.len() > MAX_VISION_REQUEST_BYTES {
        return Err(protocol_error(
            "Vision request body exceeds the reviewed byte limit.",
            false,
        ));
    }
    Ok(body)
}

fn parse_openai_compatible_response(
    model: &str,
    response: VisionEvidenceHttpResponse,
) -> Result<VisionEvidenceProviderOutput, VisionEvidenceProviderError> {
    if !(200..300).contains(&response.status) {
        return Err(VisionEvidenceProviderError::new(
            match response.status {
                401 | 403 => "VISION_EVIDENCE_AUTH_FAILED",
                408 | 429 | 500..=599 => "VISION_EVIDENCE_HTTP_RETRYABLE",
                _ => "VISION_EVIDENCE_HTTP_FAILED",
            },
            "Vision evidence Provider returned an unsuccessful HTTP status.",
            response.network_call_made,
            matches!(response.status, 408 | 429 | 500..=599),
        ));
    }
    if !response
        .content_type
        .as_deref()
        .is_some_and(|content_type| content_type.starts_with("application/json"))
        || response.body.is_empty()
        || response.body.len() > MAX_VISION_EVIDENCE_RESPONSE_BYTES
    {
        return Err(protocol_error(
            "Vision evidence response must be bounded JSON.",
            response.network_call_made,
        ));
    }
    let response_sha256 = format!("{:x}", Sha256::digest(&response.body));
    let envelope: Value = serde_json::from_slice(&response.body).map_err(|_| {
        protocol_error(
            "Vision evidence response envelope is invalid JSON.",
            response.network_call_made,
        )
    })?;
    let choices = envelope
        .get("choices")
        .and_then(Value::as_array)
        .filter(|choices| !choices.is_empty() && choices.len() <= 8)
        .ok_or_else(|| {
            protocol_error(
                "Vision evidence response has no bounded choices array.",
                response.network_call_made,
            )
        })?;
    let content = choices[0]
        .pointer("/message/content")
        .and_then(Value::as_str)
        .filter(|content| {
            !content.is_empty() && content.len() <= MAX_VISION_EVIDENCE_RESPONSE_BYTES
        })
        .ok_or_else(|| {
            protocol_error(
                "Vision evidence response has no bounded assistant JSON content.",
                response.network_call_made,
            )
        })?;
    let claims = parse_strict_claims_content(content, response.network_call_made)?;
    Ok(VisionEvidenceProviderOutput {
        provider_id: "openai_compatible_vision".into(),
        model_id: model.to_string(),
        provider_response_sha256: response_sha256,
        analyzed_at: format!(
            "unix:{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ),
        claims,
        network_call_made: response.network_call_made,
    })
}

/// Decode only the exact evidence envelope while returning a bounded, content-free
/// failure category. Provider text is intentionally neither persisted nor copied
/// into diagnostics, but a caller still needs to know whether to correct the
/// envelope, the claims array, or one individual claim.
fn parse_strict_claims_content(
    content: &str,
    network_call_made: bool,
) -> Result<Vec<VisualEvidenceClaim>, VisionEvidenceProviderError> {
    let value: Value = serde_json::from_str(content).map_err(|_| {
        protocol_error(
            "Vision evidence assistant content is not a JSON object.",
            network_call_made,
        )
    })?;
    let object = value.as_object().ok_or_else(|| {
        protocol_error(
            "Vision evidence assistant content must be a JSON object with exactly one claims field.",
            network_call_made,
        )
    })?;
    if object.len() != 1 || !object.contains_key("claims") {
        return Err(protocol_error(
            "Vision evidence assistant content must contain exactly the claims field.",
            network_call_made,
        ));
    }
    let claims = object
        .get("claims")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            protocol_error(
                "Vision evidence assistant claims field must be an array.",
                network_call_made,
            )
        })?;
    claims
        .iter()
        .enumerate()
        .map(|(index, claim)| {
            serde_json::from_value::<VisualEvidenceClaim>(claim.clone()).map_err(|_| {
                protocol_error(
                    format!(
                        "Vision evidence assistant claim at index {index} does not match the strict claim contract."
                    ),
                    network_call_made,
                )
            })
        })
        .collect()
}

fn validate_base_url(value: &str) -> Result<Url, VisionEvidenceProviderError> {
    if value.len() > 2_048 {
        return Err(credential_error(
            "Vision base URL is outside the reviewed bound.",
        ));
    }
    let url = Url::parse(value).map_err(|_| credential_error("Vision base URL is invalid."))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(credential_error(
            "Vision base URL must be an HTTPS origin/path without credentials, query or fragment.",
        ));
    }
    if url.host_str().is_none_or(|host| {
        let host = host.to_ascii_lowercase();
        host != "aliyuncs.com" && !host.ends_with(ALLOWED_VISION_HOST_SUFFIX)
    }) {
        return Err(credential_error(
            "Vision evidence is restricted to Qwen on an official aliyuncs.com HTTPS endpoint.",
        ));
    }
    Ok(url)
}

fn chat_completions_endpoint(base_url: &Url) -> Result<Url, VisionEvidenceProviderError> {
    let mut value = base_url.as_str().trim_end_matches('/').to_string();
    value.push_str("/chat/completions");
    let endpoint = validate_base_url(&value)?;
    Ok(endpoint)
}

fn validate_model(value: &str) -> Result<(), VisionEvidenceProviderError> {
    if value.is_empty()
        || value.len() > 160
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'/')
        })
    {
        return Err(credential_error(
            "Vision model identifier is outside the reviewed contract.",
        ));
    }
    if !value
        .to_ascii_lowercase()
        .starts_with(ALLOWED_VISION_MODEL_PREFIX)
    {
        return Err(credential_error(
            "Vision evidence is restricted to the qwen model family.",
        ));
    }
    Ok(())
}

fn validate_credential_id(value: &str) -> Result<(), VisionEvidenceProviderError> {
    if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(credential_error("Vision credential generation is invalid."));
    }
    Ok(())
}

fn random_credential_id() -> Result<String, VisionEvidenceProviderError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|_| credential_error("Vision credential generation could not be created."))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn ensure_private_directory(path: &Path) -> Result<(), VisionEvidenceProviderError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        if !path.exists() {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true).mode(0o700);
            builder
                .create(path)
                .map_err(|_| credential_error("Vision secret directory could not be created."))?;
        }
        validate_private_directory(path)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(credential_error(
            "Private vision credential files are not implemented on this platform.",
        ))
    }
}

fn validate_private_directory(path: &Path) -> Result<(), VisionEvidenceProviderError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::symlink_metadata(path)
            .map_err(|_| credential_error("Vision secret directory could not be inspected."))?;
        if !metadata.file_type().is_dir()
            || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(credential_error(
                "Vision secret directory must be a private regular directory.",
            ));
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(credential_error(
            "Private vision credential files are not implemented on this platform.",
        ))
    }
}

fn validate_private_file(path: &Path) -> Result<(), VisionEvidenceProviderError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::symlink_metadata(path)
            .map_err(|_| credential_error("Vision credential file could not be inspected."))?;
        if !metadata.file_type().is_file()
            || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(credential_error(
                "Vision credential must be a private regular file.",
            ));
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(credential_error(
            "Private vision credential files are not implemented on this platform.",
        ))
    }
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), VisionEvidenceProviderError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| credential_error("Vision credential file could not be created."))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| credential_error("Vision credential file could not be written."))?;
    validate_private_file(path)
}

fn atomic_replace_private_file(
    path: &Path,
    bytes: &[u8],
) -> Result<(), VisionEvidenceProviderError> {
    let parent = path
        .parent()
        .ok_or_else(|| credential_error("Vision metadata path has no private parent."))?;
    ensure_private_directory(parent)?;
    let serial = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp = parent.join(format!(
        ".vision-config-{}-{serial}.tmp",
        std::process::id()
    ));
    let result = (|| {
        write_private_file(&temp, bytes)?;
        fs::rename(&temp, path)
            .map_err(|_| credential_error("Vision metadata could not be atomically published."))?;
        validate_private_file(path)?;
        sync_directory(parent)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn sync_directory(path: &Path) -> Result<(), VisionEvidenceProviderError> {
    OpenOptions::new()
        .read(true)
        .open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| credential_error("Vision secret directory could not be synced."))
}

fn credential_error(message: &'static str) -> VisionEvidenceProviderError {
    error(
        "VISION_EVIDENCE_CREDENTIAL_STORE_INVALID",
        message,
        false,
        false,
    )
}

fn transport_error(message: &'static str) -> VisionEvidenceProviderError {
    error(
        "VISION_EVIDENCE_HTTP_TRANSPORT_INVALID",
        message,
        false,
        false,
    )
}

fn protocol_error(
    message: impl Into<String>,
    network_call_made: bool,
) -> VisionEvidenceProviderError {
    error(
        "VISION_EVIDENCE_PROTOCOL_INVALID",
        message,
        network_call_made,
        false,
    )
}

fn error(
    code: &'static str,
    message: impl Into<String>,
    network_call_made: bool,
    retryable: bool,
) -> VisionEvidenceProviderError {
    VisionEvidenceProviderError::new(code, message, network_call_made, retryable)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use forgecad_app_server::{
        VisionEvidenceImage, VisionEvidenceProviderRequest, VisualReferenceComparisonImage,
    };
    use forgecad_core::{
        semantic_sha256, MultimodalDesignLocks, MultimodalDesignRequest, MultimodalReferenceInput,
        ReferenceClass, ReferenceEvidence, ReferenceEvidenceKind, ReferenceEvidenceObservations,
        ReferenceImageBrightnessBucket, ReferenceImageColorBucket, ReferenceImageEdgeDensityBucket,
        ReferenceImageForegroundConfidence, ReferenceImageSurfaceFacts, ReferenceRole,
        VisionEvidenceProviderProvenance, VisualClaimStatus, VisualClaimTarget, VisualDetailLevel,
        VisualEvidenceClaim, VisualEvidenceGraph, VisualFixedViewEvidence,
        VisualReferenceAcceptancePolicy, VisualReferenceCandidateViewProfile,
        VisualReferenceComparisonInput, VisualReferenceSourceFingerprint,
        MULTIMODAL_DESIGN_REQUEST_SCHEMA_VERSION, REQUIRED_VISUAL_VIEW_IDS,
        VISUAL_EVIDENCE_GRAPH_SCHEMA_VERSION, VISUAL_REFERENCE_COMPARISON_INPUT_SCHEMA_VERSION,
    };
    use tempfile::TempDir;

    use super::*;

    #[derive(Clone)]
    struct FixedCredentialSource {
        snapshot: VisionEvidenceCredentialSnapshot,
        reads: Arc<Mutex<usize>>,
    }

    impl VisionEvidenceCredentialSource for FixedCredentialSource {
        fn load_snapshot(
            &self,
        ) -> Result<Option<VisionEvidenceCredentialSnapshot>, VisionEvidenceProviderError> {
            *self.reads.lock().unwrap() += 1;
            Ok(Some(self.snapshot.clone()))
        }
    }

    #[derive(Clone)]
    struct ScriptedTransport {
        requests: Arc<Mutex<Vec<VisionEvidenceHttpRequest>>>,
        responses: Arc<Mutex<VecDeque<VisionEvidenceHttpResponse>>>,
    }

    impl VisionEvidenceHttpTransport for ScriptedTransport {
        fn execute(&self, request: VisionEvidenceHttpRequest) -> VisionEvidenceHttpFuture {
            self.requests.lock().unwrap().push(request);
            let response = self.responses.lock().unwrap().pop_front().unwrap();
            Box::pin(async move { Ok(response) })
        }
    }

    fn provider_response() -> VisionEvidenceHttpResponse {
        let content = json!({
            "claims":[
                {"claim_id":"vclaim_http_macro","level":"macro","status":"observed","target":"geometry","description":"Tall articulated silhouette","critical":true,"confidence_bps":9000,"source_evidence_ids":["refevid_http_front"],"source_view_id":"front","source_region":null},
                {"claim_id":"vclaim_http_meso","level":"meso","status":"observed","target":"material","description":"Blue armor panels over a dark frame","critical":true,"confidence_bps":8500,"source_evidence_ids":["refevid_http_front"],"source_view_id":"front","source_region":null},
                {"claim_id":"vclaim_http_micro","level":"micro","status":"unknown","target":"surface","description":"Back micro surface is not visible","critical":false,"confidence_bps":0,"source_evidence_ids":[],"source_view_id":null,"source_region":null}
            ]
        })
        .to_string();
        VisionEvidenceHttpResponse {
            status: 200,
            content_type: Some("application/json".into()),
            body: serde_json::to_vec(&json!({
                "choices":[{"message":{"role":"assistant","content":content}}]
            }))
            .unwrap(),
            network_call_made: true,
        }
    }

    fn comparison_provider_response() -> VisionEvidenceHttpResponse {
        let content = json!({
            "assessments":[
                {"claim_id":"vclaim_http_macro","outcome":"matched","similarity_bps":8600,"confidence_bps":9100,"source_evidence_ids":["refevid_http_front"],"candidate_view_ids":["iso","front"],"reason":"The articulated silhouette remains visible."},
                {"claim_id":"vclaim_http_meso","outcome":"matched","similarity_bps":7900,"confidence_bps":8800,"source_evidence_ids":["refevid_http_front"],"candidate_view_ids":["iso","front"],"reason":"Blue armor panels remain distinct from the dark frame."}
            ]
        })
        .to_string();
        VisionEvidenceHttpResponse {
            status: 200,
            content_type: Some("application/json".into()),
            body: serde_json::to_vec(&json!({
                "choices":[{"message":{"role":"assistant","content":content}}]
            }))
            .unwrap(),
            network_call_made: true,
        }
    }

    fn inconsistent_comparison_provider_response() -> VisionEvidenceHttpResponse {
        let content = json!({
            "assessments":[
                {"claim_id":"vclaim_http_macro","outcome":"partial","similarity_bps":8600,"confidence_bps":9100,"source_evidence_ids":["refevid_http_front"],"candidate_view_ids":["iso","front"],"reason":"The articulated silhouette remains visible."},
                {"claim_id":"vclaim_http_meso","outcome":"matched","similarity_bps":0,"confidence_bps":8800,"source_evidence_ids":["refevid_http_front"],"candidate_view_ids":["iso","front"],"reason":"The requested material split is not visible."}
            ]
        })
        .to_string();
        VisionEvidenceHttpResponse {
            status: 200,
            content_type: Some("application/json".into()),
            body: serde_json::to_vec(&json!({
                "choices":[{"message":{"role":"assistant","content":content}}]
            }))
            .unwrap(),
            network_call_made: true,
        }
    }

    fn formal_comparison_provider_response(
        request: &VisualReferenceComparisonProviderRequest,
    ) -> VisionEvidenceHttpResponse {
        let content = json!({
            "assessments":[
                {"claim_id":"vclaim_http_macro","outcome":"matched","similarity_bps":8600,"confidence_bps":9100,"source_evidence_ids":["refevid_http_front"],"candidate_view_ids":["turntable_000"],"reason":"The articulated silhouette remains visible."},
                {"claim_id":"vclaim_http_meso","outcome":"matched","similarity_bps":7900,"confidence_bps":8800,"source_evidence_ids":["refevid_http_front"],"candidate_view_ids":["turntable_000"],"reason":"Blue armor panels remain distinct from the dark frame."}
            ],
            "e005_visual_patch_proposal":{
                "schema_version":"E005VisualPatchProposal@1",
                "patch_id":"visualpatch_adapter_formal_accept",
                "decision":"accept",
                "expected_source_sha256":request.input.source_program_sha256,
                "comparison_input_sha256":semantic_sha256(&request.input).unwrap(),
                "repair_claim_ids":[],
                "operations":[]
            }
        })
        .to_string();
        VisionEvidenceHttpResponse {
            status: 200,
            content_type: Some("application/json".into()),
            body: serde_json::to_vec(&json!({
                "choices":[{"message":{"role":"assistant","content":content}}],
                "usage":{
                    "prompt_tokens":4096,
                    "completion_tokens":512,
                    "prompt_tokens_details":{"cached_tokens":1024}
                }
            }))
            .unwrap(),
            network_call_made: true,
        }
    }

    fn request() -> VisionEvidenceProviderRequest {
        let bytes: Arc<[u8]> = Arc::from([137, 80, 78, 71, 13, 10, 26, 10]);
        let sha = format!("{:x}", Sha256::digest(bytes.as_ref()));
        let evidence = ReferenceEvidence {
            schema_version: "ReferenceEvidence@1".into(),
            evidence_id: "refevid_http_front".into(),
            project_id: "prj_http_vision".into(),
            kind: ReferenceEvidenceKind::Image,
            reference_class: ReferenceClass::SingleImage,
            domain_pack_id: "pack_robotic_arm_concept".into(),
            source_file_name: "front.png".into(),
            source_media_type: "image/png".into(),
            source_object_sha256: sha,
            source_imported_asset_version_id: None,
            source_statement: "User supplied reference".into(),
            license_statement: "Remote vision analysis authorized".into(),
            missing_views: vec!["back".into()],
            user_notes: "Use visible design evidence".into(),
            observations: ReferenceEvidenceObservations {
                silhouette_summary: "Articulated arm".into(),
                proportion_ranges: vec!["balanced arm segments".into()],
                material_zone_observations: vec!["blue and dark shell".into()],
                visible_part_hypotheses: vec![],
                uncertainties: vec!["back view missing".into()],
                image_surface_facts: Some(ReferenceImageSurfaceFacts {
                    width: 1024,
                    height: 1024,
                    aspect_ratio_milli: 1000,
                    dominant_color_buckets: vec![ReferenceImageColorBucket::Blue],
                    brightness: ReferenceImageBrightnessBucket::Dark,
                    edge_density: ReferenceImageEdgeDensityBucket::High,
                    foreground_bbox_normalized: [100, 100, 900, 900],
                    contact_sheet_layout_evidence: false,
                    foreground_confidence: ReferenceImageForegroundConfidence::Medium,
                }),
            },
            created_at: "2026-07-26T14:00:00Z".into(),
            glb_inspection: None,
        };
        VisionEvidenceProviderRequest {
            request: MultimodalDesignRequest {
                schema_version: MULTIMODAL_DESIGN_REQUEST_SCHEMA_VERSION.into(),
                request_id: "mmreq_http_vision".into(),
                project_id: evidence.project_id.clone(),
                turn_id: "turn_http_vision".into(),
                domain_pack_id: evidence.domain_pack_id.clone(),
                instruction: "Create a detailed blue industrial arm".into(),
                reference_inputs: vec![MultimodalReferenceInput {
                    evidence_id: evidence.evidence_id.clone(),
                    evidence_sha256: semantic_sha256(&evidence).unwrap(),
                    role: ReferenceRole::PrimarySilhouette,
                    view_id: Some("front".into()),
                    region: None,
                }],
                active_asset_version_id: None,
                selection: None,
                locks: MultimodalDesignLocks {
                    preserve_geometry: false,
                    preserve_material_surface: false,
                    locked_part_ids: vec![],
                    locked_material_zone_ids: vec![],
                },
            },
            evidence: vec![evidence.clone()],
            images: vec![VisionEvidenceImage {
                evidence_id: evidence.evidence_id,
                media_type: evidence.source_media_type,
                bytes,
            }],
        }
    }

    fn comparison_request() -> VisualReferenceComparisonProviderRequest {
        let evidence_request = request();
        let evidence = evidence_request.evidence[0].clone();
        let graph = VisualEvidenceGraph {
            schema_version: VISUAL_EVIDENCE_GRAPH_SCHEMA_VERSION.into(),
            graph_id: "vegraph_http_compare".into(),
            request_id: evidence_request.request.request_id.clone(),
            request_sha256: semantic_sha256(&evidence_request.request).unwrap(),
            project_id: evidence.project_id.clone(),
            domain_pack_id: evidence.domain_pack_id.clone(),
            provider: VisionEvidenceProviderProvenance {
                provider_id: "openai_compatible_vision".into(),
                model_id: "qwen3-vl-plus".into(),
                provider_response_sha256: "a".repeat(64),
                analyzed_at: "2026-07-26T14:00:00Z".into(),
            },
            claims: vec![
                VisualEvidenceClaim {
                    claim_id: "vclaim_http_macro".into(),
                    level: VisualDetailLevel::Macro,
                    status: VisualClaimStatus::Observed,
                    target: VisualClaimTarget::Geometry,
                    description: "Tall articulated silhouette".into(),
                    critical: true,
                    confidence_bps: 9000,
                    source_evidence_ids: vec![evidence.evidence_id.clone()],
                    source_view_id: Some("front".into()),
                    source_region: None,
                },
                VisualEvidenceClaim {
                    claim_id: "vclaim_http_meso".into(),
                    level: VisualDetailLevel::Meso,
                    status: VisualClaimStatus::Observed,
                    target: VisualClaimTarget::Material,
                    description: "Blue armor panels over a dark frame".into(),
                    critical: true,
                    confidence_bps: 8500,
                    source_evidence_ids: vec![evidence.evidence_id.clone()],
                    source_view_id: Some("front".into()),
                    source_region: None,
                },
            ],
        };
        let glb_sha256 = "c".repeat(64);
        let mut candidate_images = Vec::new();
        let candidate_views = REQUIRED_VISUAL_VIEW_IDS
            .into_iter()
            .enumerate()
            .map(|(index, view_id)| {
                let bytes: Arc<[u8]> = Arc::from(vec![index as u8 + 1, 80, 78, 71]);
                let image_sha256 = format!("{:x}", Sha256::digest(bytes.as_ref()));
                candidate_images.push(VisualReferenceComparisonImage {
                    image_id: view_id.into(),
                    media_type: "image/png".into(),
                    bytes,
                });
                VisualFixedViewEvidence {
                    view_id: view_id.into(),
                    glb_sha256: glb_sha256.clone(),
                    renderer_id: "forgecad-agent-software-raster@1".into(),
                    image_sha256,
                    readback_passed: true,
                }
            })
            .collect();
        VisualReferenceComparisonProviderRequest {
            authorization_id: Some("visauth_adapter_fixture".into()),
            turn_id: "turn_adapter_fixture".into(),
            input: VisualReferenceComparisonInput {
                schema_version: VISUAL_REFERENCE_COMPARISON_INPUT_SCHEMA_VERSION.into(),
                request_sha256: graph.request_sha256.clone(),
                evidence_graph_sha256: semantic_sha256(&graph).unwrap(),
                program_binding_sha256: "b".repeat(64),
                source_program_sha256: "d".repeat(64),
                glb_sha256,
                acceptance_policy: VisualReferenceAcceptancePolicy::default_policy(),
                reference_sources: vec![VisualReferenceSourceFingerprint {
                    evidence_id: evidence.evidence_id.clone(),
                    evidence_sha256: semantic_sha256(&evidence).unwrap(),
                }],
                candidate_view_profile: None,
                candidate_views,
            },
            graph,
            evidence: vec![evidence],
            reference_images: vec![VisualReferenceComparisonImage {
                image_id: evidence_request.images[0].evidence_id.clone(),
                media_type: evidence_request.images[0].media_type.clone(),
                bytes: evidence_request.images[0].bytes.clone(),
            }],
            candidate_images,
            e005_source: None,
        }
    }

    fn formal_comparison_request() -> VisualReferenceComparisonProviderRequest {
        let mut request = comparison_request();
        let source: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../packages/concept-spec/fixtures/e005-r1-unified-service-console.json"
        )))
        .unwrap();
        let lowering = forgecad_core::lower_forge_visual_author_source_v1(&source).unwrap();
        let glb_sha256 = request.input.glb_sha256.clone();
        let renderer_id = "forgecad-e005-turntable-test@1";
        let mut candidate_images = Vec::new();
        let candidate_views = [
            "turntable_000",
            "turntable_045",
            "turntable_090",
            "turntable_135",
            "turntable_180",
            "turntable_225",
            "turntable_270",
            "turntable_315",
        ]
        .into_iter()
        .enumerate()
        .map(|(index, view_id)| {
            let bytes: Arc<[u8]> = Arc::from(vec![index as u8 + 20, 80, 78, 71]);
            let image_sha256 = sha256_hex(bytes.as_ref());
            candidate_images.push(VisualReferenceComparisonImage {
                image_id: view_id.into(),
                media_type: "image/png".into(),
                bytes,
            });
            VisualFixedViewEvidence {
                view_id: view_id.into(),
                glb_sha256: glb_sha256.clone(),
                renderer_id: renderer_id.into(),
                image_sha256,
                readback_passed: true,
            }
        })
        .collect();
        request.input.source_program_sha256 = lowering.source_program_sha256;
        request.input.candidate_view_profile =
            Some(VisualReferenceCandidateViewProfile::TurntableEight);
        request.input.candidate_views = candidate_views;
        request.candidate_images = candidate_images;
        request.e005_source = Some(source);
        request
    }

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn pv006b_private_store_uses_metadata_plus_generation_secret_without_os_prompt() {
        let root = TempDir::new().unwrap();
        let store = PrivateFileVisionEvidenceCredentialStore::new(root.path().join("vision"));
        let missing = store.inspect_metadata().unwrap();
        assert!(!missing.configured);
        assert!(!missing.requires_os_prompt);
        let saved = store
            .save(
                "https://unit-test.cn-beijing.maas.aliyuncs.com/compatible-mode/v1".into(),
                "qwen3-vl-plus".into(),
                "test-only-vision-secret".into(),
            )
            .unwrap();
        assert!(saved.configured);
        assert_eq!(saved.storage, "private_secret_file");
        let snapshot = store.load_snapshot().unwrap().unwrap();
        assert_eq!(snapshot.model, "qwen3-vl-plus");
        assert!(!format!("{snapshot:?}").contains("test-only-vision-secret"));
        assert!(!store.clear().unwrap().configured);
    }

    #[test]
    fn pv006b_provider_policy_rejects_non_qwen_endpoint_or_model_family() {
        assert!(validate_base_url("https://example.test/compatible-mode/v1").is_err());
        assert!(validate_model("deepseek-v4-pro").is_err());
        assert!(validate_base_url(
            "https://unit-test.cn-beijing.maas.aliyuncs.com/compatible-mode/v1"
        )
        .is_ok());
        assert!(validate_model("qwen3-vl-plus").is_ok());
    }

    #[test]
    fn pv006b_openai_compatible_adapter_sends_multimodal_content_and_redacts_debug() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let reads = Arc::new(Mutex::new(0usize));
        let adapter = OpenAiCompatibleVisionEvidenceAdapter::new(
            Arc::new(FixedCredentialSource {
                snapshot: VisionEvidenceCredentialSnapshot {
                    base_url: Url::parse(
                        "https://unit-test.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
                    )
                    .unwrap(),
                    model: "qwen3-vl-plus".into(),
                    secret: VisionEvidenceSecret::new("test-only-vision-secret".into()).unwrap(),
                },
                reads: reads.clone(),
            }),
            Arc::new(ScriptedTransport {
                requests: requests.clone(),
                responses: Arc::new(Mutex::new(VecDeque::from([provider_response()]))),
            }),
            Duration::from_secs(30),
        )
        .unwrap();
        let output = block_on(adapter.analyze(request(), CancellationToken::new())).unwrap();
        assert_eq!(output.model_id, "qwen3-vl-plus");
        assert_eq!(output.claims.len(), 3);
        assert_eq!(*reads.lock().unwrap(), 1);
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].endpoint.path(),
            "/compatible-mode/v1/chat/completions"
        );
        let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["model"], "qwen3-vl-plus");
        assert!(body["messages"][0]["content"]
            .as_str()
            .is_some_and(|content| content.contains("exactly one top-level field, claims")));
        assert!(body["messages"][0]["content"].as_str().is_some_and(
            |content| content.contains("A claim with zero confidence must be unknown")
        ));
        let request_summary = body["messages"][1]["content"][0]["text"].as_str().unwrap();
        assert!(request_summary.contains("\"observed\":\"confidence_bps must be 1..10000"));
        assert!(request_summary.contains("\"unknown\":\"confidence_bps must be 0"));
        assert_eq!(body["messages"][1]["content"][1]["type"], "image_url");
        assert!(body["messages"][1]["content"][1]["image_url"]["url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        let debug = format!("{:?}", requests[0]);
        assert!(!debug.contains("test-only-vision-secret"));
        assert!(!debug.contains("data:image/png"));
    }

    #[test]
    fn pv006b_claims_protocol_errors_are_specific_without_copying_provider_content() {
        let response = |content: &str| VisionEvidenceHttpResponse {
            status: 200,
            content_type: Some("application/json".into()),
            body: serde_json::to_vec(&json!({
                "choices":[{"message":{"role":"assistant","content":content}}]
            }))
            .unwrap(),
            network_call_made: true,
        };

        let extra_top_level = parse_openai_compatible_response(
            "qwen3-vl-plus",
            response(r#"{"claims":[],"provider_note":"secret text"}"#),
        )
        .unwrap_err();
        assert_eq!(extra_top_level.code, "VISION_EVIDENCE_PROTOCOL_INVALID");
        assert!(extra_top_level.message.contains("exactly the claims field"));
        assert!(!extra_top_level.message.contains("secret text"));

        let non_array_claims = parse_openai_compatible_response(
            "qwen3-vl-plus",
            response(r#"{"claims":{"claim_id":"not-an-array"}}"#),
        )
        .unwrap_err();
        assert!(non_array_claims
            .message
            .contains("claims field must be an array"));

        let invalid_claim = parse_openai_compatible_response(
            "qwen3-vl-plus",
            response(r#"{"claims":[{"claim_id":"vclaim_bad","leaked":"secret text"}]}"#),
        )
        .unwrap_err();
        assert!(invalid_claim.message.contains("claim at index 0"));
        assert!(!invalid_claim.message.contains("secret text"));
    }

    #[test]
    fn pv006c_openai_compatible_adapter_compares_reference_with_exact_candidate_views() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let reads = Arc::new(Mutex::new(0usize));
        let adapter = OpenAiCompatibleVisionEvidenceAdapter::new(
            Arc::new(FixedCredentialSource {
                snapshot: VisionEvidenceCredentialSnapshot {
                    base_url: Url::parse(
                        "https://unit-test.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
                    )
                    .unwrap(),
                    model: "qwen3-vl-plus".into(),
                    secret: VisionEvidenceSecret::new("test-only-vision-secret".into()).unwrap(),
                },
                reads: reads.clone(),
            }),
            Arc::new(ScriptedTransport {
                requests: requests.clone(),
                responses: Arc::new(Mutex::new(VecDeque::from([comparison_provider_response()]))),
            }),
            Duration::from_secs(30),
        )
        .unwrap();
        let output =
            block_on(adapter.compare(comparison_request(), CancellationToken::new())).unwrap();
        assert_eq!(output.model_id, "qwen3-vl-plus");
        assert_eq!(output.assessments.len(), 2);
        assert_eq!(*reads.lock().unwrap(), 1);
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
        let content = body["messages"][1]["content"].as_array().unwrap();
        assert_eq!(
            content
                .iter()
                .filter(|item| item["type"] == "image_url")
                .count(),
            9
        );
        assert!(content.iter().any(|item| {
            item["text"]
                .as_str()
                .is_some_and(|text| text.contains("REFERENCE_IMAGE evidence_id="))
        }));
        assert!(content.iter().any(|item| {
            item["text"]
                .as_str()
                .is_some_and(|text| text == "CANDIDATE_RENDER view_id=iso")
        }));
        let debug = format!("{:?}", requests[0]);
        assert!(!debug.contains("test-only-vision-secret"));
        assert!(!debug.contains("data:image/png"));
    }

    #[test]
    fn e005_r2_formal_adapter_prepares_exact_body_before_one_idempotent_dispatch() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let reads = Arc::new(Mutex::new(0usize));
        let request = formal_comparison_request();
        let response = formal_comparison_provider_response(&request);
        let adapter = OpenAiCompatibleVisionEvidenceAdapter::new(
            Arc::new(FixedCredentialSource {
                snapshot: VisionEvidenceCredentialSnapshot {
                    base_url: Url::parse(
                        "https://unit-test.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
                    )
                    .unwrap(),
                    model: "qwen3-vl-plus".into(),
                    secret: VisionEvidenceSecret::new("test-only-vision-secret".into()).unwrap(),
                },
                reads: reads.clone(),
            }),
            Arc::new(ScriptedTransport {
                requests: requests.clone(),
                responses: Arc::new(Mutex::new(VecDeque::from([response]))),
            }),
            Duration::from_secs(30),
        )
        .unwrap()
        .with_formal_pricing(VisionEvidenceFormalPricing::new(2_000_000, 8_000_000).unwrap());

        let prepared = adapter.prepare_e005_visual_review(request).unwrap();
        assert_eq!(*reads.lock().unwrap(), 1);
        assert!(requests.lock().unwrap().is_empty());
        assert_eq!(prepared.provider_id(), "openai_compatible_vision");
        assert_eq!(prepared.model_id(), "qwen3-vl-plus");
        assert_eq!(prepared.max_output_tokens(), 8_192);
        assert!(prepared.commitment().budget_policy.input_tokens_upper_bound > 4_096);
        let committed_request_sha256 = prepared.commitment().request_sha256.clone();
        let output = block_on(prepared.dispatch(
            "e005_reservation_formal_visual_001".into(),
            CancellationToken::new(),
        ))
        .unwrap();

        assert_eq!(output.usage.input_tokens, 4_096);
        assert_eq!(output.usage.output_tokens, 512);
        assert_eq!(output.usage.prompt_cache_hit_tokens, 1_024);
        assert_eq!(output.usage.estimated_cost_microusd, 12_288);
        assert!(output.output.network_call_made);
        assert!(output.output.e005_visual_patch_proposal.is_some());
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].remote_idempotency_key.as_deref(),
            Some("e005_reservation_formal_visual_001")
        );
        assert_eq!(
            committed_request_sha256,
            sha256_hex(requests[0].body.as_ref())
        );
        let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(
            body["messages"][0]["content"],
            E005_VISUAL_REVIEW_SYSTEM_PROMPT
        );
        let task: Value =
            serde_json::from_str(body["messages"][1]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        assert_eq!(
            task["required_output"]["e005_visual_patch_proposal_schema"]["properties"]
                ["schema_version"]["const"],
            "E005VisualPatchProposal@1"
        );
    }

    #[test]
    fn pv006c_adapter_normalizes_redundant_outcome_from_similarity_without_raising_scores() {
        let output = parse_reference_comparison_response(
            "qwen3-vl-plus",
            inconsistent_comparison_provider_response(),
            false,
        )
        .unwrap();
        assert_eq!(output.assessments[0].similarity_bps, 8600);
        assert_eq!(
            output.assessments[0].outcome,
            VisualReferenceMatchOutcome::Matched
        );
        assert_eq!(output.assessments[1].similarity_bps, 0);
        assert_eq!(
            output.assessments[1].outcome,
            VisualReferenceMatchOutcome::NotVisible
        );
    }

    #[test]
    fn pv006c_adapter_bounds_multilingual_reason_without_splitting_utf8() {
        let reason = "候选模型与参考图的机械臂轮廓、蓝黑材质分区和关节层级保持一致。".repeat(20);
        let bounded = bounded_safe_comparison_reason(&reason);
        assert!(!bounded.is_empty());
        assert!(bounded.len() <= 320);
        assert!(std::str::from_utf8(bounded.as_bytes()).is_ok());
    }

    #[test]
    fn pv006c_adapter_replaces_unsafe_reason_instead_of_persisting_it() {
        let bounded = bounded_safe_comparison_reason(
            "Inspect https://example.test and bearer sk-secret before accepting the view.",
        );
        assert_eq!(
            bounded,
            "The configured Provider returned a visual similarity assessment."
        );
        assert!(!bounded.contains("example.test"));
        assert!(!bounded.contains("sk-secret"));
    }

    #[test]
    fn pv006b_http_failure_retains_network_marker_without_response_body_leak() {
        let error = parse_openai_compatible_response(
            "qwen3-vl-plus",
            VisionEvidenceHttpResponse {
                status: 401,
                content_type: Some("application/json".into()),
                body: br#"{"message":"secret diagnostic"}"#.to_vec(),
                network_call_made: true,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "VISION_EVIDENCE_AUTH_FAILED");
        assert!(error.network_call_made);
        assert!(!error.message.contains("secret diagnostic"));
    }

    #[test]
    fn pv006b_oversized_or_redirect_response_fails_closed() {
        let oversized = parse_openai_compatible_response(
            "qwen3-vl-plus",
            VisionEvidenceHttpResponse {
                status: 200,
                content_type: Some("application/json".into()),
                body: vec![b' '; MAX_VISION_EVIDENCE_RESPONSE_BYTES + 1],
                network_call_made: true,
            },
        )
        .unwrap_err();
        assert_eq!(oversized.code, "VISION_EVIDENCE_PROTOCOL_INVALID");
        assert!(oversized.network_call_made);

        let redirect = parse_openai_compatible_response(
            "qwen3-vl-plus",
            VisionEvidenceHttpResponse {
                status: 302,
                content_type: Some("application/json".into()),
                body: br#"{"redirect":"not-followed"}"#.to_vec(),
                network_call_made: true,
            },
        )
        .unwrap_err();
        assert_eq!(redirect.code, "VISION_EVIDENCE_HTTP_FAILED");
        assert!(!redirect.retryable);
    }
}
