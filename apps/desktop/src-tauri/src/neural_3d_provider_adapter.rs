//! Fal Hunyuan3D v3.1 Pro adapter for Forge Studio's first real Image-to-3D path.
//!
//! The adapter accepts only a Rust-validated concept-image CAS digest, embeds
//! the exact PNG as a bounded data URI, requests one textured PBR GLB and
//! downloads only an allowlisted HTTPS result. Provider JSON and URLs stop at
//! this boundary; the returned handle describes bytes already accepted by a
//! Rust-owned GLB sink.

use std::{sync::Arc, time::Duration};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use forgecad_app_server::{
    NeuralVisualProviderArtifactHandle, NeuralVisualProviderError, NeuralVisualProviderFuture,
    NeuralVisualProviderPort, NeuralVisualProviderReceipt, NeuralVisualProviderStatus,
    NEURAL_VISUAL_PROVIDER_RECEIPT_SCHEMA_VERSION,
};
use forgecad_core::{
    inspect_concept_png, inspect_neural_visual_glb, CoreRepository, Neural3DBackend,
    Neural3DGenerationRequest, ObjectReference, VisualQualityTier,
};
use reqwest::Url;
use serde_json::{json, Value};

use crate::visual_provider_adapters::{
    FalCredentialSource, VisualHttpMethod, VisualHttpRequest, VisualHttpResponse,
    VisualHttpTransport, VisualProviderSecret, MAX_VISUAL_HTTP_RESPONSE_BYTES,
};

const FAL_QUEUE_ORIGIN: &str = "https://queue.fal.run";
const HUNYUAN_V31_PRO_PATH: &str = "/fal-ai/hunyuan-3d/v3.1/pro/image-to-3d";
const MAX_FAL_JSON_BYTES: usize = 1024 * 1024;
const MAX_HUNYUAN_INPUT_PNG_BYTES: usize = 8 * 1024 * 1024;
const MAX_NEURAL_GLB_BYTES: usize = MAX_VISUAL_HTTP_RESPONSE_BYTES;

pub trait ConceptImageSource: Send + Sync + 'static {
    fn read_png(&self, sha256: &str) -> Result<Vec<u8>, NeuralVisualProviderError>;
}

pub trait NeuralGlbObjectSink: Send + Sync + 'static {
    fn accept_glb(
        &self,
        bytes: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<NeuralVisualProviderArtifactHandle, NeuralVisualProviderError>;
}

pub struct CoreConceptImageSource {
    repository: Arc<CoreRepository>,
}

impl CoreConceptImageSource {
    pub fn new(repository: Arc<CoreRepository>) -> Self {
        Self { repository }
    }
}

impl ConceptImageSource for CoreConceptImageSource {
    fn read_png(&self, sha256: &str) -> Result<Vec<u8>, NeuralVisualProviderError> {
        let bytes = self.repository.read_object(sha256).map_err(|error| {
            provider_error(
                "NEURAL_CONCEPT_CAS_READ_FAILED",
                format!("Concept image CAS read failed: {}", error.code()),
            )
        })?;
        let inspection = inspect_concept_png(&bytes).map_err(|error| {
            provider_error(
                "NEURAL_CONCEPT_CAS_INVALID",
                format!("Concept image CAS object failed readback: {}", error.code()),
            )
        })?;
        if inspection.sha256 != sha256 {
            return Err(provider_error(
                "NEURAL_CONCEPT_CAS_MISMATCH",
                "Concept image CAS bytes do not match the requested digest.",
            ));
        }
        Ok(bytes)
    }
}

pub struct CoreNeuralGlbObjectSink {
    repository: Arc<CoreRepository>,
    owner_id: String,
    timestamp: String,
}

impl CoreNeuralGlbObjectSink {
    pub fn new(repository: Arc<CoreRepository>, owner_id: String, timestamp: String) -> Self {
        Self {
            repository,
            owner_id,
            timestamp,
        }
    }
}

impl NeuralGlbObjectSink for CoreNeuralGlbObjectSink {
    fn accept_glb(
        &self,
        bytes: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<NeuralVisualProviderArtifactHandle, NeuralVisualProviderError> {
        if content_type.and_then(media_type_essence) != Some("model/gltf-binary") {
            return Err(provider_error(
                "NEURAL_GLB_MEDIA_TYPE_INVALID",
                "Downloaded neural artifact is not declared as model/gltf-binary.",
            ));
        }
        let inspection = inspect_neural_visual_glb(&bytes).map_err(|error| {
            provider_error(
                "NEURAL_GLB_READBACK_REJECTED",
                format!("Neural GLB failed Rust readback: {}", error.code()),
            )
        })?;
        let record = self
            .repository
            .attach_object_bytes(
                &ObjectReference {
                    reference_kind: "candidate".into(),
                    owner_id: self.owner_id.clone(),
                    role: "raw_neural_visual_glb".into(),
                },
                &bytes,
                "glb",
                &self.timestamp,
            )
            .map_err(|error| {
                provider_error(
                    "NEURAL_GLB_CAS_REJECTED",
                    format!("Neural GLB CAS commit failed: {}", error.code()),
                )
            })?;
        if record.sha256 != inspection.sha256 || record.byte_size != inspection.byte_size {
            return Err(provider_error(
                "NEURAL_GLB_CAS_READBACK_MISMATCH",
                "Neural GLB CAS record does not match inspected bytes.",
            ));
        }
        Ok(NeuralVisualProviderArtifactHandle {
            artifact_handle_id: format!("neural_glb_{}", &record.sha256[..24]),
            glb_sha256: record.sha256,
            glb_byte_size: record.byte_size,
        })
    }
}

#[derive(Clone)]
pub struct FalHunyuan3dV31ProAdapter {
    transport: Arc<dyn VisualHttpTransport>,
    credentials: Arc<dyn FalCredentialSource>,
    concept_source: Arc<dyn ConceptImageSource>,
    glb_sink: Arc<dyn NeuralGlbObjectSink>,
}

impl FalHunyuan3dV31ProAdapter {
    pub fn new(
        transport: Arc<dyn VisualHttpTransport>,
        credentials: Arc<dyn FalCredentialSource>,
        concept_source: Arc<dyn ConceptImageSource>,
        glb_sink: Arc<dyn NeuralGlbObjectSink>,
    ) -> Self {
        Self {
            transport,
            credentials,
            concept_source,
            glb_sink,
        }
    }

    fn credential(&self) -> Result<VisualProviderSecret, NeuralVisualProviderError> {
        self.credentials
            .load()
            .map_err(|_| {
                provider_error(
                    "NEURAL_PROVIDER_CREDENTIAL_FAILED",
                    "Fal credential could not be loaded.",
                )
            })?
            .ok_or_else(|| {
                provider_error(
                    "NEURAL_PROVIDER_NOT_CONFIGURED",
                    "Fal neural 3D provider is not configured.",
                )
            })
    }
}

impl NeuralVisualProviderPort for FalHunyuan3dV31ProAdapter {
    fn submit(
        &self,
        request: Neural3DGenerationRequest,
        backend: Neural3DBackend,
    ) -> NeuralVisualProviderFuture<NeuralVisualProviderReceipt> {
        let transport = self.transport.clone();
        let source = self.concept_source.clone();
        let credential = self.credential();
        Box::pin(async move {
            request.validate().map_err(|error| {
                provider_error(
                    "NEURAL_PROVIDER_REQUEST_INVALID",
                    format!("Neural request failed Rust validation: {}", error.code()),
                )
            })?;
            if backend != Neural3DBackend::Hunyuan3dV31Pro
                || !request.backend_preferences.contains(&backend)
            {
                return Err(protocol_error(
                    "Hunyuan adapter received a different neural backend.",
                ));
            }
            let bytes = source.read_png(&request.concept_reference_sha256)?;
            let inspection = inspect_concept_png(&bytes).map_err(|error| {
                provider_error(
                    "NEURAL_CONCEPT_IMAGE_INVALID",
                    format!("Concept image failed Rust validation: {}", error.code()),
                )
            })?;
            if inspection.sha256 != request.concept_reference_sha256
                || bytes.len() > MAX_HUNYUAN_INPUT_PNG_BYTES
            {
                return Err(provider_error(
                    "NEURAL_CONCEPT_IMAGE_MISMATCH",
                    "Concept image does not match the requested digest or provider byte limit.",
                ));
            }
            let image_data_uri =
                format!("data:image/png;base64,{}", BASE64_STANDARD.encode(&bytes));
            let body = serde_json::to_vec(&json!({
                "input_image_url": image_data_uri,
                "generate_type": "Normal",
                "enable_pbr": true,
                "face_count": face_count(request.quality_tier)
            }))
            .map_err(|_| protocol_error("Hunyuan submit body could not be encoded."))?;
            let response = transport
                .execute(fal_request(
                    VisualHttpMethod::Post,
                    HUNYUAN_V31_PRO_PATH,
                    Some(credential?),
                    Some("application/json"),
                    body,
                    MAX_FAL_JSON_BYTES,
                )?)
                .await
                .map_err(transport_error)?;
            require_json(&response)?;
            let value = parse_json(&response)?;
            let request_id = bounded_id(
                value.get("request_id").and_then(Value::as_str),
                "Hunyuan submit response has no valid request_id.",
            )?;
            Ok(NeuralVisualProviderReceipt {
                schema_version: NEURAL_VISUAL_PROVIDER_RECEIPT_SCHEMA_VERSION.into(),
                backend,
                provider_job_id: request_id,
            })
        })
    }

    fn poll(
        &self,
        receipt: NeuralVisualProviderReceipt,
    ) -> NeuralVisualProviderFuture<NeuralVisualProviderStatus> {
        let transport = self.transport.clone();
        let sink = self.glb_sink.clone();
        let credential = self.credential();
        Box::pin(async move {
            validate_receipt(&receipt)?;
            let credential = credential?;
            let status_path = format!(
                "{HUNYUAN_V31_PRO_PATH}/requests/{}/status",
                receipt.provider_job_id
            );
            let status_response = transport
                .execute(fal_request(
                    VisualHttpMethod::Get,
                    &status_path,
                    Some(credential.clone()),
                    None,
                    Vec::new(),
                    MAX_FAL_JSON_BYTES,
                )?)
                .await
                .map_err(transport_error)?;
            require_json(&status_response)?;
            let status = parse_json(&status_response)?;
            match status.get("status").and_then(Value::as_str) {
                Some("IN_QUEUE") => Ok(NeuralVisualProviderStatus::Queued),
                Some("IN_PROGRESS") => Ok(NeuralVisualProviderStatus::Running),
                Some("COMPLETED") => {
                    if status.get("error").is_some_and(|value| !value.is_null()) {
                        return Ok(NeuralVisualProviderStatus::Failed {
                            code: "HUNYUAN_GENERATION_FAILED".into(),
                        });
                    }
                    let result_path = format!(
                        "{HUNYUAN_V31_PRO_PATH}/requests/{}",
                        receipt.provider_job_id
                    );
                    let result_response = transport
                        .execute(fal_request(
                            VisualHttpMethod::Get,
                            &result_path,
                            Some(credential),
                            None,
                            Vec::new(),
                            MAX_FAL_JSON_BYTES,
                        )?)
                        .await
                        .map_err(transport_error)?;
                    require_json(&result_response)?;
                    let result = parse_json(&result_response)?;
                    let glb = result
                        .get("model_glb")
                        .and_then(Value::as_object)
                        .ok_or_else(|| protocol_error("Hunyuan result has no model_glb object."))?;
                    if glb.get("content_type").and_then(Value::as_str) != Some("model/gltf-binary")
                    {
                        return Err(protocol_error(
                            "Hunyuan result is not declared as model/gltf-binary.",
                        ));
                    }
                    let expected_size = glb
                        .get("file_size")
                        .and_then(Value::as_u64)
                        .filter(|size| *size > 0 && *size <= MAX_NEURAL_GLB_BYTES as u64)
                        .ok_or_else(|| {
                            protocol_error("Hunyuan GLB size is outside the reviewed limit.")
                        })?;
                    let media_url = validate_media_url(
                        glb.get("url")
                            .and_then(Value::as_str)
                            .ok_or_else(|| protocol_error("Hunyuan GLB URL is missing."))?,
                    )?;
                    let media_response = transport
                        .execute(VisualHttpRequest {
                            method: VisualHttpMethod::Get,
                            endpoint: media_url,
                            authorization: None,
                            content_type: None,
                            body: Arc::from([]),
                            max_response_bytes: MAX_NEURAL_GLB_BYTES,
                            timeout: Duration::from_secs(60),
                        })
                        .await
                        .map_err(transport_error)?;
                    if media_response.status != 200 {
                        return Err(http_error(media_response.status));
                    }
                    if media_response.body.len() as u64 != expected_size {
                        return Err(protocol_error(
                            "Downloaded Hunyuan GLB size does not match provider metadata.",
                        ));
                    }
                    let artifact = sink
                        .accept_glb(media_response.body, media_response.content_type.as_deref())?;
                    Ok(NeuralVisualProviderStatus::Ready { artifact })
                }
                _ => Err(protocol_error(
                    "Hunyuan status response has an unknown state.",
                )),
            }
        })
    }

    fn cancel(&self, receipt: NeuralVisualProviderReceipt) -> NeuralVisualProviderFuture<()> {
        let transport = self.transport.clone();
        let credential = self.credential();
        Box::pin(async move {
            validate_receipt(&receipt)?;
            let path = format!(
                "{HUNYUAN_V31_PRO_PATH}/requests/{}/cancel",
                receipt.provider_job_id
            );
            let response = transport
                .execute(fal_request(
                    VisualHttpMethod::Put,
                    &path,
                    Some(credential?),
                    None,
                    Vec::new(),
                    MAX_FAL_JSON_BYTES,
                )?)
                .await
                .map_err(transport_error)?;
            if response.status != 202 {
                return Err(http_error(response.status));
            }
            let value = parse_json(&response)?;
            if value.get("status").and_then(Value::as_str) != Some("CANCELLATION_REQUESTED") {
                return Err(protocol_error(
                    "Hunyuan cancellation did not confirm cancellation.",
                ));
            }
            Ok(())
        })
    }
}

fn face_count(tier: VisualQualityTier) -> u64 {
    match tier {
        VisualQualityTier::FastPreview => 100_000,
        VisualQualityTier::StandardAsset => 250_000,
        VisualQualityTier::CollectibleAsset => 500_000,
    }
}

fn fal_request(
    method: VisualHttpMethod,
    path: &str,
    authorization: Option<VisualProviderSecret>,
    content_type: Option<&'static str>,
    body: Vec<u8>,
    max_response_bytes: usize,
) -> Result<VisualHttpRequest, NeuralVisualProviderError> {
    if !path.starts_with(HUNYUAN_V31_PRO_PATH)
        || path.contains("..")
        || path.contains('?')
        || path.len() > 512
    {
        return Err(protocol_error(
            "Hunyuan request path is outside the reviewed route.",
        ));
    }
    let endpoint = Url::parse(&format!("{FAL_QUEUE_ORIGIN}{path}"))
        .map_err(|_| protocol_error("Hunyuan endpoint could not be constructed."))?;
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

fn validate_receipt(
    receipt: &NeuralVisualProviderReceipt,
) -> Result<(), NeuralVisualProviderError> {
    if receipt.schema_version != NEURAL_VISUAL_PROVIDER_RECEIPT_SCHEMA_VERSION
        || receipt.backend != Neural3DBackend::Hunyuan3dV31Pro
    {
        return Err(protocol_error(
            "Hunyuan receipt schema or backend is invalid.",
        ));
    }
    bounded_id(
        Some(&receipt.provider_job_id),
        "Hunyuan receipt has no valid provider job ID.",
    )
    .map(|_| ())
}

fn require_json(response: &VisualHttpResponse) -> Result<(), NeuralVisualProviderError> {
    if !(200..300).contains(&response.status) {
        return Err(http_error(response.status));
    }
    if response.body.is_empty()
        || response.body.len() > MAX_FAL_JSON_BYTES
        || response
            .content_type
            .as_deref()
            .and_then(media_type_essence)
            != Some("application/json")
    {
        return Err(protocol_error(
            "Hunyuan response is not bounded application/json.",
        ));
    }
    Ok(())
}

fn parse_json(response: &VisualHttpResponse) -> Result<Value, NeuralVisualProviderError> {
    serde_json::from_slice(&response.body)
        .map_err(|_| protocol_error("Hunyuan response JSON is invalid."))
}

fn bounded_id(
    value: Option<&str>,
    message: &'static str,
) -> Result<String, NeuralVisualProviderError> {
    let value = value.ok_or_else(|| protocol_error(message))?;
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(protocol_error(message));
    }
    Ok(value.to_owned())
}

fn validate_media_url(value: &str) -> Result<Url, NeuralVisualProviderError> {
    if value.len() > 4_096 {
        return Err(protocol_error(
            "Hunyuan media URL exceeds the reviewed bound.",
        ));
    }
    let url = Url::parse(value).map_err(|_| protocol_error("Hunyuan media URL is invalid."))?;
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
            "Hunyuan media URL is outside the reviewed HTTPS allowlist.",
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

fn http_error(status: u16) -> NeuralVisualProviderError {
    let code = match status {
        401 | 403 => "HUNYUAN_AUTHENTICATION_FAILED",
        402 => "HUNYUAN_BALANCE_REQUIRED",
        429 => "HUNYUAN_RATE_LIMITED",
        500..=599 => "HUNYUAN_SERVER_UNAVAILABLE",
        _ => "HUNYUAN_HTTP_FAILED",
    };
    provider_error(code, "Hunyuan returned an unsuccessful HTTP status.")
}

fn protocol_error(message: &'static str) -> NeuralVisualProviderError {
    provider_error("HUNYUAN_PROTOCOL_INVALID", message)
}

fn transport_error(_: forgecad_app_server::ConceptImageProviderError) -> NeuralVisualProviderError {
    provider_error(
        "HUNYUAN_HTTP_TRANSPORT_FAILED",
        "Hunyuan HTTPS transport failed.",
    )
}

fn provider_error(code: &'static str, message: impl Into<String>) -> NeuralVisualProviderError {
    NeuralVisualProviderError::new(code, message)
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, future::Future, io::Cursor, sync::Mutex};

    use forgecad_core::{
        Neural3DBackend, Neural3DGenerationRequest, NEURAL_3D_GENERATION_REQUEST_SCHEMA_VERSION,
    };
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::visual_provider_adapters::{
        VisualHttpFuture, VisualHttpRequest, VisualHttpResponse,
    };

    struct StaticCredential;

    impl FalCredentialSource for StaticCredential {
        fn load(
            &self,
        ) -> Result<Option<VisualProviderSecret>, forgecad_app_server::ConceptImageProviderError>
        {
            Ok(Some(
                VisualProviderSecret::new("test-only-fal-key".into()).unwrap(),
            ))
        }
    }

    struct StaticConcept {
        bytes: Vec<u8>,
    }

    impl ConceptImageSource for StaticConcept {
        fn read_png(&self, sha256: &str) -> Result<Vec<u8>, NeuralVisualProviderError> {
            let actual = format!("{:x}", Sha256::digest(&self.bytes));
            if sha256 != actual {
                return Err(provider_error(
                    "TEST_CONCEPT_MISMATCH",
                    "Test concept digest does not match.",
                ));
            }
            Ok(self.bytes.clone())
        }
    }

    struct FakeGlbSink;

    impl NeuralGlbObjectSink for FakeGlbSink {
        fn accept_glb(
            &self,
            bytes: Vec<u8>,
            content_type: Option<&str>,
        ) -> Result<NeuralVisualProviderArtifactHandle, NeuralVisualProviderError> {
            if content_type.and_then(media_type_essence) != Some("model/gltf-binary")
                || bytes != b"GLB_BYTES"
            {
                return Err(protocol_error("Fake GLB sink rejected bytes."));
            }
            let sha = format!("{:x}", Sha256::digest(&bytes));
            Ok(NeuralVisualProviderArtifactHandle {
                artifact_handle_id: format!("neural_glb_{}", &sha[..24]),
                glb_sha256: sha,
                glb_byte_size: bytes.len() as u64,
            })
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
                responses.lock().unwrap().pop_front().ok_or_else(|| {
                    forgecad_app_server::ConceptImageProviderError::new(
                        "TEST_TRANSPORT_EXHAUSTED",
                        "Fake response script exhausted.",
                    )
                })
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

    fn concept_png() -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(1024, 1024, image::Rgba([5, 15, 35, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn request(png: &[u8]) -> Neural3DGenerationRequest {
        Neural3DGenerationRequest {
            schema_version: NEURAL_3D_GENERATION_REQUEST_SCHEMA_VERSION.into(),
            request_id: "neural_request_1".into(),
            project_id: "project_1".into(),
            turn_id: "turn_1".into(),
            brief_id: "brief_1".into(),
            concept_reference_id: "concept_reference_1".into(),
            concept_reference_sha256: format!("{:x}", Sha256::digest(png)),
            quality_tier: VisualQualityTier::StandardAsset,
            backend_preferences: vec![Neural3DBackend::Hunyuan3dV31Pro],
            idempotency_key: "neural_key_1".into(),
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
    fn submit_embeds_exact_concept_and_requests_pbr_standard_asset() {
        run_async(async {
            let png = concept_png();
            let transport = Arc::new(FakeTransport::new(vec![json_response(json!({
                "request_id": "hunyuan-job-1"
            }))]));
            let adapter = FalHunyuan3dV31ProAdapter::new(
                transport.clone(),
                Arc::new(StaticCredential),
                Arc::new(StaticConcept { bytes: png.clone() }),
                Arc::new(FakeGlbSink),
            );
            let receipt = adapter
                .submit(request(&png), Neural3DBackend::Hunyuan3dV31Pro)
                .await
                .unwrap();
            assert_eq!(receipt.provider_job_id, "hunyuan-job-1");
            let requests = transport.requests.lock().unwrap();
            let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
            assert_eq!(body["enable_pbr"], json!(true));
            assert_eq!(body["generate_type"], json!("Normal"));
            assert_eq!(body["face_count"], json!(250_000));
            assert!(body["input_image_url"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,"));
            assert!(!format!("{:?}", requests[0]).contains("test-only-fal-key"));
        });
    }

    #[test]
    fn completed_job_downloads_one_exact_glb_without_forwarding_secret() {
        run_async(async {
            let png = concept_png();
            let transport = Arc::new(FakeTransport::new(vec![
                json_response(json!({"status": "COMPLETED"})),
                json_response(json!({
                    "model_glb": {
                        "content_type": "model/gltf-binary",
                        "file_size": 9,
                        "url": "https://v3b.fal.media/files/test/model.glb"
                    }
                })),
                VisualHttpResponse {
                    status: 200,
                    content_type: Some("model/gltf-binary".into()),
                    body: b"GLB_BYTES".to_vec(),
                    network_call_made: true,
                },
            ]));
            let adapter = FalHunyuan3dV31ProAdapter::new(
                transport.clone(),
                Arc::new(StaticCredential),
                Arc::new(StaticConcept { bytes: png }),
                Arc::new(FakeGlbSink),
            );
            let status = adapter
                .poll(NeuralVisualProviderReceipt {
                    schema_version: NEURAL_VISUAL_PROVIDER_RECEIPT_SCHEMA_VERSION.into(),
                    backend: Neural3DBackend::Hunyuan3dV31Pro,
                    provider_job_id: "hunyuan-job-1".into(),
                })
                .await
                .unwrap();
            assert!(matches!(status, NeuralVisualProviderStatus::Ready { .. }));
            let requests = transport.requests.lock().unwrap();
            assert_eq!(requests.len(), 3);
            assert!(requests[2].authorization.is_none());
        });
    }
}
