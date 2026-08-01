//! FGC-E005-R2 one-call sealed visual review and bounded rebuild.
//!
//! One validated unified author source is rendered as a generic turntable.
//! The Provider receives only exact sealed reference bytes, exact candidate
//! PNG bytes and bounded typed source JSON. It returns assessments plus one
//! ephemeral accept/patch proposal in the same call. Rust derives the report,
//! seals the proposal, and either reuses the accepted candidate or performs
//! exactly one deterministic rebuild. A patched result is deliberately marked
//! as not having received a second visual-model review.

use std::sync::Arc;

use forgecad_core::{
    apply_e005_visual_patch_v1, lower_forge_visual_author_source_v1,
    lower_visual_runtime_source_v1, normalized_geometry_sha256, semantic_sha256,
    E005VisualDecisionKindV1, E005VisualPatchV1, MultimodalDesignRequest, ReferenceEvidence,
    VisualEvidenceGraph, VisualFixedViewEvidence, VisualProgramCacheDispositionV2,
    VisualProgramPhaseReceiptV2, VisualProgramPhaseV2, VisualProgramUsageV2,
    VisualReferenceAcceptancePolicy, VisualReferenceComparisonInput,
    VisualReferenceComparisonReport,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    e005_production_review::compile_e005_surface_adornments, CancellationToken,
    E005VisualReferenceReviewOutcome, RestrictedGeometryError, RestrictedGeometryInput,
    RestrictedGeometryOutput, RestrictedGeometryPort, RestrictedQualityProfile,
    RestrictedRenderViewProfile, VisualReferenceComparisonCoordinator,
    VisualReferenceComparisonImage, VisualReferenceComparisonProviderError,
    VisualReferenceComparisonProviderOutput, VisualReferenceComparisonProviderRequest,
    RESTRICTED_GEOMETRY_INPUT_SCHEMA_VERSION, RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION,
};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum E005VisualReviewStatusV1 {
    AcceptedByVisualReview,
    PatchedPendingVisualConfirmation,
}

pub const E005_VISUAL_REVIEW_EVIDENCE_SCHEMA_VERSION: &str = "E005VisualReviewEvidence@1";
pub const E005_VISUAL_SESSION_SCHEMA_VERSION: &str = "E005VisualSession@1";
pub const E005_VISUAL_SESSION_RECEIPT_SCHEMA_VERSION: &str = "E005VisualSessionReceipt@1";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005VisualSessionReceiptV1 {
    pub schema_version: String,
    pub receipt_id: String,
    pub session_id: String,
    pub task_payload_sha256: String,
    pub request_sha256: String,
    pub source_program_sha256: String,
    pub expanded_program_sha256: String,
    pub shape_program_sha256: String,
    pub glb_sha256: String,
    pub normalized_geometry_sha256: String,
    pub fixed_view_sha256: String,
    pub compile_readback_sha256: String,
    pub restricted_geometry_evidence_sha256: String,
    pub comparison_report_sha256: String,
    pub phases: Vec<VisualProgramPhaseReceiptV2>,
    pub usage: VisualProgramUsageV2,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct E005VisualSessionV1 {
    pub schema_version: String,
    pub session_id: String,
    pub initial_source_sha256: String,
    pub final_source_sha256: String,
    pub state: E005VisualReviewStatusV1,
    pub visual_patch_sha256: String,
    pub review_evidence: E005VisualReviewEvidenceV1,
    pub receipt: E005VisualSessionReceiptV1,
    pub receipt_sha256: String,
}

impl E005VisualSessionV1 {
    pub fn from_result(
        task_payload_sha256: &str,
        request_sha256: &str,
        result: &E005VisualReviewResultV1,
        usage: VisualProgramUsageV2,
    ) -> Result<Self, forgecad_core::CoreError> {
        let valid_hash = |value: &str| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        };
        if !valid_hash(task_payload_sha256) || !valid_hash(request_sha256) {
            return Err(forgecad_core::CoreError::invalid_data(
                "E005_R2_VISUAL_SESSION_REQUEST_INVALID",
                "E005 visual session requires exact task and request SHA-256 bindings.",
            ));
        }
        let review_evidence = E005VisualReviewEvidenceV1::from_result(result)?;
        let initial_lowering = lower_visual_runtime_source_v1(&result.initial_source)?;
        let lowering = lower_visual_runtime_source_v1(&result.final_source)?;
        let initial_fixed_view_sha256 = semantic_sha256(&result.initial_geometry.view_sha256)?;
        let fixed_view_sha256 = semantic_sha256(&result.final_geometry.view_sha256)?;
        let restricted_geometry_evidence_sha256 =
            semantic_sha256(&result.final_geometry.execution_evidence)?;
        let normalized_geometry_sha256 =
            normalized_geometry_sha256(&result.final_geometry.glb_bytes)?;
        let visual_patch_sha256 = semantic_sha256(&result.visual_patch)?;
        let suffix = &request_sha256[..16];
        let session_id = format!("visualsession_e005_{suffix}");
        let mut chain = vec![
            (
                VisualProgramPhaseV2::Author,
                task_payload_sha256.to_owned(),
                result.initial_source_sha256.clone(),
                0,
                VisualProgramCacheDispositionV2::NotApplicable,
            ),
            (
                VisualProgramPhaseV2::Validate,
                result.initial_source_sha256.clone(),
                result.initial_source_sha256.clone(),
                0,
                VisualProgramCacheDispositionV2::NotApplicable,
            ),
            (
                VisualProgramPhaseV2::Expand,
                result.initial_source_sha256.clone(),
                initial_lowering.expanded_program_sha256.clone(),
                0,
                VisualProgramCacheDispositionV2::NotApplicable,
            ),
            (
                VisualProgramPhaseV2::Lower,
                initial_lowering.expanded_program_sha256.clone(),
                initial_lowering.shape_program_sha256.clone(),
                0,
                VisualProgramCacheDispositionV2::NotApplicable,
            ),
            (
                VisualProgramPhaseV2::CompileReadback,
                initial_lowering.shape_program_sha256.clone(),
                result
                    .initial_geometry
                    .readback
                    .compile_readback_sha256
                    .clone(),
                result
                    .initial_geometry
                    .execution_evidence
                    .compile_duration_ms,
                if result.initial_geometry.execution_evidence.compile_cache_hit {
                    VisualProgramCacheDispositionV2::Hit
                } else {
                    VisualProgramCacheDispositionV2::Miss
                },
            ),
            (
                VisualProgramPhaseV2::Render,
                result
                    .initial_geometry
                    .readback
                    .compile_readback_sha256
                    .clone(),
                initial_fixed_view_sha256,
                result
                    .initial_geometry
                    .execution_evidence
                    .render_duration_ms,
                VisualProgramCacheDispositionV2::NotApplicable,
            ),
            (
                VisualProgramPhaseV2::Evaluate,
                semantic_sha256(&result.initial_geometry.view_sha256)?,
                result.comparison_report.report_sha256.clone(),
                0,
                VisualProgramCacheDispositionV2::NotApplicable,
            ),
        ];
        if result.status == E005VisualReviewStatusV1::PatchedPendingVisualConfirmation {
            chain.push((
                VisualProgramPhaseV2::Patch,
                result.comparison_report.report_sha256.clone(),
                result.final_source_sha256.clone(),
                0,
                VisualProgramCacheDispositionV2::NotApplicable,
            ));
            chain.push((
                VisualProgramPhaseV2::Expand,
                result.final_source_sha256.clone(),
                lowering.expanded_program_sha256.clone(),
                0,
                VisualProgramCacheDispositionV2::NotApplicable,
            ));
            chain.push((
                VisualProgramPhaseV2::Lower,
                lowering.expanded_program_sha256.clone(),
                lowering.shape_program_sha256.clone(),
                0,
                VisualProgramCacheDispositionV2::NotApplicable,
            ));
            chain.push((
                VisualProgramPhaseV2::CompileReadback,
                lowering.shape_program_sha256.clone(),
                result
                    .final_geometry
                    .readback
                    .compile_readback_sha256
                    .clone(),
                result.final_geometry.execution_evidence.compile_duration_ms,
                if result.final_geometry.execution_evidence.compile_cache_hit {
                    VisualProgramCacheDispositionV2::Hit
                } else {
                    VisualProgramCacheDispositionV2::Miss
                },
            ));
            chain.push((
                VisualProgramPhaseV2::Render,
                result
                    .final_geometry
                    .readback
                    .compile_readback_sha256
                    .clone(),
                fixed_view_sha256.clone(),
                result.final_geometry.execution_evidence.render_duration_ms,
                VisualProgramCacheDispositionV2::NotApplicable,
            ));
            chain.push((
                VisualProgramPhaseV2::Preview,
                fixed_view_sha256.clone(),
                result.final_geometry.glb_sha256.clone(),
                0,
                VisualProgramCacheDispositionV2::NotApplicable,
            ));
        } else {
            chain.push((
                VisualProgramPhaseV2::Preview,
                result.comparison_report.report_sha256.clone(),
                result.final_geometry.glb_sha256.clone(),
                0,
                VisualProgramCacheDispositionV2::NotApplicable,
            ));
        }
        let mut compile_ordinal = 0usize;
        let phases = chain
            .into_iter()
            .enumerate()
            .map(
                |(index, (phase, input_sha256, output_sha256, duration_ms, cache))| {
                    let compile_geometry = if phase == VisualProgramPhaseV2::CompileReadback {
                        let geometry = if compile_ordinal == 0 {
                            &result.initial_geometry
                        } else {
                            &result.final_geometry
                        };
                        compile_ordinal += 1;
                        Some(geometry)
                    } else {
                        None
                    };
                    VisualProgramPhaseReceiptV2 {
                        sequence: (index + 1) as u16,
                        phase,
                        duration_ms,
                        input_sha256,
                        output_sha256,
                        cache,
                        fragment_cache_hit_operation_ids: compile_geometry
                            .map(|geometry| {
                                geometry
                                    .execution_evidence
                                    .fragment_cache_hit_operation_ids
                                    .clone()
                            })
                            .unwrap_or_default(),
                        fragment_cache_miss_operation_ids: compile_geometry
                            .map(|geometry| {
                                geometry
                                    .execution_evidence
                                    .fragment_cache_miss_operation_ids
                                    .clone()
                            })
                            .unwrap_or_default(),
                    }
                },
            )
            .collect::<Vec<_>>();
        let receipt = E005VisualSessionReceiptV1 {
            schema_version: E005_VISUAL_SESSION_RECEIPT_SCHEMA_VERSION.into(),
            receipt_id: format!("visualreceipt_e005_{suffix}"),
            session_id: session_id.clone(),
            task_payload_sha256: task_payload_sha256.into(),
            request_sha256: request_sha256.into(),
            source_program_sha256: lowering.source_program_sha256,
            expanded_program_sha256: lowering.expanded_program_sha256,
            shape_program_sha256: lowering.shape_program_sha256,
            glb_sha256: result.final_geometry.glb_sha256.clone(),
            normalized_geometry_sha256,
            fixed_view_sha256,
            compile_readback_sha256: result
                .final_geometry
                .readback
                .compile_readback_sha256
                .clone(),
            restricted_geometry_evidence_sha256,
            comparison_report_sha256: result.comparison_report.report_sha256.clone(),
            phases,
            usage,
        };
        let receipt_sha256 = semantic_sha256(&receipt)?;
        let session = Self {
            schema_version: E005_VISUAL_SESSION_SCHEMA_VERSION.into(),
            session_id,
            initial_source_sha256: result.initial_source_sha256.clone(),
            final_source_sha256: result.final_source_sha256.clone(),
            state: result.status,
            visual_patch_sha256,
            review_evidence,
            receipt,
            receipt_sha256,
        };
        session.validate()?;
        Ok(session)
    }

    pub fn validate(&self) -> Result<(), forgecad_core::CoreError> {
        self.review_evidence.validate()?;
        let hashes = [
            self.initial_source_sha256.as_str(),
            self.final_source_sha256.as_str(),
            self.visual_patch_sha256.as_str(),
            self.receipt_sha256.as_str(),
            self.receipt.task_payload_sha256.as_str(),
            self.receipt.request_sha256.as_str(),
            self.receipt.source_program_sha256.as_str(),
            self.receipt.expanded_program_sha256.as_str(),
            self.receipt.shape_program_sha256.as_str(),
            self.receipt.glb_sha256.as_str(),
            self.receipt.normalized_geometry_sha256.as_str(),
            self.receipt.fixed_view_sha256.as_str(),
            self.receipt.compile_readback_sha256.as_str(),
            self.receipt.restricted_geometry_evidence_sha256.as_str(),
            self.receipt.comparison_report_sha256.as_str(),
        ];
        let valid_hash = |value: &str| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        };
        if self.schema_version != E005_VISUAL_SESSION_SCHEMA_VERSION
            || self.receipt.schema_version != E005_VISUAL_SESSION_RECEIPT_SCHEMA_VERSION
            || self.receipt.session_id != self.session_id
            || self.receipt.source_program_sha256 != self.final_source_sha256
            || self.review_evidence.initial_source_sha256 != self.initial_source_sha256
            || self.review_evidence.final_source_sha256 != self.final_source_sha256
            || self.review_evidence.status != self.state
            || self.review_evidence.sealed_patch_sha256 != self.visual_patch_sha256
            || self.review_evidence.comparison_report_sha256
                != self.receipt.comparison_report_sha256
            || semantic_sha256(&self.receipt)? != self.receipt_sha256
            || hashes.into_iter().any(|hash| !valid_hash(hash))
            || self.receipt.usage.provider_requests != 2
            || self.receipt.phases.is_empty()
            || self.receipt.phases.len() > 13
            || self
                .receipt
                .phases
                .iter()
                .enumerate()
                .any(|(index, phase)| {
                    phase.sequence != (index + 1) as u16
                        || (index > 0
                            && self.receipt.phases[index - 1].output_sha256 != phase.input_sha256)
                })
        {
            return Err(forgecad_core::CoreError::invalid_data(
                "E005_R2_VISUAL_SESSION_INVALID",
                "E005 visual session identity, lineage, phase chain or usage is invalid.",
            ));
        }
        Ok(())
    }
}

/// Hash-only durable evidence for the R2 visual call. Image bytes remain
/// transport-only; the final GLB and final view hashes continue to live on the
/// enclosing E005 run receipt.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005VisualReviewEvidenceV1 {
    pub schema_version: String,
    pub status: E005VisualReviewStatusV1,
    pub decision: E005VisualDecisionKindV1,
    pub initial_source_sha256: String,
    pub final_source_sha256: String,
    pub initial_glb_sha256: String,
    pub initial_fixed_view_sha256: String,
    pub initial_fixed_views: std::collections::BTreeMap<String, String>,
    pub comparison_input_sha256: String,
    pub comparison_report_sha256: String,
    pub provider_response_sha256: String,
    pub sealed_patch_sha256: String,
    pub geometry_build_count: u8,
    pub visual_provider_call_count: u8,
    pub final_visual_model_recheck_performed: bool,
}

impl E005VisualReviewEvidenceV1 {
    pub fn from_result(
        result: &E005VisualReviewResultV1,
    ) -> Result<Self, forgecad_core::CoreError> {
        let evidence = Self {
            schema_version: E005_VISUAL_REVIEW_EVIDENCE_SCHEMA_VERSION.into(),
            status: result.status,
            decision: result.visual_patch.decision,
            initial_source_sha256: result.initial_source_sha256.clone(),
            final_source_sha256: result.final_source_sha256.clone(),
            initial_glb_sha256: result.initial_geometry.glb_sha256.clone(),
            initial_fixed_view_sha256: semantic_sha256(&result.initial_geometry.view_sha256)?,
            initial_fixed_views: result.initial_geometry.view_sha256.clone(),
            comparison_input_sha256: semantic_sha256(&result.comparison_input)?,
            comparison_report_sha256: result.comparison_report.report_sha256.clone(),
            provider_response_sha256: result
                .comparison_report
                .provider
                .provider_response_sha256
                .clone(),
            sealed_patch_sha256: semantic_sha256(&result.visual_patch)?,
            geometry_build_count: result.geometry_build_count,
            visual_provider_call_count: result.visual_provider_call_count,
            final_visual_model_recheck_performed: result.final_visual_model_recheck_performed,
        };
        evidence.validate()?;
        Ok(evidence)
    }

    pub fn validate(&self) -> Result<(), forgecad_core::CoreError> {
        let valid_hash = |value: &str| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        };
        let required_views = [
            "turntable_000",
            "turntable_045",
            "turntable_090",
            "turntable_135",
            "turntable_180",
            "turntable_225",
            "turntable_270",
            "turntable_315",
        ];
        if self.schema_version != E005_VISUAL_REVIEW_EVIDENCE_SCHEMA_VERSION
            || [
                self.initial_source_sha256.as_str(),
                self.final_source_sha256.as_str(),
                self.initial_glb_sha256.as_str(),
                self.initial_fixed_view_sha256.as_str(),
                self.comparison_input_sha256.as_str(),
                self.comparison_report_sha256.as_str(),
                self.provider_response_sha256.as_str(),
                self.sealed_patch_sha256.as_str(),
            ]
            .into_iter()
            .any(|hash| !valid_hash(hash))
            || self.initial_fixed_views.len() != 8
            || required_views
                .iter()
                .any(|view_id| !self.initial_fixed_views.contains_key(*view_id))
            || self
                .initial_fixed_views
                .values()
                .any(|hash| !valid_hash(hash))
            || self.initial_fixed_view_sha256 != semantic_sha256(&self.initial_fixed_views)?
            || self.visual_provider_call_count != 1
        {
            return Err(forgecad_core::CoreError::invalid_data(
                "E005_R2_VISUAL_EVIDENCE_INVALID",
                "visual review evidence identity, hashes, views or call count is invalid",
            ));
        }
        match (self.status, self.decision) {
            (
                E005VisualReviewStatusV1::AcceptedByVisualReview,
                E005VisualDecisionKindV1::Accept,
            ) if self.initial_source_sha256 == self.final_source_sha256
                && self.geometry_build_count == 1
                && self.final_visual_model_recheck_performed => {}
            (
                E005VisualReviewStatusV1::PatchedPendingVisualConfirmation,
                E005VisualDecisionKindV1::TypedVisualPatch,
            ) if self.initial_source_sha256 != self.final_source_sha256
                && self.geometry_build_count == 2
                && !self.final_visual_model_recheck_performed => {}
            _ => {
                return Err(forgecad_core::CoreError::invalid_data(
                    "E005_R2_VISUAL_EVIDENCE_STATE_INVALID",
                    "visual review evidence status does not match its decision, builds and recheck truth",
                ))
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct E005VisualReviewRequestV1 {
    pub authorization_id: Option<String>,
    pub turn_id: String,
    pub request: MultimodalDesignRequest,
    pub graph: VisualEvidenceGraph,
    pub evidence: Vec<ReferenceEvidence>,
    pub reference_images: Vec<VisualReferenceComparisonImage>,
    pub source: Value,
    pub acceptance_policy: VisualReferenceAcceptancePolicy,
}

/// Local geometry and exact multimodal request prepared before the formal
/// 0045 Patch reservation. It contains transport bytes in memory only and is
/// never serialized or persisted.
pub struct E005PreparedVisualReviewV1 {
    source: Value,
    initial_source_sha256: String,
    initial_geometry: RestrictedGeometryOutput,
    comparison_input: VisualReferenceComparisonInput,
    provider_request: VisualReferenceComparisonProviderRequest,
}

impl E005PreparedVisualReviewV1 {
    pub fn provider_request(&self) -> &VisualReferenceComparisonProviderRequest {
        &self.provider_request
    }

    pub fn comparison_input_sha256(&self) -> Result<String, forgecad_core::CoreError> {
        semantic_sha256(&self.comparison_input)
    }

    pub fn initial_source_sha256(&self) -> &str {
        &self.initial_source_sha256
    }

    pub fn initial_geometry(&self) -> &RestrictedGeometryOutput {
        &self.initial_geometry
    }
}

impl std::fmt::Debug for E005PreparedVisualReviewV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("E005PreparedVisualReviewV1")
            .field("initial_source_sha256", &self.initial_source_sha256)
            .field("initial_glb_sha256", &self.initial_geometry.glb_sha256)
            .field("comparison_input", &"[HASH_BOUND]")
            .field("provider_request", &self.provider_request)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct E005VisualReviewResultV1 {
    pub status: E005VisualReviewStatusV1,
    pub initial_source_sha256: String,
    pub initial_source: Value,
    pub final_source_sha256: String,
    pub final_source: Value,
    pub comparison_input: VisualReferenceComparisonInput,
    pub comparison_report: VisualReferenceComparisonReport,
    pub visual_patch: E005VisualPatchV1,
    pub initial_geometry: RestrictedGeometryOutput,
    pub final_geometry: RestrictedGeometryOutput,
    pub geometry_build_count: u8,
    pub visual_provider_call_count: u8,
    pub final_visual_model_recheck_performed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct E005VisualReviewFailureV1 {
    pub code: String,
    pub message: String,
    pub network_call_made: bool,
}

impl E005VisualReviewFailureV1 {
    fn new(code: impl Into<String>, message: impl Into<String>, network_call_made: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            network_call_made,
        }
    }
}

#[derive(Clone)]
pub struct E005VisualReviewCoordinatorV1 {
    geometry: Arc<dyn RestrictedGeometryPort>,
    comparison: VisualReferenceComparisonCoordinator,
}

impl E005VisualReviewCoordinatorV1 {
    pub fn new(
        geometry: Arc<dyn RestrictedGeometryPort>,
        comparison: VisualReferenceComparisonCoordinator,
    ) -> Self {
        Self {
            geometry,
            comparison,
        }
    }

    pub async fn execute(
        &self,
        request: E005VisualReviewRequestV1,
        cancellation: CancellationToken,
    ) -> Result<E005VisualReviewResultV1, E005VisualReviewFailureV1> {
        let prepared = self.prepare(request, cancellation.child_token()).await?;
        let outcome = self
            .comparison
            .compare_e005(
                prepared.provider_request.clone(),
                cancellation.child_token(),
            )
            .await
            .map_err(comparison_failure)?;
        self.complete_outcome(prepared, outcome, cancellation.child_token())
            .await
    }

    pub async fn prepare(
        &self,
        request: E005VisualReviewRequestV1,
        cancellation: CancellationToken,
    ) -> Result<E005PreparedVisualReviewV1, E005VisualReviewFailureV1> {
        if cancellation.is_cancelled() {
            return Err(E005VisualReviewFailureV1::new(
                "E005_R2_CANCELLED",
                "visual review was cancelled before candidate rendering",
                false,
            ));
        }
        let initial_lowering =
            lower_forge_visual_author_source_v1(&request.source).map_err(core_failure)?;
        let initial_geometry = self
            .render_turntable(
                &request.source,
                &initial_lowering,
                cancellation.child_token(),
            )
            .await?;
        let fixed_views = fixed_view_evidence(&initial_geometry)?;
        let comparison_input = VisualReferenceComparisonInput::build_for_e005_source(
            &request.request,
            &request.graph,
            &request.evidence,
            &request.source,
            &initial_geometry.glb_sha256,
            &fixed_views,
            request.acceptance_policy,
        )
        .map_err(core_failure)?;
        let candidate_images = candidate_images(&initial_geometry);
        let provider_request = VisualReferenceComparisonProviderRequest {
            authorization_id: request.authorization_id,
            turn_id: request.turn_id,
            input: comparison_input.clone(),
            graph: request.graph,
            evidence: request.evidence,
            reference_images: request.reference_images,
            candidate_images,
            e005_source: Some(request.source.clone()),
        };
        Ok(E005PreparedVisualReviewV1 {
            source: request.source,
            initial_source_sha256: initial_lowering.source_program_sha256,
            initial_geometry,
            comparison_input,
            provider_request,
        })
    }

    /// Completes a formal prepare-once Provider response without entering the
    /// legacy 0044 visual budget path.
    pub async fn complete_prepared_output(
        &self,
        prepared: E005PreparedVisualReviewV1,
        output: VisualReferenceComparisonProviderOutput,
        cancellation: CancellationToken,
    ) -> Result<E005VisualReviewResultV1, E005VisualReviewFailureV1> {
        let outcome = self
            .comparison
            .resolve_prepared_e005_output(prepared.provider_request.clone(), output)
            .map_err(comparison_failure)?;
        self.complete_outcome(prepared, outcome, cancellation).await
    }

    async fn complete_outcome(
        &self,
        prepared: E005PreparedVisualReviewV1,
        outcome: E005VisualReferenceReviewOutcome,
        cancellation: CancellationToken,
    ) -> Result<E005VisualReviewResultV1, E005VisualReviewFailureV1> {
        let E005PreparedVisualReviewV1 {
            source,
            initial_source_sha256,
            initial_geometry,
            comparison_input,
            provider_request: _,
        } = prepared;
        let E005VisualReferenceReviewOutcome {
            report: comparison_report,
            visual_patch,
        } = outcome;

        match visual_patch.decision {
            E005VisualDecisionKindV1::Accept => Ok(E005VisualReviewResultV1 {
                status: E005VisualReviewStatusV1::AcceptedByVisualReview,
                initial_source_sha256: initial_source_sha256.clone(),
                initial_source: source.clone(),
                final_source_sha256: initial_source_sha256,
                final_source: source,
                comparison_input,
                comparison_report,
                visual_patch,
                initial_geometry: initial_geometry.clone(),
                final_geometry: initial_geometry,
                geometry_build_count: 1,
                visual_provider_call_count: 1,
                final_visual_model_recheck_performed: true,
            }),
            E005VisualDecisionKindV1::TypedVisualPatch => {
                let patch_value = serde_json::to_value(&visual_patch).map_err(|error| {
                    E005VisualReviewFailureV1::new(
                        "E005_R2_PATCH_SERIALIZATION_FAILED",
                        error.to_string(),
                        false,
                    )
                })?;
                let patched =
                    apply_e005_visual_patch_v1(&source, &patch_value).map_err(core_failure)?;
                let final_geometry = self
                    .render_turntable(&patched.final_source, &patched.lowering, cancellation)
                    .await?;
                verify_rebuilt_candidate(&patched.final_source_sha256, &final_geometry)?;
                Ok(E005VisualReviewResultV1 {
                    status: E005VisualReviewStatusV1::PatchedPendingVisualConfirmation,
                    initial_source_sha256,
                    initial_source: source,
                    final_source_sha256: patched.final_source_sha256,
                    final_source: patched.final_source,
                    comparison_input,
                    comparison_report,
                    visual_patch,
                    initial_geometry,
                    final_geometry,
                    geometry_build_count: 2,
                    visual_provider_call_count: 1,
                    final_visual_model_recheck_performed: false,
                })
            }
        }
    }

    async fn render_turntable(
        &self,
        source: &Value,
        lowering: &forgecad_core::ForgeVisualAuthorLoweringV1,
        cancellation: CancellationToken,
    ) -> Result<RestrictedGeometryOutput, E005VisualReviewFailureV1> {
        let surface_adornment_programs = compile_e005_surface_adornments(source, lowering)
            .map_err(|error| E005VisualReviewFailureV1::new(error.code, error.message, false))?;
        let input = RestrictedGeometryInput {
            schema_version: RESTRICTED_GEOMETRY_INPUT_SCHEMA_VERSION.into(),
            shape_program: lowering.shape_program.clone(),
            profile_sketch: None,
            section_set: None,
            surface_adornment_programs,
            surface_layer_input: None,
            surface_layer_inputs: Vec::new(),
            reference_uv_evidence_bakes: Vec::new(),
            render_view_profile: RestrictedRenderViewProfile::TurntableEight,
            quality_profile: RestrictedQualityProfile {
                profile_id: "interactive_preview".into(),
                runtime_manifest_version: RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION.into(),
                max_triangle_count: 100_000,
                render_width: 384,
                render_height: 384,
                require_closed_manifold: true,
                require_surface_provenance: true,
            },
        };
        input.validate().map_err(geometry_failure)?;
        let output = self
            .geometry
            .build_compile_render(input.clone(), cancellation)
            .await
            .map_err(geometry_failure)?;
        output.validate(&input).map_err(geometry_failure)?;
        verify_surface_pbr(&output, input.surface_adornment_programs.len())?;
        Ok(output)
    }
}

fn verify_surface_pbr(
    geometry: &RestrictedGeometryOutput,
    adornment_count: usize,
) -> Result<(), E005VisualReviewFailureV1> {
    if adornment_count == 0
        || geometry.readback.visual_texture_set_count != adornment_count as u32
        || geometry.readback.visual_texture_map_count != adornment_count as u32 * 5
        || !geometry.readback.visual_texture_provenance_verified
    {
        return Err(E005VisualReviewFailureV1::new(
            "E005_R2_SURFACE_PBR_GATE_FAILED",
            "R2 candidate must bind every R1 SurfacePlan zone to one verified five-channel PBR texture set.",
            false,
        ));
    }
    Ok(())
}

fn fixed_view_evidence(
    geometry: &RestrictedGeometryOutput,
) -> Result<Vec<VisualFixedViewEvidence>, E005VisualReviewFailureV1> {
    let mut views = geometry
        .view_sha256
        .iter()
        .map(|(view_id, image_sha256)| VisualFixedViewEvidence {
            view_id: view_id.clone(),
            glb_sha256: geometry.glb_sha256.clone(),
            renderer_id: geometry.renderer_id.clone(),
            image_sha256: image_sha256.clone(),
            readback_passed: geometry.readback.glb_sha256 == geometry.glb_sha256,
        })
        .collect::<Vec<_>>();
    views.sort_by(|left, right| left.view_id.cmp(&right.view_id));
    if views.len() != 8 {
        return Err(E005VisualReviewFailureV1::new(
            "E005_R2_TURNTABLE_INCOMPLETE",
            "visual review requires exactly eight candidate render hashes",
            false,
        ));
    }
    Ok(views)
}

fn candidate_images(geometry: &RestrictedGeometryOutput) -> Vec<VisualReferenceComparisonImage> {
    geometry
        .views
        .iter()
        .map(|(view_id, bytes)| VisualReferenceComparisonImage {
            image_id: view_id.clone(),
            media_type: "image/png".into(),
            bytes: Arc::from(bytes.clone()),
        })
        .collect()
}

fn verify_rebuilt_candidate(
    source_sha256: &str,
    geometry: &RestrictedGeometryOutput,
) -> Result<(), E005VisualReviewFailureV1> {
    if source_sha256.len() != 64
        || geometry.glb_bytes.is_empty()
        || geometry.glb_sha256 != geometry.readback.glb_sha256
        || geometry.readback.triangle_count == 0
        || !geometry.readback.closed_manifold
        || !geometry.readback.surface_provenance_present
        || geometry.views.len() != 8
        || geometry.views.keys().any(|view_id| {
            !matches!(
                view_id.as_str(),
                "turntable_000"
                    | "turntable_045"
                    | "turntable_090"
                    | "turntable_135"
                    | "turntable_180"
                    | "turntable_225"
                    | "turntable_270"
                    | "turntable_315"
            )
        })
    {
        return Err(E005VisualReviewFailureV1::new(
            "E005_R2_REBUILD_GATE_FAILED",
            "patched candidate failed deterministic GLB/readback/manifold/surface/turntable validation",
            false,
        ));
    }
    Ok(())
}

fn core_failure(error: forgecad_core::CoreError) -> E005VisualReviewFailureV1 {
    E005VisualReviewFailureV1::new(error.code(), error.to_string(), false)
}

fn geometry_failure(error: RestrictedGeometryError) -> E005VisualReviewFailureV1 {
    E005VisualReviewFailureV1::new(error.code, error.message, false)
}

fn comparison_failure(error: VisualReferenceComparisonProviderError) -> E005VisualReviewFailureV1 {
    E005VisualReviewFailureV1::new(error.code, error.message, error.network_call_made)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{
        collections::BTreeMap,
        future::Future,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    };

    use forgecad_core::{
        canonical_json, semantic_sha256, MultimodalDesignLocks, MultimodalReferenceInput,
        ReferenceClass, ReferenceEvidenceKind, ReferenceEvidenceObservations,
        ReferenceImageBrightnessBucket, ReferenceImageColorBucket, ReferenceImageEdgeDensityBucket,
        ReferenceImageForegroundConfidence, ReferenceImageSurfaceFacts, ReferenceRole,
        VisionEvidenceProviderProvenance, VisualClaimStatus, VisualClaimTarget, VisualDetailLevel,
        VisualEvidenceClaim, VisualReferenceClaimAssessment, VisualReferenceMatchOutcome,
        MULTIMODAL_DESIGN_REQUEST_SCHEMA_VERSION, VISUAL_EVIDENCE_GRAPH_SCHEMA_VERSION,
    };
    use serde_json::json;
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::{
        RestrictedGeometryExecutionEvidence, RestrictedGeometryFuture, RestrictedGeometryReadback,
        VisualReferenceComparisonProviderFuture, VisualReferenceComparisonProviderOutput,
        VisualReferenceComparisonProviderPort, RESTRICTED_GEOMETRY_OUTPUT_SCHEMA_VERSION,
    };

    fn sha(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn triangle_glb(label: &str) -> Vec<u8> {
        let mut binary = Vec::new();
        for value in [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            binary.extend_from_slice(&value.to_le_bytes());
        }
        for index in [0_u16, 1, 2] {
            binary.extend_from_slice(&index.to_le_bytes());
        }
        let declared_binary_length = binary.len();
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        let mut document = serde_json::to_vec(&json!({
            "asset": {"version": "2.0", "generator": label},
            "buffers": [{"byteLength": declared_binary_length}],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": 36},
                {"buffer": 0, "byteOffset": 36, "byteLength": 6}
            ],
            "accessors": [
                {"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"},
                {"bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR"}
            ],
            "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1, "mode": 4}]}]
        }))
        .unwrap();
        while document.len() % 4 != 0 {
            document.push(b' ');
        }
        let total_length = 12 + 8 + document.len() + 8 + binary.len();
        let mut glb = Vec::with_capacity(total_length);
        glb.extend_from_slice(&0x4654_6c67_u32.to_le_bytes());
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&(total_length as u32).to_le_bytes());
        glb.extend_from_slice(&(document.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x4e4f_534a_u32.to_le_bytes());
        glb.extend_from_slice(&document);
        glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x004e_4942_u32.to_le_bytes());
        glb.extend_from_slice(&binary);
        glb
    }

    #[derive(Clone, Default)]
    pub(crate) struct GeometryFixture {
        calls: Arc<AtomicUsize>,
    }

    impl GeometryFixture {
        pub(crate) fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl RestrictedGeometryPort for GeometryFixture {
        fn build_compile_render(
            &self,
            input: RestrictedGeometryInput,
            _cancellation: CancellationToken,
        ) -> RestrictedGeometryFuture {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                let shape_sha256 = sha(canonical_json(&input.shape_program).unwrap().as_bytes());
                let artifact_identity = sha(canonical_json(&json!({
                    "shape_program_sha256":shape_sha256,
                    "quality_profile":input.quality_profile,
                    "surface_adornment_programs":input.surface_adornment_programs,
                }))
                .unwrap()
                .as_bytes());
                let glb_bytes = triangle_glb(&artifact_identity);
                let glb_sha256 = sha(&glb_bytes);
                let views = [
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
                .map(|view_id| {
                    (
                        view_id.to_string(),
                        format!("PNG-{view_id}-{glb_sha256}").into_bytes(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
                let view_sha256 = views
                    .iter()
                    .map(|(view_id, bytes)| (view_id.clone(), sha(bytes)))
                    .collect();
                let adornment_count = input.surface_adornment_programs.len() as u32;
                Ok(RestrictedGeometryOutput {
                    schema_version: RESTRICTED_GEOMETRY_OUTPUT_SCHEMA_VERSION.into(),
                    glb_sha256: glb_sha256.clone(),
                    topology_hash: shape_sha256.clone(),
                    glb_bytes: glb_bytes.clone(),
                    readback: RestrictedGeometryReadback {
                        runtime_manifest_version: RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION
                            .into(),
                        artifact_profile_id: input.quality_profile.profile_id.clone(),
                        shape_program_sha256: shape_sha256.clone(),
                        glb_sha256,
                        glb_byte_size: glb_bytes.len() as u64,
                        triangle_count: 128,
                        bounds_mm: [200.0, 120.0, 80.0],
                        mesh_count: 1,
                        primitive_count: 4,
                        material_count: 2,
                        closed_manifold: true,
                        surface_provenance_present: true,
                        compile_readback_sha256: sha(shape_sha256.as_bytes()),
                        material_zone_count: adornment_count.max(2),
                        visual_texture_set_count: adornment_count,
                        visual_texture_map_count: adornment_count * 5,
                        visual_texture_provenance_verified: true,
                        reference_appearance_projection_receipts: Vec::new(),
                    },
                    views,
                    view_sha256,
                    renderer_id: "forgecad-e005-r2-fixture@1".into(),
                    execution_evidence: RestrictedGeometryExecutionEvidence {
                        schema_version: "RestrictedGeometryExecutionEvidence@1".into(),
                        compile_cache_key_sha256: artifact_identity,
                        compile_cache_hit: false,
                        compile_duration_ms: 1,
                        render_duration_ms: 1,
                        fragment_cache_hit_operation_ids: Vec::new(),
                        fragment_cache_miss_operation_ids: Vec::new(),
                    },
                })
            })
        }
    }

    #[derive(Clone)]
    pub(crate) struct VisualFixture {
        calls: Arc<AtomicUsize>,
        patch: bool,
    }

    impl VisualFixture {
        pub(crate) fn new(patch: bool) -> Self {
            Self {
                calls: Arc::new(AtomicUsize::new(0)),
                patch,
            }
        }
    }

    impl VisualReferenceComparisonProviderPort for VisualFixture {
        fn compare(
            &self,
            request: VisualReferenceComparisonProviderRequest,
            _cancellation: CancellationToken,
        ) -> VisualReferenceComparisonProviderFuture {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let patch = self.patch;
            Box::pin(async move {
                let source_sha256 = request.input.source_program_sha256.clone();
                let input_sha256 = semantic_sha256(&request.input).unwrap();
                let assessments = request
                    .graph
                    .claims
                    .iter()
                    .filter(|claim| claim.status != VisualClaimStatus::Unknown)
                    .map(|claim| {
                        let mismatch = patch && claim.claim_id == "vclaim_macro";
                        VisualReferenceClaimAssessment {
                            claim_id: claim.claim_id.clone(),
                            outcome: if mismatch {
                                VisualReferenceMatchOutcome::Partial
                            } else {
                                VisualReferenceMatchOutcome::Matched
                            },
                            similarity_bps: if mismatch { 5_000 } else { 8_500 },
                            confidence_bps: 9_000,
                            source_evidence_ids: claim.source_evidence_ids.clone(),
                            candidate_view_ids: vec!["turntable_000".into()],
                            reason: if mismatch {
                                "The macro silhouette requires one bounded positional repair".into()
                            } else {
                                "The visible claim matches the candidate".into()
                            },
                        }
                    })
                    .collect();
                let proposal = if patch {
                    json!({
                        "schema_version":"E005VisualPatchProposal@1",
                        "patch_id":"visualpatch_e005_r2_fixture",
                        "decision":"typed_visual_patch",
                        "expected_source_sha256":source_sha256,
                        "comparison_input_sha256":input_sha256,
                        "repair_claim_ids":["vclaim_macro"],
                        "operations":[{
                            "op":"set_parameter_default",
                            "parameter_id":"param_fastener_count",
                            "value":8
                        }]
                    })
                } else {
                    json!({
                        "schema_version":"E005VisualPatchProposal@1",
                        "patch_id":"visualpatch_e005_r2_accept_fixture",
                        "decision":"accept",
                        "expected_source_sha256":source_sha256,
                        "comparison_input_sha256":input_sha256,
                        "repair_claim_ids":[],
                        "operations":[]
                    })
                };
                Ok(VisualReferenceComparisonProviderOutput {
                    provider_id: "e005_visual_fixture".into(),
                    model_id: "e005_visual_fixture".into(),
                    provider_response_sha256: "f".repeat(64),
                    analyzed_at: "2026-07-29T13:00:00Z".into(),
                    assessments,
                    network_call_made: false,
                    budget_evidence: None,
                    e005_visual_patch_proposal: Some(proposal),
                })
            })
        }
    }

    fn source() -> Value {
        serde_json::from_str(include_str!(
            "../../../../../../packages/concept-spec/fixtures/e005-r1-unified-service-console.json"
        ))
        .unwrap()
    }

    pub(crate) fn request_fixture() -> (E005VisualReviewRequestV1, Arc<[u8]>) {
        let bytes: Arc<[u8]> = Arc::from(b"sealed-e005-r2-reference".as_slice());
        let evidence = ReferenceEvidence {
            schema_version: "ReferenceEvidence@1".into(),
            evidence_id: "refevid_e005_r2".into(),
            project_id: "prj_e005_r2".into(),
            kind: ReferenceEvidenceKind::Image,
            reference_class: ReferenceClass::SingleImage,
            domain_pack_id: "pack_robotic_arm_concept".into(),
            source_file_name: "reference.png".into(),
            source_media_type: "image/png".into(),
            source_object_sha256: sha(bytes.as_ref()),
            source_imported_asset_version_id: None,
            source_statement: "User supplied reference".into(),
            license_statement: "User confirms reference rights".into(),
            missing_views: vec!["back".into()],
            user_notes: "Use visible silhouette and panel rhythm".into(),
            observations: ReferenceEvidenceObservations {
                silhouette_summary: "Compact industrial service console".into(),
                proportion_ranges: vec!["wide base and narrow top".into()],
                material_zone_observations: vec!["painted shell and dark vents".into()],
                visible_part_hypotheses: Vec::new(),
                uncertainties: vec!["back is not visible".into()],
                image_surface_facts: Some(ReferenceImageSurfaceFacts {
                    width: 1024,
                    height: 1024,
                    aspect_ratio_milli: 1000,
                    dominant_color_buckets: vec![ReferenceImageColorBucket::Blue],
                    foreground_dominant_color_buckets: Vec::new(),
                    brightness: ReferenceImageBrightnessBucket::Balanced,
                    edge_density: ReferenceImageEdgeDensityBucket::High,
                    foreground_bbox_normalized: [80, 80, 920, 940],
                    contact_sheet_layout_evidence: false,
                    foreground_confidence: ReferenceImageForegroundConfidence::Medium,
                }),
            },
            created_at: "2026-07-29T12:00:00Z".into(),
            glb_inspection: None,
        };
        let request = MultimodalDesignRequest {
            schema_version: MULTIMODAL_DESIGN_REQUEST_SCHEMA_VERSION.into(),
            request_id: "mmreq_e005_r2".into(),
            project_id: evidence.project_id.clone(),
            turn_id: "turn_e005_r2".into(),
            domain_pack_id: evidence.domain_pack_id.clone(),
            instruction: "Create the visible industrial console".into(),
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
                locked_part_ids: Vec::new(),
                locked_material_zone_ids: Vec::new(),
            },
        };
        let graph = VisualEvidenceGraph {
            schema_version: VISUAL_EVIDENCE_GRAPH_SCHEMA_VERSION.into(),
            graph_id: "vegraph_e005_r2".into(),
            request_id: request.request_id.clone(),
            request_sha256: semantic_sha256(&request).unwrap(),
            project_id: request.project_id.clone(),
            domain_pack_id: request.domain_pack_id.clone(),
            provider: VisionEvidenceProviderProvenance {
                provider_id: "vision_fixture".into(),
                model_id: "vision_fixture".into(),
                provider_response_sha256: "e".repeat(64),
                analyzed_at: "2026-07-29T12:01:00Z".into(),
            },
            claims: [
                (
                    "vclaim_macro",
                    VisualDetailLevel::Macro,
                    VisualClaimTarget::Geometry,
                ),
                (
                    "vclaim_meso",
                    VisualDetailLevel::Meso,
                    VisualClaimTarget::Geometry,
                ),
                (
                    "vclaim_micro",
                    VisualDetailLevel::Micro,
                    VisualClaimTarget::Surface,
                ),
            ]
            .into_iter()
            .map(|(claim_id, level, target)| VisualEvidenceClaim {
                claim_id: claim_id.into(),
                level,
                status: VisualClaimStatus::Observed,
                target,
                description: format!("Visible {claim_id} reference fact"),
                critical: true,
                confidence_bps: 9_000,
                source_evidence_ids: vec![evidence.evidence_id.clone()],
                source_view_id: Some("front".into()),
                source_region: None,
            })
            .collect(),
        };
        (
            E005VisualReviewRequestV1 {
                authorization_id: None,
                turn_id: request.turn_id.clone(),
                request,
                graph,
                evidence: vec![evidence.clone()],
                reference_images: vec![VisualReferenceComparisonImage {
                    image_id: evidence.evidence_id,
                    media_type: evidence.source_media_type,
                    bytes: bytes.clone(),
                }],
                source: source(),
                acceptance_policy: VisualReferenceAcceptancePolicy::default_policy(),
            },
            bytes,
        )
    }

    fn run<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn e005_r2_accept_uses_one_visual_call_and_one_geometry_build() {
        let geometry = GeometryFixture::default();
        let visual = VisualFixture {
            calls: Arc::new(AtomicUsize::new(0)),
            patch: false,
        };
        let comparison = VisualReferenceComparisonCoordinator::new(
            Arc::new(visual.clone()),
            Duration::from_secs(5),
        )
        .unwrap();
        let coordinator =
            E005VisualReviewCoordinatorV1::new(Arc::new(geometry.clone()), comparison);
        let result =
            run(coordinator.execute(request_fixture().0, CancellationToken::new())).unwrap();
        assert_eq!(
            result.status,
            E005VisualReviewStatusV1::AcceptedByVisualReview
        );
        assert_eq!(geometry.calls.load(Ordering::SeqCst), 1);
        assert_eq!(visual.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            result.initial_geometry.glb_sha256,
            result.final_geometry.glb_sha256
        );
        assert!(result.final_visual_model_recheck_performed);
        let evidence = E005VisualReviewEvidenceV1::from_result(&result).unwrap();
        assert_eq!(
            evidence.status,
            E005VisualReviewStatusV1::AcceptedByVisualReview
        );
    }

    #[test]
    fn e005_r2_patch_uses_one_visual_call_two_builds_and_never_claims_recheck() {
        let geometry = GeometryFixture::default();
        let visual = VisualFixture {
            calls: Arc::new(AtomicUsize::new(0)),
            patch: true,
        };
        let comparison = VisualReferenceComparisonCoordinator::new(
            Arc::new(visual.clone()),
            Duration::from_secs(5),
        )
        .unwrap();
        let coordinator =
            E005VisualReviewCoordinatorV1::new(Arc::new(geometry.clone()), comparison);
        let result =
            run(coordinator.execute(request_fixture().0, CancellationToken::new())).unwrap();
        assert_eq!(
            result.status,
            E005VisualReviewStatusV1::PatchedPendingVisualConfirmation
        );
        assert_eq!(geometry.calls.load(Ordering::SeqCst), 2);
        assert_eq!(visual.calls.load(Ordering::SeqCst), 1);
        assert_ne!(result.initial_source_sha256, result.final_source_sha256);
        assert_ne!(
            result.initial_geometry.glb_sha256,
            result.final_geometry.glb_sha256
        );
        assert!(!result.final_visual_model_recheck_performed);
        assert_eq!(result.final_geometry.views.len(), 8);
        let mut evidence = E005VisualReviewEvidenceV1::from_result(&result).unwrap();
        evidence.final_visual_model_recheck_performed = true;
        assert_eq!(
            evidence.validate().unwrap_err().code(),
            "E005_R2_VISUAL_EVIDENCE_STATE_INVALID"
        );
    }

    #[test]
    fn e005_r2_rejects_reference_byte_swap_before_visual_provider_dispatch() {
        let geometry = GeometryFixture::default();
        let visual = VisualFixture {
            calls: Arc::new(AtomicUsize::new(0)),
            patch: false,
        };
        let comparison = VisualReferenceComparisonCoordinator::new(
            Arc::new(visual.clone()),
            Duration::from_secs(5),
        )
        .unwrap();
        let coordinator =
            E005VisualReviewCoordinatorV1::new(Arc::new(geometry.clone()), comparison);
        let mut request = request_fixture().0;
        request.reference_images[0].bytes = Arc::from(b"swapped-reference".as_slice());
        let error = run(coordinator.execute(request, CancellationToken::new())).unwrap_err();
        assert_eq!(error.code, "VISUAL_REFERENCE_COMPARISON_REFERENCE_INVALID");
        assert_eq!(geometry.calls.load(Ordering::SeqCst), 1);
        assert_eq!(visual.calls.load(Ordering::SeqCst), 0);
        assert!(!error.network_call_made);
    }
}
