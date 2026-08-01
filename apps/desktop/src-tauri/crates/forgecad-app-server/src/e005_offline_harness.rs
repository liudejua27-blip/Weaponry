//! FGC-E005 fail-closed offline harness and receipt adapter.
//!
//! This layer accepts only an explicitly bound authored v2 source. Missing
//! source returns `not_run` before the VP204 coordinator or geometry port is
//! touched. Offline receipts can never enter the formal distribution.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Instant,
};

use forgecad_core::{
    lower_visual_runtime_source_v1, normalized_geometry_sha256, semantic_sha256,
    E005ProviderBudgetEvidence, E005ProviderCallKind, VisualProgramAuthoringStateV2,
    VisualProgramGateOutcomeV2, VisualProgramGateVerdictV2, VisualProgramPhaseReceiptV2,
    VisualProgramPhaseV2, VisualProgramUsageV2, VISUAL_PROGRAM_GATE_OUTCOME_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    CancellationToken, RestrictedGeometryOutput, RestrictedGeometryPort, Vp204GateEvaluator,
    Vp204RuntimeCoordinator, Vp204RuntimeFailure, Vp204RuntimeRequest,
    RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION,
};

pub const E005_TASK_SET_SHA256: &str =
    "471c592b5f328f6e899b430b49eb042d3c6955f498b14fd1d2558a0934e18dde";
pub const E005_RUN_RECEIPT_SCHEMA_VERSION: &str = "E005RunReceipt@1";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct E005OfflineHarnessRequest {
    pub task_set_sha256: String,
    pub task_id: String,
    pub task_payload: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum E005RunStatus {
    NotRun,
    PassedWithoutPatch,
    PassedAfterPatch,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum E005HumanReviewStatus {
    NotRun,
    Pending,
    Complete,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct E005RunReceipt {
    pub schema_version: String,
    pub run_id: String,
    pub task_set_sha256: String,
    pub task_id: String,
    pub status: E005RunStatus,
    pub run_mode: String,
    pub distribution_eligible: bool,
    pub author_source_mode: String,
    pub task_payload_sha256: String,
    pub request_sha256: String,
    pub authoring_count: u8,
    pub patch_count: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_authorization_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_authorization_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_call_evidence: Option<Vec<E005ProviderBudgetEvidence>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_call_evidence_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_review_evidence: Option<crate::E005VisualReviewEvidenceV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub production_review_evidence: Option<crate::E005ProductionReviewV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub production_review_evidence_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_program_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expanded_program_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape_program_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_descriptor_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_structure_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_geometry_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology_signature_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_sequence_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_signature_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_zone_signature_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glb_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_view_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_views: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vp204_session_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vp204_receipt_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_session_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_session_receipt_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_outcome_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compile_readback_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restricted_geometry_evidence_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_manifest_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triangle_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds_mm: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primitive_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<VisualProgramUsageV2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase_receipts: Option<Vec<VisualProgramPhaseReceiptV2>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
    pub network_provider_calls: u8,
    pub billable_cost_microusd: u64,
    pub failure_codes: Vec<String>,
    pub human_review_status: E005HumanReviewStatus,
}

impl E005RunReceipt {
    pub fn validate(&self) -> Result<(), Vp204RuntimeFailure> {
        if self.schema_version != E005_RUN_RECEIPT_SCHEMA_VERSION
            || self.task_set_sha256 != E005_TASK_SET_SHA256
            || !valid_hash(&self.task_payload_sha256)
            || !valid_hash(&self.request_sha256)
        {
            return Err(failure(
                "E005_RECEIPT_ENVELOPE_INVALID",
                "E005 receipt identity or task binding is invalid",
            ));
        }
        let formal = match self.run_mode.as_str() {
            "offline_deterministic" => {
                if self.distribution_eligible
                    || self.network_provider_calls != 0
                    || self.billable_cost_microusd != 0
                    || self.provider_authorization_id.is_some()
                    || self.provider_authorization_sha256.is_some()
                    || self.provider_call_evidence.is_some()
                    || self.provider_call_evidence_sha256.is_some()
                    || self.visual_review_evidence.is_some()
                    || self.human_review_status != E005HumanReviewStatus::NotRun
                {
                    return Err(failure(
                        "E005_RECEIPT_ENVELOPE_INVALID",
                        "offline E005 receipt crossed the Provider or review boundary",
                    ));
                }
                false
            }
            "formal_provider" => {
                validate_formal_receipt_envelope(self)?;
                true
            }
            _ => {
                return Err(failure(
                    "E005_RECEIPT_ENVELOPE_INVALID",
                    "E005 run mode is unsupported",
                ))
            }
        };
        let artifact_hashes = [
            self.source_program_sha256.as_ref(),
            self.expanded_program_sha256.as_ref(),
            self.shape_program_sha256.as_ref(),
            self.structural_descriptor_sha256.as_ref(),
            self.semantic_structure_sha256.as_ref(),
            self.normalized_geometry_sha256.as_ref(),
            self.topology_signature_sha256.as_ref(),
            self.operation_sequence_sha256.as_ref(),
            self.profile_signature_sha256.as_ref(),
            self.part_zone_signature_sha256.as_ref(),
            self.glb_sha256.as_ref(),
            self.fixed_view_sha256.as_ref(),
            self.gate_outcome_sha256.as_ref(),
            self.compile_readback_sha256.as_ref(),
            self.restricted_geometry_evidence_sha256.as_ref(),
        ];
        if artifact_hashes
            .iter()
            .flatten()
            .any(|hash| !valid_hash(hash))
        {
            return Err(failure(
                "E005_RECEIPT_HASH_INVALID",
                "E005 evidence hashes must be lowercase SHA-256 values",
            ));
        }
        let session_hashes = [
            self.vp204_session_sha256.as_ref(),
            self.vp204_receipt_sha256.as_ref(),
            self.visual_session_sha256.as_ref(),
            self.visual_session_receipt_sha256.as_ref(),
        ];
        if session_hashes
            .iter()
            .flatten()
            .any(|hash| !valid_hash(hash))
        {
            return Err(failure(
                "E005_RECEIPT_SESSION_HASH_INVALID",
                "E005 session evidence hashes must be lowercase SHA-256 values",
            ));
        }
        if let Some(visual) = &self.visual_review_evidence {
            visual.validate().map_err(|error| {
                failure(
                    error.code(),
                    "E005 visual-review evidence is invalid or internally inconsistent",
                )
            })?;
            if self.source_program_sha256.as_deref() != Some(&visual.final_source_sha256)
                || self.patch_count
                    != u8::from(
                        visual.status
                            == crate::E005VisualReviewStatusV1::PatchedPendingVisualConfirmation,
                    )
            {
                return Err(failure(
                    "E005_R2_VISUAL_RECEIPT_LINEAGE_INVALID",
                    "E005 receipt source/patch counters do not match visual-review evidence",
                ));
            }
        }
        match (
            self.production_review_evidence.as_ref(),
            self.production_review_evidence_sha256.as_deref(),
        ) {
            (Some(production), Some(production_sha256)) => {
                production.validate().map_err(|error| {
                    failure(
                        error.code(),
                        "E005 production-review evidence is invalid or internally inconsistent",
                    )
                })?;
                if semantic_sha256(production).map_err(core_failure)? != production_sha256
                    || self.source_program_sha256.as_deref()
                        != Some(production.source_program_sha256.as_str())
                    || self.glb_sha256.as_deref() != Some(production.glb_sha256.as_str())
                    || self.normalized_geometry_sha256.as_deref()
                        != Some(production.normalized_geometry_sha256.as_str())
                    || self.fixed_view_sha256.as_deref()
                        != Some(production.fixed_view_sha256.as_str())
                    || self.fixed_views.as_ref() != Some(&production.fixed_views)
                    || self.compile_readback_sha256.as_deref()
                        != Some(production.compile_readback_sha256.as_str())
                    || self.restricted_geometry_evidence_sha256.as_deref()
                        != Some(production.restricted_geometry_evidence_sha256.as_str())
                    || self.artifact_profile_id.as_deref()
                        != Some(production.artifact_profile_id.as_str())
                {
                    return Err(failure(
                        "E005_R3_PRODUCTION_RECEIPT_LINEAGE_INVALID",
                        "E005 run receipt does not bind the exact production review evidence.",
                    ));
                }
            }
            (None, None) => {}
            _ => {
                return Err(failure(
                    "E005_R3_PRODUCTION_RECEIPT_INCOMPLETE",
                    "E005 production review and its semantic hash must appear together.",
                ))
            }
        }
        match self.status {
            E005RunStatus::NotRun => {
                if formal
                    || self.author_source_mode != "missing"
                    || self.authoring_count != 0
                    || self.patch_count != 0
                    || artifact_hashes.iter().any(|value| value.is_some())
                    || session_hashes.iter().any(|value| value.is_some())
                    || self.artifact_profile_id.is_some()
                    || self.fixed_views.is_some()
                    || self.visual_review_evidence.is_some()
                    || self.production_review_evidence.is_some()
                    || self.production_review_evidence_sha256.is_some()
                    || self.runtime_manifest_version.is_some()
                    || self.triangle_count.is_some()
                    || self.bounds_mm.is_some()
                    || self.mesh_count.is_some()
                    || self.primitive_count.is_some()
                    || self.material_count.is_some()
                    || self.usage.is_some()
                    || self.phase_receipts.is_some()
                    || self.elapsed_ms.is_some()
                    || self.failure_codes.len() != 1
                {
                    return Err(failure(
                        "E005_NOT_RUN_RECEIPT_INVALID",
                        "not-run receipt must contain no execution or artifact evidence",
                    ));
                }
            }
            E005RunStatus::PassedWithoutPatch | E005RunStatus::PassedAfterPatch => {
                let expected_patch_count = if self.status == E005RunStatus::PassedAfterPatch {
                    1
                } else {
                    0
                };
                let expected_source_mode = if formal {
                    "provider_authored_v2"
                } else {
                    "offline_authored_v2"
                };
                let session_binding_valid = if self.visual_review_evidence.is_some() {
                    self.visual_session_sha256.is_some()
                        && self.visual_session_receipt_sha256.is_some()
                        && self.vp204_session_sha256.is_none()
                        && self.vp204_receipt_sha256.is_none()
                } else {
                    self.vp204_session_sha256.is_some()
                        && self.vp204_receipt_sha256.is_some()
                        && self.visual_session_sha256.is_none()
                        && self.visual_session_receipt_sha256.is_none()
                };
                if self.author_source_mode != expected_source_mode
                    || self.authoring_count != 1
                    || self.patch_count != expected_patch_count
                    || artifact_hashes.iter().any(|value| value.is_none())
                    || !session_binding_valid
                    || self.artifact_profile_id.as_deref()
                        != Some(if self.production_review_evidence.is_some() {
                            "production_concept"
                        } else {
                            "interactive_preview"
                        })
                    || self.fixed_views.as_ref().is_none_or(|views| {
                        let expected = if self.visual_review_evidence.is_some() {
                            BTreeSet::from([
                                "turntable_000",
                                "turntable_045",
                                "turntable_090",
                                "turntable_135",
                                "turntable_180",
                                "turntable_225",
                                "turntable_270",
                                "turntable_315",
                            ])
                        } else {
                            BTreeSet::from(["front", "iso", "side", "top"])
                        };
                        views.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected
                            || views.values().any(|hash| !valid_hash(hash))
                    })
                    || self.runtime_manifest_version.as_deref()
                        != Some(RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION)
                    || self.triangle_count.is_none()
                    || self.bounds_mm.is_none()
                    || self.mesh_count.is_none()
                    || self.primitive_count.is_none()
                    || self.material_count.is_none()
                    || self.usage.is_none()
                    || self.phase_receipts.as_ref().is_none_or(Vec::is_empty)
                    || self.elapsed_ms.is_none()
                    || !self.failure_codes.is_empty()
                    || (formal && self.human_review_status == E005HumanReviewStatus::NotRun)
                {
                    return Err(failure(
                        "E005_SUCCESS_RECEIPT_INVALID",
                        "successful E005 receipt is missing VP204, geometry, or review lineage",
                    ));
                }
            }
            E005RunStatus::Failed | E005RunStatus::Cancelled => {
                if self.authoring_count != 1
                    || self.failure_codes.is_empty()
                    || self.human_review_status != E005HumanReviewStatus::NotRun
                {
                    return Err(failure(
                        "E005_TERMINAL_RECEIPT_INVALID",
                        "failed or cancelled receipt must retain one author and a stable failure code",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn validate_formal_receipt_envelope(receipt: &E005RunReceipt) -> Result<(), Vp204RuntimeFailure> {
    let authorization_id = receipt
        .provider_authorization_id
        .as_deref()
        .ok_or_else(|| {
            failure(
                "E005_FORMAL_AUTHORIZATION_MISSING",
                "formal receipt is missing its Provider authorization",
            )
        })?;
    if !receipt.distribution_eligible
        || receipt
            .provider_authorization_sha256
            .as_deref()
            .is_none_or(|hash| !valid_hash(hash))
    {
        return Err(failure(
            "E005_FORMAL_AUTHORIZATION_INVALID",
            "formal receipt authorization hash or eligibility is invalid",
        ));
    }
    let evidence = receipt.provider_call_evidence.as_ref().ok_or_else(|| {
        failure(
            "E005_FORMAL_PROVIDER_EVIDENCE_MISSING",
            "formal receipt is missing persisted Provider budget evidence",
        )
    })?;
    let evidence_sha256 = semantic_sha256(evidence).map_err(core_failure)?;
    if evidence.is_empty()
        || evidence.len() > 2
        || evidence.len() != receipt.network_provider_calls as usize
        || receipt.provider_call_evidence_sha256.as_deref() != Some(evidence_sha256.as_str())
    {
        return Err(failure(
            "E005_FORMAL_PROVIDER_EVIDENCE_INVALID",
            "formal Provider evidence count or canonical hash is invalid",
        ));
    }
    let provider_id = &evidence[0].provider_id;
    let model_id = &evidence[0].model_id;
    let mut reservations = BTreeSet::new();
    for (index, item) in evidence.iter().enumerate() {
        let expected_kind = if index == 0 {
            E005ProviderCallKind::Author
        } else {
            E005ProviderCallKind::Patch
        };
        if item.authorization_id != authorization_id
            || item.task_id != receipt.task_id
            || item.task_payload_sha256 != receipt.task_payload_sha256
            || item.call_kind != expected_kind
            || item.provider_id != provider_id.as_str()
            || item.model_id != model_id.as_str()
            || item.settlement != "accounted"
            || !item.network_call_made
            || !reservations.insert(item.reservation_id.as_str())
        {
            return Err(failure(
                "E005_FORMAL_PROVIDER_EVIDENCE_LINEAGE_INVALID",
                "formal Provider evidence does not bind one ordered task execution",
            ));
        }
    }
    if receipt.usage.as_ref().is_none_or(|usage| {
        usage.provider_requests != receipt.network_provider_calls
            || usage.estimated_cost_microusd != receipt.billable_cost_microusd
    }) {
        return Err(failure(
            "E005_FORMAL_USAGE_MISMATCH",
            "formal receipt usage does not match Provider calls and cost",
        ));
    }
    if let Some(visual) = &receipt.visual_review_evidence {
        if !matches!(
            receipt.status,
            E005RunStatus::PassedWithoutPatch | E005RunStatus::PassedAfterPatch
        ) || evidence.len() != 2
            || evidence[0].outcome_code != "PROVIDER_COMPLETED_REPAIRABLE"
            || evidence[0].output_source_sha256.as_deref()
                != Some(visual.initial_source_sha256.as_str())
            || evidence[1].outcome_code != "PROVIDER_COMPLETED_PASSED"
            || evidence[1].output_source_sha256.as_deref()
                != Some(visual.final_source_sha256.as_str())
            || evidence[1].output_gate_sha256.as_deref()
                != Some(visual.comparison_report_sha256.as_str())
            || receipt.source_program_sha256.as_deref() != Some(visual.final_source_sha256.as_str())
            || receipt.gate_outcome_sha256.as_deref()
                != Some(visual.comparison_report_sha256.as_str())
            || (receipt.status == E005RunStatus::PassedWithoutPatch
                && visual.status != crate::E005VisualReviewStatusV1::AcceptedByVisualReview)
            || (receipt.status == E005RunStatus::PassedAfterPatch
                && visual.status
                    != crate::E005VisualReviewStatusV1::PatchedPendingVisualConfirmation)
        {
            return Err(failure(
                "E005_R2_FORMAL_VISUAL_LINEAGE_INVALID",
                "formal R2 receipt does not bind its Author, visual decision and final source",
            ));
        }
        return Ok(());
    }
    match receipt.status {
        E005RunStatus::PassedWithoutPatch => {
            if evidence.len() != 1
                || evidence[0].outcome_code != "PROVIDER_COMPLETED_PASSED"
                || evidence[0].output_source_sha256 != receipt.source_program_sha256
                || evidence[0].output_gate_sha256 != receipt.gate_outcome_sha256
            {
                return Err(failure(
                    "E005_FORMAL_FIRST_PASS_LINEAGE_INVALID",
                    "first-pass receipt does not match author settlement evidence",
                ));
            }
        }
        E005RunStatus::PassedAfterPatch => {
            if evidence.len() != 2
                || evidence[0].outcome_code != "PROVIDER_COMPLETED_REPAIRABLE"
                || evidence[1].outcome_code != "PROVIDER_COMPLETED_PASSED"
                || evidence[1].output_source_sha256 != receipt.source_program_sha256
                || evidence[1].output_gate_sha256 != receipt.gate_outcome_sha256
            {
                return Err(failure(
                    "E005_FORMAL_PATCH_LINEAGE_INVALID",
                    "patched receipt does not match author and patch settlement evidence",
                ));
            }
        }
        E005RunStatus::Failed | E005RunStatus::Cancelled => {}
        E005RunStatus::NotRun => {
            return Err(failure(
                "E005_FORMAL_NOT_RUN_FORBIDDEN",
                "formal Provider receipts cannot represent a not-run task",
            ))
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct E005OfflineHarness {
    coordinator: Vp204RuntimeCoordinator,
}

impl E005OfflineHarness {
    pub fn new(geometry: Arc<dyn RestrictedGeometryPort>) -> Self {
        Self {
            coordinator: Vp204RuntimeCoordinator::new(
                geometry,
                Arc::new(E005EngineeringGateEvaluator),
            ),
        }
    }

    pub async fn execute(
        &self,
        request: E005OfflineHarnessRequest,
        cancellation: CancellationToken,
    ) -> Result<E005RunReceipt, Vp204RuntimeFailure> {
        validate_task_binding(&request)?;
        let task_payload_sha256 = semantic_sha256(&request.task_payload).map_err(core_failure)?;
        let request_sha256 = semantic_sha256(&request).map_err(core_failure)?;
        let Some(source) = request.source.clone() else {
            if request.patch.is_some() {
                return Err(failure(
                    "E005_PATCH_WITHOUT_SOURCE",
                    "an E005 typed patch cannot run without one authored source",
                ));
            }
            let receipt = E005RunReceipt {
                schema_version: E005_RUN_RECEIPT_SCHEMA_VERSION.into(),
                run_id: format!("run_{}_not_run", request.task_id),
                task_set_sha256: request.task_set_sha256,
                task_id: request.task_id,
                status: E005RunStatus::NotRun,
                run_mode: "offline_deterministic".into(),
                distribution_eligible: false,
                author_source_mode: "missing".into(),
                task_payload_sha256,
                request_sha256,
                authoring_count: 0,
                patch_count: 0,
                provider_authorization_id: None,
                provider_authorization_sha256: None,
                provider_call_evidence: None,
                provider_call_evidence_sha256: None,
                visual_review_evidence: None,
                production_review_evidence: None,
                production_review_evidence_sha256: None,
                source_program_sha256: None,
                expanded_program_sha256: None,
                shape_program_sha256: None,
                structural_descriptor_sha256: None,
                semantic_structure_sha256: None,
                normalized_geometry_sha256: None,
                topology_signature_sha256: None,
                operation_sequence_sha256: None,
                profile_signature_sha256: None,
                part_zone_signature_sha256: None,
                glb_sha256: None,
                fixed_view_sha256: None,
                fixed_views: None,
                vp204_session_sha256: None,
                vp204_receipt_sha256: None,
                visual_session_sha256: None,
                visual_session_receipt_sha256: None,
                gate_outcome_sha256: None,
                compile_readback_sha256: None,
                restricted_geometry_evidence_sha256: None,
                artifact_profile_id: None,
                runtime_manifest_version: None,
                triangle_count: None,
                bounds_mm: None,
                mesh_count: None,
                primitive_count: None,
                material_count: None,
                usage: None,
                phase_receipts: None,
                elapsed_ms: None,
                network_provider_calls: 0,
                billable_cost_microusd: 0,
                failure_codes: vec!["E005_SOURCE_UNAVAILABLE".into()],
                human_review_status: E005HumanReviewStatus::NotRun,
            };
            receipt.validate()?;
            return Ok(receipt);
        };
        validate_source_operation_allowlist(&request.task_payload, &source)?;
        let suffix = &request_sha256[..16];
        let started = Instant::now();
        let result = self
            .coordinator
            .execute(
                Vp204RuntimeRequest {
                    session_id: format!("vpsession_e005_{suffix}"),
                    idempotency_key: format!("idem_e005_{suffix}"),
                    request_sha256: request_sha256.clone(),
                    source,
                    patch: request.patch.clone(),
                    usage: VisualProgramUsageV2::default(),
                },
                cancellation,
            )
            .await?;
        let receipt = adapt_result(
            request,
            task_payload_sha256,
            request_sha256,
            result,
            bounded_elapsed_ms(started),
        )?;
        receipt.validate()?;
        Ok(receipt)
    }
}

pub(crate) struct E005EngineeringGateEvaluator;

impl Vp204GateEvaluator for E005EngineeringGateEvaluator {
    fn evaluate(
        &self,
        source_program_sha256: &str,
        geometry: &RestrictedGeometryOutput,
    ) -> VisualProgramGateOutcomeV2 {
        let expected_views = BTreeSet::from(["front", "iso", "side", "top"]);
        let actual_views = geometry
            .view_sha256
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let (failure_report_id, repairable) = if geometry.glb_bytes.is_empty() {
            (Some("gate_e005_fail_empty_glb"), false)
        } else if geometry.glb_sha256 != geometry.readback.glb_sha256 {
            (Some("gate_e005_fail_glb_readback"), false)
        } else if geometry.readback.shape_program_sha256.len() != 64 {
            (Some("gate_e005_fail_shape_lineage"), false)
        } else if geometry.readback.triangle_count == 0 {
            (Some("gate_e005_fail_empty_mesh"), false)
        } else if !geometry
            .readback
            .bounds_mm
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
        {
            (Some("gate_e005_fail_bounds"), true)
        } else if !geometry.readback.closed_manifold {
            (Some("gate_e005_fail_closed_manifold"), true)
        } else if !geometry.readback.surface_provenance_present {
            (Some("gate_e005_fail_surface_provenance"), true)
        } else if actual_views != expected_views {
            (Some("gate_e005_fail_fixed_views"), false)
        } else {
            (None, false)
        };
        let pass = failure_report_id.is_none();
        VisualProgramGateOutcomeV2 {
            schema_version: VISUAL_PROGRAM_GATE_OUTCOME_SCHEMA_VERSION.into(),
            gate_report_id: failure_report_id
                .unwrap_or("gate_e005_engineering_pass")
                .into(),
            source_program_sha256: source_program_sha256.into(),
            verdict: if pass {
                VisualProgramGateVerdictV2::Pass
            } else {
                VisualProgramGateVerdictV2::Fail
            },
            repairable,
        }
    }
}

pub(crate) fn adapt_result(
    request: E005OfflineHarnessRequest,
    task_payload_sha256: String,
    request_sha256: String,
    result: crate::Vp204RuntimeResult,
    elapsed_ms: u64,
) -> Result<E005RunReceipt, Vp204RuntimeFailure> {
    let session = result.session;
    let receipt = &session.receipt;
    let session_sha256 = semantic_sha256(&session).map_err(core_failure)?;
    let receipt_sha256 = semantic_sha256(receipt).map_err(core_failure)?;
    if receipt_sha256 != session.receipt_sha256 {
        return Err(failure(
            "E005_VP204_RECEIPT_HASH_MISMATCH",
            "VP204 session receipt hash does not match its canonical receipt",
        ));
    }
    let geometry = result
        .current_geometry
        .as_ref()
        .or(result.initial_geometry.as_ref());
    let fixed_view_sha256 = phase_output(receipt, VisualProgramPhaseV2::Render);
    let gate_outcome_sha256 = phase_output(receipt, VisualProgramPhaseV2::Evaluate);
    let (status, failure_codes) = match session.state {
        VisualProgramAuthoringStateV2::ReadyForPreview if session.patch_count == 0 => {
            (E005RunStatus::PassedWithoutPatch, Vec::new())
        }
        VisualProgramAuthoringStateV2::ReadyForPreview => {
            (E005RunStatus::PassedAfterPatch, Vec::new())
        }
        VisualProgramAuthoringStateV2::AwaitingPatch => {
            (E005RunStatus::Failed, vec!["E005_PATCH_FAILED".into()])
        }
        VisualProgramAuthoringStateV2::Failed => {
            let code = if session
                .gate_report_id
                .as_deref()
                .is_some_and(|value| value.starts_with("gate_e005_fail_"))
            {
                "E005_HARD_GATE_FAILED".into()
            } else {
                map_failure_code(receipt.failure_code.as_deref())
            };
            (E005RunStatus::Failed, vec![code])
        }
        VisualProgramAuthoringStateV2::Cancelled => {
            (E005RunStatus::Cancelled, vec!["E005_CANCELLED".into()])
        }
        VisualProgramAuthoringStateV2::AwaitingInitialGate
        | VisualProgramAuthoringStateV2::AwaitingPatchedGate => {
            return Err(failure(
                "E005_VP204_SESSION_NOT_TERMINAL",
                "E005 adapter requires a terminal or explicitly repairable VP204 state",
            ));
        }
    };
    if matches!(
        status,
        E005RunStatus::PassedWithoutPatch | E005RunStatus::PassedAfterPatch
    ) && (geometry.is_none() || fixed_view_sha256.is_none() || gate_outcome_sha256.is_none())
    {
        return Err(failure(
            "E005_SUCCESS_EVIDENCE_INCOMPLETE",
            "successful E005 receipt requires geometry, render and gate evidence",
        ));
    }
    let execution_evidence_sha256 = geometry
        .map(|value| semantic_sha256(&value.execution_evidence).map_err(core_failure))
        .transpose()?;
    let structural = matches!(
        status,
        E005RunStatus::PassedWithoutPatch | E005RunStatus::PassedAfterPatch
    )
    .then(|| {
        if lower_visual_runtime_source_v1(&session.current_source)
            .map_err(core_failure)?
            .source_program_sha256
            != receipt.source_program_sha256
        {
            return Err(failure(
                "E005_FINAL_SOURCE_MISMATCH",
                "successful E005 structural fingerprints must bind the final VP204 source",
            ));
        }
        structural_fingerprints(&session.current_source)
    })
    .transpose()?;
    let normalized_geometry = matches!(
        status,
        E005RunStatus::PassedWithoutPatch | E005RunStatus::PassedAfterPatch
    )
    .then(|| {
        geometry
            .ok_or_else(|| {
                failure(
                    "E005_NORMALIZED_GEOMETRY_MISSING",
                    "successful E005 receipt lost final geometry bytes",
                )
            })
            .and_then(|value| normalized_geometry_sha256(&value.glb_bytes).map_err(core_failure))
    })
    .transpose()?;
    let structural_descriptor = structural
        .as_ref()
        .zip(normalized_geometry.as_ref())
        .map(|(structure, normalized)| {
            semantic_sha256(&json!({
                "final_source_program_sha256": receipt.source_program_sha256,
                "shape_program_sha256": receipt.shape_program_sha256,
                "glb_sha256": receipt.glb_sha256,
                "semantic_structure_sha256": structure.semantic_structure_sha256,
                "normalized_geometry_sha256": normalized
            }))
            .map_err(core_failure)
        })
        .transpose()?;
    let adapted = E005RunReceipt {
        schema_version: E005_RUN_RECEIPT_SCHEMA_VERSION.into(),
        run_id: format!(
            "run_{}_{suffix}",
            request.task_id,
            suffix = &request_sha256[..16]
        ),
        task_set_sha256: request.task_set_sha256,
        task_id: request.task_id,
        status,
        run_mode: "offline_deterministic".into(),
        distribution_eligible: false,
        author_source_mode: "offline_authored_v2".into(),
        task_payload_sha256,
        request_sha256,
        authoring_count: session.authoring_count,
        patch_count: session.patch_count,
        provider_authorization_id: None,
        provider_authorization_sha256: None,
        provider_call_evidence: None,
        provider_call_evidence_sha256: None,
        visual_review_evidence: None,
        production_review_evidence: None,
        production_review_evidence_sha256: None,
        source_program_sha256: Some(receipt.source_program_sha256.clone()),
        expanded_program_sha256: Some(receipt.expanded_program_sha256.clone()),
        shape_program_sha256: Some(receipt.shape_program_sha256.clone()),
        structural_descriptor_sha256: structural_descriptor,
        semantic_structure_sha256: structural
            .as_ref()
            .map(|value| value.semantic_structure_sha256.clone()),
        normalized_geometry_sha256: normalized_geometry,
        topology_signature_sha256: structural
            .as_ref()
            .map(|value| value.topology_signature_sha256.clone()),
        operation_sequence_sha256: structural
            .as_ref()
            .map(|value| value.operation_sequence_sha256.clone()),
        profile_signature_sha256: structural
            .as_ref()
            .map(|value| value.profile_signature_sha256.clone()),
        part_zone_signature_sha256: structural
            .as_ref()
            .map(|value| value.part_zone_signature_sha256.clone()),
        glb_sha256: receipt.glb_sha256.clone(),
        fixed_view_sha256,
        fixed_views: geometry.map(|value| value.view_sha256.clone()),
        vp204_session_sha256: Some(session_sha256),
        vp204_receipt_sha256: Some(receipt_sha256),
        visual_session_sha256: None,
        visual_session_receipt_sha256: None,
        gate_outcome_sha256,
        compile_readback_sha256: geometry
            .map(|value| value.readback.compile_readback_sha256.clone()),
        restricted_geometry_evidence_sha256: execution_evidence_sha256,
        artifact_profile_id: geometry.map(|value| value.readback.artifact_profile_id.clone()),
        runtime_manifest_version: geometry
            .map(|value| value.readback.runtime_manifest_version.clone()),
        triangle_count: geometry.map(|value| value.readback.triangle_count),
        bounds_mm: geometry.map(|value| value.readback.bounds_mm),
        mesh_count: geometry.map(|value| value.readback.mesh_count),
        primitive_count: geometry.map(|value| value.readback.primitive_count),
        material_count: geometry.map(|value| value.readback.material_count),
        usage: Some(receipt.usage.clone()),
        phase_receipts: Some(receipt.phases.clone()),
        elapsed_ms: Some(elapsed_ms),
        network_provider_calls: receipt.usage.provider_requests,
        billable_cost_microusd: receipt.usage.estimated_cost_microusd,
        failure_codes,
        human_review_status: E005HumanReviewStatus::NotRun,
    };
    Ok(adapted)
}

struct StructuralFingerprints {
    semantic_structure_sha256: String,
    topology_signature_sha256: String,
    operation_sequence_sha256: String,
    profile_signature_sha256: String,
    part_zone_signature_sha256: String,
}

fn canonical_node_signature(
    node_id: &str,
    nodes_by_id: &BTreeMap<String, &Value>,
    memo: &mut BTreeMap<String, String>,
    visiting: &mut BTreeSet<String>,
) -> Result<String, Vp204RuntimeFailure> {
    if let Some(signature) = memo.get(node_id) {
        return Ok(signature.clone());
    }
    if !visiting.insert(node_id.to_owned()) {
        return Err(failure(
            "E005_STRUCTURAL_GRAPH_CYCLE",
            "E005 structural graph contains a cycle",
        ));
    }
    let node = nodes_by_id.get(node_id).ok_or_else(|| {
        failure(
            "E005_STRUCTURAL_NODE_MISSING",
            "E005 structural graph references a missing node",
        )
    })?;
    let kind = node
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| failure("E005_STRUCTURAL_KIND_MISSING", "E005 node kind is missing"))?;
    let mut inputs = Vec::new();
    if let Some(input) = node.get("input_node_id").and_then(Value::as_str) {
        inputs.push(canonical_node_signature(
            input,
            nodes_by_id,
            memo,
            visiting,
        )?);
    }
    if let Some(input_ids) = node.get("input_node_ids").and_then(Value::as_array) {
        for input in input_ids.iter().filter_map(Value::as_str) {
            inputs.push(canonical_node_signature(
                input,
                nodes_by_id,
                memo,
                visiting,
            )?);
        }
        if kind == "union" {
            inputs.sort_unstable();
        } else if kind == "subtract" && inputs.len() > 2 {
            inputs[1..].sort_unstable();
        }
    }
    let signature = semantic_sha256(&json!({
        "kind": kind,
        "inputs": inputs,
        "axis": node.get("axis"),
        "has_profile": node.get("profile_id").is_some(),
        "has_section_set": node.get("section_set_id").is_some(),
        "path_closed": node.get("path_closed"),
        "cap_start": node.get("cap_start"),
        "cap_end": node.get("cap_end"),
        "part_role": (kind == "part").then(|| node.get("role")).flatten()
    }))
    .map_err(core_failure)?;
    visiting.remove(node_id);
    memo.insert(node_id.to_owned(), signature.clone());
    Ok(signature)
}

fn collect_reachable_nodes(
    node_id: &str,
    nodes_by_id: &BTreeMap<String, &Value>,
    reachable: &mut BTreeSet<String>,
) -> Result<(), Vp204RuntimeFailure> {
    if !reachable.insert(node_id.to_owned()) {
        return Ok(());
    }
    let node = nodes_by_id.get(node_id).ok_or_else(|| {
        failure(
            "E005_STRUCTURAL_NODE_MISSING",
            "E005 structural graph references a missing node",
        )
    })?;
    if let Some(input) = node.get("input_node_id").and_then(Value::as_str) {
        collect_reachable_nodes(input, nodes_by_id, reachable)?;
    }
    if let Some(input_ids) = node.get("input_node_ids").and_then(Value::as_array) {
        for input in input_ids.iter().filter_map(Value::as_str) {
            collect_reachable_nodes(input, nodes_by_id, reachable)?;
        }
    }
    Ok(())
}

fn structural_fingerprints(source: &Value) -> Result<StructuralFingerprints, Vp204RuntimeFailure> {
    let geometry_source = if source.get("schema_version").and_then(Value::as_str)
        == Some("ForgeVisualAuthorSource@1")
    {
        source.get("geometry_templates").ok_or_else(|| {
            failure(
                "E005_SOURCE_GEOMETRY_TEMPLATES_MISSING",
                "unified author source geometry templates are missing",
            )
        })?
    } else {
        source
    };
    let nodes = geometry_source
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            failure(
                "E005_SOURCE_NODES_MISSING",
                "authored v2 source nodes are missing",
            )
        })?;
    let nodes_by_id = nodes
        .iter()
        .filter_map(|node| {
            node.get("node_id")
                .and_then(Value::as_str)
                .map(|node_id| (node_id.to_owned(), node))
        })
        .collect::<BTreeMap<_, _>>();
    let profiles = geometry_source
        .get("profiles")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let profile_indexes = profiles
        .iter()
        .enumerate()
        .filter_map(|(index, profile)| {
            profile
                .get("profile_id")
                .and_then(Value::as_str)
                .map(|profile_id| (profile_id.to_owned(), index))
        })
        .collect::<BTreeMap<_, _>>();
    let section_sets = geometry_source
        .get("section_sets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let output_node_ids = geometry_source
        .get("outputs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|output| output.get("node_id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let mut memo = BTreeMap::new();
    let mut visiting = BTreeSet::new();
    let mut output_signatures = output_node_ids
        .iter()
        .map(|node_id| canonical_node_signature(node_id, &nodes_by_id, &mut memo, &mut visiting))
        .collect::<Result<Vec<_>, _>>()?;
    output_signatures.sort_unstable();
    let mut reachable = BTreeSet::new();
    for node_id in output_node_ids {
        collect_reachable_nodes(node_id, &nodes_by_id, &mut reachable)?;
    }
    let mut operation_counts = BTreeMap::<String, usize>::new();
    let mut part_zone_signatures = Vec::new();
    for node_id in &reachable {
        let node = nodes_by_id[node_id];
        let kind = node
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| failure("E005_STRUCTURAL_KIND_MISSING", "E005 node kind is missing"))?;
        *operation_counts.entry(kind.to_owned()).or_default() += 1;
        if matches!(kind, "part" | "material_zone") {
            part_zone_signatures.push(canonical_node_signature(
                node_id,
                &nodes_by_id,
                &mut memo,
                &mut visiting,
            )?);
        }
    }
    part_zone_signatures.sort_unstable();
    let mut normalized_profiles = profiles
        .iter()
        .map(|profile| {
            json!({
                "points": profile.get("points"),
                "resample_count": profile.get("resample_count")
            })
        })
        .collect::<Vec<_>>();
    normalized_profiles.sort_by_key(|profile| semantic_sha256(profile).unwrap_or_default());
    let mut normalized_section_sets = section_sets
        .iter()
        .map(|section_set| {
            let sections = section_set
                .get("sections")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|section| {
                    json!({
                        "position": section.get("position"),
                        "profile_index": section.get("profile_id").and_then(Value::as_str).and_then(|profile_id| profile_indexes.get(profile_id)).copied(),
                        "scale": section.get("scale"),
                        "twist_degrees": section.get("twist_degrees"),
                        "cap_policy": section.get("cap_policy")
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "main_axis": section_set.get("main_axis"),
                "sections": sections
            })
        })
        .collect::<Vec<_>>();
    normalized_section_sets
        .sort_by_key(|section_set| semantic_sha256(section_set).unwrap_or_default());
    let topology_signature_sha256 = semantic_sha256(&output_signatures).map_err(core_failure)?;
    let operation_sequence_sha256 = semantic_sha256(&operation_counts).map_err(core_failure)?;
    let part_zone_signature_sha256 =
        semantic_sha256(&part_zone_signatures).map_err(core_failure)?;
    Ok(StructuralFingerprints {
        semantic_structure_sha256: semantic_sha256(&json!({
            "topology": topology_signature_sha256,
            "operation_multiset": operation_sequence_sha256,
            "part_zone_relations": part_zone_signature_sha256
        }))
        .map_err(core_failure)?,
        topology_signature_sha256,
        operation_sequence_sha256,
        profile_signature_sha256: semantic_sha256(&json!({
            "profiles": normalized_profiles,
            "section_sets": normalized_section_sets
        }))
        .map_err(core_failure)?,
        part_zone_signature_sha256,
    })
}

pub(crate) fn adapt_r2_formal_result(
    task_set_sha256: String,
    task_id: String,
    task_payload_sha256: String,
    request_sha256: String,
    authorization_id: String,
    authorization_sha256: String,
    evidence: Vec<E005ProviderBudgetEvidence>,
    final_source: Value,
    final_geometry: &RestrictedGeometryOutput,
    visual_session: crate::E005VisualSessionV1,
    elapsed_ms: u64,
) -> Result<E005RunReceipt, Vp204RuntimeFailure> {
    visual_session.validate().map_err(core_failure)?;
    let lowering = lower_visual_runtime_source_v1(&final_source).map_err(core_failure)?;
    let structural = structural_fingerprints(&final_source)?;
    let evidence_sha256 = semantic_sha256(&evidence).map_err(core_failure)?;
    let visual_session_sha256 = semantic_sha256(&visual_session).map_err(core_failure)?;
    let structural_descriptor_sha256 = semantic_sha256(&json!({
        "final_source_program_sha256": lowering.source_program_sha256,
        "shape_program_sha256": lowering.shape_program_sha256,
        "glb_sha256": final_geometry.glb_sha256,
        "semantic_structure_sha256": structural.semantic_structure_sha256,
        "normalized_geometry_sha256": visual_session.receipt.normalized_geometry_sha256,
    }))
    .map_err(core_failure)?;
    let status = match visual_session.state {
        crate::E005VisualReviewStatusV1::AcceptedByVisualReview => {
            E005RunStatus::PassedWithoutPatch
        }
        crate::E005VisualReviewStatusV1::PatchedPendingVisualConfirmation => {
            E005RunStatus::PassedAfterPatch
        }
    };
    let patch_count = u8::from(status == E005RunStatus::PassedAfterPatch);
    let usage = visual_session.receipt.usage.clone();
    let receipt = E005RunReceipt {
        schema_version: E005_RUN_RECEIPT_SCHEMA_VERSION.into(),
        run_id: format!("run_{task_id}_{suffix}", suffix = &request_sha256[..16]),
        task_set_sha256,
        task_id,
        status,
        run_mode: "formal_provider".into(),
        distribution_eligible: true,
        author_source_mode: "provider_authored_v2".into(),
        task_payload_sha256,
        request_sha256,
        authoring_count: 1,
        patch_count,
        provider_authorization_id: Some(authorization_id),
        provider_authorization_sha256: Some(authorization_sha256),
        provider_call_evidence: Some(evidence),
        provider_call_evidence_sha256: Some(evidence_sha256),
        visual_review_evidence: Some(visual_session.review_evidence.clone()),
        production_review_evidence: None,
        production_review_evidence_sha256: None,
        source_program_sha256: Some(lowering.source_program_sha256),
        expanded_program_sha256: Some(lowering.expanded_program_sha256),
        shape_program_sha256: Some(lowering.shape_program_sha256),
        structural_descriptor_sha256: Some(structural_descriptor_sha256),
        semantic_structure_sha256: Some(structural.semantic_structure_sha256),
        normalized_geometry_sha256: Some(visual_session.receipt.normalized_geometry_sha256.clone()),
        topology_signature_sha256: Some(structural.topology_signature_sha256),
        operation_sequence_sha256: Some(structural.operation_sequence_sha256),
        profile_signature_sha256: Some(structural.profile_signature_sha256),
        part_zone_signature_sha256: Some(structural.part_zone_signature_sha256),
        glb_sha256: Some(final_geometry.glb_sha256.clone()),
        fixed_view_sha256: Some(visual_session.receipt.fixed_view_sha256.clone()),
        fixed_views: Some(final_geometry.view_sha256.clone()),
        vp204_session_sha256: None,
        vp204_receipt_sha256: None,
        visual_session_sha256: Some(visual_session_sha256),
        visual_session_receipt_sha256: Some(visual_session.receipt_sha256.clone()),
        gate_outcome_sha256: Some(visual_session.receipt.comparison_report_sha256.clone()),
        compile_readback_sha256: Some(final_geometry.readback.compile_readback_sha256.clone()),
        restricted_geometry_evidence_sha256: Some(
            visual_session
                .receipt
                .restricted_geometry_evidence_sha256
                .clone(),
        ),
        artifact_profile_id: Some(final_geometry.readback.artifact_profile_id.clone()),
        runtime_manifest_version: Some(final_geometry.readback.runtime_manifest_version.clone()),
        triangle_count: Some(final_geometry.readback.triangle_count),
        bounds_mm: Some(final_geometry.readback.bounds_mm),
        mesh_count: Some(final_geometry.readback.mesh_count),
        primitive_count: Some(final_geometry.readback.primitive_count),
        material_count: Some(final_geometry.readback.material_count),
        usage: Some(usage.clone()),
        phase_receipts: Some(visual_session.receipt.phases.clone()),
        elapsed_ms: Some(elapsed_ms),
        network_provider_calls: usage.provider_requests,
        billable_cost_microusd: usage.estimated_cost_microusd,
        failure_codes: Vec::new(),
        human_review_status: E005HumanReviewStatus::Pending,
    };
    receipt.validate()?;
    Ok(receipt)
}

pub(crate) fn upgrade_r3_formal_receipt(
    mut receipt: E005RunReceipt,
    production: crate::E005ProductionReviewV1,
    geometry: &RestrictedGeometryOutput,
    elapsed_ms: u64,
) -> Result<E005RunReceipt, Vp204RuntimeFailure> {
    production.validate().map_err(core_failure)?;
    if receipt.run_mode != "formal_provider"
        || receipt.visual_review_evidence.is_none()
        || receipt.source_program_sha256.as_deref()
            != Some(production.source_program_sha256.as_str())
        || geometry.glb_sha256 != production.glb_sha256
        || geometry.readback.artifact_profile_id != "production_concept"
    {
        return Err(failure(
            "E005_R3_UPGRADE_INVALID",
            "R3 may upgrade only the exact R2 final source and production geometry.",
        ));
    }
    let production_sha256 = semantic_sha256(&production).map_err(core_failure)?;
    receipt.structural_descriptor_sha256 = Some(
        semantic_sha256(&json!({
            "final_source_program_sha256":receipt.source_program_sha256,
            "shape_program_sha256":receipt.shape_program_sha256,
            "glb_sha256":production.glb_sha256,
            "semantic_structure_sha256":receipt.semantic_structure_sha256,
            "normalized_geometry_sha256":production.normalized_geometry_sha256,
            "production_review_sha256":production_sha256,
        }))
        .map_err(core_failure)?,
    );
    receipt.normalized_geometry_sha256 = Some(production.normalized_geometry_sha256.clone());
    receipt.glb_sha256 = Some(production.glb_sha256.clone());
    receipt.fixed_view_sha256 = Some(production.fixed_view_sha256.clone());
    receipt.fixed_views = Some(production.fixed_views.clone());
    receipt.compile_readback_sha256 = Some(production.compile_readback_sha256.clone());
    receipt.restricted_geometry_evidence_sha256 =
        Some(production.restricted_geometry_evidence_sha256.clone());
    receipt.artifact_profile_id = Some(production.artifact_profile_id.clone());
    receipt.runtime_manifest_version = Some(geometry.readback.runtime_manifest_version.clone());
    receipt.triangle_count = Some(geometry.readback.triangle_count);
    receipt.bounds_mm = Some(geometry.readback.bounds_mm);
    receipt.mesh_count = Some(geometry.readback.mesh_count);
    receipt.primitive_count = Some(geometry.readback.primitive_count);
    receipt.material_count = Some(geometry.readback.material_count);
    receipt.production_review_evidence = Some(production);
    receipt.production_review_evidence_sha256 = Some(production_sha256);
    receipt.elapsed_ms = Some(elapsed_ms);
    receipt.validate()?;
    Ok(receipt)
}

fn validate_task_binding(request: &E005OfflineHarnessRequest) -> Result<(), Vp204RuntimeFailure> {
    if request.task_set_sha256 != E005_TASK_SET_SHA256 {
        return Err(failure(
            "E005_TASK_SET_HASH_MISMATCH",
            "offline harness requires the frozen E005 task set",
        ));
    }
    if request.task_payload.get("task_id").and_then(Value::as_str) != Some(request.task_id.as_str())
    {
        return Err(failure(
            "E005_TASK_PAYLOAD_ID_MISMATCH",
            "task payload must bind the requested frozen task id",
        ));
    }
    Ok(())
}

pub(crate) fn validate_source_operation_allowlist(
    task: &Value,
    source: &Value,
) -> Result<(), Vp204RuntimeFailure> {
    let allowed = task
        .get("allowed_operation_families")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            failure(
                "E005_TASK_OPERATION_ALLOWLIST_MISSING",
                "task allowlist is missing",
            )
        })?
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    let geometry_source = if source.get("schema_version").and_then(Value::as_str)
        == Some("ForgeVisualAuthorSource@1")
    {
        source.get("geometry_templates").ok_or_else(|| {
            failure(
                "E005_SOURCE_GEOMETRY_TEMPLATES_MISSING",
                "unified author source geometry templates are missing",
            )
        })?
    } else {
        source
    };
    let nodes = geometry_source
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            failure(
                "E005_SOURCE_NODES_MISSING",
                "authored v2 source nodes are missing",
            )
        })?;
    for node in nodes {
        let Some(kind) = node.get("kind").and_then(Value::as_str) else {
            return Err(failure(
                "E005_SOURCE_NODE_KIND_MISSING",
                "source node kind is missing",
            ));
        };
        let family = match kind {
            "box" => Some("box"),
            "extrude" => Some("extrude"),
            "revolve" => Some("revolve"),
            "loft" => Some("loft"),
            "sweep" => Some("sweep"),
            "union" | "subtract" => Some("boolean"),
            "mirror" => Some("mirror"),
            "array" => Some("array"),
            "part" | "material_zone" => None,
            _ => {
                return Err(failure(
                    "E005_SOURCE_OPERATION_UNSUPPORTED",
                    format!("source node kind {kind} is outside the E005 language"),
                ))
            }
        };
        if family.is_some_and(|value| !allowed.contains(value)) {
            return Err(failure(
                "E005_SOURCE_OPERATION_NOT_ALLOWED",
                format!(
                    "source operation family {} is outside the frozen task allowlist",
                    family.unwrap()
                ),
            ));
        }
    }
    Ok(())
}

fn phase_output(
    receipt: &forgecad_core::VisualProgramExecutionReceiptV2,
    target: VisualProgramPhaseV2,
) -> Option<String> {
    receipt
        .phases
        .iter()
        .rev()
        .find(|phase| phase.phase == target)
        .map(|phase| phase.output_sha256.clone())
}

fn map_failure_code(code: Option<&str>) -> String {
    match code.unwrap_or_default() {
        value if value.contains("CANCEL") => "E005_CANCELLED",
        value if value.contains("TIMEOUT") => "E005_TIMEOUT",
        value if value.contains("SCHEMA") || value.contains("INVALID_INPUT") => {
            "E005_SCHEMA_INVALID"
        }
        value if value.contains("LOWER") => "E005_LOWERING_FAILED",
        value if value.contains("READBACK") => "E005_READBACK_FAILED",
        value if value.contains("RENDER") => "E005_RENDER_FAILED",
        value if value.contains("COMPILE") || value.contains("GEOMETRY") => "E005_COMPILE_FAILED",
        _ => "E005_INTERNAL_ERROR",
    }
    .into()
}

fn bounded_elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis())
        .unwrap_or(u64::MAX)
        .min(900_000)
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn failure(code: impl Into<String>, message: impl Into<String>) -> Vp204RuntimeFailure {
    Vp204RuntimeFailure {
        code: code.into(),
        message: message.into(),
    }
}

fn core_failure(error: forgecad_core::CoreError) -> Vp204RuntimeFailure {
    failure(error.code(), error.to_string())
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{
        collections::BTreeMap,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use crate::{
        RestrictedGeometryError, RestrictedGeometryErrorKind, RestrictedGeometryExecutionEvidence,
        RestrictedGeometryFuture, RestrictedGeometryInput, RestrictedGeometryReadback,
        RESTRICTED_GEOMETRY_OUTPUT_SCHEMA_VERSION,
    };

    struct PanicGeometryPort {
        calls: Arc<AtomicUsize>,
    }

    impl RestrictedGeometryPort for PanicGeometryPort {
        fn build_compile_render(
            &self,
            _input: RestrictedGeometryInput,
            _cancellation: CancellationToken,
        ) -> RestrictedGeometryFuture {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { panic!("missing E005 source must not reach geometry") })
        }
    }

    fn task() -> Value {
        serde_json::from_str::<Value>(include_str!(
            "../../../../../../packages/concept-spec/fixtures/e005-unseen-mechanical-hard-surface-task-set.json"
        ))
        .unwrap()["tasks"][0]
            .clone()
    }

    fn source() -> Value {
        serde_json::from_str(include_str!(
            "../../../../../../packages/concept-spec/fixtures/e005-harness-sensor-pod-source.json"
        ))
        .unwrap()
    }

    #[test]
    fn e005_missing_source_returns_not_run_before_geometry() {
        let calls = Arc::new(AtomicUsize::new(0));
        let harness = E005OfflineHarness::new(Arc::new(PanicGeometryPort {
            calls: calls.clone(),
        }));
        let receipt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(harness.execute(
                E005OfflineHarnessRequest {
                    task_set_sha256: E005_TASK_SET_SHA256.into(),
                    task_id: "e005_enclosure_sensor_pod".into(),
                    task_payload: task(),
                    source: None,
                    patch: None,
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(receipt.status, E005RunStatus::NotRun);
        assert!(!receipt.distribution_eligible);
        assert_eq!(receipt.authoring_count, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(receipt.glb_sha256.is_none());
    }

    #[test]
    fn e005_source_outside_task_allowlist_fails_before_geometry() {
        let calls = Arc::new(AtomicUsize::new(0));
        let harness = E005OfflineHarness::new(Arc::new(PanicGeometryPort {
            calls: calls.clone(),
        }));
        let source = serde_json::from_str::<Value>(include_str!(
            "../../../../../../packages/concept-spec/fixtures/forge-visual-geometry-v2-rotor.json"
        ))
        .unwrap();
        let error = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(harness.execute(
                E005OfflineHarnessRequest {
                    task_set_sha256: E005_TASK_SET_SHA256.into(),
                    task_id: "e005_enclosure_sensor_pod".into(),
                    task_payload: task(),
                    source: Some(source),
                    patch: None,
                },
                CancellationToken::new(),
            ))
            .unwrap_err();
        assert_eq!(error.code, "E005_SOURCE_OPERATION_NOT_ALLOWED");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn e005_patch_without_source_fails_before_geometry() {
        let calls = Arc::new(AtomicUsize::new(0));
        let harness = E005OfflineHarness::new(Arc::new(PanicGeometryPort {
            calls: calls.clone(),
        }));
        let error = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(harness.execute(
                E005OfflineHarnessRequest {
                    task_set_sha256: E005_TASK_SET_SHA256.into(),
                    task_id: "e005_enclosure_sensor_pod".into(),
                    task_payload: task(),
                    source: None,
                    patch: Some(serde_json::json!({"schema_version":"ForgeVisualGeometryPatch@1"})),
                },
                CancellationToken::new(),
            ))
            .unwrap_err();
        assert_eq!(error.code, "E005_PATCH_WITHOUT_SOURCE");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn e005_structural_fingerprint_ignores_source_order_but_tracks_real_relations() {
        let original = source();
        let original_fingerprints = structural_fingerprints(&original).unwrap();
        let mut reordered = original.clone();
        reordered["nodes"].as_array_mut().unwrap().reverse();
        reordered["outputs"].as_array_mut().unwrap().reverse();
        let reordered_fingerprints = structural_fingerprints(&reordered).unwrap();
        assert_eq!(
            original_fingerprints.semantic_structure_sha256,
            reordered_fingerprints.semantic_structure_sha256
        );
        assert_eq!(
            original_fingerprints.topology_signature_sha256,
            reordered_fingerprints.topology_signature_sha256
        );
        assert_eq!(
            original_fingerprints.operation_sequence_sha256,
            reordered_fingerprints.operation_sequence_sha256
        );
        assert_eq!(
            original_fingerprints.part_zone_signature_sha256,
            reordered_fingerprints.part_zone_signature_sha256
        );

        let mut changed_relation = original;
        let lower_part = changed_relation["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["node_id"] == "node_lower_shell_part")
            .unwrap();
        lower_part["input_node_id"] = json!("node_upper_shell");
        assert_ne!(
            original_fingerprints.semantic_structure_sha256,
            structural_fingerprints(&changed_relation)
                .unwrap()
                .semantic_structure_sha256
        );
    }

    #[test]
    fn e005_precancelled_run_returns_terminal_cancelled_receipt_before_geometry() {
        let calls = Arc::new(AtomicUsize::new(0));
        let harness = E005OfflineHarness::new(Arc::new(PanicGeometryPort {
            calls: calls.clone(),
        }));
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let receipt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(harness.execute(
                E005OfflineHarnessRequest {
                    task_set_sha256: E005_TASK_SET_SHA256.into(),
                    task_id: "e005_enclosure_sensor_pod".into(),
                    task_payload: task(),
                    source: Some(source()),
                    patch: None,
                },
                cancellation,
            ))
            .unwrap();
        assert_eq!(receipt.status, E005RunStatus::Cancelled);
        assert_eq!(receipt.failure_codes, ["E005_CANCELLED"]);
        assert!(receipt.glb_sha256.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
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
                    message: "E005 bounded geometry deadline exceeded".into(),
                    recoverable: true,
                })
            })
        }
    }

    #[test]
    fn e005_timeout_maps_to_stable_failed_receipt_without_artifact() {
        let harness = E005OfflineHarness::new(Arc::new(TimeoutGeometryPort));
        let receipt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(harness.execute(
                E005OfflineHarnessRequest {
                    task_set_sha256: E005_TASK_SET_SHA256.into(),
                    task_id: "e005_enclosure_sensor_pod".into(),
                    task_payload: task(),
                    source: Some(source()),
                    patch: None,
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(receipt.status, E005RunStatus::Failed);
        assert_eq!(receipt.failure_codes, ["E005_TIMEOUT"]);
        assert!(receipt.glb_sha256.is_none());
        assert!(receipt.compile_readback_sha256.is_none());
    }

    #[derive(Default)]
    pub(crate) struct RepairableGeometryPort {
        calls: AtomicUsize,
    }

    impl RepairableGeometryPort {
        pub(crate) fn passing() -> Self {
            Self {
                calls: AtomicUsize::new(1),
            }
        }

        pub(crate) fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    fn e005_test_triangle_glb(label: &str) -> Vec<u8> {
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

    impl RestrictedGeometryPort for RepairableGeometryPort {
        fn build_compile_render(
            &self,
            input: RestrictedGeometryInput,
            _cancellation: CancellationToken,
        ) -> RestrictedGeometryFuture {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            let shape_sha256 = crate::canonical::sha256_hex(
                crate::canonical::canonical_json(&input.shape_program).as_bytes(),
            );
            Box::pin(async move {
                let glb_bytes = e005_test_triangle_glb(&shape_sha256);
                let glb_sha256 = crate::canonical::sha256_hex(&glb_bytes);
                let views = ["front", "iso", "side", "top"]
                    .into_iter()
                    .map(|name| {
                        (
                            name.to_string(),
                            format!("PNG E005 patch {name} {shape_sha256}").into_bytes(),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                let view_sha256 = views
                    .iter()
                    .map(|(name, bytes)| (name.clone(), crate::canonical::sha256_hex(bytes)))
                    .collect();
                let operation_ids = input
                    .shape_program
                    .get("operations")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|operation| {
                        operation
                            .get("operation_id")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .collect::<Vec<_>>();
                let (fragment_cache_hit_operation_ids, fragment_cache_miss_operation_ids) =
                    if call == 0 {
                        (Vec::new(), operation_ids)
                    } else {
                        operation_ids
                            .into_iter()
                            .partition(|operation_id| operation_id != "op_upper_shell")
                    };
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
                        triangle_count: 100,
                        bounds_mm: [760.0, 600.0, 612.0],
                        mesh_count: 1,
                        primitive_count: 7,
                        material_count: 8,
                        closed_manifold: call > 0,
                        surface_provenance_present: true,
                        compile_readback_sha256: crate::canonical::sha256_hex(
                            format!("e005_patch_readback_{call}").as_bytes(),
                        ),
                        material_zone_count: 5,
                        visual_texture_set_count: 0,
                        visual_texture_map_count: 0,
                        visual_texture_provenance_verified: true,
                        reference_appearance_projection_receipts: Vec::new(),
                    },
                    views,
                    view_sha256,
                    renderer_id: "forgecad-agent-software-raster@1".into(),
                    execution_evidence: RestrictedGeometryExecutionEvidence {
                        schema_version: "RestrictedGeometryExecutionEvidence@1".into(),
                        compile_cache_key_sha256: crate::canonical::sha256_hex(
                            shape_sha256.as_bytes(),
                        ),
                        compile_cache_hit: false,
                        compile_duration_ms: 4,
                        render_duration_ms: 2,
                        fragment_cache_hit_operation_ids,
                        fragment_cache_miss_operation_ids,
                    },
                })
            })
        }
    }

    #[test]
    fn e005_one_typed_patch_repairs_initial_engineering_gate_and_emits_receipt() {
        let source = source();
        let patch = serde_json::json!({
            "schema_version": "ForgeVisualGeometryPatch@1",
            "patch_id": "patch_e005_upper_shell_position",
            "expected_source_sha256": semantic_sha256(&source).unwrap(),
            "operations": [{
                "op": "set_node_position",
                "node_id": "node_upper_shell",
                "position": [0.0, 0.0, 300.0]
            }]
        });
        let geometry = Arc::new(RepairableGeometryPort::default());
        let harness = E005OfflineHarness::new(geometry.clone());
        let receipt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(harness.execute(
                E005OfflineHarnessRequest {
                    task_set_sha256: E005_TASK_SET_SHA256.into(),
                    task_id: "e005_enclosure_sensor_pod".into(),
                    task_payload: task(),
                    source: Some(source),
                    patch: Some(patch),
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(receipt.status, E005RunStatus::PassedAfterPatch);
        assert_eq!(receipt.authoring_count, 1);
        assert_eq!(receipt.patch_count, 1);
        assert!(receipt.failure_codes.is_empty());
        assert_eq!(geometry.calls.load(Ordering::SeqCst), 2);
        let phases = receipt.phase_receipts.as_ref().unwrap();
        assert!(phases
            .iter()
            .any(|phase| phase.phase == VisualProgramPhaseV2::Patch));
        assert_eq!(
            phases
                .iter()
                .filter(|phase| phase.phase == VisualProgramPhaseV2::CompileReadback)
                .count(),
            2
        );
        let patched_compile = phases
            .iter()
            .rev()
            .find(|phase| phase.phase == VisualProgramPhaseV2::CompileReadback)
            .unwrap();
        assert!(patched_compile
            .fragment_cache_hit_operation_ids
            .iter()
            .any(|operation_id| operation_id == "op_lower_shell"));
        assert_eq!(
            patched_compile.fragment_cache_miss_operation_ids,
            ["op_upper_shell"]
        );
    }
}
