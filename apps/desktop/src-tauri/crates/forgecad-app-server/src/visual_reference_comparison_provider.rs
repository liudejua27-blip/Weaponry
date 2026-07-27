//! Transport-neutral reference-versus-candidate visual comparison boundary.
//!
//! The Provider can observe sealed reference pixels and the exact eight
//! candidate renders, but it cannot decide pass/fail. It returns bounded
//! per-claim assessments; Rust validates their lineage and derives scores,
//! repair targets and the final decision.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use forgecad_core::{
    semantic_sha256, ReferenceEvidence, ReferenceEvidenceKind, VisionEvidenceProviderProvenance,
    VisualEvidenceGraph, VisualReferenceClaimAssessment, VisualReferenceComparisonInput,
    VisualReferenceComparisonReport,
};
use sha2::{Digest, Sha256};

use crate::CancellationToken;

pub const MAX_REFERENCE_COMPARISON_IMAGE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_REFERENCE_COMPARISON_TOTAL_BYTES: usize = 96 * 1024 * 1024;

pub type VisualReferenceComparisonProviderFuture = Pin<
    Box<
        dyn Future<
                Output = Result<
                    VisualReferenceComparisonProviderOutput,
                    VisualReferenceComparisonProviderError,
                >,
            > + Send
            + 'static,
    >,
>;

#[derive(Clone, PartialEq, Eq)]
pub struct VisualReferenceComparisonImage {
    pub image_id: String,
    pub media_type: String,
    pub bytes: Arc<[u8]>,
}

impl fmt::Debug for VisualReferenceComparisonImage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VisualReferenceComparisonImage")
            .field("image_id", &self.image_id)
            .field("media_type", &self.media_type)
            .field("byte_size", &self.bytes.len())
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone)]
pub struct VisualReferenceComparisonProviderRequest {
    pub input: VisualReferenceComparisonInput,
    pub graph: VisualEvidenceGraph,
    pub evidence: Vec<ReferenceEvidence>,
    pub reference_images: Vec<VisualReferenceComparisonImage>,
    pub candidate_images: Vec<VisualReferenceComparisonImage>,
}

impl fmt::Debug for VisualReferenceComparisonProviderRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VisualReferenceComparisonProviderRequest")
            .field("glb_sha256", &self.input.glb_sha256)
            .field("claim_count", &self.graph.claims.len())
            .field("reference_image_count", &self.reference_images.len())
            .field("candidate_image_count", &self.candidate_images.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualReferenceComparisonProviderOutput {
    pub provider_id: String,
    pub model_id: String,
    pub provider_response_sha256: String,
    pub analyzed_at: String,
    pub assessments: Vec<VisualReferenceClaimAssessment>,
    pub network_call_made: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualReferenceComparisonProviderError {
    pub code: &'static str,
    pub message: String,
    pub network_call_made: bool,
    pub retryable: bool,
}

impl VisualReferenceComparisonProviderError {
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

pub trait VisualReferenceComparisonProviderPort: Send + Sync + 'static {
    fn compare(
        &self,
        request: VisualReferenceComparisonProviderRequest,
        cancellation: CancellationToken,
    ) -> VisualReferenceComparisonProviderFuture;
}

#[derive(Clone)]
pub struct VisualReferenceComparisonCoordinator {
    provider: Arc<dyn VisualReferenceComparisonProviderPort>,
    timeout: Duration,
}

impl VisualReferenceComparisonCoordinator {
    pub fn new(
        provider: Arc<dyn VisualReferenceComparisonProviderPort>,
        timeout: Duration,
    ) -> Result<Self, VisualReferenceComparisonProviderError> {
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(error(
                "VISUAL_REFERENCE_COMPARISON_TIMEOUT_INVALID",
                "Reference comparison timeout must be between one millisecond and two minutes.",
                false,
                false,
            ));
        }
        Ok(Self { provider, timeout })
    }

    pub async fn compare(
        &self,
        request: VisualReferenceComparisonProviderRequest,
        cancellation: CancellationToken,
    ) -> Result<VisualReferenceComparisonReport, VisualReferenceComparisonProviderError> {
        validate_request(&request)?;
        if cancellation.is_cancelled() {
            return Err(error(
                "VISUAL_REFERENCE_COMPARISON_CANCELLED",
                "Reference comparison was cancelled before Provider execution.",
                false,
                false,
            ));
        }
        let input = request.input.clone();
        let graph = request.graph.clone();
        let child = cancellation.child_token();
        let provider = self.provider.compare(request, child.clone());
        let output = match tokio::time::timeout(self.timeout, provider).await {
            Ok(result) => result?,
            Err(_) => {
                child.cancel();
                return Err(error(
                    "VISUAL_REFERENCE_COMPARISON_TIMEOUT",
                    "Reference comparison Provider exceeded the reviewed timeout.",
                    true,
                    true,
                ));
            }
        };
        if cancellation.is_cancelled() {
            return Err(error(
                "VISUAL_REFERENCE_COMPARISON_CANCELLED",
                "A late reference comparison was discarded after cancellation.",
                output.network_call_made,
                false,
            ));
        }
        let report = VisualReferenceComparisonReport::build(
            &input,
            &graph,
            VisionEvidenceProviderProvenance {
                provider_id: output.provider_id,
                model_id: output.model_id,
                provider_response_sha256: output.provider_response_sha256,
                analyzed_at: output.analyzed_at,
            },
            output.assessments,
        )
        .map_err(|core| {
            VisualReferenceComparisonProviderError::new(
                "VISUAL_REFERENCE_COMPARISON_OUTPUT_REJECTED",
                format!("Rust rejected reference comparison output: {}", core.code()),
                output.network_call_made,
                false,
            )
        })?;
        report.validate_against(&input, &graph).map_err(|core| {
            VisualReferenceComparisonProviderError::new(
                "VISUAL_REFERENCE_COMPARISON_OUTPUT_REJECTED",
                format!("Rust rejected reference comparison report: {}", core.code()),
                output.network_call_made,
                false,
            )
        })?;
        Ok(report)
    }
}

fn validate_request(
    request: &VisualReferenceComparisonProviderRequest,
) -> Result<(), VisualReferenceComparisonProviderError> {
    if request.input.evidence_graph_sha256 != semantic_sha256(&request.graph).map_err(core_error)? {
        return Err(error(
            "VISUAL_REFERENCE_COMPARISON_INPUT_REJECTED",
            "Comparison input does not match the exact visual evidence graph.",
            false,
            false,
        ));
    }
    let sealed = request
        .evidence
        .iter()
        .map(|item| (item.evidence_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let expected_reference_ids = request
        .input
        .reference_sources
        .iter()
        .filter_map(|source| {
            sealed
                .get(source.evidence_id.as_str())
                .filter(|evidence| evidence.kind == ReferenceEvidenceKind::Image)
                .map(|_| source.evidence_id.as_str())
        })
        .collect::<BTreeSet<_>>();
    let expected_candidate = request
        .input
        .candidate_views
        .iter()
        .map(|view| (view.view_id.as_str(), view.image_sha256.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut reference_ids = BTreeSet::new();
    let mut candidate_ids = BTreeSet::new();
    let mut total_bytes = 0usize;

    for image in &request.reference_images {
        let evidence = sealed.get(image.image_id.as_str()).ok_or_else(|| {
            error(
                "VISUAL_REFERENCE_COMPARISON_REFERENCE_INVALID",
                "Comparison reference image does not resolve to sealed evidence.",
                false,
                false,
            )
        })?;
        if !reference_ids.insert(image.image_id.as_str())
            || !expected_reference_ids.contains(image.image_id.as_str())
            || evidence.kind != ReferenceEvidenceKind::Image
            || image.media_type != evidence.source_media_type
            || image.bytes.is_empty()
            || image.bytes.len() > MAX_REFERENCE_COMPARISON_IMAGE_BYTES
            || format!("{:x}", Sha256::digest(image.bytes.as_ref()))
                != evidence.source_object_sha256
        {
            return Err(error(
                "VISUAL_REFERENCE_COMPARISON_REFERENCE_INVALID",
                "Comparison reference pixels must match the exact sealed image evidence.",
                false,
                false,
            ));
        }
        total_bytes = total_bytes.saturating_add(image.bytes.len());
    }
    if reference_ids != expected_reference_ids || reference_ids.is_empty() {
        return Err(error(
            "VISUAL_REFERENCE_COMPARISON_REFERENCE_INCOMPLETE",
            "Reference comparison requires every sealed image in the exact request.",
            false,
            false,
        ));
    }

    for image in &request.candidate_images {
        let expected_sha256 = expected_candidate.get(image.image_id.as_str());
        let actual_sha256 = format!("{:x}", Sha256::digest(image.bytes.as_ref()));
        if !candidate_ids.insert(image.image_id.as_str())
            || image.media_type != "image/png"
            || image.bytes.is_empty()
            || image.bytes.len() > MAX_REFERENCE_COMPARISON_IMAGE_BYTES
            || expected_sha256.copied() != Some(actual_sha256.as_str())
        {
            return Err(error(
                "VISUAL_REFERENCE_COMPARISON_CANDIDATE_INVALID",
                "Candidate pixels must match the exact eight renderer outputs.",
                false,
                false,
            ));
        }
        total_bytes = total_bytes.saturating_add(image.bytes.len());
    }
    if candidate_ids != expected_candidate.keys().copied().collect::<BTreeSet<_>>() {
        return Err(error(
            "VISUAL_REFERENCE_COMPARISON_CANDIDATE_INCOMPLETE",
            "Reference comparison requires the exact eight candidate views.",
            false,
            false,
        ));
    }
    if total_bytes > MAX_REFERENCE_COMPARISON_TOTAL_BYTES {
        return Err(error(
            "VISUAL_REFERENCE_COMPARISON_IMAGE_LIMIT_EXCEEDED",
            "Reference and candidate images exceed the reviewed total byte limit.",
            false,
            false,
        ));
    }
    Ok(())
}

fn core_error(error: forgecad_core::CoreError) -> VisualReferenceComparisonProviderError {
    VisualReferenceComparisonProviderError::new(
        "VISUAL_REFERENCE_COMPARISON_INPUT_REJECTED",
        format!("Rust rejected reference comparison input: {}", error.code()),
        false,
        false,
    )
}

fn error(
    code: &'static str,
    message: impl Into<String>,
    network_call_made: bool,
    retryable: bool,
) -> VisualReferenceComparisonProviderError {
    VisualReferenceComparisonProviderError::new(code, message, network_call_made, retryable)
}
