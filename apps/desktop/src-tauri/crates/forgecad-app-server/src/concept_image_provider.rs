//! Bounded concept-image provider port for the visual-first pipeline.
//!
//! A concrete adapter is responsible for downloading provider output and
//! hashing it before returning `Ready`. URLs, credentials, logs and arbitrary
//! provider JSON never cross this port.

use std::{future::Future, pin::Pin};

use forgecad_core::{
    ConceptImageBackend, ConceptImageGenerationRequest, ConceptImageResumeBinding,
    ConceptReferenceArtifact, HiddenSurfacePolicy, VisualDesignBrief,
    CONCEPT_REFERENCE_ARTIFACT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};

pub const CONCEPT_IMAGE_PROVIDER_RECEIPT_SCHEMA_VERSION: &str = "ConceptImageProviderReceipt@1";
pub type ConceptImageProviderFuture<T> =
    Pin<Box<dyn Future<Output = Result<T, ConceptImageProviderError>> + Send + 'static>>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConceptImageProviderReceipt {
    pub schema_version: String,
    pub backend: ConceptImageBackend,
    pub provider_job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConceptImageOutputHandle {
    pub image_object_sha256: String,
    pub byte_size: u64,
    pub media_type: String,
    pub width: u16,
    pub height: u16,
    pub safety_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum ConceptImageProviderStatus {
    Queued,
    Running,
    Ready { output: ConceptImageOutputHandle },
    Failed { code: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptImageProviderError {
    pub code: &'static str,
    pub message: String,
}

impl ConceptImageProviderError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub trait ConceptImageProviderPort: Send + Sync + 'static {
    fn submit(
        &self,
        request: ConceptImageGenerationRequest,
        backend: ConceptImageBackend,
    ) -> ConceptImageProviderFuture<ConceptImageProviderReceipt>;

    fn poll(
        &self,
        receipt: ConceptImageProviderReceipt,
    ) -> ConceptImageProviderFuture<ConceptImageProviderStatus>;

    fn cancel(&self, receipt: ConceptImageProviderReceipt) -> ConceptImageProviderFuture<()>;
}

pub fn validate_concept_receipt(
    receipt: &ConceptImageProviderReceipt,
    request: &ConceptImageGenerationRequest,
) -> Result<(), ConceptImageProviderError> {
    if receipt.schema_version != CONCEPT_IMAGE_PROVIDER_RECEIPT_SCHEMA_VERSION
        || !request.backend_preferences.contains(&receipt.backend)
    {
        return Err(ConceptImageProviderError::new(
            "CONCEPT_IMAGE_RECEIPT_INVALID",
            "Concept image receipt schema or backend does not match the Rust request.",
        ));
    }
    require_id("provider_job_id", &receipt.provider_job_id)
}

pub fn accept_concept_output(
    brief: &VisualDesignBrief,
    request: &ConceptImageGenerationRequest,
    receipt: &ConceptImageProviderReceipt,
    output: &ConceptImageOutputHandle,
    reference_id: String,
) -> Result<ConceptReferenceArtifact, ConceptImageProviderError> {
    request.validate_against(brief).map_err(|error| {
        ConceptImageProviderError::new(
            "CONCEPT_IMAGE_REQUEST_INVALID",
            format!("Concept request failed Rust validation: {}", error.code()),
        )
    })?;
    validate_concept_receipt(receipt, request)?;
    require_id("reference_id", &reference_id)?;
    output.validate()?;
    let reference = ConceptReferenceArtifact {
        schema_version: CONCEPT_REFERENCE_ARTIFACT_SCHEMA_VERSION.into(),
        reference_id,
        brief_id: brief.brief_id.clone(),
        image_object_sha256: output.image_object_sha256.clone(),
        media_type: output.media_type.clone(),
        provider_id: concept_backend_id(receipt.backend).into(),
        provider_job_id: receipt.provider_job_id.clone(),
        isolated_subject: request.isolated_subject,
        clean_background: request.clean_background,
        // This contract produces one concept image. Even when the brief was
        // informed by multiple uploads, the generated handoff itself cannot
        // prove unobserved surfaces. A later registered multi-view pipeline
        // must earn `MultiviewSupported` independently.
        hidden_surface_policy: HiddenSurfacePolicy::AiInferred,
    };
    reference.validate().map_err(|error| {
        ConceptImageProviderError::new(
            "CONCEPT_IMAGE_REFERENCE_INVALID",
            format!("Accepted concept reference is invalid: {}", error.code()),
        )
    })?;
    Ok(reference)
}

/// Accepts a queue result after desktop restart without persisting or
/// reconstructing the provider prompt. The prompt digest and all product
/// lineage were sealed in Core when the remote queue returned its receipt.
pub fn accept_resumed_concept_output(
    binding: &ConceptImageResumeBinding,
    output: &ConceptImageOutputHandle,
) -> Result<ConceptReferenceArtifact, ConceptImageProviderError> {
    binding.validate().map_err(|error| {
        ConceptImageProviderError::new(
            "CONCEPT_IMAGE_RESUME_BINDING_INVALID",
            format!(
                "Concept recovery binding failed Rust validation: {}",
                error.code()
            ),
        )
    })?;
    output.validate()?;
    let reference = ConceptReferenceArtifact {
        schema_version: CONCEPT_REFERENCE_ARTIFACT_SCHEMA_VERSION.into(),
        reference_id: binding.reference_id.clone(),
        brief_id: binding.brief.brief_id.clone(),
        image_object_sha256: output.image_object_sha256.clone(),
        media_type: output.media_type.clone(),
        provider_id: concept_backend_id(binding.backend).into(),
        provider_job_id: binding.provider_job_id.clone(),
        isolated_subject: binding.isolated_subject,
        clean_background: binding.clean_background,
        hidden_surface_policy: HiddenSurfacePolicy::AiInferred,
    };
    reference.validate().map_err(|error| {
        ConceptImageProviderError::new(
            "CONCEPT_IMAGE_REFERENCE_INVALID",
            format!("Recovered concept reference is invalid: {}", error.code()),
        )
    })?;
    Ok(reference)
}

impl ConceptImageOutputHandle {
    fn validate(&self) -> Result<(), ConceptImageProviderError> {
        require_sha256("image_object_sha256", &self.image_object_sha256)?;
        if self.byte_size == 0
            || self.byte_size > 32 * 1024 * 1024
            || self.media_type != "image/png"
            || self.width != 1024
            || self.height != 1024
            || !self.safety_passed
        {
            return Err(ConceptImageProviderError::new(
                "CONCEPT_IMAGE_OUTPUT_INVALID",
                "Concept output must be a safety-approved non-empty 1024x1024 PNG within the byte limit.",
            ));
        }
        Ok(())
    }
}

fn concept_backend_id(backend: ConceptImageBackend) -> &'static str {
    match backend {
        ConceptImageBackend::FalFlux2 => "fal_flux_2",
        ConceptImageBackend::OpenAiGptImage => "openai_gpt_image",
    }
}

fn require_id(field: &str, value: &str) -> Result<(), ConceptImageProviderError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'@'))
    {
        return Err(ConceptImageProviderError::new(
            "CONCEPT_IMAGE_ID_INVALID",
            format!("{field} must be a bounded stable identifier."),
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), ConceptImageProviderError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(ConceptImageProviderError::new(
            "CONCEPT_IMAGE_SHA256_INVALID",
            format!("{field} must be a lowercase SHA-256 digest."),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_core::{
        ConceptImageGenerationRequest, VisualInputEvidence, VisualInputKind,
        CONCEPT_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION, VISUAL_DESIGN_BRIEF_SCHEMA_VERSION,
    };

    fn sha(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    fn brief() -> VisualDesignBrief {
        VisualDesignBrief {
            schema_version: VISUAL_DESIGN_BRIEF_SCHEMA_VERSION.into(),
            brief_id: "brief_1".into(),
            project_id: "project_1".into(),
            turn_id: "turn_1".into(),
            input_kind: VisualInputKind::TextAndImage,
            user_intent_sha256: sha('a'),
            object_class: "fictional mechanical prop".into(),
            visual_summary: "A refined hard-surface collectible visual asset.".into(),
            style_terms: vec!["deep_sea".into()],
            material_terms: vec!["dark_metal".into()],
            input_evidence: vec![VisualInputEvidence {
                evidence_id: "evidence_1".into(),
                object_sha256: sha('b'),
                media_type: "image/png".into(),
                rights_confirmed: true,
                remote_processing_authorized: true,
            }],
        }
    }

    fn request() -> ConceptImageGenerationRequest {
        ConceptImageGenerationRequest {
            schema_version: CONCEPT_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION.into(),
            request_id: "concept_request_1".into(),
            project_id: "project_1".into(),
            turn_id: "turn_1".into(),
            brief_id: "brief_1".into(),
            prompt: "One isolated fictional mechanical collectible, three-quarter view, dark metal, clean neutral background.".into(),
            input_image_object_sha256: Some(sha('b')),
            input_image_media_type: Some("image/png".into()),
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

    #[test]
    fn accepted_output_becomes_exact_lineage_reference() {
        let brief = brief();
        let request = request();
        let receipt = ConceptImageProviderReceipt {
            schema_version: CONCEPT_IMAGE_PROVIDER_RECEIPT_SCHEMA_VERSION.into(),
            backend: ConceptImageBackend::FalFlux2,
            provider_job_id: "fal_job_1".into(),
        };
        let output = ConceptImageOutputHandle {
            image_object_sha256: sha('c'),
            byte_size: 2048,
            media_type: "image/png".into(),
            width: 1024,
            height: 1024,
            safety_passed: true,
        };
        let reference =
            accept_concept_output(&brief, &request, &receipt, &output, "reference_1".into())
                .unwrap();
        assert_eq!(reference.image_object_sha256, sha('c'));
        assert_eq!(reference.provider_id, "fal_flux_2");
        assert_eq!(
            reference.hidden_surface_policy,
            HiddenSurfacePolicy::AiInferred
        );
    }

    #[test]
    fn prompt_free_resume_binding_accepts_the_same_exact_queue_result() {
        let brief = brief();
        let request = request();
        let binding = ConceptImageResumeBinding::from_submitted_request(
            brief,
            &request,
            ConceptImageBackend::FalFlux2,
            "fal_job_resume".into(),
            "reference_resume".into(),
            forgecad_core::VisualQualityTier::StandardAsset,
        )
        .unwrap();
        let serialized = serde_json::to_string(&binding).unwrap();
        assert!(!serialized.contains(&request.prompt));
        let output = ConceptImageOutputHandle {
            image_object_sha256: sha('e'),
            byte_size: 2048,
            media_type: "image/png".into(),
            width: 1024,
            height: 1024,
            safety_passed: true,
        };
        let reference = accept_resumed_concept_output(&binding, &output).unwrap();
        assert_eq!(reference.provider_job_id, "fal_job_resume");
        assert_eq!(reference.image_object_sha256, sha('e'));
    }

    #[test]
    fn unsafe_or_unbound_output_is_rejected() {
        let brief = brief();
        let request = request();
        let receipt = ConceptImageProviderReceipt {
            schema_version: CONCEPT_IMAGE_PROVIDER_RECEIPT_SCHEMA_VERSION.into(),
            backend: ConceptImageBackend::OpenAiGptImage,
            provider_job_id: "other_job".into(),
        };
        let output = ConceptImageOutputHandle {
            image_object_sha256: sha('d'),
            byte_size: 2048,
            media_type: "image/png".into(),
            width: 1024,
            height: 1024,
            safety_passed: false,
        };
        assert_eq!(
            accept_concept_output(&brief, &request, &receipt, &output, "reference_1".into())
                .unwrap_err()
                .code,
            "CONCEPT_IMAGE_RECEIPT_INVALID"
        );

        let bound_receipt = ConceptImageProviderReceipt {
            schema_version: CONCEPT_IMAGE_PROVIDER_RECEIPT_SCHEMA_VERSION.into(),
            backend: ConceptImageBackend::FalFlux2,
            provider_job_id: "fal_job_unsafe".into(),
        };
        assert_eq!(
            accept_concept_output(
                &brief,
                &request,
                &bound_receipt,
                &output,
                "reference_2".into()
            )
            .unwrap_err()
            .code,
            "CONCEPT_IMAGE_OUTPUT_INVALID"
        );
    }
}
