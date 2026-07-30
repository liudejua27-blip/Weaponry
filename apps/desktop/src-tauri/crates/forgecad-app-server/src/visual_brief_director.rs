//! Bounded DeepSeek visual-direction pass for the visual-first MVP.
//!
//! The Provider may describe appearance, but it never chooses product IDs,
//! consent, lineage, backend, image count or persistence state. Rust derives
//! those facts and validates the resulting Core contracts before returning.

use std::sync::Arc;

use forgecad_core::{
    ConceptImageBackend, ConceptImageGenerationRequest, VisualDesignBrief, VisualInputEvidence,
    VisualInputKind, CONCEPT_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION,
    VISUAL_DESIGN_BRIEF_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{
    CancellationToken, ProviderClient, ProviderError, ProviderFinishReason, ProviderMessage,
    ProviderRequest, ProviderRole, ProviderToolDefinition,
};

const VISUAL_BRIEF_TOOL_NAME: &str = "submit_visual_design_direction";
const MAX_USER_INTENT_BYTES: usize = 8_192;
const MAX_VISUAL_BRIEF_OUTPUT_TOKENS: u64 = 2_048;
const MAX_CONCEPT_PROMPT_BYTES: usize = 4_096;
const REFERENCE_COMPLETION_GUARD: &str = "The supplied reference image is authoritative for every visible feature. Preserve the exact subject identity, visible silhouette, proportions, pose, part layout, material zones, colors, panel seams, openings, lights and asymmetry. Do not redesign, simplify, stylize, replace or add unrelated parts. Only reconstruct cropped or occluded portions coherently, remove the background, and expand the canvas so the entire single subject is visible from head/top to feet/base in a centered three-quarter product view. Keep fine visible details sharp. Clean neutral background, no text, no stand, no detached parts.";

const SYSTEM_MESSAGE: &str = r#"You are Forge Studio's visual director.
Convert the user's request into one visually coherent 3D asset direction.
Use the submit_visual_design_direction tool exactly once.
Describe only visible exterior form, materials and surface language.
The concept prompt must request one complete isolated object, centered in a
three-quarter view, fully visible, no detached parts, no people, no labels or
unrelated text, and a clean neutral background suitable for image-to-3D.
For fictional weapons, stay strictly within non-functional game/film prop
appearance: never include dimensions, mechanisms, manufacturing, ammunition,
performance or operational instructions.
Do not invent uploaded-reference rights, consent, IDs, hashes or backend names."#;

#[derive(Clone)]
pub struct VisualBriefDirector {
    provider: Arc<dyn ProviderClient>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualBriefDirectorInput {
    pub project_id: String,
    pub turn_id: String,
    pub user_intent: String,
    #[serde(default)]
    pub input_evidence: Vec<VisualInputEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualBriefDirectorOutput {
    pub brief: VisualDesignBrief,
    pub concept_request: ConceptImageGenerationRequest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderVisualDirection {
    object_class: String,
    visual_summary: String,
    style_terms: Vec<String>,
    material_terms: Vec<String>,
    concept_prompt: String,
}

impl VisualBriefDirector {
    pub fn new(provider: Arc<dyn ProviderClient>) -> Self {
        Self { provider }
    }

    pub async fn direct(
        &self,
        input: VisualBriefDirectorInput,
        cancellation: CancellationToken,
    ) -> Result<VisualBriefDirectorOutput, ProviderError> {
        validate_input(&input)?;
        if cancellation.is_cancelled() {
            return Err(ProviderError::cancelled(false));
        }

        let provider = self
            .provider
            .turn_session()?
            .unwrap_or_else(|| self.provider.clone());
        let preflight = provider.preflight(cancellation.clone()).await?;
        if !preflight.configured || !preflight.tool_calls {
            return Err(ProviderError::schema_mismatch_with_code(
                "VISUAL_BRIEF_PROVIDER_UNAVAILABLE",
                "The configured Provider cannot produce the required visual brief tool call.",
                preflight.network_call_made,
            ));
        }

        let user_intent_sha256 = sha256(input.user_intent.trim().as_bytes());
        let context_digest = sha256(
            format!(
                "visual-brief@1:{}:{}:{}",
                input.project_id, input.turn_id, user_intent_sha256
            )
            .as_bytes(),
        );
        let request = ProviderRequest {
            provider_id: preflight.provider_id,
            model: preflight.model,
            context_digest,
            messages: vec![
                ProviderMessage {
                    role: ProviderRole::System,
                    content: SYSTEM_MESSAGE.into(),
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    ephemeral_reasoning: None,
                },
                ProviderMessage {
                    role: ProviderRole::User,
                    content: input.user_intent.trim().into(),
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    ephemeral_reasoning: None,
                },
            ],
            tools: vec![visual_direction_tool()],
            require_tool_call: true,
            max_output_tokens: MAX_VISUAL_BRIEF_OUTPUT_TOKENS,
        };
        provider.request_budget_policy(&request)?.validate()?;
        let response = provider
            .stream(request, cancellation, Box::new(|_| {}))
            .await?
            .validate()?;
        if response.finish_reason != ProviderFinishReason::ToolCalls
            || response.tool_calls.len() != 1
            || response.tool_calls[0].name != VISUAL_BRIEF_TOOL_NAME
        {
            return Err(ProviderError::schema_mismatch_with_code(
                "VISUAL_BRIEF_TOOL_CALL_INVALID",
                "The Provider did not return exactly one visual direction tool call.",
                response.network_call_made,
            ));
        }
        let direction: ProviderVisualDirection =
            serde_json::from_value(response.tool_calls[0].arguments.clone()).map_err(|_| {
                ProviderError::schema_mismatch_with_code(
                    "VISUAL_BRIEF_OUTPUT_INVALID",
                    "The Provider visual direction did not match the reviewed schema.",
                    response.network_call_made,
                )
            })?;

        let seed = sha256(
            format!(
                "{}:{}:{}",
                input.project_id, input.turn_id, user_intent_sha256
            )
            .as_bytes(),
        );
        let brief_id = stable_id("visual_brief", &seed);
        let brief = VisualDesignBrief {
            schema_version: VISUAL_DESIGN_BRIEF_SCHEMA_VERSION.into(),
            brief_id: brief_id.clone(),
            project_id: input.project_id.clone(),
            turn_id: input.turn_id.clone(),
            input_kind: match input.input_evidence.is_empty() {
                true => VisualInputKind::Text,
                false => VisualInputKind::TextAndImage,
            },
            user_intent_sha256,
            object_class: direction.object_class,
            visual_summary: direction.visual_summary,
            style_terms: direction.style_terms,
            material_terms: direction.material_terms,
            input_evidence: input.input_evidence,
        };
        brief.validate().map_err(|_| {
            ProviderError::schema_mismatch_with_code(
                "VISUAL_BRIEF_OUTPUT_INVALID",
                "The Provider visual direction failed the Rust visual brief contract.",
                response.network_call_made,
            )
        })?;

        let concept_prompt = if brief.input_evidence.is_empty() {
            direction.concept_prompt
        } else {
            reference_completion_prompt(&direction.concept_prompt)
        };
        let concept_request = ConceptImageGenerationRequest {
            schema_version: CONCEPT_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION.into(),
            request_id: stable_id("concept_request", &seed),
            project_id: brief.project_id.clone(),
            turn_id: brief.turn_id.clone(),
            brief_id,
            prompt: concept_prompt,
            input_image_object_sha256: brief
                .input_evidence
                .first()
                .map(|evidence| evidence.object_sha256.clone()),
            input_image_media_type: brief
                .input_evidence
                .first()
                .map(|evidence| evidence.media_type.clone()),
            backend_preferences: vec![ConceptImageBackend::FalFlux2],
            width: 1024,
            height: 1024,
            output_media_type: "image/png".into(),
            isolated_subject: true,
            clean_background: true,
            image_count: 1,
            idempotency_key: stable_id("concept_idempotency", &seed),
        };
        concept_request.validate_against(&brief).map_err(|_| {
            ProviderError::schema_mismatch_with_code(
                "VISUAL_BRIEF_CONCEPT_REQUEST_INVALID",
                "The visual direction could not form a valid concept image request.",
                response.network_call_made,
            )
        })?;

        Ok(VisualBriefDirectorOutput {
            brief,
            concept_request,
        })
    }
}

fn validate_input(input: &VisualBriefDirectorInput) -> Result<(), ProviderError> {
    if !valid_id(&input.project_id) || !valid_id(&input.turn_id) {
        return Err(ProviderError::schema_mismatch_with_code(
            "VISUAL_BRIEF_INPUT_INVALID",
            "Visual brief project and turn identifiers are invalid.",
            false,
        ));
    }
    let intent = input.user_intent.trim();
    if intent.is_empty() || intent.len() > MAX_USER_INTENT_BYTES || intent.contains('\0') {
        return Err(ProviderError::schema_mismatch_with_code(
            "VISUAL_BRIEF_INPUT_INVALID",
            "Visual brief intent must be bounded non-empty text.",
            false,
        ));
    }
    for evidence in &input.input_evidence {
        evidence.validate().map_err(|_| {
            ProviderError::schema_mismatch_with_code(
                "VISUAL_BRIEF_EVIDENCE_INVALID",
                "Visual brief reference evidence requires valid rights and remote consent.",
                false,
            )
        })?;
    }
    Ok(())
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'@'))
}

fn stable_id(prefix: &str, digest: &str) -> String {
    format!("{prefix}_{}", &digest[..24])
}

fn reference_completion_prompt(provider_prompt: &str) -> String {
    let separator = "\n\n";
    let maximum_provider_bytes = MAX_CONCEPT_PROMPT_BYTES
        .saturating_sub(REFERENCE_COMPLETION_GUARD.len())
        .saturating_sub(separator.len());
    let mut end = provider_prompt.len().min(maximum_provider_bytes);
    while end > 0 && !provider_prompt.is_char_boundary(end) {
        end -= 1;
    }
    let provider_direction = provider_prompt[..end].trim();
    if provider_direction.is_empty() {
        REFERENCE_COMPLETION_GUARD.into()
    } else {
        format!("{REFERENCE_COMPLETION_GUARD}{separator}{provider_direction}")
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn visual_direction_tool() -> ProviderToolDefinition {
    ProviderToolDefinition {
        name: VISUAL_BRIEF_TOOL_NAME.into(),
        description: "Submit one bounded exterior visual design direction.".into(),
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "required": [
                "object_class",
                "visual_summary",
                "style_terms",
                "material_terms",
                "concept_prompt"
            ],
            "properties": {
                "object_class": {"type": "string", "minLength": 1, "maxLength": 128},
                "visual_summary": {"type": "string", "minLength": 1, "maxLength": 2048},
                "style_terms": {
                    "type": "array",
                    "maxItems": 16,
                    "uniqueItems": true,
                    "items": {"type": "string", "minLength": 1, "maxLength": 96}
                },
                "material_terms": {
                    "type": "array",
                    "maxItems": 16,
                    "uniqueItems": true,
                    "items": {"type": "string", "minLength": 1, "maxLength": 96}
                },
                "concept_prompt": {"type": "string", "minLength": 1, "maxLength": 4096}
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EphemeralReasoning, FakeDeepSeekClient, ProviderFinishReason, ProviderResponse,
        ProviderToolCall, ProviderUsage,
    };

    fn response(arguments: serde_json::Value) -> ProviderResponse {
        ProviderResponse {
            content: None,
            tool_calls: vec![ProviderToolCall {
                call_id: "call_visual_1".into(),
                name: VISUAL_BRIEF_TOOL_NAME.into(),
                arguments,
            }],
            ephemeral_reasoning: Some(EphemeralReasoning::new("private")),
            usage: ProviderUsage {
                input_tokens: 50,
                output_tokens: 30,
                prompt_cache_hit_tokens: 0,
                prompt_cache_miss_tokens: 50,
                estimated_cost_microusd: 10,
            },
            finish_reason: ProviderFinishReason::ToolCalls,
            network_call_made: true,
        }
    }

    fn input() -> VisualBriefDirectorInput {
        VisualBriefDirectorInput {
            project_id: "project_1".into(),
            turn_id: "turn_1".into(),
            user_intent: "设计一个深海文明风格的精致机械道具".into(),
            input_evidence: Vec::new(),
        }
    }

    #[test]
    fn provider_only_supplies_visual_fields_while_rust_owns_contract_facts() {
        let provider = FakeDeepSeekClient::scripted(
            "deepseek-chat",
            true,
            true,
            vec![Ok(response(json!({
                "object_class": "fictional mechanical prop",
                "visual_summary": "Layered dark metal shell with blue bioluminescent seams.",
                "style_terms": ["deep sea", "premium hard surface"],
                "material_terms": ["brushed titanium", "dark ceramic"],
                "concept_prompt": "One complete isolated fictional mechanical prop, centered three-quarter view, fully visible, dark titanium and ceramic, blue bioluminescent seams, clean neutral background, no text."
            })))],
        );
        let director = VisualBriefDirector::new(Arc::new(provider.clone()));
        let output = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(director.direct(input(), CancellationToken::new()))
            .unwrap();

        output.brief.validate().unwrap();
        output
            .concept_request
            .validate_against(&output.brief)
            .unwrap();
        assert_eq!(output.brief.input_kind, VisualInputKind::Text);
        assert_eq!(
            output.concept_request.backend_preferences,
            vec![ConceptImageBackend::FalFlux2]
        );
        assert_eq!(provider.records().len(), 1);
        assert_eq!(
            provider.records()[0].tool_names,
            vec![VISUAL_BRIEF_TOOL_NAME]
        );
        let serialized = serde_json::to_string(&provider.records()).unwrap();
        assert!(!serialized.contains("深海"));
        assert!(!serialized.contains("private"));
    }

    #[test]
    fn malformed_provider_direction_fails_closed_without_a_brief() {
        let provider = FakeDeepSeekClient::scripted(
            "deepseek-chat",
            true,
            true,
            vec![Ok(response(json!({
                "object_class": "tool",
                "visual_summary": "A tool.",
                "style_terms": [],
                "material_terms": []
            })))],
        );
        let director = VisualBriefDirector::new(Arc::new(provider));
        let error = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(director.direct(input(), CancellationToken::new()))
            .unwrap_err();
        assert_eq!(error.code, "VISUAL_BRIEF_OUTPUT_INVALID");
    }

    #[test]
    fn image_evidence_requires_rights_and_explicit_remote_processing_consent() {
        let mut bad = input();
        bad.input_evidence.push(VisualInputEvidence {
            evidence_id: "evidence_1".into(),
            object_sha256: "a".repeat(64),
            media_type: "image/png".into(),
            rights_confirmed: true,
            remote_processing_authorized: false,
        });
        let director = VisualBriefDirector::new(Arc::new(FakeDeepSeekClient::scripted(
            "deepseek-chat",
            true,
            true,
            Vec::new(),
        )));
        let error = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(director.direct(bad, CancellationToken::new()))
            .unwrap_err();
        assert_eq!(error.code, "VISUAL_BRIEF_EVIDENCE_INVALID");
    }

    #[test]
    fn image_evidence_gets_a_rust_owned_identity_preserving_completion_prompt() {
        let provider_prompt = "Make a complete premium object on a clean background.";
        let provider = FakeDeepSeekClient::scripted(
            "deepseek-chat",
            true,
            true,
            vec![Ok(response(json!({
                "object_class": "open subject",
                "visual_summary": "Preserve the uploaded subject.",
                "style_terms": ["reference faithful"],
                "material_terms": ["reference materials"],
                "concept_prompt": provider_prompt
            })))],
        );
        let mut request = input();
        request.input_evidence.push(VisualInputEvidence {
            evidence_id: "evidence_1".into(),
            object_sha256: "a".repeat(64),
            media_type: "image/png".into(),
            rights_confirmed: true,
            remote_processing_authorized: true,
        });
        let output = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(
                VisualBriefDirector::new(Arc::new(provider))
                    .direct(request, CancellationToken::new()),
            )
            .unwrap();

        assert!(output
            .concept_request
            .prompt
            .starts_with("The supplied reference image is authoritative"));
        assert!(output.concept_request.prompt.contains("Do not redesign"));
        assert!(output
            .concept_request
            .prompt
            .contains("entire single subject"));
        assert!(output.concept_request.prompt.ends_with(provider_prompt));
        assert!(output.concept_request.prompt.len() <= MAX_CONCEPT_PROMPT_BYTES);
        output
            .concept_request
            .validate_against(&output.brief)
            .unwrap();
    }
}
