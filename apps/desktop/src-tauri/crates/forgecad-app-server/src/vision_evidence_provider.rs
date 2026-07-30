//! Transport-neutral visual-evidence Provider boundary.
//!
//! A concrete adapter may send authorized images to an external vision model,
//! but it returns only bounded claims. Rust constructs and validates the final
//! `VisualEvidenceGraph@1`; the Provider cannot author product state, geometry,
//! filesystem paths, endpoints or credentials.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use forgecad_core::{
    semantic_sha256, MultimodalDesignRequest, ReferenceEvidence, ReferenceEvidenceKind,
    VisionEvidenceProviderProvenance, VisualEvidenceClaim, VisualEvidenceGraph,
    VISUAL_EVIDENCE_GRAPH_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::CancellationToken;

pub const MAX_VISION_EVIDENCE_IMAGES: usize = 12;
pub const MAX_VISION_EVIDENCE_IMAGE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_VISION_EVIDENCE_TOTAL_BYTES: usize = 48 * 1024 * 1024;
pub const MAX_VISION_EVIDENCE_RESPONSE_BYTES: usize = 1024 * 1024;

pub type VisionEvidenceProviderFuture = Pin<
    Box<
        dyn Future<Output = Result<VisionEvidenceProviderOutput, VisionEvidenceProviderError>>
            + Send
            + 'static,
    >,
>;

#[derive(Clone, PartialEq, Eq)]
pub struct VisionEvidenceImage {
    pub evidence_id: String,
    pub media_type: String,
    pub bytes: Arc<[u8]>,
}

impl fmt::Debug for VisionEvidenceImage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VisionEvidenceImage")
            .field("evidence_id", &self.evidence_id)
            .field("media_type", &self.media_type)
            .field("byte_size", &self.bytes.len())
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone)]
pub struct VisionEvidenceProviderRequest {
    pub request: MultimodalDesignRequest,
    pub evidence: Vec<ReferenceEvidence>,
    pub images: Vec<VisionEvidenceImage>,
}

impl fmt::Debug for VisionEvidenceProviderRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VisionEvidenceProviderRequest")
            .field("request_id", &self.request.request_id)
            .field("reference_count", &self.request.reference_inputs.len())
            .field("sealed_evidence_count", &self.evidence.len())
            .field("image_count", &self.images.len())
            .field("instruction", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisionEvidenceProviderOutput {
    pub provider_id: String,
    pub model_id: String,
    pub provider_response_sha256: String,
    pub analyzed_at: String,
    pub claims: Vec<VisualEvidenceClaim>,
    pub network_call_made: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionEvidenceProviderError {
    pub code: &'static str,
    pub message: String,
    pub network_call_made: bool,
    pub retryable: bool,
}

impl VisionEvidenceProviderError {
    pub fn new(
        code: &'static str,
        message: impl Into<String>,
        network_call_made: bool,
        retryable: bool,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            network_call_made,
            retryable,
        }
    }
}

pub trait VisionEvidenceProviderPort: Send + Sync + 'static {
    fn analyze(
        &self,
        request: VisionEvidenceProviderRequest,
        cancellation: CancellationToken,
    ) -> VisionEvidenceProviderFuture;
}

pub struct VisionEvidenceCoordinator {
    provider: Arc<dyn VisionEvidenceProviderPort>,
    timeout: Duration,
}

impl VisionEvidenceCoordinator {
    pub fn new(
        provider: Arc<dyn VisionEvidenceProviderPort>,
        timeout: Duration,
    ) -> Result<Self, VisionEvidenceProviderError> {
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(error(
                "VISION_EVIDENCE_TIMEOUT_INVALID",
                "Vision evidence timeout must be between one millisecond and two minutes.",
                false,
                false,
            ));
        }
        Ok(Self { provider, timeout })
    }

    pub async fn analyze(
        &self,
        input: VisionEvidenceProviderRequest,
        cancellation: CancellationToken,
    ) -> Result<VisualEvidenceGraph, VisionEvidenceProviderError> {
        validate_input(&input)?;
        if cancellation.is_cancelled() {
            return Err(error(
                "VISION_EVIDENCE_CANCELLED",
                "Vision evidence analysis was cancelled before Provider execution.",
                false,
                false,
            ));
        }
        let request = input.request.clone();
        let evidence = input.evidence.clone();
        let child = cancellation.child_token();
        let provider_future = self.provider.analyze(input, child.clone());
        let output = match tokio::time::timeout(self.timeout, provider_future).await {
            Ok(result) => result?,
            Err(_) => {
                child.cancel();
                return Err(error(
                    "VISION_EVIDENCE_TIMEOUT",
                    "Vision evidence Provider exceeded the reviewed timeout.",
                    true,
                    true,
                ));
            }
        };
        if cancellation.is_cancelled() {
            return Err(error(
                "VISION_EVIDENCE_CANCELLED",
                "A late vision evidence result was discarded after cancellation.",
                output.network_call_made,
                false,
            ));
        }
        require_sha256(&output.provider_response_sha256)?;
        let request_sha256 = semantic_sha256(&request).map_err(core_error)?;
        let graph_seed = semantic_sha256(&serde_json::json!({
            "request_sha256": request_sha256,
            "provider_id": output.provider_id,
            "model_id": output.model_id,
            "provider_response_sha256": output.provider_response_sha256,
        }))
        .map_err(core_error)?;
        let mut claims = output.claims;
        assign_rust_owned_ids_to_invalid_provider_claims(&mut claims, &graph_seed);
        let graph = VisualEvidenceGraph {
            schema_version: VISUAL_EVIDENCE_GRAPH_SCHEMA_VERSION.into(),
            graph_id: format!("vegraph_{}", &graph_seed[..24]),
            request_id: request.request_id.clone(),
            request_sha256,
            project_id: request.project_id.clone(),
            domain_pack_id: request.domain_pack_id.clone(),
            provider: VisionEvidenceProviderProvenance {
                provider_id: output.provider_id,
                model_id: output.model_id,
                provider_response_sha256: output.provider_response_sha256,
                analyzed_at: output.analyzed_at,
            },
            claims,
        };
        graph
            .validate_against(&request, &evidence)
            .map_err(|error| {
                VisionEvidenceProviderError::new(
                    "VISION_EVIDENCE_OUTPUT_REJECTED",
                    format!("Rust rejected visual evidence output: {}", error.code()),
                    output.network_call_made,
                    false,
                )
            })?;
        Ok(graph)
    }
}

fn assign_rust_owned_ids_to_invalid_provider_claims(
    claims: &mut [VisualEvidenceClaim],
    graph_seed: &str,
) {
    for (index, claim) in claims.iter_mut().enumerate() {
        if !is_reviewed_visual_claim_id(&claim.claim_id) {
            claim.claim_id = format!("vclaim_{}_{:03}", &graph_seed[..24], index + 1);
        }
    }
}

fn is_reviewed_visual_claim_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value.starts_with("vclaim_")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

fn validate_input(
    input: &VisionEvidenceProviderRequest,
) -> Result<(), VisionEvidenceProviderError> {
    input
        .request
        .validate_with_evidence(&input.evidence)
        .map_err(core_error)?;
    if input.images.len() > MAX_VISION_EVIDENCE_IMAGES {
        return Err(error(
            "VISION_EVIDENCE_IMAGE_LIMIT_EXCEEDED",
            "Vision evidence request exceeds the reviewed image count.",
            false,
            false,
        ));
    }
    let referenced = input
        .request
        .reference_inputs
        .iter()
        .map(|reference| reference.evidence_id.as_str())
        .collect::<BTreeSet<_>>();
    let sealed = input
        .evidence
        .iter()
        .map(|evidence| (evidence.evidence_id.as_str(), evidence))
        .collect::<BTreeMap<_, _>>();
    let mut image_ids = BTreeSet::new();
    let mut total_bytes = 0usize;
    for image in &input.images {
        if !image_ids.insert(image.evidence_id.as_str())
            || !referenced.contains(image.evidence_id.as_str())
        {
            return Err(error(
                "VISION_EVIDENCE_IMAGE_SCOPE_INVALID",
                "Vision input images must be unique references from the exact request.",
                false,
                false,
            ));
        }
        let source = sealed.get(image.evidence_id.as_str()).ok_or_else(|| {
            error(
                "VISION_EVIDENCE_IMAGE_SCOPE_INVALID",
                "Vision input image does not resolve to sealed evidence.",
                false,
                false,
            )
        })?;
        if source.kind != ReferenceEvidenceKind::Image
            || image.media_type != source.source_media_type
            || image.bytes.is_empty()
            || image.bytes.len() > MAX_VISION_EVIDENCE_IMAGE_BYTES
            || format!("{:x}", Sha256::digest(image.bytes.as_ref())) != source.source_object_sha256
        {
            return Err(error(
                "VISION_EVIDENCE_IMAGE_READBACK_INVALID",
                "Vision input bytes must match the exact sealed image evidence.",
                false,
                false,
            ));
        }
        total_bytes = total_bytes.checked_add(image.bytes.len()).ok_or_else(|| {
            error(
                "VISION_EVIDENCE_IMAGE_LIMIT_EXCEEDED",
                "Vision input image bytes overflowed the reviewed limit.",
                false,
                false,
            )
        })?;
    }
    if total_bytes > MAX_VISION_EVIDENCE_TOTAL_BYTES {
        return Err(error(
            "VISION_EVIDENCE_IMAGE_LIMIT_EXCEEDED",
            "Vision evidence request exceeds the reviewed total image bytes.",
            false,
            false,
        ));
    }
    let required_image_ids = input
        .evidence
        .iter()
        .filter(|evidence| {
            evidence.kind == ReferenceEvidenceKind::Image
                && referenced.contains(evidence.evidence_id.as_str())
        })
        .map(|evidence| evidence.evidence_id.as_str())
        .collect::<BTreeSet<_>>();
    if image_ids != required_image_ids {
        return Err(error(
            "VISION_EVIDENCE_IMAGE_SET_INCOMPLETE",
            "Every requested image reference must have exact CAS bytes, and no extra image is allowed.",
            false,
            false,
        ));
    }
    Ok(())
}

fn require_sha256(value: &str) -> Result<(), VisionEvidenceProviderError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(error(
            "VISION_EVIDENCE_RESPONSE_HASH_INVALID",
            "Vision Provider response hash must be lowercase SHA-256.",
            false,
            false,
        ));
    }
    Ok(())
}

fn core_error(error: forgecad_core::CoreError) -> VisionEvidenceProviderError {
    VisionEvidenceProviderError::new(
        "VISION_EVIDENCE_INPUT_REJECTED",
        format!("Rust rejected vision evidence input: {}", error.code()),
        false,
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
    use std::sync::{Arc, Mutex};

    use forgecad_core::{
        semantic_sha256, MultimodalDesignLocks, MultimodalReferenceInput, ReferenceClass,
        ReferenceEvidenceObservations, ReferenceImageBrightnessBucket, ReferenceImageColorBucket,
        ReferenceImageEdgeDensityBucket, ReferenceImageForegroundConfidence,
        ReferenceImageSurfaceFacts, ReferenceRole, VisualClaimStatus, VisualClaimTarget,
        VisualDetailLevel, MULTIMODAL_DESIGN_REQUEST_SCHEMA_VERSION,
    };

    use super::*;

    #[derive(Clone)]
    struct FakeVisionProvider {
        calls: Arc<Mutex<usize>>,
        output: VisionEvidenceProviderOutput,
    }

    #[derive(Clone)]
    struct PendingVisionProvider;

    impl VisionEvidenceProviderPort for PendingVisionProvider {
        fn analyze(
            &self,
            _request: VisionEvidenceProviderRequest,
            _cancellation: CancellationToken,
        ) -> VisionEvidenceProviderFuture {
            Box::pin(std::future::pending())
        }
    }

    impl VisionEvidenceProviderPort for FakeVisionProvider {
        fn analyze(
            &self,
            _request: VisionEvidenceProviderRequest,
            _cancellation: CancellationToken,
        ) -> VisionEvidenceProviderFuture {
            *self.calls.lock().unwrap() += 1;
            let output = self.output.clone();
            Box::pin(async move { Ok(output) })
        }
    }

    fn input() -> VisionEvidenceProviderRequest {
        // PNG signature is sufficient here because the sealed record already
        // owns image decode validation; this boundary verifies exact CAS hash.
        let bytes: Arc<[u8]> = Arc::from([137, 80, 78, 71, 13, 10, 26, 10]);
        let sha = format!("{:x}", Sha256::digest(bytes.as_ref()));
        let evidence = ReferenceEvidence {
            schema_version: "ReferenceEvidence@1".into(),
            evidence_id: "refevid_provider_front".into(),
            project_id: "prj_provider".into(),
            kind: ReferenceEvidenceKind::Image,
            reference_class: ReferenceClass::SingleImage,
            domain_pack_id: "pack_robotic_arm_concept".into(),
            source_file_name: "front.png".into(),
            source_media_type: "image/png".into(),
            source_object_sha256: sha,
            source_imported_asset_version_id: None,
            source_statement: "User supplied reference".into(),
            license_statement: "User confirms remote analysis rights".into(),
            missing_views: vec!["back".into()],
            user_notes: "Analyze visible design language".into(),
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
            created_at: "2026-07-26T13:00:00Z".into(),
            glb_inspection: None,
        };
        let request = MultimodalDesignRequest {
            schema_version: MULTIMODAL_DESIGN_REQUEST_SCHEMA_VERSION.into(),
            request_id: "mmreq_provider_001".into(),
            project_id: evidence.project_id.clone(),
            turn_id: "turn_provider_001".into(),
            domain_pack_id: evidence.domain_pack_id.clone(),
            instruction: "Create a refined mechanical arm from the visible reference".into(),
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
        };
        VisionEvidenceProviderRequest {
            request,
            evidence: vec![evidence.clone()],
            images: vec![VisionEvidenceImage {
                evidence_id: evidence.evidence_id,
                media_type: evidence.source_media_type,
                bytes,
            }],
        }
    }

    fn output() -> VisionEvidenceProviderOutput {
        VisionEvidenceProviderOutput {
            provider_id: "fake_openai_compatible_vision".into(),
            model_id: "fake_qwen_vl".into(),
            provider_response_sha256: "d".repeat(64),
            analyzed_at: "2026-07-26T13:01:00Z".into(),
            claims: vec![
                VisualEvidenceClaim {
                    claim_id: "vclaim_provider_macro".into(),
                    level: VisualDetailLevel::Macro,
                    status: VisualClaimStatus::Observed,
                    target: VisualClaimTarget::Geometry,
                    description: "Tall articulated arm silhouette".into(),
                    critical: true,
                    confidence_bps: 9000,
                    source_evidence_ids: vec!["refevid_provider_front".into()],
                    source_view_id: Some("front".into()),
                    source_region: None,
                },
                VisualEvidenceClaim {
                    claim_id: "vclaim_provider_meso".into(),
                    level: VisualDetailLevel::Meso,
                    status: VisualClaimStatus::Observed,
                    target: VisualClaimTarget::Material,
                    description: "Blue panels contrast with dark structure".into(),
                    critical: true,
                    confidence_bps: 8500,
                    source_evidence_ids: vec!["refevid_provider_front".into()],
                    source_view_id: Some("front".into()),
                    source_region: None,
                },
                VisualEvidenceClaim {
                    claim_id: "vclaim_provider_micro".into(),
                    level: VisualDetailLevel::Micro,
                    status: VisualClaimStatus::Unknown,
                    target: VisualClaimTarget::Surface,
                    description: "Back surface detail is not visible".into(),
                    critical: false,
                    confidence_bps: 0,
                    source_evidence_ids: vec![],
                    source_view_id: None,
                    source_region: None,
                },
            ],
            network_call_made: true,
        }
    }

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn pv006b_coordinator_constructs_and_validates_graph_after_provider_output() {
        let provider = FakeVisionProvider {
            calls: Arc::new(Mutex::new(0)),
            output: output(),
        };
        let calls = provider.calls.clone();
        let coordinator =
            VisionEvidenceCoordinator::new(Arc::new(provider), Duration::from_secs(1)).unwrap();
        let graph = block_on(coordinator.analyze(input(), CancellationToken::new())).unwrap();
        assert_eq!(graph.schema_version, "VisualEvidenceGraph@1");
        assert_eq!(graph.claims.len(), 3);
        assert_eq!(*calls.lock().unwrap(), 1);
    }

    #[test]
    fn pv006b_coordinator_replaces_only_invalid_provider_claim_ids_with_stable_rust_ids() {
        let mut provider_output = output();
        provider_output.claims[0].claim_id = "Macro Silhouette / 主轮廓".into();
        let valid_provider_claim_id = provider_output.claims[1].claim_id.clone();
        let analyze = || {
            let coordinator = VisionEvidenceCoordinator::new(
                Arc::new(FakeVisionProvider {
                    calls: Arc::new(Mutex::new(0)),
                    output: provider_output.clone(),
                }),
                Duration::from_secs(1),
            )
            .unwrap();
            block_on(coordinator.analyze(input(), CancellationToken::new())).unwrap()
        };

        let first = analyze();
        let second = analyze();
        assert!(first.claims[0].claim_id.starts_with("vclaim_"));
        assert_ne!(first.claims[0].claim_id, "Macro Silhouette / 主轮廓");
        assert_eq!(first.claims[0].claim_id, second.claims[0].claim_id);
        assert_eq!(first.claims[1].claim_id, valid_provider_claim_id);
    }

    #[test]
    fn pv006b_invalid_provider_source_id_remains_fail_closed() {
        let mut provider_output = output();
        provider_output.claims[0].source_evidence_ids = vec!["Reference Front / 主图".into()];
        let coordinator = VisionEvidenceCoordinator::new(
            Arc::new(FakeVisionProvider {
                calls: Arc::new(Mutex::new(0)),
                output: provider_output,
            }),
            Duration::from_secs(1),
        )
        .unwrap();

        let error = block_on(coordinator.analyze(input(), CancellationToken::new())).unwrap_err();
        assert_eq!(error.code, "VISION_EVIDENCE_OUTPUT_REJECTED");
        assert!(error.message.contains("MULTIMODAL_ID_INVALID"));
        assert!(error.network_call_made);
    }

    #[test]
    fn pv006b_invalid_cas_bytes_fail_before_provider_call() {
        let provider = FakeVisionProvider {
            calls: Arc::new(Mutex::new(0)),
            output: output(),
        };
        let calls = provider.calls.clone();
        let coordinator =
            VisionEvidenceCoordinator::new(Arc::new(provider), Duration::from_secs(1)).unwrap();
        let mut input = input();
        input.images[0].bytes = Arc::from([0, 1, 2, 3]);
        let error = block_on(coordinator.analyze(input, CancellationToken::new())).unwrap_err();
        assert_eq!(error.code, "VISION_EVIDENCE_IMAGE_READBACK_INVALID");
        assert_eq!(*calls.lock().unwrap(), 0);
    }

    #[test]
    fn pv006b_provider_schema_drift_is_rejected_after_network_marker() {
        let mut output = output();
        output.claims[0].source_view_id = Some("back".into());
        let coordinator = VisionEvidenceCoordinator::new(
            Arc::new(FakeVisionProvider {
                calls: Arc::new(Mutex::new(0)),
                output,
            }),
            Duration::from_secs(1),
        )
        .unwrap();
        let error = block_on(coordinator.analyze(input(), CancellationToken::new())).unwrap_err();
        assert_eq!(error.code, "VISION_EVIDENCE_OUTPUT_REJECTED");
        assert!(error.network_call_made);
    }

    #[test]
    fn pv006b_precancelled_request_has_zero_provider_calls() {
        let provider = FakeVisionProvider {
            calls: Arc::new(Mutex::new(0)),
            output: output(),
        };
        let calls = provider.calls.clone();
        let coordinator =
            VisionEvidenceCoordinator::new(Arc::new(provider), Duration::from_secs(1)).unwrap();
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let error = block_on(coordinator.analyze(input(), cancellation)).unwrap_err();
        assert_eq!(error.code, "VISION_EVIDENCE_CANCELLED");
        assert_eq!(*calls.lock().unwrap(), 0);
    }

    #[test]
    fn pv006b_timeout_is_bounded_and_retryable() {
        let coordinator = VisionEvidenceCoordinator::new(
            Arc::new(PendingVisionProvider),
            Duration::from_millis(5),
        )
        .unwrap();
        let error = block_on(coordinator.analyze(input(), CancellationToken::new())).unwrap_err();
        assert_eq!(error.code, "VISION_EVIDENCE_TIMEOUT");
        assert!(error.network_call_made);
        assert!(error.retryable);
    }
}
