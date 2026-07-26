//! Rust-owned contracts for Forge Studio's visual-first neural 3D pipeline.
//!
//! Remote image and 3D providers may create bytes, but they never own product
//! state.  This module records only bounded intent, consent, lineage, lifecycle
//! and accepted artifact facts.  It deliberately owns no credentials, network
//! client, arbitrary provider payload, prompt text, SQLite handle or version
//! promotion path.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{analyze_reference_image_bytes, CoreError, CoreResult};

pub const VISUAL_DESIGN_BRIEF_SCHEMA_VERSION: &str = "VisualDesignBrief@1";
pub const CONCEPT_REFERENCE_ARTIFACT_SCHEMA_VERSION: &str = "ConceptReferenceArtifact@1";
pub const CONCEPT_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION: &str = "ConceptImageGenerationRequest@1";
pub const NEURAL_3D_GENERATION_REQUEST_SCHEMA_VERSION: &str = "Neural3DGenerationRequest@1";
pub const NEURAL_VISUAL_GENERATION_JOB_SCHEMA_VERSION: &str = "NeuralVisualGenerationJob@1";
pub const VISUAL_REMOTE_JOB_RECORD_SCHEMA_VERSION: &str = "VisualRemoteJobRecord@1";
pub const NEURAL_VISUAL_ARTIFACT_SCHEMA_VERSION: &str = "NeuralVisualArtifact@1";
pub const FORGE_ASSET_PACKAGE_SCHEMA_VERSION: &str = "ForgeAssetPackage@1";
pub const REQUIRED_MULTIVIEW_RENDER_COUNT: u8 = 8;
pub const CONCEPT_REFERENCE_WIDTH: u16 = 1024;
pub const CONCEPT_REFERENCE_HEIGHT: u16 = 1024;
pub const MAX_CONCEPT_REFERENCE_PNG_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_NEURAL_VISUAL_GLB_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_NEURAL_VISUAL_TRIANGLES: u64 = 1_500_000;

/// Prompt-free, Rust-owned receipt used to resume a concept-image queue job.
/// The full provider prompt remains transient; only its digest is retained so
/// recovery cannot silently bind a different direction to the same job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConceptImageResumeBinding {
    pub brief: VisualDesignBrief,
    pub request_id: String,
    pub prompt_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_image_object_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_image_media_type: Option<String>,
    pub backend: ConceptImageBackend,
    pub provider_job_id: String,
    pub reference_id: String,
    pub quality_tier: VisualQualityTier,
    pub output_media_type: String,
    pub width: u16,
    pub height: u16,
    pub isolated_subject: bool,
    pub clean_background: bool,
}

impl ConceptImageResumeBinding {
    pub fn from_submitted_request(
        brief: VisualDesignBrief,
        request: &ConceptImageGenerationRequest,
        backend: ConceptImageBackend,
        provider_job_id: String,
        reference_id: String,
        quality_tier: VisualQualityTier,
    ) -> CoreResult<Self> {
        request.validate_against(&brief)?;
        if !request.backend_preferences.contains(&backend) {
            return Err(invalid(
                "VISUAL_REMOTE_CONCEPT_BACKEND_INVALID",
                "Persisted concept backend must be present in the reviewed request.",
            ));
        }
        let binding = Self {
            brief,
            request_id: request.request_id.clone(),
            prompt_sha256: format!("{:x}", Sha256::digest(request.prompt.as_bytes())),
            input_image_object_sha256: request.input_image_object_sha256.clone(),
            input_image_media_type: request.input_image_media_type.clone(),
            backend,
            provider_job_id,
            reference_id,
            quality_tier,
            output_media_type: request.output_media_type.clone(),
            width: request.width,
            height: request.height,
            isolated_subject: request.isolated_subject,
            clean_background: request.clean_background,
        };
        binding.validate()?;
        Ok(binding)
    }

    pub fn validate(&self) -> CoreResult<()> {
        self.brief.validate()?;
        for (field, value) in [
            ("request_id", self.request_id.as_str()),
            ("provider_job_id", self.provider_job_id.as_str()),
            ("reference_id", self.reference_id.as_str()),
        ] {
            require_id(field, value)?;
        }
        require_sha256("prompt_sha256", &self.prompt_sha256)?;
        if let Some(value) = &self.input_image_object_sha256 {
            require_sha256("input_image_object_sha256", value)?;
        }
        let expected_input = self.brief.input_evidence.first();
        let input_matches = match (
            self.input_image_object_sha256.as_deref(),
            self.input_image_media_type.as_deref(),
            expected_input,
        ) {
            (None, None, None) => true,
            (Some(actual_sha), Some(actual_media), Some(expected)) => {
                actual_sha == expected.object_sha256 && actual_media == expected.media_type
            }
            _ => false,
        };
        if !input_matches {
            return Err(invalid(
                "VISUAL_REMOTE_CONCEPT_INPUT_MISMATCH",
                "Recovered concept input digest and media type must match the authorized Brief evidence.",
            ));
        }
        if self.output_media_type != "image/png"
            || self.width != CONCEPT_REFERENCE_WIDTH
            || self.height != CONCEPT_REFERENCE_HEIGHT
            || !self.isolated_subject
            || !self.clean_background
        {
            return Err(invalid(
                "VISUAL_REMOTE_CONCEPT_COMPOSITION_INVALID",
                "Recovered concept jobs must retain the exact v1 image composition contract.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Neural3DResumeBinding {
    pub brief: VisualDesignBrief,
    pub concept_reference: ConceptReferenceArtifact,
    pub request: Neural3DGenerationRequest,
    pub backend: Neural3DBackend,
    pub provider_job_id: String,
}

impl Neural3DResumeBinding {
    pub fn validate(&self) -> CoreResult<()> {
        self.brief.validate()?;
        self.concept_reference.validate()?;
        self.request.validate()?;
        require_id("provider_job_id", &self.provider_job_id)?;
        if self.request.project_id != self.brief.project_id
            || self.request.turn_id != self.brief.turn_id
            || self.request.brief_id != self.brief.brief_id
            || self.request.concept_reference_id != self.concept_reference.reference_id
            || self.request.concept_reference_sha256 != self.concept_reference.image_object_sha256
            || !self.request.backend_preferences.contains(&self.backend)
        {
            return Err(invalid(
                "VISUAL_REMOTE_NEURAL_LINEAGE_INVALID",
                "Recovered neural generation must retain exact Brief and concept-reference lineage.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "stage")]
pub enum VisualRemoteJobState {
    ConceptSubmitted {
        binding: ConceptImageResumeBinding,
    },
    NeuralSubmitted {
        binding: Neural3DResumeBinding,
    },
    Completed {
        binding: Neural3DResumeBinding,
        inspection: NeuralVisualGlbInspection,
    },
    Failed {
        code: String,
    },
    Cancelled {
        code: String,
    },
}

impl VisualRemoteJobState {
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::ConceptSubmitted { .. } | Self::NeuralSubmitted { .. }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualRemoteJobRecord {
    pub schema_version: String,
    pub client_request_id: String,
    pub project_id: String,
    pub turn_id: String,
    pub state: VisualRemoteJobState,
    pub created_at: String,
    pub updated_at: String,
}

impl VisualRemoteJobRecord {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != VISUAL_REMOTE_JOB_RECORD_SCHEMA_VERSION {
            return Err(invalid(
                "VISUAL_REMOTE_JOB_SCHEMA_INVALID",
                "Remote visual jobs must use the exact v1 Rust record schema.",
            ));
        }
        for (field, value) in [
            ("client_request_id", self.client_request_id.as_str()),
            ("project_id", self.project_id.as_str()),
            ("turn_id", self.turn_id.as_str()),
        ] {
            require_id(field, value)?;
        }
        require_text("created_at", &self.created_at, 64)?;
        require_text("updated_at", &self.updated_at, 64)?;
        match &self.state {
            VisualRemoteJobState::ConceptSubmitted { binding } => {
                binding.validate()?;
                require_job_scope(self, &binding.brief.project_id, &binding.brief.turn_id)
            }
            VisualRemoteJobState::NeuralSubmitted { binding } => {
                binding.validate()?;
                require_job_scope(self, &binding.brief.project_id, &binding.brief.turn_id)
            }
            VisualRemoteJobState::Completed {
                binding,
                inspection,
            } => {
                binding.validate()?;
                require_job_scope(self, &binding.brief.project_id, &binding.brief.turn_id)?;
                require_sha256("completed_glb_sha256", &inspection.sha256)?;
                if inspection.byte_size == 0
                    || inspection.triangle_count == 0
                    || inspection.mesh_count == 0
                    || inspection.primitive_count == 0
                    || inspection.material_count == 0
                {
                    return Err(invalid(
                        "VISUAL_REMOTE_COMPLETED_READBACK_INVALID",
                        "Completed remote visual jobs require non-empty Rust GLB readback facts.",
                    ));
                }
                Ok(())
            }
            VisualRemoteJobState::Failed { code } | VisualRemoteJobState::Cancelled { code } => {
                require_code(code)
            }
        }
    }
}

fn require_job_scope(
    record: &VisualRemoteJobRecord,
    project_id: &str,
    turn_id: &str,
) -> CoreResult<()> {
    if record.project_id != project_id || record.turn_id != turn_id {
        return Err(invalid(
            "VISUAL_REMOTE_JOB_SCOPE_MISMATCH",
            "Remote visual job state must remain bound to one Project and Turn.",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConceptPngInspection {
    pub sha256: String,
    pub byte_size: u64,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NeuralVisualGlbInspection {
    pub sha256: String,
    pub byte_size: u64,
    pub triangle_count: u64,
    pub mesh_count: u64,
    pub primitive_count: u64,
    pub material_count: u64,
    pub node_count: u64,
    pub pbr_channels: BTreeSet<PbrChannel>,
    pub every_primitive_has_uv0: bool,
    pub every_primitive_has_tangent: bool,
}

/// Fully decodes the exact provider PNG within the existing reference-image
/// allocation limits. Header metadata alone is not sufficient evidence: a
/// truncated or decompression-bomb payload must fail before entering CAS.
pub fn inspect_concept_png(bytes: &[u8]) -> CoreResult<ConceptPngInspection> {
    if bytes.is_empty() || bytes.len() > MAX_CONCEPT_REFERENCE_PNG_BYTES {
        return Err(invalid(
            "CONCEPT_PNG_SIZE_INVALID",
            "Concept PNG bytes are empty or exceed the reviewed limit.",
        ));
    }
    let facts = analyze_reference_image_bytes("image/png", bytes).map_err(|_| {
        invalid(
            "CONCEPT_PNG_DECODE_INVALID",
            "Concept PNG could not be decoded within the reviewed image limits.",
        )
    })?;
    if facts.width != u32::from(CONCEPT_REFERENCE_WIDTH)
        || facts.height != u32::from(CONCEPT_REFERENCE_HEIGHT)
    {
        return Err(invalid(
            "CONCEPT_PNG_DIMENSIONS_INVALID",
            "Concept PNG must be exactly 1024x1024 pixels.",
        ));
    }
    Ok(ConceptPngInspection {
        sha256: format!("{:x}", Sha256::digest(bytes)),
        byte_size: bytes.len() as u64,
        width: CONCEPT_REFERENCE_WIDTH,
        height: CONCEPT_REFERENCE_HEIGHT,
    })
}

/// Strict structural readback for remote neural GLB bytes. This proves a
/// bounded self-contained GLB 2.0 container and records actual material and
/// vertex bindings. It does not claim that texture content is visually good;
/// later PBR/multiview gates decide acceptance.
pub fn inspect_neural_visual_glb(bytes: &[u8]) -> CoreResult<NeuralVisualGlbInspection> {
    let (inspection, document) = crate::external_glb::inspect_embedded_glb_with_limits(
        bytes,
        MAX_NEURAL_VISUAL_GLB_BYTES,
        MAX_NEURAL_VISUAL_TRIANGLES,
    )
    .map_err(|message| {
        invalid(
            "NEURAL_VISUAL_GLB_REJECTED",
            format!("Neural GLB failed structural readback: {message}"),
        )
    })?;
    let meshes = document
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("NEURAL_VISUAL_GLB_REJECTED", "Neural GLB has no meshes."))?;
    let mut every_primitive_has_uv0 = true;
    let mut every_primitive_has_tangent = true;
    for primitive in meshes
        .iter()
        .filter_map(|mesh| mesh.get("primitives").and_then(Value::as_array))
        .flatten()
    {
        let attributes = primitive
            .get("attributes")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                invalid(
                    "NEURAL_VISUAL_GLB_REJECTED",
                    "Neural GLB primitive attributes are invalid.",
                )
            })?;
        every_primitive_has_uv0 &= attributes
            .get("TEXCOORD_0")
            .and_then(Value::as_u64)
            .is_some();
        every_primitive_has_tangent &= attributes.get("TANGENT").and_then(Value::as_u64).is_some();
    }

    let materials = document
        .get("materials")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let textures = document
        .get("textures")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let images = document
        .get("images")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut pbr_channels = BTreeSet::new();
    for material in &materials {
        let pbr = material.get("pbrMetallicRoughness");
        if pbr
            .and_then(|value| value.get("baseColorTexture"))
            .and_then(|value| value.get("index"))
            .and_then(Value::as_u64)
            .is_some_and(|index| valid_texture_binding(index, &textures, &images))
        {
            pbr_channels.insert(PbrChannel::BaseColor);
        }
        if pbr
            .and_then(|value| value.get("metallicRoughnessTexture"))
            .and_then(|value| value.get("index"))
            .and_then(Value::as_u64)
            .is_some_and(|index| valid_texture_binding(index, &textures, &images))
        {
            pbr_channels.insert(PbrChannel::Metallic);
            pbr_channels.insert(PbrChannel::Roughness);
        }
        for (field, channel) in [
            ("normalTexture", PbrChannel::Normal),
            ("occlusionTexture", PbrChannel::AmbientOcclusion),
            ("emissiveTexture", PbrChannel::Emissive),
        ] {
            if material
                .get(field)
                .and_then(|value| value.get("index"))
                .and_then(Value::as_u64)
                .is_some_and(|index| valid_texture_binding(index, &textures, &images))
            {
                pbr_channels.insert(channel);
            }
        }
    }
    Ok(NeuralVisualGlbInspection {
        sha256: inspection.sha256,
        byte_size: inspection.byte_size,
        triangle_count: inspection.triangle_count,
        mesh_count: inspection.mesh_count,
        primitive_count: inspection.primitive_count,
        material_count: inspection.material_count,
        node_count: inspection.node_count,
        pbr_channels,
        every_primitive_has_uv0,
        every_primitive_has_tangent,
    })
}

fn valid_texture_binding(index: u64, textures: &[Value], images: &[Value]) -> bool {
    let Some(texture) = usize::try_from(index)
        .ok()
        .and_then(|index| textures.get(index))
    else {
        return false;
    };
    let source = texture.get("source").and_then(Value::as_u64).or_else(|| {
        texture
            .pointer("/extensions/KHR_texture_basisu/source")
            .and_then(Value::as_u64)
    });
    source
        .and_then(|source| usize::try_from(source).ok())
        .is_some_and(|source| source < images.len())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualInputKind {
    Text,
    Image,
    TextAndImage,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HiddenSurfacePolicy {
    /// The source supplied enough registered views to support the visible
    /// exterior. This is still visual evidence, not engineering truth.
    MultiviewSupported,
    /// A provider generated surfaces that were not observable in the source.
    AiInferred,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Neural3DBackend {
    Pixal3d,
    Trellis2,
    Hunyuan3d21,
    Hunyuan3dV31Pro,
    StableFast3d,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ConceptImageBackend {
    FalFlux2,
    OpenAiGptImage,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualQualityTier {
    FastPreview,
    StandardAsset,
    CollectibleAsset,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PbrChannel {
    BaseColor,
    Normal,
    Roughness,
    Metallic,
    AmbientOcclusion,
    Emissive,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualInputEvidence {
    pub evidence_id: String,
    pub object_sha256: String,
    pub media_type: String,
    pub rights_confirmed: bool,
    pub remote_processing_authorized: bool,
}

impl VisualInputEvidence {
    pub fn validate(&self) -> CoreResult<()> {
        require_id("evidence_id", &self.evidence_id)?;
        require_sha256("object_sha256", &self.object_sha256)?;
        if !matches!(
            self.media_type.as_str(),
            "image/png" | "image/jpeg" | "image/webp" | "model/gltf-binary"
        ) {
            return Err(invalid(
                "VISUAL_INPUT_MEDIA_TYPE_INVALID",
                "Visual input must use an explicitly supported image or GLB media type.",
            ));
        }
        if !self.rights_confirmed || !self.remote_processing_authorized {
            return Err(invalid(
                "VISUAL_INPUT_CONSENT_REQUIRED",
                "Remote visual generation requires source-rights confirmation and explicit remote-processing authorization.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualDesignBrief {
    pub schema_version: String,
    pub brief_id: String,
    pub project_id: String,
    pub turn_id: String,
    pub input_kind: VisualInputKind,
    /// Hash of the normalized user request. Raw text is retained by the
    /// existing Thread/Turn system, not duplicated into provider audit data.
    pub user_intent_sha256: String,
    pub object_class: String,
    pub visual_summary: String,
    #[serde(default)]
    pub style_terms: Vec<String>,
    #[serde(default)]
    pub material_terms: Vec<String>,
    #[serde(default)]
    pub input_evidence: Vec<VisualInputEvidence>,
}

impl VisualDesignBrief {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != VISUAL_DESIGN_BRIEF_SCHEMA_VERSION {
            return Err(invalid(
                "VISUAL_DESIGN_BRIEF_SCHEMA_INVALID",
                "Visual design brief must use its exact v1 schema.",
            ));
        }
        for (field, value) in [
            ("brief_id", self.brief_id.as_str()),
            ("project_id", self.project_id.as_str()),
            ("turn_id", self.turn_id.as_str()),
        ] {
            require_id(field, value)?;
        }
        require_sha256("user_intent_sha256", &self.user_intent_sha256)?;
        require_text("object_class", &self.object_class, 128)?;
        require_text("visual_summary", &self.visual_summary, 2_048)?;
        require_terms("style_terms", &self.style_terms)?;
        require_terms("material_terms", &self.material_terms)?;
        for evidence in &self.input_evidence {
            evidence.validate()?;
        }
        match self.input_kind {
            VisualInputKind::Text if !self.input_evidence.is_empty() => Err(invalid(
                "VISUAL_DESIGN_BRIEF_INPUT_MISMATCH",
                "Text-only briefs cannot bind uploaded input evidence.",
            )),
            VisualInputKind::Image | VisualInputKind::TextAndImage
                if self.input_evidence.is_empty() =>
            {
                Err(invalid(
                    "VISUAL_DESIGN_BRIEF_INPUT_MISMATCH",
                    "Image-based briefs require at least one authorized input evidence object.",
                ))
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConceptReferenceArtifact {
    pub schema_version: String,
    pub reference_id: String,
    pub brief_id: String,
    pub image_object_sha256: String,
    pub media_type: String,
    pub provider_id: String,
    pub provider_job_id: String,
    pub isolated_subject: bool,
    pub clean_background: bool,
    pub hidden_surface_policy: HiddenSurfacePolicy,
}

impl ConceptReferenceArtifact {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != CONCEPT_REFERENCE_ARTIFACT_SCHEMA_VERSION {
            return Err(invalid(
                "CONCEPT_REFERENCE_SCHEMA_INVALID",
                "Concept reference artifact must use its exact v1 schema.",
            ));
        }
        for (field, value) in [
            ("reference_id", self.reference_id.as_str()),
            ("brief_id", self.brief_id.as_str()),
            ("provider_id", self.provider_id.as_str()),
            ("provider_job_id", self.provider_job_id.as_str()),
        ] {
            require_id(field, value)?;
        }
        require_sha256("image_object_sha256", &self.image_object_sha256)?;
        if !matches!(
            self.media_type.as_str(),
            "image/png" | "image/jpeg" | "image/webp"
        ) {
            return Err(invalid(
                "CONCEPT_REFERENCE_MEDIA_TYPE_INVALID",
                "Concept reference must be a supported raster image.",
            ));
        }
        if !self.isolated_subject || !self.clean_background {
            return Err(invalid(
                "CONCEPT_REFERENCE_COMPOSITION_INVALID",
                "The neural 3D handoff requires one isolated subject on a clean background.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConceptImageGenerationRequest {
    pub schema_version: String,
    pub request_id: String,
    pub project_id: String,
    pub turn_id: String,
    pub brief_id: String,
    /// Provider-facing visual direction. This value is transient and must not
    /// be copied into redacted traces or artifact metadata.
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_image_object_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_image_media_type: Option<String>,
    pub backend_preferences: Vec<ConceptImageBackend>,
    pub width: u16,
    pub height: u16,
    pub output_media_type: String,
    pub isolated_subject: bool,
    pub clean_background: bool,
    pub image_count: u8,
    pub idempotency_key: String,
}

impl ConceptImageGenerationRequest {
    pub fn validate_against(&self, brief: &VisualDesignBrief) -> CoreResult<()> {
        brief.validate()?;
        if self.schema_version != CONCEPT_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION {
            return Err(invalid(
                "CONCEPT_IMAGE_REQUEST_SCHEMA_INVALID",
                "Concept image request must use its exact v1 schema.",
            ));
        }
        for (field, value) in [
            ("request_id", self.request_id.as_str()),
            ("project_id", self.project_id.as_str()),
            ("turn_id", self.turn_id.as_str()),
            ("brief_id", self.brief_id.as_str()),
            ("idempotency_key", self.idempotency_key.as_str()),
        ] {
            require_id(field, value)?;
        }
        if self.project_id != brief.project_id
            || self.turn_id != brief.turn_id
            || self.brief_id != brief.brief_id
        {
            return Err(invalid(
                "CONCEPT_IMAGE_REQUEST_BRIEF_MISMATCH",
                "Concept image request must bind the exact Rust-validated visual brief.",
            ));
        }
        require_text("prompt", &self.prompt, 4_096)?;
        if let Some(value) = &self.input_image_object_sha256 {
            require_sha256("input_image_object_sha256", value)?;
        }
        let expected_input = brief.input_evidence.first();
        match (
            self.input_image_object_sha256.as_deref(),
            self.input_image_media_type.as_deref(),
            expected_input,
        ) {
            (None, None, None) => {}
            (Some(actual_sha), Some(actual_media), Some(expected))
                if actual_sha == expected.object_sha256
                    && actual_media == expected.media_type => {}
            _ => {
                return Err(invalid(
                    "CONCEPT_IMAGE_REQUEST_INPUT_MISMATCH",
                    "Concept image input digest and media type must exactly match the first authorized Brief evidence object.",
                ))
            }
        }
        if self.backend_preferences.is_empty() || self.backend_preferences.len() > 2 {
            return Err(invalid(
                "CONCEPT_IMAGE_BACKEND_PREFERENCES_INVALID",
                "One or two ordered concept image backends are required.",
            ));
        }
        let unique = self
            .backend_preferences
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if unique.len() != self.backend_preferences.len() {
            return Err(invalid(
                "CONCEPT_IMAGE_BACKEND_PREFERENCES_INVALID",
                "Concept image backend preferences cannot contain duplicates.",
            ));
        }
        if self.width != 1024
            || self.height != 1024
            || self.output_media_type != "image/png"
            || !self.isolated_subject
            || !self.clean_background
            || self.image_count != 1
        {
            return Err(invalid(
                "CONCEPT_IMAGE_COMPOSITION_INVALID",
                "The v1 neural handoff requires exactly one 1024x1024 PNG isolated subject on a clean background.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Neural3DGenerationRequest {
    pub schema_version: String,
    pub request_id: String,
    pub project_id: String,
    pub turn_id: String,
    pub brief_id: String,
    pub concept_reference_id: String,
    pub concept_reference_sha256: String,
    pub quality_tier: VisualQualityTier,
    pub backend_preferences: Vec<Neural3DBackend>,
    pub idempotency_key: String,
}

impl Neural3DGenerationRequest {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != NEURAL_3D_GENERATION_REQUEST_SCHEMA_VERSION {
            return Err(invalid(
                "NEURAL_3D_REQUEST_SCHEMA_INVALID",
                "Neural 3D generation request must use its exact v1 schema.",
            ));
        }
        for (field, value) in [
            ("request_id", self.request_id.as_str()),
            ("project_id", self.project_id.as_str()),
            ("turn_id", self.turn_id.as_str()),
            ("brief_id", self.brief_id.as_str()),
            ("concept_reference_id", self.concept_reference_id.as_str()),
            ("idempotency_key", self.idempotency_key.as_str()),
        ] {
            require_id(field, value)?;
        }
        require_sha256("concept_reference_sha256", &self.concept_reference_sha256)?;
        if self.backend_preferences.is_empty() || self.backend_preferences.len() > 4 {
            return Err(invalid(
                "NEURAL_3D_BACKEND_PREFERENCES_INVALID",
                "One to four ordered neural 3D backend preferences are required.",
            ));
        }
        let unique = self
            .backend_preferences
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if unique.len() != self.backend_preferences.len() {
            return Err(invalid(
                "NEURAL_3D_BACKEND_PREFERENCES_INVALID",
                "Neural 3D backend preferences cannot contain duplicates.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NeuralVisualStage {
    Queued,
    ConceptReady,
    GeometryGenerating,
    PbrRefining,
    GlbReadback,
    MultiviewReview,
    Ready,
    Failed,
    Cancelled,
}

impl NeuralVisualStage {
    fn is_terminal(self) -> bool {
        matches!(self, Self::Ready | Self::Failed | Self::Cancelled)
    }

    fn can_advance_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Queued, Self::ConceptReady)
                | (Self::ConceptReady, Self::GeometryGenerating)
                | (Self::GeometryGenerating, Self::PbrRefining)
                | (Self::PbrRefining, Self::GlbReadback)
                | (Self::GlbReadback, Self::MultiviewReview)
                | (Self::MultiviewReview, Self::Ready)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NeuralVisualGenerationJob {
    pub schema_version: String,
    pub job_id: String,
    pub request_id: String,
    pub project_id: String,
    pub turn_id: String,
    pub stage: NeuralVisualStage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_backend: Option<Neural3DBackend>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_job_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_code: Option<String>,
}

impl NeuralVisualGenerationJob {
    pub fn queued(job_id: String, request: &Neural3DGenerationRequest) -> CoreResult<Self> {
        request.validate()?;
        let job = Self {
            schema_version: NEURAL_VISUAL_GENERATION_JOB_SCHEMA_VERSION.into(),
            job_id,
            request_id: request.request_id.clone(),
            project_id: request.project_id.clone(),
            turn_id: request.turn_id.clone(),
            stage: NeuralVisualStage::Queued,
            selected_backend: None,
            provider_job_id: None,
            terminal_code: None,
        };
        job.validate()?;
        Ok(job)
    }

    pub fn bind_backend(
        &mut self,
        backend: Neural3DBackend,
        provider_job_id: String,
    ) -> CoreResult<()> {
        if self.stage != NeuralVisualStage::ConceptReady
            || self.selected_backend.is_some()
            || self.provider_job_id.is_some()
        {
            return Err(CoreError::conflict(
                "NEURAL_VISUAL_BACKEND_BINDING_STATE_INVALID",
                "A neural backend can be bound exactly once after the concept reference is ready.",
            ));
        }
        require_id("provider_job_id", &provider_job_id)?;
        self.selected_backend = Some(backend);
        self.provider_job_id = Some(provider_job_id);
        Ok(())
    }

    pub fn advance(&mut self, next: NeuralVisualStage) -> CoreResult<()> {
        if self.stage.is_terminal() || !self.stage.can_advance_to(next) {
            return Err(CoreError::conflict(
                "NEURAL_VISUAL_STAGE_TRANSITION_INVALID",
                "Neural visual generation stages must advance in their exact reviewed order.",
            ));
        }
        if matches!(
            next,
            NeuralVisualStage::GeometryGenerating
                | NeuralVisualStage::PbrRefining
                | NeuralVisualStage::GlbReadback
                | NeuralVisualStage::MultiviewReview
                | NeuralVisualStage::Ready
        ) && (self.selected_backend.is_none() || self.provider_job_id.is_none())
        {
            return Err(CoreError::conflict(
                "NEURAL_VISUAL_BACKEND_BINDING_REQUIRED",
                "Geometry generation cannot start before Rust records the selected backend job.",
            ));
        }
        self.stage = next;
        self.validate()
    }

    pub fn fail(&mut self, code: String) -> CoreResult<()> {
        self.finish(NeuralVisualStage::Failed, code)
    }

    pub fn cancel(&mut self, code: String) -> CoreResult<()> {
        self.finish(NeuralVisualStage::Cancelled, code)
    }

    fn finish(&mut self, stage: NeuralVisualStage, code: String) -> CoreResult<()> {
        if self.stage.is_terminal() {
            return Err(CoreError::conflict(
                "NEURAL_VISUAL_JOB_TERMINAL",
                "A terminal neural visual job cannot transition again.",
            ));
        }
        require_code(&code)?;
        self.stage = stage;
        self.terminal_code = Some(code);
        self.validate()
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != NEURAL_VISUAL_GENERATION_JOB_SCHEMA_VERSION {
            return Err(invalid(
                "NEURAL_VISUAL_JOB_SCHEMA_INVALID",
                "Neural visual generation job must use its exact v1 schema.",
            ));
        }
        for (field, value) in [
            ("job_id", self.job_id.as_str()),
            ("request_id", self.request_id.as_str()),
            ("project_id", self.project_id.as_str()),
            ("turn_id", self.turn_id.as_str()),
        ] {
            require_id(field, value)?;
        }
        if self.selected_backend.is_some() != self.provider_job_id.is_some() {
            return Err(invalid(
                "NEURAL_VISUAL_BACKEND_BINDING_INVALID",
                "Selected backend and provider job identity must be present together.",
            ));
        }
        if let Some(provider_job_id) = &self.provider_job_id {
            require_id("provider_job_id", provider_job_id)?;
        }
        match (self.stage, self.terminal_code.as_deref()) {
            (NeuralVisualStage::Failed | NeuralVisualStage::Cancelled, Some(code)) => {
                require_code(code)
            }
            (NeuralVisualStage::Failed | NeuralVisualStage::Cancelled, None) => Err(invalid(
                "NEURAL_VISUAL_TERMINAL_CODE_REQUIRED",
                "Failed and cancelled jobs require one stable terminal code.",
            )),
            (_, Some(_)) => Err(invalid(
                "NEURAL_VISUAL_TERMINAL_CODE_INVALID",
                "Only failed or cancelled jobs may carry a terminal code.",
            )),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NeuralVisualArtifact {
    pub schema_version: String,
    pub artifact_id: String,
    pub job_id: String,
    pub project_id: String,
    pub turn_id: String,
    pub source_kind: String,
    pub backend: Neural3DBackend,
    pub provider_job_id: String,
    pub concept_reference_sha256: String,
    pub glb_object_sha256: String,
    pub glb_byte_size: u64,
    pub triangle_count: u64,
    pub material_count: u64,
    pub pbr_channels: BTreeSet<PbrChannel>,
    pub multiview_render_count: u8,
    pub multiview_bundle_sha256: String,
    pub quality_report_sha256: String,
    pub hidden_surface_policy: HiddenSurfacePolicy,
}

impl NeuralVisualArtifact {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != NEURAL_VISUAL_ARTIFACT_SCHEMA_VERSION
            || self.source_kind != "neural_visual_glb"
        {
            return Err(invalid(
                "NEURAL_VISUAL_ARTIFACT_SCHEMA_INVALID",
                "Neural visual artifact must use its exact v1 schema and source kind.",
            ));
        }
        for (field, value) in [
            ("artifact_id", self.artifact_id.as_str()),
            ("job_id", self.job_id.as_str()),
            ("project_id", self.project_id.as_str()),
            ("turn_id", self.turn_id.as_str()),
            ("provider_job_id", self.provider_job_id.as_str()),
        ] {
            require_id(field, value)?;
        }
        for (field, value) in [
            (
                "concept_reference_sha256",
                self.concept_reference_sha256.as_str(),
            ),
            ("glb_object_sha256", self.glb_object_sha256.as_str()),
            (
                "multiview_bundle_sha256",
                self.multiview_bundle_sha256.as_str(),
            ),
            ("quality_report_sha256", self.quality_report_sha256.as_str()),
        ] {
            require_sha256(field, value)?;
        }
        if self.glb_byte_size == 0
            || self.triangle_count == 0
            || self.material_count == 0
            || self.multiview_render_count != REQUIRED_MULTIVIEW_RENDER_COUNT
        {
            return Err(invalid(
                "NEURAL_VISUAL_ARTIFACT_READBACK_INVALID",
                "Accepted neural GLB artifacts require non-empty geometry/material readback and exactly eight reviewed views.",
            ));
        }
        let required = [
            PbrChannel::BaseColor,
            PbrChannel::Normal,
            PbrChannel::Roughness,
            PbrChannel::Metallic,
        ];
        if required
            .iter()
            .any(|channel| !self.pbr_channels.contains(channel))
        {
            return Err(invalid(
                "NEURAL_VISUAL_ARTIFACT_PBR_INCOMPLETE",
                "Accepted neural GLB artifacts require Base Color, Normal, Roughness and Metallic evidence.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ForgeAssetPackageFile {
    pub relative_path: String,
    pub media_type: String,
    pub sha256: String,
    pub byte_size: u64,
}

impl ForgeAssetPackageFile {
    fn validate(&self) -> CoreResult<()> {
        if self.relative_path.is_empty()
            || self.relative_path.len() > 128
            || self.relative_path.starts_with('/')
            || self.relative_path.contains("..")
            || self.relative_path.contains('\\')
        {
            return Err(invalid(
                "FORGE_ASSET_PACKAGE_PATH_INVALID",
                "Package members must use bounded relative POSIX paths.",
            ));
        }
        require_text("media_type", &self.media_type, 128)?;
        require_sha256("sha256", &self.sha256)?;
        if self.byte_size == 0 {
            return Err(invalid(
                "FORGE_ASSET_PACKAGE_FILE_EMPTY",
                "Package members cannot be empty.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ForgeAssetPackage {
    pub schema_version: String,
    pub package_id: String,
    pub project_id: String,
    pub asset_version_id: String,
    pub source_artifact_sha256: String,
    pub files: Vec<ForgeAssetPackageFile>,
}

impl ForgeAssetPackage {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != FORGE_ASSET_PACKAGE_SCHEMA_VERSION {
            return Err(invalid(
                "FORGE_ASSET_PACKAGE_SCHEMA_INVALID",
                "Forge asset package must use its exact v1 schema.",
            ));
        }
        for (field, value) in [
            ("package_id", self.package_id.as_str()),
            ("project_id", self.project_id.as_str()),
            ("asset_version_id", self.asset_version_id.as_str()),
        ] {
            require_id(field, value)?;
        }
        require_sha256("source_artifact_sha256", &self.source_artifact_sha256)?;
        let required = [
            "asset.glb",
            "thumbnail.webp",
            "turntable.mp4",
            "manifest.json",
            "quality-report.json",
            "license-metadata.json",
        ];
        let paths = self
            .files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect::<BTreeSet<_>>();
        if self.files.len() != required.len()
            || paths.len() != self.files.len()
            || required.iter().any(|required| !paths.contains(required))
        {
            return Err(invalid(
                "FORGE_ASSET_PACKAGE_MEMBERS_INVALID",
                "ForgeAssetPackage@1 requires exactly the six canonical members.",
            ));
        }
        for file in &self.files {
            file.validate()?;
        }
        let asset = self
            .files
            .iter()
            .find(|file| file.relative_path == "asset.glb")
            .expect("required member checked above");
        if asset.sha256 != self.source_artifact_sha256 || asset.media_type != "model/gltf-binary" {
            return Err(invalid(
                "FORGE_ASSET_PACKAGE_SOURCE_MISMATCH",
                "asset.glb must exactly match the accepted source artifact digest and media type.",
            ));
        }
        Ok(())
    }
}

fn invalid(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message)
}

fn require_id(field: &str, value: &str) -> CoreResult<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'@'))
    {
        return Err(invalid(
            "NEURAL_VISUAL_ID_INVALID",
            format!("{field} must be a bounded stable identifier."),
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> CoreResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid(
            "NEURAL_VISUAL_SHA256_INVALID",
            format!("{field} must be a lowercase SHA-256 digest."),
        ));
    }
    Ok(())
}

fn require_text(field: &str, value: &str, max_len: usize) -> CoreResult<()> {
    if value.trim().is_empty() || value.len() > max_len || value.contains('\0') {
        return Err(invalid(
            "NEURAL_VISUAL_TEXT_INVALID",
            format!("{field} must be bounded non-empty text."),
        ));
    }
    Ok(())
}

fn require_terms(field: &str, values: &[String]) -> CoreResult<()> {
    if values.len() > 16 {
        return Err(invalid(
            "NEURAL_VISUAL_TERMS_INVALID",
            format!("{field} cannot contain more than 16 terms."),
        ));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        require_text(field, value, 96)?;
        if !unique.insert(value) {
            return Err(invalid(
                "NEURAL_VISUAL_TERMS_INVALID",
                format!("{field} cannot contain duplicate terms."),
            ));
        }
    }
    Ok(())
}

fn require_code(value: &str) -> CoreResult<()> {
    if value.is_empty()
        || value.len() > 96
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(invalid(
            "NEURAL_VISUAL_TERMINAL_CODE_INVALID",
            "Terminal codes must use bounded uppercase snake case.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    fn request() -> Neural3DGenerationRequest {
        Neural3DGenerationRequest {
            schema_version: NEURAL_3D_GENERATION_REQUEST_SCHEMA_VERSION.into(),
            request_id: "request_1".into(),
            project_id: "project_1".into(),
            turn_id: "turn_1".into(),
            brief_id: "brief_1".into(),
            concept_reference_id: "reference_1".into(),
            concept_reference_sha256: sha('a'),
            quality_tier: VisualQualityTier::StandardAsset,
            backend_preferences: vec![Neural3DBackend::Pixal3d, Neural3DBackend::Trellis2],
            idempotency_key: "idempotency_1".into(),
        }
    }

    #[test]
    fn visual_brief_requires_explicit_remote_consent_for_images() {
        let mut brief = VisualDesignBrief {
            schema_version: VISUAL_DESIGN_BRIEF_SCHEMA_VERSION.into(),
            brief_id: "brief_1".into(),
            project_id: "project_1".into(),
            turn_id: "turn_1".into(),
            input_kind: VisualInputKind::Image,
            user_intent_sha256: sha('b'),
            object_class: "fictional mechanical prop".into(),
            visual_summary: "Layered hard-surface prop with dark metal and blue emissive lines."
                .into(),
            style_terms: vec!["deep_sea".into()],
            material_terms: vec!["brushed_titanium".into()],
            input_evidence: vec![VisualInputEvidence {
                evidence_id: "evidence_1".into(),
                object_sha256: sha('c'),
                media_type: "image/png".into(),
                rights_confirmed: true,
                remote_processing_authorized: false,
            }],
        };
        assert_eq!(
            brief.validate().unwrap_err().code(),
            "VISUAL_INPUT_CONSENT_REQUIRED"
        );
        brief.input_evidence[0].remote_processing_authorized = true;
        brief.validate().unwrap();
    }

    #[test]
    fn concept_request_is_single_image_and_binds_authorized_evidence() {
        let brief = VisualDesignBrief {
            schema_version: VISUAL_DESIGN_BRIEF_SCHEMA_VERSION.into(),
            brief_id: "brief_image".into(),
            project_id: "project_1".into(),
            turn_id: "turn_1".into(),
            input_kind: VisualInputKind::Image,
            user_intent_sha256: sha('8'),
            object_class: "industrial tool".into(),
            visual_summary: "A compact industrial visual asset.".into(),
            style_terms: vec![],
            material_terms: vec![],
            input_evidence: vec![VisualInputEvidence {
                evidence_id: "evidence_1".into(),
                object_sha256: sha('9'),
                media_type: "image/png".into(),
                rights_confirmed: true,
                remote_processing_authorized: true,
            }],
        };
        let mut request = ConceptImageGenerationRequest {
            schema_version: CONCEPT_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION.into(),
            request_id: "concept_request_1".into(),
            project_id: "project_1".into(),
            turn_id: "turn_1".into(),
            brief_id: "brief_image".into(),
            prompt: "One isolated compact industrial tool, three-quarter view, clean background."
                .into(),
            input_image_object_sha256: Some(sha('9')),
            input_image_media_type: Some("image/png".into()),
            backend_preferences: vec![ConceptImageBackend::FalFlux2],
            width: 1024,
            height: 1024,
            output_media_type: "image/png".into(),
            isolated_subject: true,
            clean_background: true,
            image_count: 1,
            idempotency_key: "concept_key_1".into(),
        };
        request.validate_against(&brief).unwrap();
        request.input_image_media_type = Some("image/jpeg".into());
        assert_eq!(
            request.validate_against(&brief).unwrap_err().code(),
            "CONCEPT_IMAGE_REQUEST_INPUT_MISMATCH"
        );
        request.input_image_media_type = Some("image/png".into());
        request.input_image_object_sha256 = Some(sha('7'));
        assert_eq!(
            request.validate_against(&brief).unwrap_err().code(),
            "CONCEPT_IMAGE_REQUEST_INPUT_MISMATCH"
        );
    }

    #[test]
    fn neural_job_enforces_order_backend_binding_and_terminal_state() {
        let request = request();
        let mut job =
            NeuralVisualGenerationJob::queued("job_1".into(), &request).expect("queued job");
        assert_eq!(
            job.advance(NeuralVisualStage::GeometryGenerating)
                .unwrap_err()
                .code(),
            "NEURAL_VISUAL_STAGE_TRANSITION_INVALID"
        );
        job.advance(NeuralVisualStage::ConceptReady).unwrap();
        assert_eq!(
            job.advance(NeuralVisualStage::GeometryGenerating)
                .unwrap_err()
                .code(),
            "NEURAL_VISUAL_BACKEND_BINDING_REQUIRED"
        );
        job.bind_backend(Neural3DBackend::Pixal3d, "pixal_job_1".into())
            .unwrap();
        for stage in [
            NeuralVisualStage::GeometryGenerating,
            NeuralVisualStage::PbrRefining,
            NeuralVisualStage::GlbReadback,
            NeuralVisualStage::MultiviewReview,
            NeuralVisualStage::Ready,
        ] {
            job.advance(stage).unwrap();
        }
        assert_eq!(
            job.cancel("USER_CANCELLED".into()).unwrap_err().code(),
            "NEURAL_VISUAL_JOB_TERMINAL"
        );
    }

    #[test]
    fn accepted_artifact_requires_pbr_and_exactly_eight_views() {
        let mut artifact = NeuralVisualArtifact {
            schema_version: NEURAL_VISUAL_ARTIFACT_SCHEMA_VERSION.into(),
            artifact_id: "artifact_1".into(),
            job_id: "job_1".into(),
            project_id: "project_1".into(),
            turn_id: "turn_1".into(),
            source_kind: "neural_visual_glb".into(),
            backend: Neural3DBackend::Trellis2,
            provider_job_id: "trellis_job_1".into(),
            concept_reference_sha256: sha('d'),
            glb_object_sha256: sha('e'),
            glb_byte_size: 1024,
            triangle_count: 42_000,
            material_count: 3,
            pbr_channels: [
                PbrChannel::BaseColor,
                PbrChannel::Normal,
                PbrChannel::Roughness,
                PbrChannel::Metallic,
            ]
            .into_iter()
            .collect(),
            multiview_render_count: REQUIRED_MULTIVIEW_RENDER_COUNT,
            multiview_bundle_sha256: sha('f'),
            quality_report_sha256: sha('1'),
            hidden_surface_policy: HiddenSurfacePolicy::AiInferred,
        };
        artifact.validate().unwrap();
        artifact.pbr_channels.remove(&PbrChannel::Normal);
        assert_eq!(
            artifact.validate().unwrap_err().code(),
            "NEURAL_VISUAL_ARTIFACT_PBR_INCOMPLETE"
        );
    }

    #[test]
    fn package_is_exact_and_binds_asset_digest() {
        let source = sha('2');
        let members = [
            ("asset.glb", "model/gltf-binary", source.clone()),
            ("thumbnail.webp", "image/webp", sha('3')),
            ("turntable.mp4", "video/mp4", sha('4')),
            ("manifest.json", "application/json", sha('5')),
            ("quality-report.json", "application/json", sha('6')),
            ("license-metadata.json", "application/json", sha('7')),
        ]
        .into_iter()
        .map(
            |(relative_path, media_type, sha256)| ForgeAssetPackageFile {
                relative_path: relative_path.into(),
                media_type: media_type.into(),
                sha256,
                byte_size: 100,
            },
        )
        .collect();
        let package = ForgeAssetPackage {
            schema_version: FORGE_ASSET_PACKAGE_SCHEMA_VERSION.into(),
            package_id: "package_1".into(),
            project_id: "project_1".into(),
            asset_version_id: "asset_version_1".into(),
            source_artifact_sha256: source,
            files: members,
        };
        package.validate().unwrap();
    }

    #[test]
    fn concept_png_requires_a_fully_decodable_exact_1024_square() {
        use std::io::Cursor;

        let image = image::RgbaImage::from_pixel(1024, 1024, image::Rgba([12, 20, 30, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        let inspected = inspect_concept_png(&bytes).unwrap();
        assert_eq!((inspected.width, inspected.height), (1024, 1024));
        assert_eq!(inspected.byte_size, bytes.len() as u64);

        bytes.truncate(bytes.len() / 2);
        assert_eq!(
            inspect_concept_png(&bytes).unwrap_err().code(),
            "CONCEPT_PNG_DECODE_INVALID"
        );
    }

    #[test]
    fn neural_glb_readback_accepts_a_bounded_embedded_triangle() {
        let glb = minimal_embedded_triangle_glb();
        let inspected = inspect_neural_visual_glb(&glb).unwrap();
        assert_eq!(inspected.triangle_count, 1);
        assert_eq!(inspected.mesh_count, 1);
        assert_eq!(inspected.primitive_count, 1);
        assert!(!inspected.every_primitive_has_uv0);
        assert!(!inspected.every_primitive_has_tangent);

        let mut truncated = glb;
        truncated.pop();
        assert_eq!(
            inspect_neural_visual_glb(&truncated).unwrap_err().code(),
            "NEURAL_VISUAL_GLB_REJECTED"
        );
    }

    fn minimal_embedded_triangle_glb() -> Vec<u8> {
        let mut binary = Vec::new();
        for value in [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            binary.extend_from_slice(&value.to_le_bytes());
        }
        for index in [0_u16, 1, 2] {
            binary.extend_from_slice(&index.to_le_bytes());
        }
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        let document = serde_json::json!({
            "asset": {"version": "2.0"},
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [{"mesh": 0}],
            "buffers": [{"byteLength": binary.len()}],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": 36},
                {"buffer": 0, "byteOffset": 36, "byteLength": 6}
            ],
            "accessors": [
                {
                    "bufferView": 0,
                    "componentType": 5126,
                    "count": 3,
                    "type": "VEC3",
                    "min": [0.0, 0.0, 0.0],
                    "max": [1.0, 1.0, 0.0]
                },
                {
                    "bufferView": 1,
                    "componentType": 5123,
                    "count": 3,
                    "type": "SCALAR"
                }
            ],
            "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1}]}]
        });
        let mut json = serde_json::to_vec(&document).unwrap();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let total_length = 12 + 8 + json.len() + 8 + binary.len();
        let mut glb = Vec::with_capacity(total_length);
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&(total_length as u32).to_le_bytes());
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x4e4f534a_u32.to_le_bytes());
        glb.extend_from_slice(&json);
        glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x004e4942_u32.to_le_bytes());
        glb.extend_from_slice(&binary);
        glb
    }
}
