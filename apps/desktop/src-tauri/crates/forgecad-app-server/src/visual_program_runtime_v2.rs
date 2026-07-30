//! FGC-VP204 Rust-owned low-roundtrip coordinator.
//!
//! This module is the app-server execution boundary for an already-authored
//! `ForgeVisualGeometryProgram@2`. It never persists a candidate, calls a
//! Provider, or accepts executable code. One source is lowered, compiled,
//! rendered and gated; only one hash-bound typed patch may repeat that path.

use std::{sync::Arc, time::Instant};

use forgecad_core::{
    apply_forge_visual_geometry_patch_v2, lower_visual_runtime_source_v1, semantic_sha256,
    VisualProgramAuthoringSessionV2, VisualProgramAuthoringStateV2,
    VisualProgramCacheDispositionV2, VisualProgramExecutionReceiptV2, VisualProgramGateOutcomeV2,
    VisualProgramGateVerdictV2, VisualProgramPhaseReceiptV2, VisualProgramPhaseV2,
    VisualProgramUsageV2, FORGE_VISUAL_AUTHOR_SOURCE_SCHEMA_VERSION,
    VISUAL_PROGRAM_EXECUTION_RECEIPT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    CancellationToken, RestrictedGeometryError, RestrictedGeometryErrorKind,
    RestrictedGeometryInput, RestrictedGeometryOutput, RestrictedGeometryPort,
    RestrictedQualityProfile, RestrictedRenderViewProfile,
    RESTRICTED_GEOMETRY_INPUT_SCHEMA_VERSION, RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION,
};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Vp204RuntimeFailure {
    pub code: String,
    pub message: String,
}

impl Vp204RuntimeFailure {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Vp204RuntimeRequest {
    pub session_id: String,
    pub idempotency_key: String,
    pub request_sha256: String,
    pub source: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<Value>,
    pub usage: VisualProgramUsageV2,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Vp204RuntimeResult {
    pub session: VisualProgramAuthoringSessionV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_geometry: Option<RestrictedGeometryOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_geometry: Option<RestrictedGeometryOutput>,
}

/// One-shot in-memory continuation for the only permitted VP204 repair.
///
/// The fields are deliberately private and this type is neither `Clone` nor
/// serializable. Callers can inspect the repairable initial result, but only
/// `Vp204RuntimeCoordinator::resume_with_patch` can consume the retained stage
/// and advance the same authoring session.
pub struct Vp204RuntimeContinuation {
    request: Vp204RuntimeRequest,
    initial: CompletedStage,
    session: VisualProgramAuthoringSessionV2,
}

impl Vp204RuntimeContinuation {
    pub fn session(&self) -> &VisualProgramAuthoringSessionV2 {
        &self.session
    }

    pub fn initial_geometry(&self) -> &RestrictedGeometryOutput {
        &self.initial.geometry
    }

    pub fn into_result(self) -> Vp204RuntimeResult {
        Vp204RuntimeResult {
            session: self.session,
            initial_geometry: Some(self.initial.geometry.clone()),
            current_geometry: Some(self.initial.geometry),
        }
    }
}

pub enum Vp204RuntimeInitialOutcome {
    Complete(Vp204RuntimeResult),
    AwaitingPatch(Vp204RuntimeContinuation),
}

impl Vp204RuntimeInitialOutcome {
    pub fn session(&self) -> &VisualProgramAuthoringSessionV2 {
        match self {
            Self::Complete(result) => &result.session,
            Self::AwaitingPatch(continuation) => continuation.session(),
        }
    }

    pub fn current_geometry(&self) -> Option<&RestrictedGeometryOutput> {
        match self {
            Self::Complete(result) => result.current_geometry.as_ref(),
            Self::AwaitingPatch(continuation) => Some(continuation.initial_geometry()),
        }
    }

    pub fn into_result(self) -> Vp204RuntimeResult {
        match self {
            Self::Complete(result) => result,
            Self::AwaitingPatch(continuation) => continuation.into_result(),
        }
    }
}

pub trait Vp204GateEvaluator: Send + Sync + 'static {
    fn evaluate(
        &self,
        source_program_sha256: &str,
        geometry: &RestrictedGeometryOutput,
    ) -> VisualProgramGateOutcomeV2;
}

#[derive(Clone)]
pub struct Vp204RuntimeCoordinator {
    geometry: Arc<dyn RestrictedGeometryPort>,
    gate: Arc<dyn Vp204GateEvaluator>,
}

#[derive(Clone)]
struct CompletedStage {
    source_sha256: String,
    expanded_sha256: String,
    shape_sha256: String,
    lower_duration_ms: u64,
    geometry: RestrictedGeometryOutput,
    render_sha256: String,
    gate: VisualProgramGateOutcomeV2,
    gate_sha256: String,
}

impl Vp204RuntimeCoordinator {
    pub fn new(
        geometry: Arc<dyn RestrictedGeometryPort>,
        gate: Arc<dyn Vp204GateEvaluator>,
    ) -> Self {
        Self { geometry, gate }
    }

    pub async fn execute(
        &self,
        mut request: Vp204RuntimeRequest,
        cancellation: CancellationToken,
    ) -> Result<Vp204RuntimeResult, Vp204RuntimeFailure> {
        let patch = request.patch.take();
        let cumulative_usage = request.usage.clone();
        let initial = self.execute_initial(request, cancellation.clone()).await?;
        match (initial, patch) {
            (Vp204RuntimeInitialOutcome::AwaitingPatch(continuation), Some(patch)) => {
                self.resume_with_patch(continuation, patch, cumulative_usage, cancellation)
                    .await
            }
            (Vp204RuntimeInitialOutcome::AwaitingPatch(continuation), None) => {
                Ok(continuation.into_result())
            }
            (Vp204RuntimeInitialOutcome::Complete(result), Some(_))
                if result.session.state == VisualProgramAuthoringStateV2::ReadyForPreview =>
            {
                Err(Vp204RuntimeFailure::new(
                    "FORGE_VISUAL_VP204_PATCH_NOT_AUTHORIZED",
                    "a typed patch is allowed only after one repairable initial hard-gate failure",
                ))
            }
            (Vp204RuntimeInitialOutcome::Complete(result), _) => Ok(result),
        }
    }

    pub async fn execute_initial(
        &self,
        request: Vp204RuntimeRequest,
        cancellation: CancellationToken,
    ) -> Result<Vp204RuntimeInitialOutcome, Vp204RuntimeFailure> {
        if request.patch.is_some() {
            return Err(Vp204RuntimeFailure::new(
                "FORGE_VISUAL_VP204_INITIAL_PATCH_FORBIDDEN",
                "execute_initial accepts exactly one authored source and no patch",
            ));
        }
        let lower_started = Instant::now();
        let lowering = lower_visual_runtime_source_v1(&request.source)
            .map_err(|error| Vp204RuntimeFailure::new(error.code(), error.to_string()))?;
        let lower_duration_ms = bounded_elapsed_ms(lower_started);
        let initial = self
            .execute_lowering(
                lowering.source_program_sha256.clone(),
                lowering.expanded_program_sha256.clone(),
                lowering.shape_program_sha256.clone(),
                lowering.shape_program.clone(),
                lower_duration_ms,
                cancellation.clone(),
            )
            .await;
        let initial = match initial {
            Ok(stage) => stage,
            Err(error) => {
                let receipt = terminal_receipt(
                    &request,
                    &lowering.source_program_sha256,
                    &lowering.expanded_program_sha256,
                    &lowering.shape_program_sha256,
                    lower_duration_ms,
                    0,
                    &error,
                )?;
                let mut session = VisualProgramAuthoringSessionV2::begin(
                    request.session_id,
                    request.idempotency_key,
                    request.request_sha256,
                    request.source,
                    receipt.clone(),
                )
                .map_err(core_failure)?;
                finish_terminal(&mut session, receipt, &error)?;
                return Ok(Vp204RuntimeInitialOutcome::Complete(Vp204RuntimeResult {
                    session,
                    initial_geometry: None,
                    current_geometry: None,
                }));
            }
        };
        let initial_receipt = successful_initial_receipt(&request, &initial)?;
        let mut session = VisualProgramAuthoringSessionV2::begin(
            request.session_id.clone(),
            request.idempotency_key.clone(),
            request.request_sha256.clone(),
            request.source.clone(),
            initial_receipt,
        )
        .map_err(core_failure)?;
        session
            .record_gate(initial.gate.clone())
            .map_err(core_failure)?;

        if session.state == VisualProgramAuthoringStateV2::AwaitingPatch {
            return Ok(Vp204RuntimeInitialOutcome::AwaitingPatch(
                Vp204RuntimeContinuation {
                    request,
                    initial,
                    session,
                },
            ));
        }
        Ok(Vp204RuntimeInitialOutcome::Complete(Vp204RuntimeResult {
            session,
            initial_geometry: Some(initial.geometry.clone()),
            current_geometry: Some(initial.geometry),
        }))
    }

    pub async fn resume_with_patch(
        &self,
        continuation: Vp204RuntimeContinuation,
        patch: Value,
        cumulative_usage: VisualProgramUsageV2,
        cancellation: CancellationToken,
    ) -> Result<Vp204RuntimeResult, Vp204RuntimeFailure> {
        let Vp204RuntimeContinuation {
            mut request,
            initial,
            mut session,
        } = continuation;
        if session.state != VisualProgramAuthoringStateV2::AwaitingPatch {
            return Err(Vp204RuntimeFailure::new(
                "FORGE_VISUAL_VP204_PATCH_NOT_AUTHORIZED",
                "the continuation no longer represents a repairable initial hard-gate failure",
            ));
        }
        request.patch = Some(patch.clone());
        request.usage = cumulative_usage;

        if request.source.get("schema_version").and_then(Value::as_str)
            == Some(FORGE_VISUAL_AUTHOR_SOURCE_SCHEMA_VERSION)
        {
            return Err(Vp204RuntimeFailure::new(
                "FORGE_VISUAL_R1_PATCH_NOT_IMPLEMENTED",
                "the unified author source remains immutable until E005-R2 adds a hash-bound visual patch",
            ));
        }

        let patch_started = Instant::now();
        let patched = apply_forge_visual_geometry_patch_v2(&request.source, &patch)
            .map_err(|error| Vp204RuntimeFailure::new(error.code(), error.to_string()))?;
        let patch_duration_ms = bounded_elapsed_ms(patch_started);
        let patched_stage = self
            .execute_lowering(
                patched.lowering.source_program_sha256.clone(),
                patched
                    .lowering
                    .expanded_dag
                    .expanded_program_sha256
                    .clone(),
                patched.lowering.shape_program_sha256.clone(),
                patched.lowering.shape_program.clone(),
                patch_duration_ms,
                cancellation,
            )
            .await;
        let patched_stage = match patched_stage {
            Ok(stage) => stage,
            Err(error) => {
                let receipt = terminal_patched_receipt(
                    &request,
                    &initial,
                    &patched.lowering.source_program_sha256,
                    &patched.lowering.expanded_dag.expanded_program_sha256,
                    &patched.lowering.shape_program_sha256,
                    patch_duration_ms,
                    &error,
                )?;
                session
                    .apply_patch(&patch, receipt.clone())
                    .map_err(core_failure)?;
                finish_terminal(&mut session, receipt, &error)?;
                return Ok(Vp204RuntimeResult {
                    session,
                    initial_geometry: Some(initial.geometry),
                    current_geometry: None,
                });
            }
        };
        let patched_receipt =
            successful_patched_receipt(&request, &initial, &patched_stage, patch_duration_ms)?;
        session
            .apply_patch(&patch, patched_receipt)
            .map_err(core_failure)?;
        session
            .record_gate(patched_stage.gate.clone())
            .map_err(core_failure)?;
        Ok(Vp204RuntimeResult {
            session,
            initial_geometry: Some(initial.geometry),
            current_geometry: Some(patched_stage.geometry),
        })
    }

    async fn execute_lowering(
        &self,
        source_sha256: String,
        expanded_sha256: String,
        shape_sha256: String,
        shape_program: Value,
        lower_duration_ms: u64,
        cancellation: CancellationToken,
    ) -> Result<CompletedStage, RestrictedGeometryError> {
        if cancellation.is_cancelled() {
            return Err(RestrictedGeometryError::cancelled());
        }
        let input = RestrictedGeometryInput {
            schema_version: RESTRICTED_GEOMETRY_INPUT_SCHEMA_VERSION.into(),
            shape_program,
            profile_sketch: None,
            section_set: None,
            surface_adornment_programs: Vec::new(),
            surface_layer_input: None,
            surface_layer_inputs: Vec::new(),
            reference_uv_evidence_bakes: Vec::new(),
            render_view_profile: RestrictedRenderViewProfile::WorkbenchFour,
            quality_profile: RestrictedQualityProfile {
                profile_id: "interactive_preview".into(),
                runtime_manifest_version: RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION.into(),
                max_triangle_count: 100_000,
                render_width: 320,
                render_height: 320,
                require_closed_manifold: true,
                require_surface_provenance: true,
            },
        };
        input.validate()?;
        let geometry = self
            .geometry
            .build_compile_render(input.clone(), cancellation)
            .await?;
        geometry.validate(&input)?;
        if geometry.readback.shape_program_sha256 != shape_sha256 {
            return Err(RestrictedGeometryError::execution(
                "VP204_SHAPE_PROGRAM_LINEAGE_MISMATCH",
                "restricted geometry output does not bind the lowered ShapeProgram",
            ));
        }
        let render_sha256 = semantic_sha256(&geometry.view_sha256)
            .map_err(|error| RestrictedGeometryError::execution(error.code(), error.to_string()))?;
        let gate = self.gate.evaluate(&source_sha256, &geometry);
        let gate_sha256 = semantic_sha256(&gate)
            .map_err(|error| RestrictedGeometryError::execution(error.code(), error.to_string()))?;
        Ok(CompletedStage {
            source_sha256,
            expanded_sha256,
            shape_sha256,
            lower_duration_ms,
            geometry,
            render_sha256,
            gate,
            gate_sha256,
        })
    }
}

fn bounded_elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis())
        .unwrap_or(u64::MAX)
        .min(240_000)
}

fn cache_disposition(stage: &CompletedStage) -> VisualProgramCacheDispositionV2 {
    if stage.geometry.execution_evidence.compile_cache_hit {
        VisualProgramCacheDispositionV2::Hit
    } else {
        VisualProgramCacheDispositionV2::Miss
    }
}

fn phase(
    sequence: u16,
    phase: VisualProgramPhaseV2,
    duration_ms: u64,
    input_sha256: String,
    output_sha256: String,
    cache: VisualProgramCacheDispositionV2,
) -> VisualProgramPhaseReceiptV2 {
    VisualProgramPhaseReceiptV2 {
        sequence,
        phase,
        duration_ms,
        input_sha256,
        output_sha256,
        cache,
        fragment_cache_hit_operation_ids: Vec::new(),
        fragment_cache_miss_operation_ids: Vec::new(),
    }
}

fn initial_phases(
    request_sha256: &str,
    stage: &CompletedStage,
) -> Vec<VisualProgramPhaseReceiptV2> {
    let mut phases = vec![
        phase(
            1,
            VisualProgramPhaseV2::Author,
            0,
            request_sha256.into(),
            stage.source_sha256.clone(),
            VisualProgramCacheDispositionV2::NotApplicable,
        ),
        phase(
            2,
            VisualProgramPhaseV2::Validate,
            0,
            stage.source_sha256.clone(),
            stage.source_sha256.clone(),
            VisualProgramCacheDispositionV2::NotApplicable,
        ),
        phase(
            3,
            VisualProgramPhaseV2::Expand,
            0,
            stage.source_sha256.clone(),
            stage.expanded_sha256.clone(),
            VisualProgramCacheDispositionV2::NotApplicable,
        ),
        phase(
            4,
            VisualProgramPhaseV2::Lower,
            stage.lower_duration_ms,
            stage.expanded_sha256.clone(),
            stage.shape_sha256.clone(),
            VisualProgramCacheDispositionV2::NotApplicable,
        ),
        phase(
            5,
            VisualProgramPhaseV2::CompileReadback,
            stage.geometry.execution_evidence.compile_duration_ms,
            stage.shape_sha256.clone(),
            stage.geometry.glb_sha256.clone(),
            cache_disposition(stage),
        ),
        phase(
            6,
            VisualProgramPhaseV2::Render,
            stage.geometry.execution_evidence.render_duration_ms,
            stage.geometry.glb_sha256.clone(),
            stage.render_sha256.clone(),
            VisualProgramCacheDispositionV2::Miss,
        ),
        phase(
            7,
            VisualProgramPhaseV2::Evaluate,
            0,
            stage.render_sha256.clone(),
            stage.gate_sha256.clone(),
            VisualProgramCacheDispositionV2::NotApplicable,
        ),
    ];
    phases[4].fragment_cache_hit_operation_ids = stage
        .geometry
        .execution_evidence
        .fragment_cache_hit_operation_ids
        .clone();
    phases[4].fragment_cache_miss_operation_ids = stage
        .geometry
        .execution_evidence
        .fragment_cache_miss_operation_ids
        .clone();
    if stage.gate.verdict == VisualProgramGateVerdictV2::Pass {
        phases.push(phase(
            8,
            VisualProgramPhaseV2::Preview,
            0,
            stage.gate_sha256.clone(),
            stage.source_sha256.clone(),
            VisualProgramCacheDispositionV2::NotApplicable,
        ));
    }
    phases
}

fn successful_initial_receipt(
    request: &Vp204RuntimeRequest,
    stage: &CompletedStage,
) -> Result<VisualProgramExecutionReceiptV2, Vp204RuntimeFailure> {
    let receipt = VisualProgramExecutionReceiptV2 {
        schema_version: VISUAL_PROGRAM_EXECUTION_RECEIPT_SCHEMA_VERSION.into(),
        receipt_id: format!("receipt_{}_0", request.session_id),
        session_id: request.session_id.clone(),
        authoring_count: 1,
        patch_count: 0,
        source_program_sha256: stage.source_sha256.clone(),
        expanded_program_sha256: stage.expanded_sha256.clone(),
        shape_program_sha256: stage.shape_sha256.clone(),
        glb_sha256: Some(stage.geometry.glb_sha256.clone()),
        phases: initial_phases(&request.request_sha256, stage),
        usage: request.usage.clone(),
        cancelled: false,
        failure_code: None,
    };
    receipt.validate().map_err(core_failure)?;
    Ok(receipt)
}

fn successful_patched_receipt(
    request: &Vp204RuntimeRequest,
    initial: &CompletedStage,
    patched: &CompletedStage,
    patch_duration_ms: u64,
) -> Result<VisualProgramExecutionReceiptV2, Vp204RuntimeFailure> {
    let mut phases = initial_phases(&request.request_sha256, initial);
    if phases
        .last()
        .is_some_and(|phase| phase.phase == VisualProgramPhaseV2::Preview)
    {
        return Err(Vp204RuntimeFailure::new(
            "FORGE_VISUAL_VP204_PATCH_NOT_AUTHORIZED",
            "a passed initial gate cannot enter the patch path",
        ));
    }
    let mut sequence = phases.len() as u16 + 1;
    phases.push(phase(
        sequence,
        VisualProgramPhaseV2::Patch,
        patch_duration_ms,
        initial.gate_sha256.clone(),
        patched.source_sha256.clone(),
        VisualProgramCacheDispositionV2::NotApplicable,
    ));
    sequence += 1;
    phases.push(phase(
        sequence,
        VisualProgramPhaseV2::Validate,
        0,
        patched.source_sha256.clone(),
        patched.source_sha256.clone(),
        VisualProgramCacheDispositionV2::NotApplicable,
    ));
    sequence += 1;
    phases.push(phase(
        sequence,
        VisualProgramPhaseV2::Expand,
        0,
        patched.source_sha256.clone(),
        patched.expanded_sha256.clone(),
        VisualProgramCacheDispositionV2::NotApplicable,
    ));
    sequence += 1;
    phases.push(phase(
        sequence,
        VisualProgramPhaseV2::Lower,
        patched.lower_duration_ms,
        patched.expanded_sha256.clone(),
        patched.shape_sha256.clone(),
        VisualProgramCacheDispositionV2::NotApplicable,
    ));
    sequence += 1;
    let mut compile_phase = phase(
        sequence,
        VisualProgramPhaseV2::CompileReadback,
        patched.geometry.execution_evidence.compile_duration_ms,
        patched.shape_sha256.clone(),
        patched.geometry.glb_sha256.clone(),
        cache_disposition(patched),
    );
    compile_phase.fragment_cache_hit_operation_ids = patched
        .geometry
        .execution_evidence
        .fragment_cache_hit_operation_ids
        .clone();
    compile_phase.fragment_cache_miss_operation_ids = patched
        .geometry
        .execution_evidence
        .fragment_cache_miss_operation_ids
        .clone();
    phases.push(compile_phase);
    sequence += 1;
    phases.push(phase(
        sequence,
        VisualProgramPhaseV2::Render,
        patched.geometry.execution_evidence.render_duration_ms,
        patched.geometry.glb_sha256.clone(),
        patched.render_sha256.clone(),
        VisualProgramCacheDispositionV2::Miss,
    ));
    sequence += 1;
    phases.push(phase(
        sequence,
        VisualProgramPhaseV2::Evaluate,
        0,
        patched.render_sha256.clone(),
        patched.gate_sha256.clone(),
        VisualProgramCacheDispositionV2::NotApplicable,
    ));
    if patched.gate.verdict == VisualProgramGateVerdictV2::Pass {
        sequence += 1;
        phases.push(phase(
            sequence,
            VisualProgramPhaseV2::Preview,
            0,
            patched.gate_sha256.clone(),
            patched.source_sha256.clone(),
            VisualProgramCacheDispositionV2::NotApplicable,
        ));
    }
    let receipt = VisualProgramExecutionReceiptV2 {
        schema_version: VISUAL_PROGRAM_EXECUTION_RECEIPT_SCHEMA_VERSION.into(),
        receipt_id: format!("receipt_{}_1", request.session_id),
        session_id: request.session_id.clone(),
        authoring_count: 1,
        patch_count: 1,
        source_program_sha256: patched.source_sha256.clone(),
        expanded_program_sha256: patched.expanded_sha256.clone(),
        shape_program_sha256: patched.shape_sha256.clone(),
        glb_sha256: Some(patched.geometry.glb_sha256.clone()),
        phases,
        usage: request.usage.clone(),
        cancelled: false,
        failure_code: None,
    };
    receipt.validate().map_err(core_failure)?;
    Ok(receipt)
}

fn terminal_receipt(
    request: &Vp204RuntimeRequest,
    source_sha256: &str,
    expanded_sha256: &str,
    shape_sha256: &str,
    lower_duration_ms: u64,
    patch_count: u8,
    error: &RestrictedGeometryError,
) -> Result<VisualProgramExecutionReceiptV2, Vp204RuntimeFailure> {
    let receipt = VisualProgramExecutionReceiptV2 {
        schema_version: VISUAL_PROGRAM_EXECUTION_RECEIPT_SCHEMA_VERSION.into(),
        receipt_id: format!("receipt_{}_terminal", request.session_id),
        session_id: request.session_id.clone(),
        authoring_count: 1,
        patch_count,
        source_program_sha256: source_sha256.into(),
        expanded_program_sha256: expanded_sha256.into(),
        shape_program_sha256: shape_sha256.into(),
        glb_sha256: None,
        phases: vec![
            phase(
                1,
                VisualProgramPhaseV2::Author,
                0,
                request.request_sha256.clone(),
                source_sha256.into(),
                VisualProgramCacheDispositionV2::NotApplicable,
            ),
            phase(
                2,
                VisualProgramPhaseV2::Validate,
                0,
                source_sha256.into(),
                source_sha256.into(),
                VisualProgramCacheDispositionV2::NotApplicable,
            ),
            phase(
                3,
                VisualProgramPhaseV2::Expand,
                0,
                source_sha256.into(),
                expanded_sha256.into(),
                VisualProgramCacheDispositionV2::NotApplicable,
            ),
            phase(
                4,
                VisualProgramPhaseV2::Lower,
                lower_duration_ms,
                expanded_sha256.into(),
                shape_sha256.into(),
                VisualProgramCacheDispositionV2::NotApplicable,
            ),
            phase(
                5,
                VisualProgramPhaseV2::CompileReadback,
                0,
                shape_sha256.into(),
                shape_sha256.into(),
                VisualProgramCacheDispositionV2::Miss,
            ),
        ],
        usage: request.usage.clone(),
        cancelled: error.kind == RestrictedGeometryErrorKind::Cancelled,
        failure_code: (error.kind != RestrictedGeometryErrorKind::Cancelled)
            .then(|| bounded_failure_code(&error.code)),
    };
    receipt.validate().map_err(core_failure)?;
    Ok(receipt)
}

fn terminal_patched_receipt(
    request: &Vp204RuntimeRequest,
    initial: &CompletedStage,
    source_sha256: &str,
    expanded_sha256: &str,
    shape_sha256: &str,
    patch_duration_ms: u64,
    error: &RestrictedGeometryError,
) -> Result<VisualProgramExecutionReceiptV2, Vp204RuntimeFailure> {
    let mut receipt = terminal_receipt(
        request,
        source_sha256,
        expanded_sha256,
        shape_sha256,
        patch_duration_ms,
        1,
        error,
    )?;
    receipt.phases = initial_phases(&request.request_sha256, initial);
    let sequence = receipt.phases.len() as u16 + 1;
    receipt.phases.push(phase(
        sequence,
        VisualProgramPhaseV2::Patch,
        patch_duration_ms,
        initial.gate_sha256.clone(),
        source_sha256.into(),
        VisualProgramCacheDispositionV2::NotApplicable,
    ));
    receipt.phases.push(phase(
        sequence + 1,
        VisualProgramPhaseV2::Validate,
        0,
        source_sha256.into(),
        source_sha256.into(),
        VisualProgramCacheDispositionV2::NotApplicable,
    ));
    receipt.phases.push(phase(
        sequence + 2,
        VisualProgramPhaseV2::Expand,
        0,
        source_sha256.into(),
        expanded_sha256.into(),
        VisualProgramCacheDispositionV2::NotApplicable,
    ));
    receipt.phases.push(phase(
        sequence + 3,
        VisualProgramPhaseV2::Lower,
        patch_duration_ms,
        expanded_sha256.into(),
        shape_sha256.into(),
        VisualProgramCacheDispositionV2::NotApplicable,
    ));
    receipt.phases.push(phase(
        sequence + 4,
        VisualProgramPhaseV2::CompileReadback,
        0,
        shape_sha256.into(),
        shape_sha256.into(),
        VisualProgramCacheDispositionV2::Miss,
    ));
    receipt.validate().map_err(core_failure)?;
    Ok(receipt)
}

fn finish_terminal(
    session: &mut VisualProgramAuthoringSessionV2,
    receipt: VisualProgramExecutionReceiptV2,
    error: &RestrictedGeometryError,
) -> Result<(), Vp204RuntimeFailure> {
    if error.kind == RestrictedGeometryErrorKind::Cancelled {
        session.cancel(receipt).map_err(core_failure)
    } else {
        session.fail(receipt).map_err(core_failure)
    }
}

fn bounded_failure_code(code: &str) -> String {
    let filtered = code
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || "_.@-".contains(*character))
        .take(128)
        .collect::<String>();
    if filtered.is_empty() {
        "VP204_RUNTIME_FAILURE".into()
    } else {
        filtered
    }
}

fn core_failure(error: forgecad_core::CoreError) -> Vp204RuntimeFailure {
    Vp204RuntimeFailure::new(error.code(), error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::{
            atomic::{AtomicUsize, Ordering},
            Mutex,
        },
    };

    use forgecad_core::{
        lower_forge_visual_geometry_program_v2, VisualProgramAuthoringStateV2,
        VisualProgramGateVerdictV2, VISUAL_PROGRAM_GATE_OUTCOME_SCHEMA_VERSION,
    };
    use serde_json::json;

    use super::*;
    use crate::{
        RestrictedGeometryExecutionEvidence, RestrictedGeometryFuture, RestrictedGeometryReadback,
        RESTRICTED_GEOMETRY_OUTPUT_SCHEMA_VERSION,
    };

    #[derive(Default)]
    struct FixtureGeometryPort {
        compiled: Mutex<BTreeSet<String>>,
        invocations: AtomicUsize,
    }

    impl RestrictedGeometryPort for FixtureGeometryPort {
        fn build_compile_render(
            &self,
            input: RestrictedGeometryInput,
            _cancellation: CancellationToken,
        ) -> RestrictedGeometryFuture {
            self.invocations.fetch_add(1, Ordering::SeqCst);
            let shape_sha256 = crate::canonical::sha256_hex(
                crate::canonical::canonical_json(&input.shape_program).as_bytes(),
            );
            let operation_ids = input
                .shape_program
                .get("operations")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|operation| operation.get("operation_id").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>();
            let cache_hit = self
                .compiled
                .lock()
                .expect("fixture compile cache mutex")
                .replace(shape_sha256.clone())
                .is_some();
            Box::pin(async move {
                let mut glb_bytes = b"glTF VP204 app-server fixture ".to_vec();
                glb_bytes.extend_from_slice(shape_sha256.as_bytes());
                let glb_sha256 = crate::canonical::sha256_hex(&glb_bytes);
                let views = ["front", "iso", "side", "top"]
                    .into_iter()
                    .map(|name| {
                        (
                            name.to_string(),
                            format!("PNG VP204 {name} {shape_sha256}").into_bytes(),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                let view_sha256 = views
                    .iter()
                    .map(|(name, bytes)| (name.clone(), crate::canonical::sha256_hex(bytes)))
                    .collect();
                Ok(RestrictedGeometryOutput {
                    schema_version: RESTRICTED_GEOMETRY_OUTPUT_SCHEMA_VERSION.into(),
                    glb_bytes: glb_bytes.clone(),
                    glb_sha256: glb_sha256.clone(),
                    topology_hash: shape_sha256.clone(),
                    readback: RestrictedGeometryReadback {
                        runtime_manifest_version: RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION
                            .into(),
                        artifact_profile_id: input.quality_profile.profile_id,
                        shape_program_sha256: shape_sha256.clone(),
                        glb_sha256,
                        glb_byte_size: glb_bytes.len() as u64,
                        triangle_count: 256,
                        bounds_mm: [400.0, 300.0, 200.0],
                        mesh_count: 1,
                        primitive_count: 1,
                        material_count: 1,
                        closed_manifold: true,
                        surface_provenance_present: true,
                        compile_readback_sha256: crate::canonical::sha256_hex(
                            b"vp204_app_server_readback",
                        ),
                        material_zone_count: 1,
                        visual_texture_set_count: 0,
                        visual_texture_map_count: 0,
                        visual_texture_provenance_verified: true,
                    },
                    views,
                    view_sha256,
                    renderer_id: "forgecad-agent-software-raster@1".into(),
                    execution_evidence: RestrictedGeometryExecutionEvidence {
                        schema_version: "RestrictedGeometryExecutionEvidence@1".into(),
                        compile_cache_key_sha256: crate::canonical::sha256_hex(
                            shape_sha256.as_bytes(),
                        ),
                        compile_cache_hit: cache_hit,
                        compile_duration_ms: if cache_hit { 0 } else { 4 },
                        render_duration_ms: 2,
                        fragment_cache_hit_operation_ids: Vec::new(),
                        fragment_cache_miss_operation_ids: if cache_hit {
                            Vec::new()
                        } else {
                            operation_ids
                        },
                    },
                })
            })
        }
    }

    struct RepairInitialGate {
        initial_source_sha256: String,
    }

    impl Vp204GateEvaluator for RepairInitialGate {
        fn evaluate(
            &self,
            source_program_sha256: &str,
            _geometry: &RestrictedGeometryOutput,
        ) -> VisualProgramGateOutcomeV2 {
            let initial = source_program_sha256 == self.initial_source_sha256;
            VisualProgramGateOutcomeV2 {
                schema_version: VISUAL_PROGRAM_GATE_OUTCOME_SCHEMA_VERSION.into(),
                gate_report_id: if initial {
                    "gate_vp204_initial_repair".into()
                } else {
                    "gate_vp204_patched_pass".into()
                },
                source_program_sha256: source_program_sha256.into(),
                verdict: if initial {
                    VisualProgramGateVerdictV2::Fail
                } else {
                    VisualProgramGateVerdictV2::Pass
                },
                repairable: initial,
            }
        }
    }

    struct AlwaysPassGate;

    impl Vp204GateEvaluator for AlwaysPassGate {
        fn evaluate(
            &self,
            source_program_sha256: &str,
            _geometry: &RestrictedGeometryOutput,
        ) -> VisualProgramGateOutcomeV2 {
            VisualProgramGateOutcomeV2 {
                schema_version: VISUAL_PROGRAM_GATE_OUTCOME_SCHEMA_VERSION.into(),
                gate_report_id: "gate_vp204_replay_pass".into(),
                source_program_sha256: source_program_sha256.into(),
                verdict: VisualProgramGateVerdictV2::Pass,
                repairable: false,
            }
        }
    }

    fn rotor() -> Value {
        serde_json::from_str(include_str!(
            "../../../../../../packages/concept-spec/fixtures/forge-visual-geometry-v2-rotor.json"
        ))
        .unwrap()
    }

    fn e005_harness_sensor_pod() -> Value {
        serde_json::from_str(include_str!(
            "../../../../../../packages/concept-spec/fixtures/e005-harness-sensor-pod-source.json"
        ))
        .unwrap()
    }

    fn request(source: Value, patch: Option<Value>, suffix: &str) -> Vp204RuntimeRequest {
        Vp204RuntimeRequest {
            session_id: format!("vpsession_runtime_{suffix}"),
            idempotency_key: format!("idem_runtime_{suffix}"),
            request_sha256: crate::canonical::sha256_hex(suffix.as_bytes()),
            source,
            patch,
            usage: VisualProgramUsageV2::default(),
        }
    }

    #[test]
    fn vp204_app_server_runs_one_patch_then_exact_replay_uses_compile_cache() {
        let source = rotor();
        let lowering = lower_forge_visual_geometry_program_v2(&source).unwrap();
        let patch = json!({
            "schema_version":"ForgeVisualGeometryPatch@1",
            "patch_id":"patch_runtime_rotor",
            "expected_source_sha256":lowering.source_program_sha256,
            "operations":[{"op":"set_array","node_id":"node_rotor_bank","count":4,"spacing":760.0}]
        });
        let geometry = Arc::new(FixtureGeometryPort::default());
        let coordinator = Vp204RuntimeCoordinator::new(
            geometry.clone(),
            Arc::new(RepairInitialGate {
                initial_source_sha256: lowering.source_program_sha256,
            }),
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let result = runtime
            .block_on(coordinator.execute(
                request(source.clone(), Some(patch), "patched"),
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(
            result.session.state,
            VisualProgramAuthoringStateV2::ReadyForPreview
        );
        assert_eq!(result.session.authoring_count, 1);
        assert_eq!(result.session.patch_count, 1);
        assert_eq!(result.session.current_revision, 2);
        assert!(result.session.receipt.phases.iter().any(|phase| {
            phase.phase == VisualProgramPhaseV2::CompileReadback
                && phase
                    .fragment_cache_miss_operation_ids
                    .iter()
                    .any(|operation_id| operation_id == "op_rotor_bank")
        }));
        assert!(
            !result
                .initial_geometry
                .as_ref()
                .unwrap()
                .execution_evidence
                .compile_cache_hit
        );
        assert!(
            !result
                .current_geometry
                .as_ref()
                .unwrap()
                .execution_evidence
                .compile_cache_hit
        );

        let replay = Vp204RuntimeCoordinator::new(geometry, Arc::new(AlwaysPassGate));
        let replayed = runtime
            .block_on(replay.execute(request(source, None, "replay"), CancellationToken::new()))
            .unwrap();
        assert_eq!(replayed.session.patch_count, 0);
        assert!(
            replayed
                .current_geometry
                .unwrap()
                .execution_evidence
                .compile_cache_hit
        );
        assert!(replayed.session.receipt.phases.iter().any(|phase| {
            phase.phase == VisualProgramPhaseV2::CompileReadback
                && phase.cache == VisualProgramCacheDispositionV2::Hit
        }));
    }

    #[test]
    fn vp204_split_resume_reuses_initial_stage_and_records_cumulative_usage() {
        let source = rotor();
        let lowering = lower_forge_visual_geometry_program_v2(&source).unwrap();
        let patch = json!({
            "schema_version":"ForgeVisualGeometryPatch@1",
            "patch_id":"patch_runtime_split",
            "expected_source_sha256":lowering.source_program_sha256,
            "operations":[{"op":"set_array","node_id":"node_rotor_bank","count":5,"spacing":720.0}]
        });
        let geometry = Arc::new(FixtureGeometryPort::default());
        let coordinator = Vp204RuntimeCoordinator::new(
            geometry.clone(),
            Arc::new(RepairInitialGate {
                initial_source_sha256: lowering.source_program_sha256,
            }),
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut initial_request = request(source, None, "split_resume");
        initial_request.usage = VisualProgramUsageV2 {
            provider_requests: 1,
            input_tokens: 120,
            output_tokens: 80,
            estimated_cost_microusd: 50,
            ..VisualProgramUsageV2::default()
        };
        let initial = runtime
            .block_on(coordinator.execute_initial(initial_request, CancellationToken::new()))
            .unwrap();
        assert_eq!(geometry.invocations.load(Ordering::SeqCst), 1);
        assert_eq!(
            initial.session().state,
            VisualProgramAuthoringStateV2::AwaitingPatch
        );
        assert!(initial.current_geometry().is_some());
        let Vp204RuntimeInitialOutcome::AwaitingPatch(continuation) = initial else {
            panic!("repairable initial gate must return a one-shot continuation");
        };
        let cumulative_usage = VisualProgramUsageV2 {
            provider_requests: 2,
            input_tokens: 180,
            output_tokens: 104,
            estimated_cost_microusd: 70,
            ..VisualProgramUsageV2::default()
        };
        let result = runtime
            .block_on(coordinator.resume_with_patch(
                continuation,
                patch,
                cumulative_usage.clone(),
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(geometry.invocations.load(Ordering::SeqCst), 2);
        assert_eq!(
            result.session.state,
            VisualProgramAuthoringStateV2::ReadyForPreview
        );
        assert_eq!(result.session.receipt.usage, cumulative_usage);
        assert_eq!(
            result
                .session
                .receipt
                .phases
                .iter()
                .filter(|phase| phase.phase == VisualProgramPhaseV2::CompileReadback)
                .count(),
            2
        );
    }

    #[test]
    fn e005_authored_source_runs_once_through_vp204_coordinator_without_patch() {
        let source = e005_harness_sensor_pod();
        let expected = lower_forge_visual_geometry_program_v2(&source).unwrap();
        let coordinator = Vp204RuntimeCoordinator::new(
            Arc::new(FixtureGeometryPort::default()),
            Arc::new(AlwaysPassGate),
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let result = runtime
            .block_on(coordinator.execute(
                request(source, None, "e005_sensor_pod"),
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(
            result.session.state,
            VisualProgramAuthoringStateV2::ReadyForPreview
        );
        assert_eq!(result.session.authoring_count, 1);
        assert_eq!(result.session.patch_count, 0);
        assert_eq!(
            result.session.receipt.source_program_sha256,
            expected.source_program_sha256
        );
        assert_eq!(
            result.session.receipt.expanded_program_sha256,
            expected.expanded_dag.expanded_program_sha256
        );
        assert_eq!(
            result.session.receipt.shape_program_sha256,
            expected.shape_program_sha256
        );
        let geometry = result.current_geometry.unwrap();
        assert_eq!(
            geometry.readback.shape_program_sha256,
            expected.shape_program_sha256
        );
        assert!(!geometry.execution_evidence.compile_cache_hit);
    }

    #[test]
    fn vp204_app_server_cancellation_creates_terminal_receipt_without_geometry() {
        let source = rotor();
        let geometry = Arc::new(FixtureGeometryPort::default());
        let coordinator = Vp204RuntimeCoordinator::new(geometry, Arc::new(AlwaysPassGate));
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let result = runtime
            .block_on(coordinator.execute(request(source, None, "cancel"), cancellation))
            .unwrap();
        assert_eq!(
            result.session.state,
            VisualProgramAuthoringStateV2::Cancelled
        );
        assert!(result.session.receipt.cancelled);
        assert!(result.current_geometry.is_none());
        assert!(result.session.receipt.glb_sha256.is_none());
    }

    struct TimeoutGeometryPort;

    impl RestrictedGeometryPort for TimeoutGeometryPort {
        fn build_compile_render(
            &self,
            _input: RestrictedGeometryInput,
            _cancellation: CancellationToken,
        ) -> RestrictedGeometryFuture {
            Box::pin(async {
                Err(RestrictedGeometryError {
                    code: "RESTRICTED_GEOMETRY_TIMEOUT".into(),
                    kind: RestrictedGeometryErrorKind::Timeout,
                    message: "Restricted geometry work exceeded its bounded deadline.".into(),
                    recoverable: true,
                })
            })
        }
    }

    #[test]
    fn vp204_app_server_timeout_creates_failed_receipt_without_geometry() {
        let coordinator =
            Vp204RuntimeCoordinator::new(Arc::new(TimeoutGeometryPort), Arc::new(AlwaysPassGate));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let result = runtime
            .block_on(
                coordinator.execute(request(rotor(), None, "timeout"), CancellationToken::new()),
            )
            .unwrap();
        assert_eq!(result.session.state, VisualProgramAuthoringStateV2::Failed);
        assert_eq!(
            result.session.receipt.failure_code.as_deref(),
            Some("RESTRICTED_GEOMETRY_TIMEOUT")
        );
        assert!(!result.session.receipt.cancelled);
        assert!(result.session.receipt.glb_sha256.is_none());
        assert!(result.current_geometry.is_none());
    }
}
