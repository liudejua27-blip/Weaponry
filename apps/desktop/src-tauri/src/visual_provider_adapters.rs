//! Concrete protocol adapter for Forge Studio remote visual generation.
//!
//! This module currently implements the Fal Flux 2 concept-image queue. It is
//! deliberately injected with transport, credential and object-sink ports so
//! tests make zero network calls and production can keep secrets in Keychain.

use std::{
    fmt,
    fs::{self, OpenOptions},
    future::Future,
    io::Write,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use forgecad_app_server::{
    ConceptImageOutputHandle, ConceptImageProviderError, ConceptImageProviderFuture,
    ConceptImageProviderPort, ConceptImageProviderReceipt, ConceptImageProviderStatus,
    CONCEPT_IMAGE_PROVIDER_RECEIPT_SCHEMA_VERSION,
};
use forgecad_core::{
    analyze_reference_image_bytes, inspect_concept_png, ConceptImageBackend,
    ConceptImageGenerationRequest, CoreRepository, ObjectReference,
};
use reqwest::{
    header::{self, HeaderValue},
    redirect, Client, Url,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

const FAL_QUEUE_ORIGIN: &str = "https://queue.fal.run";
const FAL_FLUX_2_MODEL_PATH: &str = "/fal-ai/flux-2";
const FAL_FLUX_2_EDIT_MODEL_PATH: &str = "/fal-ai/flux-2/edit";
const MAX_FAL_JSON_BYTES: usize = 1024 * 1024;
const MAX_CONCEPT_PNG_BYTES: usize = 32 * 1024 * 1024;
const MAX_CONCEPT_INPUT_IMAGE_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_VISUAL_HTTP_RESPONSE_BYTES: usize = 256 * 1024 * 1024;

pub type VisualHttpFuture<T> =
    Pin<Box<dyn Future<Output = Result<T, ConceptImageProviderError>> + Send + 'static>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualHttpMethod {
    Get,
    Post,
    Put,
}

#[derive(Clone)]
pub struct VisualHttpRequest {
    pub method: VisualHttpMethod,
    pub endpoint: Url,
    pub authorization: Option<VisualProviderSecret>,
    pub content_type: Option<&'static str>,
    pub body: Arc<[u8]>,
    pub max_response_bytes: usize,
    pub timeout: Duration,
}

impl fmt::Debug for VisualHttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VisualHttpRequest")
            .field("method", &self.method)
            .field(
                "endpoint_origin",
                &self.endpoint.origin().ascii_serialization(),
            )
            .field("endpoint_path", &self.endpoint.path())
            .field(
                "authorization",
                &self.authorization.as_ref().map(|_| "[REDACTED]"),
            )
            .field("content_type", &self.content_type)
            .field("body_bytes", &self.body.len())
            .field("max_response_bytes", &self.max_response_bytes)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualHttpResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
    pub network_call_made: bool,
}

pub trait VisualHttpTransport: Send + Sync + 'static {
    fn execute(&self, request: VisualHttpRequest) -> VisualHttpFuture<VisualHttpResponse>;
}

/// Production HTTPS transport for reviewed visual-provider adapters.
///
/// Redirects are disabled so an authenticated queue request cannot carry its
/// credential to a different origin. Response bodies are streamed into a
/// caller-provided hard bound; `Content-Length` is treated only as an early
/// rejection hint, never as proof that the body is small.
#[derive(Clone)]
pub struct ReqwestVisualHttpTransport {
    client: Client,
}

impl ReqwestVisualHttpTransport {
    pub fn new() -> Result<Self, ConceptImageProviderError> {
        let client = Client::builder()
            .https_only(true)
            .redirect(redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| transport_error("Visual HTTPS client could not be initialized."))?;
        Ok(Self { client })
    }
}

impl VisualHttpTransport for ReqwestVisualHttpTransport {
    fn execute(&self, request: VisualHttpRequest) -> VisualHttpFuture<VisualHttpResponse> {
        let client = self.client.clone();
        Box::pin(async move {
            if request.endpoint.scheme() != "https"
                || !request.endpoint.username().is_empty()
                || request.endpoint.password().is_some()
                || request.endpoint.fragment().is_some()
                || request.max_response_bytes == 0
                || request.max_response_bytes > MAX_VISUAL_HTTP_RESPONSE_BYTES
                || request.timeout.is_zero()
                || request.timeout > Duration::from_secs(60)
            {
                return Err(transport_error(
                    "Visual HTTPS request is outside the reviewed transport bounds.",
                ));
            }

            let fal_queue_request = request.endpoint.host_str() == Some("queue.fal.run");
            let mut builder = match request.method {
                VisualHttpMethod::Get => client.get(request.endpoint),
                VisualHttpMethod::Post => client.post(request.endpoint),
                VisualHttpMethod::Put => client.put(request.endpoint),
            }
            .timeout(request.timeout);

            if fal_queue_request {
                // Do not retain user prompts or provider payload history.
                // Generated CDN media remains available for one hour so this
                // process can finish result download/readback after the queue
                // completes, then expires independently at the Provider.
                builder = builder.header("X-Fal-Store-IO", "0").header(
                    "X-Fal-Object-Lifecycle-Preference",
                    r#"{"expiration_duration_seconds":3600}"#,
                );
            }
            if let Some(content_type) = request.content_type {
                builder = builder.header(header::CONTENT_TYPE, content_type);
            }
            if let Some(secret) = request.authorization {
                let value = secret.authorization_value();
                let mut header_value = HeaderValue::from_bytes(value.as_bytes()).map_err(|_| {
                    transport_error("Visual provider credential cannot form an HTTP header.")
                })?;
                header_value.set_sensitive(true);
                builder = builder.header(header::AUTHORIZATION, header_value);
            }
            if !request.body.is_empty() {
                builder = builder.body(request.body.as_ref().to_vec());
            }

            let mut response = builder
                .send()
                .await
                .map_err(|_| transport_error("Visual provider HTTPS request failed."))?;
            if response
                .content_length()
                .is_some_and(|length| length > request.max_response_bytes as u64)
            {
                return Err(transport_error(
                    "Visual provider response exceeds the reviewed byte limit.",
                ));
            }
            let status = response.status().as_u16();
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .filter(|value| value.len() <= 256)
                .map(str::to_owned);
            let mut body = Vec::with_capacity(
                response
                    .content_length()
                    .unwrap_or_default()
                    .min(request.max_response_bytes as u64) as usize,
            );
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| transport_error("Visual provider response stream failed."))?
            {
                if body
                    .len()
                    .checked_add(chunk.len())
                    .is_none_or(|length| length > request.max_response_bytes)
                {
                    return Err(transport_error(
                        "Visual provider response exceeds the reviewed byte limit.",
                    ));
                }
                body.extend_from_slice(&chunk);
            }
            Ok(VisualHttpResponse {
                status,
                content_type,
                body,
                network_call_made: true,
            })
        })
    }
}

#[derive(Clone)]
pub struct VisualProviderSecret(Arc<Zeroizing<String>>);

impl VisualProviderSecret {
    pub fn new(value: String) -> Result<Self, ConceptImageProviderError> {
        if value.trim().is_empty() || value.len() > 4_096 || value.contains('\0') {
            return Err(ConceptImageProviderError::new(
                "VISUAL_PROVIDER_CREDENTIAL_INVALID",
                "Visual provider credential is empty or outside the reviewed bound.",
            ));
        }
        Ok(Self(Arc::new(Zeroizing::new(value))))
    }

    pub fn authorization_value(&self) -> Zeroizing<String> {
        Zeroizing::new(format!("Key {}", self.0.as_str()))
    }
}

impl fmt::Debug for VisualProviderSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("VisualProviderSecret([REDACTED])")
    }
}

pub trait FalCredentialSource: Send + Sync + 'static {
    fn load(&self) -> Result<Option<VisualProviderSecret>, ConceptImageProviderError>;
}

/// Explicit, prompt-free Alpha credential store.
///
/// The file is outside the repository, must live under a private directory and
/// must itself be mode 0600. Merely constructing or inspecting this source
/// performs no read; the Fal adapter calls `load` only for submit/poll/cancel
/// initiated by an active visual-generation job.
#[derive(Clone)]
pub struct PrivateFileFalCredentialSource {
    path: PathBuf,
}

impl PrivateFileFalCredentialSource {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn configured(&self) -> Result<bool, ConceptImageProviderError> {
        match fs::symlink_metadata(&self.path) {
            Ok(metadata) => {
                let parent = self.path.parent().ok_or_else(|| {
                    credential_file_error("Visual provider credential path has no private parent.")
                })?;
                validate_private_secret_directory(parent)?;
                validate_private_secret_metadata(&metadata)?;
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(_) => Err(credential_file_error(
                "Visual provider credential metadata could not be read.",
            )),
        }
    }

    pub fn save(&self, value: String) -> Result<(), ConceptImageProviderError> {
        let secret = VisualProviderSecret::new(value)?;
        let parent = self.path.parent().ok_or_else(|| {
            credential_file_error("Visual provider credential path has no private parent.")
        })?;
        ensure_private_secret_directory(parent)?;
        if self.configured()? {
            validate_private_secret_path(&self.path)?;
        }
        let serial = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp_path = parent.join(format!(
            ".visual-provider-secret.{}.{}.tmp",
            std::process::id(),
            serial
        ));
        let write_result = (|| {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temp_path).map_err(|_| {
                credential_file_error(
                    "Visual provider credential staging file could not be created.",
                )
            })?;
            file.write_all(secret.0.as_bytes()).map_err(|_| {
                credential_file_error(
                    "Visual provider credential staging file could not be written.",
                )
            })?;
            file.sync_all().map_err(|_| {
                credential_file_error(
                    "Visual provider credential staging file could not be synced.",
                )
            })?;
            validate_private_secret_path(&temp_path)?;
            fs::rename(&temp_path, &self.path).map_err(|_| {
                credential_file_error(
                    "Visual provider credential could not be atomically published.",
                )
            })?;
            validate_private_secret_path(&self.path)?;
            sync_secret_directory(parent)?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        write_result
    }

    pub fn delete(&self) -> Result<(), ConceptImageProviderError> {
        match fs::symlink_metadata(&self.path) {
            Ok(_) => {
                validate_private_secret_path(&self.path)?;
                fs::remove_file(&self.path).map_err(|_| {
                    credential_file_error("Visual provider credential could not be deleted.")
                })?;
                if let Some(parent) = self.path.parent() {
                    sync_secret_directory(parent)?;
                }
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(credential_file_error(
                "Visual provider credential metadata could not be read.",
            )),
        }
    }
}

impl fmt::Debug for PrivateFileFalCredentialSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateFileFalCredentialSource([REDACTED_PATH])")
    }
}

impl FalCredentialSource for PrivateFileFalCredentialSource {
    fn load(&self) -> Result<Option<VisualProviderSecret>, ConceptImageProviderError> {
        if !self.configured()? {
            return Ok(None);
        }
        validate_private_secret_path(&self.path)?;
        let metadata = fs::metadata(&self.path).map_err(|_| {
            credential_file_error("Visual provider credential metadata could not be read.")
        })?;
        if metadata.len() == 0 || metadata.len() > 4_096 {
            return Err(credential_file_error(
                "Visual provider credential file is empty or outside the reviewed bound.",
            ));
        }
        let value = fs::read_to_string(&self.path).map_err(|_| {
            credential_file_error("Visual provider credential file could not be read.")
        })?;
        Ok(Some(VisualProviderSecret::new(
            value.trim_end_matches(['\r', '\n']).to_owned(),
        )?))
    }
}

pub trait ConceptImageObjectSink: Send + Sync + 'static {
    /// Validates and stores one downloaded PNG, returning CAS-backed facts.
    fn accept_png(
        &self,
        bytes: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<ConceptImageOutputHandle, ConceptImageProviderError>;
}

pub trait ConceptInputImageSource: Send + Sync + 'static {
    fn read_image(
        &self,
        sha256: &str,
    ) -> Result<(Vec<u8>, &'static str), ConceptImageProviderError>;
}

pub struct CoreConceptInputImageSource {
    repository: Arc<CoreRepository>,
}

impl CoreConceptInputImageSource {
    pub fn new(repository: Arc<CoreRepository>) -> Self {
        Self { repository }
    }
}

impl ConceptInputImageSource for CoreConceptInputImageSource {
    fn read_image(
        &self,
        sha256: &str,
    ) -> Result<(Vec<u8>, &'static str), ConceptImageProviderError> {
        let bytes = self
            .repository
            .read_object(sha256)
            .map_err(|_| protocol_error("Authorized input image CAS read failed."))?;
        if bytes.is_empty() || bytes.len() > MAX_CONCEPT_INPUT_IMAGE_BYTES {
            return Err(protocol_error(
                "Authorized input image exceeds the reviewed 8 MiB remote-processing limit.",
            ));
        }
        let media_type = detect_supported_image_media_type(&bytes).ok_or_else(|| {
            protocol_error("Authorized input image bytes are not PNG, JPEG or WebP.")
        })?;
        analyze_reference_image_bytes(media_type, &bytes)
            .map_err(|_| protocol_error("Authorized input image failed full decode."))?;
        if format!("{:x}", Sha256::digest(&bytes)) != sha256 {
            return Err(protocol_error(
                "Authorized input image bytes do not match the requested CAS digest.",
            ));
        }
        Ok((bytes, media_type))
    }
}

/// Stores a fully decoded, exact-size concept PNG through the Rust-owned
/// repository so SQLite metadata and CAS reference publication remain atomic.
pub struct CoreConceptImageObjectSink {
    repository: Arc<CoreRepository>,
    owner_id: String,
    timestamp: String,
}

impl CoreConceptImageObjectSink {
    pub fn new(repository: Arc<CoreRepository>, owner_id: String, timestamp: String) -> Self {
        Self {
            repository,
            owner_id,
            timestamp,
        }
    }
}

impl ConceptImageObjectSink for CoreConceptImageObjectSink {
    fn accept_png(
        &self,
        bytes: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<ConceptImageOutputHandle, ConceptImageProviderError> {
        if content_type.and_then(media_type_essence) != Some("image/png") {
            return Err(ConceptImageProviderError::new(
                "CONCEPT_IMAGE_MEDIA_TYPE_INVALID",
                "Downloaded concept image is not declared as image/png.",
            ));
        }
        let inspection = inspect_concept_png(&bytes).map_err(|error| {
            ConceptImageProviderError::new(
                "CONCEPT_IMAGE_BYTES_INVALID",
                format!(
                    "Downloaded concept image failed Rust validation: {}",
                    error.code()
                ),
            )
        })?;
        let record = self
            .repository
            .attach_object_bytes(
                &ObjectReference {
                    reference_kind: "reference".into(),
                    owner_id: self.owner_id.clone(),
                    role: "generated_concept_image".into(),
                },
                &bytes,
                "png",
                &self.timestamp,
            )
            .map_err(|error| {
                ConceptImageProviderError::new(
                    "CONCEPT_IMAGE_CAS_REJECTED",
                    format!(
                        "Concept image could not be committed to Rust-owned storage: {}",
                        error.code()
                    ),
                )
            })?;
        if record.sha256 != inspection.sha256
            || record.byte_size != inspection.byte_size
            || record.extension != "png"
        {
            return Err(ConceptImageProviderError::new(
                "CONCEPT_IMAGE_CAS_READBACK_MISMATCH",
                "Rust-owned concept image record does not match validated bytes.",
            ));
        }
        Ok(ConceptImageOutputHandle {
            image_object_sha256: record.sha256,
            byte_size: record.byte_size,
            media_type: "image/png".into(),
            width: inspection.width,
            height: inspection.height,
            safety_passed: true,
        })
    }
}

#[derive(Clone)]
pub struct FalFlux2ConceptImageAdapter {
    transport: Arc<dyn VisualHttpTransport>,
    credentials: Arc<dyn FalCredentialSource>,
    object_sink: Arc<dyn ConceptImageObjectSink>,
    input_source: Option<Arc<dyn ConceptInputImageSource>>,
    model_path: &'static str,
}

impl FalFlux2ConceptImageAdapter {
    pub fn new(
        transport: Arc<dyn VisualHttpTransport>,
        credentials: Arc<dyn FalCredentialSource>,
        object_sink: Arc<dyn ConceptImageObjectSink>,
    ) -> Self {
        Self {
            transport,
            credentials,
            object_sink,
            input_source: None,
            model_path: FAL_FLUX_2_MODEL_PATH,
        }
    }

    pub fn new_edit(
        transport: Arc<dyn VisualHttpTransport>,
        credentials: Arc<dyn FalCredentialSource>,
        object_sink: Arc<dyn ConceptImageObjectSink>,
        input_source: Arc<dyn ConceptInputImageSource>,
    ) -> Self {
        Self {
            transport,
            credentials,
            object_sink,
            input_source: Some(input_source),
            model_path: FAL_FLUX_2_EDIT_MODEL_PATH,
        }
    }

    fn credential(&self) -> Result<VisualProviderSecret, ConceptImageProviderError> {
        self.credentials.load()?.ok_or_else(|| {
            ConceptImageProviderError::new(
                "FAL_PROVIDER_NOT_CONFIGURED",
                "Fal concept-image provider is not configured.",
            )
        })
    }
}

impl ConceptImageProviderPort for FalFlux2ConceptImageAdapter {
    fn submit(
        &self,
        request: ConceptImageGenerationRequest,
        backend: ConceptImageBackend,
    ) -> ConceptImageProviderFuture<ConceptImageProviderReceipt> {
        let transport = self.transport.clone();
        let credential = self.credential();
        let input_source = self.input_source.clone();
        let model_path = self.model_path;
        Box::pin(async move {
            if backend != ConceptImageBackend::FalFlux2
                || !request.backend_preferences.contains(&backend)
            {
                return Err(ConceptImageProviderError::new(
                    "FAL_CONCEPT_BACKEND_INVALID",
                    "Fal Flux 2 adapter received a different concept-image backend.",
                ));
            }
            let credential = credential?;
            let mut payload = json!({
                "prompt": request.prompt,
                "image_size": {"width": request.width, "height": request.height},
                "num_images": 1,
                "acceleration": "regular",
                "enable_prompt_expansion": false,
                "enable_safety_checker": true,
                "sync_mode": false,
                "output_format": "png"
            });
            match (
                request.input_image_object_sha256.as_deref(),
                request.input_image_media_type.as_deref(),
                input_source.as_ref(),
                model_path,
            ) {
                (None, None, None, FAL_FLUX_2_MODEL_PATH) => {}
                (
                    Some(sha256),
                    Some(expected_media_type),
                    Some(source),
                    FAL_FLUX_2_EDIT_MODEL_PATH,
                ) => {
                    let (bytes, media_type) = source.read_image(sha256)?;
                    if media_type != expected_media_type {
                        return Err(ConceptImageProviderError::new(
                            "FAL_FLUX_2_INPUT_MEDIA_MISMATCH",
                            "Authorized input image media type does not match its fully decoded CAS bytes.",
                        ));
                    }
                    payload["image_urls"] = json!([format!(
                        "data:{media_type};base64,{}",
                        BASE64_STANDARD.encode(bytes)
                    )]);
                }
                _ => {
                    return Err(ConceptImageProviderError::new(
                        "FAL_FLUX_2_INPUT_MODE_MISMATCH",
                        "Fal Flux 2 text/edit adapter does not match the authorized brief input.",
                    ))
                }
            }
            let body = serde_json::to_vec(&payload)
                .map_err(|_| protocol_error("Fal submit body could not be encoded."))?;
            let response = transport
                .execute(fal_request(
                    VisualHttpMethod::Post,
                    model_path,
                    Some(credential),
                    Some("application/json"),
                    body,
                    MAX_FAL_JSON_BYTES,
                )?)
                .await?;
            require_success_json(&response)?;
            let value = parse_json(&response)?;
            let request_id = bounded_id(
                value.get("request_id").and_then(Value::as_str),
                "Fal submit response has no valid request_id.",
            )?;
            Ok(ConceptImageProviderReceipt {
                schema_version: CONCEPT_IMAGE_PROVIDER_RECEIPT_SCHEMA_VERSION.into(),
                backend,
                provider_job_id: request_id,
            })
        })
    }

    fn poll(
        &self,
        receipt: ConceptImageProviderReceipt,
    ) -> ConceptImageProviderFuture<ConceptImageProviderStatus> {
        let transport = self.transport.clone();
        let object_sink = self.object_sink.clone();
        let credential = self.credential();
        let model_path = self.model_path;
        Box::pin(async move {
            validate_fal_receipt(&receipt)?;
            let credential = credential?;
            let status_path = format!("{}/requests/{}/status", model_path, receipt.provider_job_id);
            let status_response = transport
                .execute(fal_request(
                    VisualHttpMethod::Get,
                    &status_path,
                    Some(credential.clone()),
                    None,
                    Vec::new(),
                    MAX_FAL_JSON_BYTES,
                )?)
                .await?;
            require_success_json(&status_response)?;
            let status = parse_json(&status_response)?;
            match status.get("status").and_then(Value::as_str) {
                Some("IN_QUEUE") => Ok(ConceptImageProviderStatus::Queued),
                Some("IN_PROGRESS") => Ok(ConceptImageProviderStatus::Running),
                Some("COMPLETED") => {
                    if status.get("error").is_some_and(|value| !value.is_null()) {
                        return Ok(ConceptImageProviderStatus::Failed {
                            code: "FAL_GENERATION_FAILED".into(),
                        });
                    }
                    let result_path =
                        format!("{}/requests/{}", model_path, receipt.provider_job_id);
                    let result_response = transport
                        .execute(fal_request(
                            VisualHttpMethod::Get,
                            &result_path,
                            Some(credential),
                            None,
                            Vec::new(),
                            MAX_FAL_JSON_BYTES,
                        )?)
                        .await?;
                    require_success_json(&result_response)?;
                    let result = parse_json(&result_response)?;
                    let safety = result
                        .get("has_nsfw_concepts")
                        .and_then(Value::as_array)
                        .filter(|values| values.len() == 1)
                        .and_then(|values| values[0].as_bool())
                        .ok_or_else(|| {
                            protocol_error("Fal result has no exact one-image safety result.")
                        })?;
                    if safety {
                        return Ok(ConceptImageProviderStatus::Failed {
                            code: "CONCEPT_IMAGE_SAFETY_REJECTED".into(),
                        });
                    }
                    let image = result
                        .get("images")
                        .and_then(Value::as_array)
                        .filter(|values| values.len() == 1)
                        .and_then(|values| values[0].as_object())
                        .ok_or_else(|| {
                            protocol_error("Fal result must contain exactly one image.")
                        })?;
                    if image.get("width").and_then(Value::as_u64) != Some(1024)
                        || image.get("height").and_then(Value::as_u64) != Some(1024)
                        || image.get("content_type").and_then(Value::as_str) != Some("image/png")
                    {
                        return Err(protocol_error(
                            "Fal result metadata is not the requested 1024x1024 PNG.",
                        ));
                    }
                    let media_url = validate_fal_media_url(
                        image
                            .get("url")
                            .and_then(Value::as_str)
                            .ok_or_else(|| protocol_error("Fal result image URL is missing."))?,
                    )?;
                    let media_response = transport
                        .execute(VisualHttpRequest {
                            method: VisualHttpMethod::Get,
                            endpoint: media_url,
                            authorization: None,
                            content_type: None,
                            body: Arc::from([]),
                            max_response_bytes: MAX_CONCEPT_PNG_BYTES,
                            timeout: Duration::from_secs(30),
                        })
                        .await?;
                    if media_response.status != 200 {
                        return Err(http_error(media_response.status));
                    }
                    let output = object_sink
                        .accept_png(media_response.body, media_response.content_type.as_deref())?;
                    Ok(ConceptImageProviderStatus::Ready { output })
                }
                _ => Err(protocol_error("Fal status response has an unknown state.")),
            }
        })
    }

    fn cancel(&self, receipt: ConceptImageProviderReceipt) -> ConceptImageProviderFuture<()> {
        let transport = self.transport.clone();
        let credential = self.credential();
        let model_path = self.model_path;
        Box::pin(async move {
            validate_fal_receipt(&receipt)?;
            let credential = credential?;
            let path = format!("{}/requests/{}/cancel", model_path, receipt.provider_job_id);
            let response = transport
                .execute(fal_request(
                    VisualHttpMethod::Put,
                    &path,
                    Some(credential),
                    None,
                    Vec::new(),
                    MAX_FAL_JSON_BYTES,
                )?)
                .await?;
            if response.status != 202 {
                return Err(http_error(response.status));
            }
            let value = parse_json(&response)?;
            if value.get("status").and_then(Value::as_str) != Some("CANCELLATION_REQUESTED") {
                return Err(protocol_error(
                    "Fal cancellation response did not confirm cancellation.",
                ));
            }
            Ok(())
        })
    }
}

fn fal_request(
    method: VisualHttpMethod,
    path: &str,
    authorization: Option<VisualProviderSecret>,
    content_type: Option<&'static str>,
    body: Vec<u8>,
    max_response_bytes: usize,
) -> Result<VisualHttpRequest, ConceptImageProviderError> {
    if !path.starts_with(FAL_FLUX_2_MODEL_PATH)
        || path.contains("..")
        || path.contains('?')
        || path.len() > 512
    {
        return Err(protocol_error(
            "Fal request path is outside the reviewed model route.",
        ));
    }
    let endpoint = Url::parse(&format!("{FAL_QUEUE_ORIGIN}{path}"))
        .map_err(|_| protocol_error("Fal request endpoint could not be constructed."))?;
    Ok(VisualHttpRequest {
        method,
        endpoint,
        authorization,
        content_type,
        body: Arc::from(body),
        max_response_bytes,
        timeout: Duration::from_secs(30),
    })
}

fn validate_fal_receipt(
    receipt: &ConceptImageProviderReceipt,
) -> Result<(), ConceptImageProviderError> {
    if receipt.schema_version != CONCEPT_IMAGE_PROVIDER_RECEIPT_SCHEMA_VERSION
        || receipt.backend != ConceptImageBackend::FalFlux2
    {
        return Err(protocol_error("Fal receipt schema or backend is invalid."));
    }
    bounded_id(
        Some(&receipt.provider_job_id),
        "Fal receipt has no valid provider job ID.",
    )
    .map(|_| ())
}

fn require_success_json(response: &VisualHttpResponse) -> Result<(), ConceptImageProviderError> {
    if !(200..300).contains(&response.status) {
        return Err(http_error(response.status));
    }
    if response.body.is_empty()
        || response.body.len() > MAX_FAL_JSON_BYTES
        || !response
            .content_type
            .as_deref()
            .is_some_and(|value| value.split(';').next() == Some("application/json"))
    {
        return Err(protocol_error(
            "Fal response is not bounded application/json.",
        ));
    }
    Ok(())
}

fn parse_json(response: &VisualHttpResponse) -> Result<Value, ConceptImageProviderError> {
    serde_json::from_slice(&response.body)
        .map_err(|_| protocol_error("Fal response JSON is invalid."))
}

fn bounded_id(
    value: Option<&str>,
    message: &'static str,
) -> Result<String, ConceptImageProviderError> {
    let value = value.ok_or_else(|| protocol_error(message))?;
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(protocol_error(message));
    }
    Ok(value.to_string())
}

fn validate_fal_media_url(value: &str) -> Result<Url, ConceptImageProviderError> {
    if value.len() > 4_096 {
        return Err(protocol_error("Fal media URL exceeds the reviewed bound."));
    }
    let url = Url::parse(value).map_err(|_| protocol_error("Fal media URL is invalid."))?;
    let host = url.host_str().unwrap_or_default();
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !(host == "fal.media"
            || host.ends_with(".fal.media")
            || host == "storage.googleapis.com")
    {
        return Err(protocol_error(
            "Fal media URL is outside the reviewed HTTPS host allowlist.",
        ));
    }
    Ok(url)
}

fn media_type_essence(value: &str) -> Option<&str> {
    value
        .split(';')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn detect_supported_image_media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn http_error(status: u16) -> ConceptImageProviderError {
    let code = match status {
        401 | 403 => "FAL_AUTHENTICATION_FAILED",
        402 => "FAL_BALANCE_REQUIRED",
        429 => "FAL_RATE_LIMITED",
        500..=599 => "FAL_SERVER_UNAVAILABLE",
        _ => "FAL_HTTP_FAILED",
    };
    ConceptImageProviderError::new(code, "Fal returned an unsuccessful HTTP status.")
}

fn protocol_error(message: &'static str) -> ConceptImageProviderError {
    ConceptImageProviderError::new("FAL_PROTOCOL_INVALID", message)
}

fn transport_error(message: &'static str) -> ConceptImageProviderError {
    ConceptImageProviderError::new("VISUAL_HTTP_TRANSPORT_FAILED", message)
}

fn credential_file_error(message: &'static str) -> ConceptImageProviderError {
    ConceptImageProviderError::new("VISUAL_PROVIDER_CREDENTIAL_FILE_INVALID", message)
}

fn ensure_private_secret_directory(path: &Path) -> Result<(), ConceptImageProviderError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;

        if !path.exists() {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true).mode(0o700);
            builder.create(path).map_err(|_| {
                credential_file_error("Visual provider secret directory could not be created.")
            })?;
        }
        validate_private_secret_directory(path)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(credential_file_error(
            "Private visual credential files are not implemented on this platform.",
        ))
    }
}

fn validate_private_secret_directory(path: &Path) -> Result<(), ConceptImageProviderError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = fs::symlink_metadata(path).map_err(|_| {
            credential_file_error("Visual provider secret directory could not be inspected.")
        })?;
        if !metadata.file_type().is_dir()
            || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(credential_file_error(
                "Visual provider secret directory is not a private regular directory.",
            ));
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(credential_file_error(
            "Private visual credential files are not implemented on this platform.",
        ))
    }
}

fn validate_private_secret_path(path: &Path) -> Result<(), ConceptImageProviderError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| {
        credential_file_error("Visual provider credential file could not be inspected.")
    })?;
    validate_private_secret_metadata(&metadata)
}

fn validate_private_secret_metadata(
    metadata: &fs::Metadata,
) -> Result<(), ConceptImageProviderError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if !metadata.file_type().is_file()
            || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(credential_file_error(
                "Visual provider credential must be a private regular file.",
            ));
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Err(credential_file_error(
            "Private visual credential files are not implemented on this platform.",
        ))
    }
}

fn sync_secret_directory(path: &Path) -> Result<(), ConceptImageProviderError> {
    OpenOptions::new()
        .read(true)
        .open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| credential_file_error("Visual provider secret directory could not be synced."))
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        io::Cursor,
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };

    use forgecad_core::{
        ConceptImageBackend, ConceptImageGenerationRequest,
        CONCEPT_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION,
    };
    use sha2::{Digest, Sha256};

    use super::*;

    struct StaticCredential;

    impl FalCredentialSource for StaticCredential {
        fn load(&self) -> Result<Option<VisualProviderSecret>, ConceptImageProviderError> {
            Ok(Some(VisualProviderSecret::new("test-secret".into())?))
        }
    }

    struct FakeSink;

    impl ConceptImageObjectSink for FakeSink {
        fn accept_png(
            &self,
            bytes: Vec<u8>,
            content_type: Option<&str>,
        ) -> Result<ConceptImageOutputHandle, ConceptImageProviderError> {
            if content_type != Some("image/png") || bytes != b"PNG_BYTES" {
                return Err(protocol_error("Fake PNG sink rejected bytes."));
            }
            Ok(ConceptImageOutputHandle {
                image_object_sha256: format!("{:x}", Sha256::digest(&bytes)),
                byte_size: bytes.len() as u64,
                media_type: "image/png".into(),
                width: 1024,
                height: 1024,
                safety_passed: true,
            })
        }
    }

    struct FakeInputSource;

    impl ConceptInputImageSource for FakeInputSource {
        fn read_image(
            &self,
            sha256: &str,
        ) -> Result<(Vec<u8>, &'static str), ConceptImageProviderError> {
            if sha256 != "a".repeat(64) {
                return Err(protocol_error("Fake input image hash mismatch."));
            }
            Ok((b"AUTHORIZED_IMAGE".to_vec(), "image/png"))
        }
    }

    struct FakeTransport {
        responses: Arc<Mutex<VecDeque<VisualHttpResponse>>>,
        requests: Arc<Mutex<Vec<VisualHttpRequest>>>,
    }

    impl FakeTransport {
        fn new(responses: Vec<VisualHttpResponse>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses.into())),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl VisualHttpTransport for FakeTransport {
        fn execute(&self, request: VisualHttpRequest) -> VisualHttpFuture<VisualHttpResponse> {
            let responses = self.responses.clone();
            let requests = self.requests.clone();
            Box::pin(async move {
                requests.lock().unwrap().push(request);
                responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .ok_or_else(|| protocol_error("Fake transport response script was exhausted."))
            })
        }
    }

    fn json_response(value: Value) -> VisualHttpResponse {
        VisualHttpResponse {
            status: 200,
            content_type: Some("application/json".into()),
            body: serde_json::to_vec(&value).unwrap(),
            network_call_made: true,
        }
    }

    fn request() -> ConceptImageGenerationRequest {
        ConceptImageGenerationRequest {
            schema_version: CONCEPT_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION.into(),
            request_id: "concept_request_1".into(),
            project_id: "project_1".into(),
            turn_id: "turn_1".into(),
            brief_id: "brief_1".into(),
            prompt: "One isolated fictional mechanical collectible on a clean neutral background."
                .into(),
            input_image_object_sha256: None,
            input_image_media_type: None,
            backend_preferences: vec![ConceptImageBackend::FalFlux2],
            width: 1024,
            height: 1024,
            output_media_type: "image/png".into(),
            isolated_subject: true,
            clean_background: true,
            image_count: 1,
            idempotency_key: "concept_key_1".into(),
        }
    }

    fn run_async<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn fal_submit_builds_one_bounded_flux_request_and_redacts_secret() {
        run_async(async {
            let transport = Arc::new(FakeTransport::new(vec![json_response(json!({
                "request_id": "fal-request-1"
            }))]));
            let adapter = FalFlux2ConceptImageAdapter::new(
                transport.clone(),
                Arc::new(StaticCredential),
                Arc::new(FakeSink),
            );
            let receipt = adapter
                .submit(request(), ConceptImageBackend::FalFlux2)
                .await
                .unwrap();
            assert_eq!(receipt.provider_job_id, "fal-request-1");
            let requests = transport.requests.lock().unwrap();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].method, VisualHttpMethod::Post);
            assert_eq!(requests[0].endpoint.path(), FAL_FLUX_2_MODEL_PATH);
            let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
            assert_eq!(body["num_images"], json!(1));
            assert_eq!(body["enable_safety_checker"], json!(true));
            assert!(!format!("{:?}", requests[0]).contains("test-secret"));
        });
    }

    #[test]
    fn fal_edit_submit_embeds_only_the_exact_authorized_cas_image() {
        run_async(async {
            let transport = Arc::new(FakeTransport::new(vec![json_response(json!({
                "request_id": "fal-edit-request-1"
            }))]));
            let adapter = FalFlux2ConceptImageAdapter::new_edit(
                transport.clone(),
                Arc::new(StaticCredential),
                Arc::new(FakeSink),
                Arc::new(FakeInputSource),
            );
            let mut edit = request();
            edit.input_image_object_sha256 = Some("a".repeat(64));
            edit.input_image_media_type = Some("image/png".into());
            adapter
                .submit(edit, ConceptImageBackend::FalFlux2)
                .await
                .unwrap();

            let requests = transport.requests.lock().unwrap();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].endpoint.path(), FAL_FLUX_2_EDIT_MODEL_PATH);
            let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
            assert_eq!(body["image_urls"].as_array().unwrap().len(), 1);
            assert_eq!(
                body["image_urls"][0],
                format!(
                    "data:image/png;base64,{}",
                    BASE64_STANDARD.encode(b"AUTHORIZED_IMAGE")
                )
            );
            let debug = format!("{:?}", requests[0]);
            assert!(!debug.contains("AUTHORIZED_IMAGE"));
            assert!(!debug.contains("test-secret"));
        });
    }

    #[test]
    fn fal_completed_result_downloads_allowlisted_png_and_returns_hash_handle() {
        run_async(async {
            let transport = Arc::new(FakeTransport::new(vec![
                json_response(json!({"status": "COMPLETED", "request_id": "fal-request-1"})),
                json_response(json!({
                    "images": [{
                        "url": "https://v3.fal.media/files/test/concept.png",
                        "width": 1024,
                        "height": 1024,
                        "content_type": "image/png"
                    }],
                    "has_nsfw_concepts": [false],
                    "seed": 42,
                    "prompt": "redacted by adapter"
                })),
                VisualHttpResponse {
                    status: 200,
                    content_type: Some("image/png".into()),
                    body: b"PNG_BYTES".to_vec(),
                    network_call_made: true,
                },
            ]));
            let adapter = FalFlux2ConceptImageAdapter::new(
                transport.clone(),
                Arc::new(StaticCredential),
                Arc::new(FakeSink),
            );
            let status = adapter
                .poll(ConceptImageProviderReceipt {
                    schema_version: CONCEPT_IMAGE_PROVIDER_RECEIPT_SCHEMA_VERSION.into(),
                    backend: ConceptImageBackend::FalFlux2,
                    provider_job_id: "fal-request-1".into(),
                })
                .await
                .unwrap();
            let ConceptImageProviderStatus::Ready { output } = status else {
                panic!("expected ready");
            };
            assert_eq!(output.byte_size, 9);
            let requests = transport.requests.lock().unwrap();
            assert_eq!(requests.len(), 3);
            assert!(requests[2].authorization.is_none());
            assert_eq!(requests[2].endpoint.host_str(), Some("v3.fal.media"));
        });
    }

    #[test]
    fn fal_rejects_unsafe_result_and_untrusted_media_host() {
        run_async(async {
            let unsafe_transport = Arc::new(FakeTransport::new(vec![
                json_response(json!({"status": "COMPLETED"})),
                json_response(json!({
                    "images": [{
                        "url": "https://v3.fal.media/files/test/concept.png",
                        "width": 1024,
                        "height": 1024,
                        "content_type": "image/png"
                    }],
                    "has_nsfw_concepts": [true]
                })),
            ]));
            let unsafe_adapter = FalFlux2ConceptImageAdapter::new(
                unsafe_transport,
                Arc::new(StaticCredential),
                Arc::new(FakeSink),
            );
            assert!(matches!(
                unsafe_adapter
                    .poll(ConceptImageProviderReceipt {
                        schema_version: CONCEPT_IMAGE_PROVIDER_RECEIPT_SCHEMA_VERSION.into(),
                        backend: ConceptImageBackend::FalFlux2,
                        provider_job_id: "fal-request-1".into(),
                    })
                    .await
                    .unwrap(),
                ConceptImageProviderStatus::Failed { ref code }
                    if code == "CONCEPT_IMAGE_SAFETY_REJECTED"
            ));

            assert_eq!(
                validate_fal_media_url("https://attacker.example/concept.png")
                    .unwrap_err()
                    .code,
                "FAL_PROTOCOL_INVALID"
            );
        });
    }

    #[test]
    fn fal_cancel_uses_put_and_requires_acknowledgement() {
        run_async(async {
            let transport = Arc::new(FakeTransport::new(vec![VisualHttpResponse {
                status: 202,
                content_type: Some("application/json".into()),
                body: serde_json::to_vec(&json!({
                    "status": "CANCELLATION_REQUESTED"
                }))
                .unwrap(),
                network_call_made: true,
            }]));
            let adapter = FalFlux2ConceptImageAdapter::new(
                transport.clone(),
                Arc::new(StaticCredential),
                Arc::new(FakeSink),
            );
            adapter
                .cancel(ConceptImageProviderReceipt {
                    schema_version: CONCEPT_IMAGE_PROVIDER_RECEIPT_SCHEMA_VERSION.into(),
                    backend: ConceptImageBackend::FalFlux2,
                    provider_job_id: "fal-request-1".into(),
                })
                .await
                .unwrap();
            assert_eq!(
                transport.requests.lock().unwrap()[0].method,
                VisualHttpMethod::Put
            );
        });
    }

    #[test]
    fn production_transport_rejects_non_https_before_network() {
        run_async(async {
            let transport = ReqwestVisualHttpTransport::new().unwrap();
            let error = transport
                .execute(VisualHttpRequest {
                    method: VisualHttpMethod::Get,
                    endpoint: Url::parse("http://example.invalid/image.png").unwrap(),
                    authorization: None,
                    content_type: None,
                    body: Arc::from([]),
                    max_response_bytes: 1024,
                    timeout: Duration::from_secs(1),
                })
                .await
                .unwrap_err();
            assert_eq!(error.code, "VISUAL_HTTP_TRANSPORT_FAILED");
        });
    }

    #[test]
    fn core_sink_decodes_and_commits_exact_png_to_repository_cas() {
        let serial = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "forgecad-concept-png-sink-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let repository = Arc::new(
            CoreRepository::open(
                root.join("state.sqlite3"),
                root.join("library"),
                format!("concept_sink_test_{serial}"),
            )
            .unwrap(),
        );
        repository.publish().unwrap();
        let image = image::RgbaImage::from_pixel(1024, 1024, image::Rgba([4, 18, 40, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        let sink = CoreConceptImageObjectSink::new(
            repository.clone(),
            "concept_reference_test_1".into(),
            "2026-07-26T00:00:00.000Z".into(),
        );
        let output = sink
            .accept_png(bytes.clone(), Some("image/png; charset=binary"))
            .unwrap();
        assert_eq!(
            repository.read_object(&output.image_object_sha256).unwrap(),
            bytes
        );
        assert_eq!((output.width, output.height), (1024, 1024));
        let source = CoreConceptInputImageSource::new(repository.clone());
        let (authorized, media_type) = source.read_image(&output.image_object_sha256).unwrap();
        assert_eq!(authorized, bytes);
        assert_eq!(media_type, "image/png");

        drop(sink);
        drop(repository);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn private_file_credential_is_prompt_free_private_and_redacted() {
        let serial = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "forgecad-visual-secret-{}-{serial}",
            std::process::id()
        ));
        let source = PrivateFileFalCredentialSource::new(root.join("fal.key"));
        assert!(!source.configured().unwrap());
        source.save("test-only-secret".into()).unwrap();
        assert!(source.configured().unwrap());
        assert_eq!(
            source
                .load()
                .unwrap()
                .unwrap()
                .authorization_value()
                .as_str(),
            "Key test-only-secret"
        );
        assert!(!format!("{source:?}").contains("test-only-secret"));
        assert!(!format!("{source:?}").contains(root.to_string_lossy().as_ref()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(root.join("fal.key"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o077,
                0
            );
        }
        source.delete().unwrap();
        assert!(!source.configured().unwrap());
        fs::remove_dir_all(root).unwrap();
    }
}
