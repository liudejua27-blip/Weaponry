//! Rust-owned multimodal evidence contracts for programmatic visual design.
//!
//! Images and GLBs remain sealed `ReferenceEvidence@1` objects. A vision
//! provider may describe those objects, but it cannot create geometry, mutate
//! product state, or turn an inference into an observation. Rust binds the
//! resulting claims to one exact `ForgeVisualProgram@1` through explicit
//! dispositions, preserving a single executable design truth.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    semantic_sha256, CoreError, CoreResult, ForgeVisualProgram, ReferenceEvidence,
    VisualDetailLevel, VisualDetailStatus, VisualFixedViewEvidence, REQUIRED_VISUAL_VIEW_IDS,
};

pub const MULTIMODAL_DESIGN_REQUEST_SCHEMA_VERSION: &str = "MultimodalDesignRequest@1";
pub const VISUAL_EVIDENCE_GRAPH_SCHEMA_VERSION: &str = "VisualEvidenceGraph@1";
pub const MULTIMODAL_PROGRAM_EVIDENCE_BINDING_SCHEMA_VERSION: &str =
    "MultimodalProgramEvidenceBinding@1";
pub const VISUAL_REFERENCE_COMPARISON_INPUT_SCHEMA_VERSION: &str =
    "VisualReferenceComparisonInput@2";
pub const VISUAL_REFERENCE_COMPARISON_REPORT_SCHEMA_VERSION: &str =
    "VisualReferenceComparisonReport@2";
pub const VISUAL_REFERENCE_ACCEPTANCE_POLICY_SCHEMA_VERSION: &str =
    "VisualReferenceAcceptancePolicy@1";

const MAX_REFERENCE_INPUTS: usize = 12;
const MAX_VISUAL_CLAIMS: usize = 256;
const MAX_MULTIMODAL_INSTRUCTION_CHARS: usize = 200_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceRole {
    PrimarySilhouette,
    Structure,
    Material,
    Surface,
    LocalDetail,
    Style,
    Multiview,
    ExistingAsset,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualClaimStatus {
    Observed,
    Inferred,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualClaimTarget {
    Geometry,
    Assembly,
    Material,
    Surface,
    Style,
    EvaluationOnly,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualClaimDispositionKind {
    Bound,
    Unresolved,
    EvaluationOnly,
}

/// Coordinates use integer per-mille units so evidence hashes are stable and
/// no floating-point or image bytes enter the semantic graph.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NormalizedEvidenceRegion {
    pub left: u16,
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
}

impl NormalizedEvidenceRegion {
    fn validate(&self) -> CoreResult<()> {
        if self.left >= self.right
            || self.top >= self.bottom
            || self.right > 1_000
            || self.bottom > 1_000
        {
            return Err(invalid(
                "MULTIMODAL_REGION_INVALID",
                "Visual evidence regions must be ordered integer per-mille coordinates.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MultimodalReferenceInput {
    pub evidence_id: String,
    pub evidence_sha256: String,
    pub role: ReferenceRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<NormalizedEvidenceRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MultimodalSelectionScope {
    #[serde(default)]
    pub part_ids: Vec<String>,
    #[serde(default)]
    pub material_zone_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_region: Option<NormalizedEvidenceRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MultimodalDesignLocks {
    pub preserve_geometry: bool,
    pub preserve_material_surface: bool,
    #[serde(default)]
    pub locked_part_ids: Vec<String>,
    #[serde(default)]
    pub locked_material_zone_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MultimodalDesignRequest {
    pub schema_version: String,
    pub request_id: String,
    pub project_id: String,
    pub turn_id: String,
    pub domain_pack_id: String,
    pub instruction: String,
    #[serde(default)]
    pub reference_inputs: Vec<MultimodalReferenceInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_asset_version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<MultimodalSelectionScope>,
    pub locks: MultimodalDesignLocks,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisionEvidenceProviderProvenance {
    pub provider_id: String,
    pub model_id: String,
    pub provider_response_sha256: String,
    pub analyzed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualEvidenceClaim {
    pub claim_id: String,
    pub level: VisualDetailLevel,
    pub status: VisualClaimStatus,
    pub target: VisualClaimTarget,
    pub description: String,
    pub critical: bool,
    /// Confidence in basis points. Unknown claims must be zero.
    pub confidence_bps: u16,
    #[serde(default)]
    pub source_evidence_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_view_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_region: Option<NormalizedEvidenceRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualEvidenceGraph {
    pub schema_version: String,
    pub graph_id: String,
    pub request_id: String,
    pub request_sha256: String,
    pub project_id: String,
    pub domain_pack_id: String,
    pub provider: VisionEvidenceProviderProvenance,
    pub claims: Vec<VisualEvidenceClaim>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualClaimDisposition {
    pub claim_id: String,
    pub disposition: VisualClaimDispositionKind,
    #[serde(default)]
    pub detail_ids: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MultimodalProgramEvidenceBinding {
    pub schema_version: String,
    pub binding_id: String,
    pub request_sha256: String,
    pub evidence_graph_sha256: String,
    pub source_program_sha256: String,
    pub project_id: String,
    pub domain_pack_id: String,
    pub program_id: String,
    pub dispositions: Vec<VisualClaimDisposition>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualReferenceMatchOutcome {
    Matched,
    Partial,
    Contradicted,
    NotVisible,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualReferenceSourceFingerprint {
    pub evidence_id: String,
    pub evidence_sha256: String,
}

/// Rust-owned scoring policy sealed into one comparison input. Provider
/// output can supply bounded observations, but cannot select or weaken these
/// thresholds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualReferenceAcceptancePolicy {
    pub schema_version: String,
    pub policy_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_contract_sha256: Option<String>,
    pub critical_minimum_bps: u16,
    pub macro_minimum_bps: u16,
    pub meso_minimum_bps: u16,
    pub micro_minimum_bps: u16,
    pub critical_requires_matched: bool,
    pub critical_not_visible_allowed: bool,
}

impl VisualReferenceAcceptancePolicy {
    pub fn default_policy() -> Self {
        Self {
            schema_version: VISUAL_REFERENCE_ACCEPTANCE_POLICY_SCHEMA_VERSION.into(),
            policy_id: "visual_reference_default_v1".into(),
            source_contract_sha256: None,
            critical_minimum_bps: 6_500,
            macro_minimum_bps: 6_500,
            meso_minimum_bps: 5_500,
            micro_minimum_bps: 4_500,
            critical_requires_matched: true,
            critical_not_visible_allowed: false,
        }
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != VISUAL_REFERENCE_ACCEPTANCE_POLICY_SCHEMA_VERSION {
            return Err(invalid(
                "VISUAL_REFERENCE_ACCEPTANCE_POLICY_INVALID",
                "Reference acceptance policy uses an unsupported schema version.",
            ));
        }
        require_safe_token("acceptance_policy.policy_id", &self.policy_id, 160)?;
        if let Some(source_contract_sha256) = self.source_contract_sha256.as_deref() {
            require_sha256(
                "acceptance_policy.source_contract_sha256",
                source_contract_sha256,
            )?;
        }
        if [
            self.critical_minimum_bps,
            self.macro_minimum_bps,
            self.meso_minimum_bps,
            self.micro_minimum_bps,
        ]
        .into_iter()
        .any(|minimum| minimum > 10_000)
        {
            return Err(invalid(
                "VISUAL_REFERENCE_ACCEPTANCE_POLICY_INVALID",
                "Reference acceptance thresholds must use bounded basis points.",
            ));
        }
        Ok(())
    }

    fn minimum_for_level(&self, level: VisualDetailLevel) -> u16 {
        match level {
            VisualDetailLevel::Macro => self.macro_minimum_bps,
            VisualDetailLevel::Meso => self.meso_minimum_bps,
            VisualDetailLevel::Micro => self.micro_minimum_bps,
        }
    }
}

/// Hash-only comparison envelope created by Rust after the candidate GLB has
/// been rendered. Provider image bytes remain transport-only and never enter
/// this durable identity contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualReferenceComparisonInput {
    pub schema_version: String,
    pub request_sha256: String,
    pub evidence_graph_sha256: String,
    pub program_binding_sha256: String,
    pub source_program_sha256: String,
    pub glb_sha256: String,
    pub acceptance_policy: VisualReferenceAcceptancePolicy,
    pub reference_sources: Vec<VisualReferenceSourceFingerprint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_view_profile: Option<VisualReferenceCandidateViewProfile>,
    pub candidate_views: Vec<VisualFixedViewEvidence>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualReferenceCandidateViewProfile {
    ConvergenceEight,
    TurntableEight,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualReferenceClaimAssessment {
    pub claim_id: String,
    pub outcome: VisualReferenceMatchOutcome,
    pub similarity_bps: u16,
    pub confidence_bps: u16,
    pub source_evidence_ids: Vec<String>,
    pub candidate_view_ids: Vec<String>,
    pub reason: String,
}

/// The Provider supplies claim assessments; Rust derives the level scores,
/// repair targets, failure codes and pass/fail decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualReferenceComparisonReport {
    pub schema_version: String,
    pub report_sha256: String,
    pub comparison_input_sha256: String,
    pub provider: VisionEvidenceProviderProvenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_evidence: Option<crate::VisualReferenceComparisonBudgetEvidence>,
    pub assessments: Vec<VisualReferenceClaimAssessment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macro_similarity_bps: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meso_similarity_bps: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub micro_similarity_bps: Option<u16>,
    pub passed: bool,
    pub failure_codes: Vec<String>,
    pub repair_claim_ids: Vec<String>,
}

impl MultimodalDesignRequest {
    pub fn validate_with_evidence(&self, evidence: &[ReferenceEvidence]) -> CoreResult<()> {
        if self.schema_version != MULTIMODAL_DESIGN_REQUEST_SCHEMA_VERSION {
            return Err(invalid(
                "MULTIMODAL_REQUEST_SCHEMA_INVALID",
                "Multimodal requests must use MultimodalDesignRequest@1.",
            ));
        }
        require_id("request_id", &self.request_id, Some("mmreq_"))?;
        require_project_id(&self.project_id)?;
        require_id("turn_id", &self.turn_id, Some("turn_"))?;
        require_id("domain_pack_id", &self.domain_pack_id, Some("pack_"))?;
        require_safe_text(
            "instruction",
            &self.instruction,
            1,
            MAX_MULTIMODAL_INSTRUCTION_CHARS,
        )?;
        if self.reference_inputs.len() > MAX_REFERENCE_INPUTS {
            return Err(invalid(
                "MULTIMODAL_REFERENCE_LIMIT_EXCEEDED",
                "A multimodal request may use at most twelve sealed references.",
            ));
        }
        if let Some(asset_version_id) = self.active_asset_version_id.as_deref() {
            require_id(
                "active_asset_version_id",
                asset_version_id,
                Some("assetver_"),
            )?;
        }
        self.locks
            .validate(self.active_asset_version_id.is_some())?;
        if let Some(selection) = &self.selection {
            selection.validate(self.active_asset_version_id.is_some())?;
        }

        let evidence_by_id = evidence
            .iter()
            .map(|item| (item.evidence_id.as_str(), item))
            .collect::<BTreeMap<_, _>>();
        let mut input_ids = BTreeSet::new();
        let mut roles = BTreeSet::new();
        for input in &self.reference_inputs {
            require_id(
                "reference_inputs.evidence_id",
                &input.evidence_id,
                Some("refevid_"),
            )?;
            require_sha256("reference_inputs.evidence_sha256", &input.evidence_sha256)?;
            if !input_ids.insert(input.evidence_id.as_str()) {
                return Err(invalid(
                    "MULTIMODAL_REFERENCE_DUPLICATE",
                    "Each sealed reference may appear only once in a multimodal request.",
                ));
            }
            roles.insert(input.role);
            if let Some(view_id) = input.view_id.as_deref() {
                require_safe_token("reference_inputs.view_id", view_id, 64)?;
            }
            if let Some(region) = input.region {
                region.validate()?;
            }
            let sealed = evidence_by_id
                .get(input.evidence_id.as_str())
                .ok_or_else(|| {
                    invalid(
                        "MULTIMODAL_REFERENCE_NOT_FOUND",
                        "Every multimodal reference must resolve to sealed ReferenceEvidence.",
                    )
                })?;
            sealed.validate()?;
            if sealed.project_id != self.project_id || sealed.domain_pack_id != self.domain_pack_id
            {
                return Err(invalid(
                    "MULTIMODAL_REFERENCE_SCOPE_MISMATCH",
                    "Multimodal references must belong to the same Project and Domain Pack.",
                ));
            }
            if semantic_sha256(*sealed)? != input.evidence_sha256 {
                return Err(invalid(
                    "MULTIMODAL_REFERENCE_HASH_MISMATCH",
                    "A multimodal reference hash does not match its sealed evidence record.",
                ));
            }
        }
        if roles.contains(&ReferenceRole::Multiview) && self.reference_inputs.len() < 2 {
            return Err(invalid(
                "MULTIMODAL_MULTIVIEW_INSUFFICIENT",
                "Multiview role requires at least two independently sealed references.",
            ));
        }
        Ok(())
    }
}

impl MultimodalSelectionScope {
    fn validate(&self, has_active_asset: bool) -> CoreResult<()> {
        if !has_active_asset {
            return Err(invalid(
                "MULTIMODAL_SELECTION_REQUIRES_ACTIVE_ASSET",
                "Part, material-zone or local-region selection requires an active asset version.",
            ));
        }
        validate_unique_ids("selection.part_ids", &self.part_ids, "part_", 64)?;
        validate_unique_ids(
            "selection.material_zone_ids",
            &self.material_zone_ids,
            "zone_",
            64,
        )?;
        if self.part_ids.is_empty()
            && self.material_zone_ids.is_empty()
            && self.reference_region.is_none()
        {
            return Err(invalid(
                "MULTIMODAL_SELECTION_EMPTY",
                "A selection scope must select a part, material zone or normalized region.",
            ));
        }
        if let Some(region) = self.reference_region {
            region.validate()?;
        }
        Ok(())
    }
}

impl MultimodalDesignLocks {
    fn validate(&self, has_active_asset: bool) -> CoreResult<()> {
        validate_unique_ids("locks.locked_part_ids", &self.locked_part_ids, "part_", 256)?;
        validate_unique_ids(
            "locks.locked_material_zone_ids",
            &self.locked_material_zone_ids,
            "zone_",
            256,
        )?;
        if !has_active_asset
            && (self.preserve_geometry
                || self.preserve_material_surface
                || !self.locked_part_ids.is_empty()
                || !self.locked_material_zone_ids.is_empty())
        {
            return Err(invalid(
                "MULTIMODAL_LOCK_REQUIRES_ACTIVE_ASSET",
                "Preservation locks require an active asset version.",
            ));
        }
        Ok(())
    }
}

impl VisualEvidenceGraph {
    pub fn validate_against(
        &self,
        request: &MultimodalDesignRequest,
        evidence: &[ReferenceEvidence],
    ) -> CoreResult<()> {
        request.validate_with_evidence(evidence)?;
        if self.schema_version != VISUAL_EVIDENCE_GRAPH_SCHEMA_VERSION
            || self.request_id != request.request_id
            || self.request_sha256 != semantic_sha256(request)?
            || self.project_id != request.project_id
            || self.domain_pack_id != request.domain_pack_id
        {
            return Err(invalid(
                "VISUAL_EVIDENCE_GRAPH_LINEAGE_INVALID",
                "Visual evidence graph lineage must match one exact validated request.",
            ));
        }
        require_id("graph_id", &self.graph_id, Some("vegraph_"))?;
        self.provider.validate()?;
        if self.claims.is_empty() || self.claims.len() > MAX_VISUAL_CLAIMS {
            return Err(invalid(
                "VISUAL_EVIDENCE_CLAIMS_INVALID",
                "Visual evidence graph must contain one to 256 bounded claims.",
            ));
        }

        let request_inputs = request
            .reference_inputs
            .iter()
            .map(|input| input.evidence_id.as_str())
            .collect::<BTreeSet<_>>();
        let evidence_by_id = evidence
            .iter()
            .map(|item| (item.evidence_id.as_str(), item))
            .collect::<BTreeMap<_, _>>();
        let mut claim_ids = BTreeSet::new();
        let mut levels = BTreeSet::new();
        for claim in &self.claims {
            require_id("claims.claim_id", &claim.claim_id, Some("vclaim_"))?;
            require_safe_text("claims.description", &claim.description, 1, 480)?;
            if !claim_ids.insert(claim.claim_id.as_str()) {
                return Err(invalid(
                    "VISUAL_EVIDENCE_CLAIM_DUPLICATE",
                    "Visual evidence claim identifiers must be unique.",
                ));
            }
            levels.insert(match claim.level {
                VisualDetailLevel::Macro => "macro",
                VisualDetailLevel::Meso => "meso",
                VisualDetailLevel::Micro => "micro",
            });
            if claim.confidence_bps > 10_000 {
                return Err(invalid(
                    "VISUAL_EVIDENCE_CONFIDENCE_INVALID",
                    "Visual evidence confidence must be between 0 and 10000 basis points.",
                ));
            }
            let mut sources = BTreeSet::new();
            for source_id in &claim.source_evidence_ids {
                require_id("claims.source_evidence_ids", source_id, Some("refevid_"))?;
                if !sources.insert(source_id.as_str())
                    || !request_inputs.contains(source_id.as_str())
                {
                    return Err(invalid(
                        "VISUAL_EVIDENCE_SOURCE_INVALID",
                        "Claim sources must be unique references from the exact request.",
                    ));
                }
            }
            match claim.status {
                VisualClaimStatus::Observed
                    if claim.source_evidence_ids.is_empty() || claim.confidence_bps == 0 =>
                {
                    return Err(invalid(
                        "VISUAL_EVIDENCE_OBSERVATION_UNSUPPORTED",
                        "Observed claims require visible source evidence and positive confidence.",
                    ));
                }
                VisualClaimStatus::Inferred if claim.confidence_bps == 0 => {
                    return Err(invalid(
                        "VISUAL_EVIDENCE_INFERENCE_INVALID",
                        "Inferred claims require positive confidence.",
                    ));
                }
                VisualClaimStatus::Unknown
                    if claim.confidence_bps != 0 || !claim.source_evidence_ids.is_empty() =>
                {
                    return Err(invalid(
                        "VISUAL_EVIDENCE_UNKNOWN_INVALID",
                        "Unknown claims must have zero confidence and no claimed source evidence.",
                    ));
                }
                _ => {}
            }
            if let Some(view_id) = claim.source_view_id.as_deref() {
                require_safe_token("claims.source_view_id", view_id, 64)?;
                for source_id in &claim.source_evidence_ids {
                    let source = evidence_by_id
                        .get(source_id.as_str())
                        .expect("request validated");
                    if source
                        .missing_views
                        .iter()
                        .any(|missing| missing.eq_ignore_ascii_case(view_id))
                        && claim.status == VisualClaimStatus::Observed
                    {
                        return Err(invalid(
                            "VISUAL_EVIDENCE_MISSING_VIEW_OBSERVED",
                            "A declared missing view cannot support an observed claim.",
                        ));
                    }
                }
            }
            if let Some(region) = claim.source_region {
                region.validate()?;
                if claim.source_evidence_ids.is_empty() {
                    return Err(invalid(
                        "VISUAL_EVIDENCE_REGION_SOURCE_REQUIRED",
                        "A source region must name at least one sealed reference.",
                    ));
                }
            }
        }
        if levels.len() != 3 {
            return Err(invalid(
                "VISUAL_EVIDENCE_LEVEL_COVERAGE_INVALID",
                "Visual evidence graph must cover macro, meso and micro design evidence.",
            ));
        }
        Ok(())
    }
}

impl VisionEvidenceProviderProvenance {
    fn validate(&self) -> CoreResult<()> {
        require_safe_token("provider.provider_id", &self.provider_id, 120)?;
        require_safe_token("provider.model_id", &self.model_id, 160)?;
        require_sha256(
            "provider.provider_response_sha256",
            &self.provider_response_sha256,
        )?;
        require_safe_text("provider.analyzed_at", &self.analyzed_at, 1, 64)
    }
}

impl MultimodalProgramEvidenceBinding {
    pub fn validate_against(
        &self,
        request: &MultimodalDesignRequest,
        graph: &VisualEvidenceGraph,
        evidence: &[ReferenceEvidence],
        program: &ForgeVisualProgram,
    ) -> CoreResult<()> {
        graph.validate_against(request, evidence)?;
        program.validate()?;
        if self.schema_version != MULTIMODAL_PROGRAM_EVIDENCE_BINDING_SCHEMA_VERSION
            || self.request_sha256 != semantic_sha256(request)?
            || self.evidence_graph_sha256 != semantic_sha256(graph)?
            || self.source_program_sha256 != semantic_sha256(program)?
            || self.project_id != request.project_id
            || self.domain_pack_id != request.domain_pack_id
            || self.domain_pack_id != program.domain_pack_id
            || self.program_id != program.program_id
        {
            return Err(invalid(
                "MULTIMODAL_PROGRAM_BINDING_LINEAGE_INVALID",
                "Multimodal binding must match the exact request, evidence graph and visual program.",
            ));
        }
        require_id("binding_id", &self.binding_id, Some("mmbind_"))?;
        if self.dispositions.len() != graph.claims.len() {
            return Err(invalid(
                "MULTIMODAL_PROGRAM_DISPOSITION_INCOMPLETE",
                "Every visual evidence claim requires exactly one explicit disposition.",
            ));
        }

        let claims = graph
            .claims
            .iter()
            .map(|claim| (claim.claim_id.as_str(), claim))
            .collect::<BTreeMap<_, _>>();
        let details = program
            .detail_inventory
            .iter()
            .map(|detail| (detail.detail_id.as_str(), detail))
            .collect::<BTreeMap<_, _>>();
        let mut disposed = BTreeSet::new();
        for disposition in &self.dispositions {
            require_id(
                "dispositions.claim_id",
                &disposition.claim_id,
                Some("vclaim_"),
            )?;
            require_safe_text("dispositions.reason", &disposition.reason, 1, 320)?;
            if !disposed.insert(disposition.claim_id.as_str()) {
                return Err(invalid(
                    "MULTIMODAL_PROGRAM_DISPOSITION_DUPLICATE",
                    "Each visual evidence claim may be disposed only once.",
                ));
            }
            let claim = claims.get(disposition.claim_id.as_str()).ok_or_else(|| {
                invalid(
                    "MULTIMODAL_PROGRAM_CLAIM_UNKNOWN",
                    "A disposition references a claim outside the exact evidence graph.",
                )
            })?;
            let mut detail_ids = BTreeSet::new();
            for detail_id in &disposition.detail_ids {
                require_id("dispositions.detail_ids", detail_id, Some("detail_"))?;
                if !detail_ids.insert(detail_id.as_str()) {
                    return Err(invalid(
                        "MULTIMODAL_PROGRAM_DETAIL_DUPLICATE",
                        "Disposition detail identifiers must be unique.",
                    ));
                }
            }
            match disposition.disposition {
                VisualClaimDispositionKind::Bound => {
                    if claim.status == VisualClaimStatus::Unknown
                        || claim.target == VisualClaimTarget::EvaluationOnly
                        || disposition.detail_ids.is_empty()
                    {
                        return Err(invalid(
                            "MULTIMODAL_PROGRAM_BOUND_INVALID",
                            "Only observed or inferred design claims may bind real program details.",
                        ));
                    }
                    for detail_id in &disposition.detail_ids {
                        let detail = details.get(detail_id.as_str()).ok_or_else(|| {
                            invalid(
                                "MULTIMODAL_PROGRAM_DETAIL_UNKNOWN",
                                "A bound claim must reference a real visual-program detail.",
                            )
                        })?;
                        if detail.level != claim.level
                            || detail.status != VisualDetailStatus::Bound
                            || detail.bindings.is_empty()
                        {
                            return Err(invalid(
                                "MULTIMODAL_PROGRAM_DETAIL_UNBOUND",
                                "A bound claim must resolve to a same-level detail with real output bindings.",
                            ));
                        }
                    }
                }
                VisualClaimDispositionKind::Unresolved => {
                    if claim.target == VisualClaimTarget::EvaluationOnly
                        || disposition.detail_ids.is_empty()
                    {
                        return Err(invalid(
                            "MULTIMODAL_PROGRAM_UNRESOLVED_INVALID",
                            "Unresolved design claims must remain explicit in the visual-program inventory.",
                        ));
                    }
                    for detail_id in &disposition.detail_ids {
                        let detail = details.get(detail_id.as_str()).ok_or_else(|| {
                            invalid(
                                "MULTIMODAL_PROGRAM_DETAIL_UNKNOWN",
                                "An unresolved claim must reference a real visual-program detail.",
                            )
                        })?;
                        if detail.level != claim.level
                            || detail.status != VisualDetailStatus::Unresolved
                            || !detail.bindings.is_empty()
                        {
                            return Err(invalid(
                                "MULTIMODAL_PROGRAM_UNRESOLVED_INVALID",
                                "Unresolved evidence must map to a same-level unbound program detail.",
                            ));
                        }
                    }
                }
                VisualClaimDispositionKind::EvaluationOnly => {
                    if claim.target != VisualClaimTarget::EvaluationOnly
                        || !disposition.detail_ids.is_empty()
                    {
                        return Err(invalid(
                            "MULTIMODAL_PROGRAM_EVALUATION_INVALID",
                            "Evaluation-only claims cannot mutate or bind visual-program details.",
                        ));
                    }
                }
            }
        }
        if disposed.len() != claims.len() {
            return Err(invalid(
                "MULTIMODAL_PROGRAM_DISPOSITION_INCOMPLETE",
                "Every visual evidence claim requires exactly one explicit disposition.",
            ));
        }
        Ok(())
    }
}

impl VisualReferenceComparisonInput {
    pub fn build(
        request: &MultimodalDesignRequest,
        graph: &VisualEvidenceGraph,
        binding: &MultimodalProgramEvidenceBinding,
        evidence: &[ReferenceEvidence],
        program: &ForgeVisualProgram,
        glb_sha256: &str,
        candidate_views: &[VisualFixedViewEvidence],
    ) -> CoreResult<Self> {
        Self::build_with_policy(
            request,
            graph,
            binding,
            evidence,
            program,
            glb_sha256,
            candidate_views,
            VisualReferenceAcceptancePolicy::default_policy(),
        )
    }

    pub fn build_with_policy(
        request: &MultimodalDesignRequest,
        graph: &VisualEvidenceGraph,
        binding: &MultimodalProgramEvidenceBinding,
        evidence: &[ReferenceEvidence],
        program: &ForgeVisualProgram,
        glb_sha256: &str,
        candidate_views: &[VisualFixedViewEvidence],
        acceptance_policy: VisualReferenceAcceptancePolicy,
    ) -> CoreResult<Self> {
        binding.validate_against(request, graph, evidence, program)?;
        require_sha256("comparison.glb_sha256", glb_sha256)?;
        acceptance_policy.validate()?;
        let mut reference_sources = request
            .reference_inputs
            .iter()
            .map(|source| VisualReferenceSourceFingerprint {
                evidence_id: source.evidence_id.clone(),
                evidence_sha256: source.evidence_sha256.clone(),
            })
            .collect::<Vec<_>>();
        reference_sources.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
        let mut candidate_views = candidate_views.to_vec();
        candidate_views.sort_by(|left, right| left.view_id.cmp(&right.view_id));
        let input = Self {
            schema_version: VISUAL_REFERENCE_COMPARISON_INPUT_SCHEMA_VERSION.into(),
            request_sha256: semantic_sha256(request)?,
            evidence_graph_sha256: semantic_sha256(graph)?,
            program_binding_sha256: semantic_sha256(binding)?,
            source_program_sha256: semantic_sha256(program)?,
            glb_sha256: glb_sha256.into(),
            acceptance_policy,
            reference_sources,
            candidate_view_profile: None,
            candidate_views,
        };
        input.validate_against(request, graph, binding, evidence, program)?;
        Ok(input)
    }

    /// Build the same sealed-reference comparison envelope for the E005
    /// unified author source without fabricating a legacy
    /// `ForgeVisualProgram@1` or multimodal program binding.
    pub fn build_for_e005_source(
        request: &MultimodalDesignRequest,
        graph: &VisualEvidenceGraph,
        evidence: &[ReferenceEvidence],
        source: &Value,
        glb_sha256: &str,
        candidate_views: &[VisualFixedViewEvidence],
        acceptance_policy: VisualReferenceAcceptancePolicy,
    ) -> CoreResult<Self> {
        graph.validate_against(request, evidence)?;
        let lowering = crate::lower_forge_visual_author_source_v1(source)?;
        require_sha256("comparison.glb_sha256", glb_sha256)?;
        acceptance_policy.validate()?;
        let mut reference_sources = request
            .reference_inputs
            .iter()
            .map(|source| VisualReferenceSourceFingerprint {
                evidence_id: source.evidence_id.clone(),
                evidence_sha256: source.evidence_sha256.clone(),
            })
            .collect::<Vec<_>>();
        reference_sources.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
        let mut candidate_views = candidate_views.to_vec();
        candidate_views.sort_by(|left, right| left.view_id.cmp(&right.view_id));
        let binding = json!({
            "schema_version":"E005VisualComparisonBinding@1",
            "request_sha256":semantic_sha256(request)?,
            "evidence_graph_sha256":semantic_sha256(graph)?,
            "source_program_sha256":lowering.source_program_sha256,
            "assembly_graph_sha256":lowering.assembly_graph_sha256,
            "surface_plan_sha256":lowering.surface_plan_sha256,
            "lineage_sha256":lowering.lineage_sha256,
        });
        let input = Self {
            schema_version: VISUAL_REFERENCE_COMPARISON_INPUT_SCHEMA_VERSION.into(),
            request_sha256: semantic_sha256(request)?,
            evidence_graph_sha256: semantic_sha256(graph)?,
            program_binding_sha256: semantic_sha256(&binding)?,
            source_program_sha256: lowering.source_program_sha256,
            glb_sha256: glb_sha256.into(),
            acceptance_policy,
            reference_sources,
            candidate_view_profile: Some(VisualReferenceCandidateViewProfile::TurntableEight),
            candidate_views,
        };
        input.validate_for_e005_source(request, graph, evidence, source)?;
        Ok(input)
    }

    pub fn validate_for_e005_source(
        &self,
        request: &MultimodalDesignRequest,
        graph: &VisualEvidenceGraph,
        evidence: &[ReferenceEvidence],
        source: &Value,
    ) -> CoreResult<()> {
        graph.validate_against(request, evidence)?;
        let lowering = crate::lower_forge_visual_author_source_v1(source)?;
        let binding = json!({
            "schema_version":"E005VisualComparisonBinding@1",
            "request_sha256":semantic_sha256(request)?,
            "evidence_graph_sha256":semantic_sha256(graph)?,
            "source_program_sha256":lowering.source_program_sha256,
            "assembly_graph_sha256":lowering.assembly_graph_sha256,
            "surface_plan_sha256":lowering.surface_plan_sha256,
            "lineage_sha256":lowering.lineage_sha256,
        });
        if self.schema_version != VISUAL_REFERENCE_COMPARISON_INPUT_SCHEMA_VERSION
            || self.request_sha256 != semantic_sha256(request)?
            || self.evidence_graph_sha256 != semantic_sha256(graph)?
            || self.program_binding_sha256 != semantic_sha256(&binding)?
            || self.source_program_sha256 != lowering.source_program_sha256
            || self.candidate_view_profile
                != Some(VisualReferenceCandidateViewProfile::TurntableEight)
        {
            return Err(invalid(
                "E005_R2_COMPARISON_LINEAGE_INVALID",
                "E005 comparison must bind the exact request, graph, unified source and compiled artifacts",
            ));
        }
        self.validate_reference_and_candidate_payload(request)
    }

    fn validate_reference_and_candidate_payload(
        &self,
        request: &MultimodalDesignRequest,
    ) -> CoreResult<()> {
        require_sha256("comparison.glb_sha256", &self.glb_sha256)?;
        self.acceptance_policy.validate()?;
        let expected_sources = request
            .reference_inputs
            .iter()
            .map(|source| (source.evidence_id.as_str(), source.evidence_sha256.as_str()))
            .collect::<BTreeMap<_, _>>();
        let mut actual_sources = BTreeMap::new();
        for source in &self.reference_sources {
            require_id(
                "comparison.reference_sources.evidence_id",
                &source.evidence_id,
                Some("refevid_"),
            )?;
            require_sha256(
                "comparison.reference_sources.evidence_sha256",
                &source.evidence_sha256,
            )?;
            if actual_sources
                .insert(source.evidence_id.as_str(), source.evidence_sha256.as_str())
                .is_some()
            {
                return Err(invalid(
                    "VISUAL_REFERENCE_COMPARISON_SOURCE_INVALID",
                    "Reference comparison sources must be unique.",
                ));
            }
        }
        if actual_sources != expected_sources {
            return Err(invalid(
                "VISUAL_REFERENCE_COMPARISON_SOURCE_INVALID",
                "Reference comparison sources must match the exact sealed request inputs.",
            ));
        }
        let required_views = [
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
        .collect::<BTreeSet<_>>();
        let mut view_ids = BTreeSet::new();
        let mut renderer_ids = BTreeSet::new();
        for view in &self.candidate_views {
            require_safe_token("comparison.candidate_views.view_id", &view.view_id, 64)?;
            require_safe_renderer_token(
                "comparison.candidate_views.renderer_id",
                &view.renderer_id,
                120,
            )?;
            require_sha256(
                "comparison.candidate_views.image_sha256",
                &view.image_sha256,
            )?;
            require_sha256("comparison.candidate_views.glb_sha256", &view.glb_sha256)?;
            if !view_ids.insert(view.view_id.as_str())
                || view.glb_sha256 != self.glb_sha256
                || !view.readback_passed
            {
                return Err(invalid(
                    "VISUAL_REFERENCE_COMPARISON_VIEW_INVALID",
                    "Every candidate view must be unique, read back and belong to the exact candidate GLB.",
                ));
            }
            renderer_ids.insert(view.renderer_id.as_str());
        }
        if view_ids != required_views || renderer_ids.len() != 1 {
            return Err(invalid(
                "VISUAL_REFERENCE_COMPARISON_VIEW_INVALID",
                "E005 reference comparison requires the exact generic turntable eight-view set from one renderer.",
            ));
        }
        Ok(())
    }

    pub fn validate_against(
        &self,
        request: &MultimodalDesignRequest,
        graph: &VisualEvidenceGraph,
        binding: &MultimodalProgramEvidenceBinding,
        evidence: &[ReferenceEvidence],
        program: &ForgeVisualProgram,
    ) -> CoreResult<()> {
        binding.validate_against(request, graph, evidence, program)?;
        if self.schema_version != VISUAL_REFERENCE_COMPARISON_INPUT_SCHEMA_VERSION
            || self.request_sha256 != semantic_sha256(request)?
            || self.evidence_graph_sha256 != semantic_sha256(graph)?
            || self.program_binding_sha256 != semantic_sha256(binding)?
            || self.source_program_sha256 != semantic_sha256(program)?
        {
            return Err(invalid(
                "VISUAL_REFERENCE_COMPARISON_LINEAGE_INVALID",
                "Reference comparison must bind the exact request, evidence graph, program binding and program.",
            ));
        }
        require_sha256("comparison.glb_sha256", &self.glb_sha256)?;
        self.acceptance_policy.validate()?;

        let expected_sources = request
            .reference_inputs
            .iter()
            .map(|source| (source.evidence_id.as_str(), source.evidence_sha256.as_str()))
            .collect::<BTreeMap<_, _>>();
        let mut actual_sources = BTreeMap::new();
        for source in &self.reference_sources {
            require_id(
                "comparison.reference_sources.evidence_id",
                &source.evidence_id,
                Some("refevid_"),
            )?;
            require_sha256(
                "comparison.reference_sources.evidence_sha256",
                &source.evidence_sha256,
            )?;
            if actual_sources
                .insert(source.evidence_id.as_str(), source.evidence_sha256.as_str())
                .is_some()
            {
                return Err(invalid(
                    "VISUAL_REFERENCE_COMPARISON_SOURCE_INVALID",
                    "Reference comparison sources must be unique.",
                ));
            }
        }
        if actual_sources != expected_sources {
            return Err(invalid(
                "VISUAL_REFERENCE_COMPARISON_SOURCE_INVALID",
                "Reference comparison sources must match the exact sealed request inputs.",
            ));
        }

        let required_views = match self.candidate_view_profile {
            None | Some(VisualReferenceCandidateViewProfile::ConvergenceEight) => {
                REQUIRED_VISUAL_VIEW_IDS
                    .into_iter()
                    .collect::<BTreeSet<_>>()
            }
            Some(VisualReferenceCandidateViewProfile::TurntableEight) => [
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
            .collect::<BTreeSet<_>>(),
        };
        let mut view_ids = BTreeSet::new();
        let mut renderer_ids = BTreeSet::new();
        for view in &self.candidate_views {
            require_safe_token("comparison.candidate_views.view_id", &view.view_id, 64)?;
            require_safe_renderer_token(
                "comparison.candidate_views.renderer_id",
                &view.renderer_id,
                120,
            )?;
            require_sha256(
                "comparison.candidate_views.image_sha256",
                &view.image_sha256,
            )?;
            require_sha256("comparison.candidate_views.glb_sha256", &view.glb_sha256)?;
            if !view_ids.insert(view.view_id.as_str())
                || view.glb_sha256 != self.glb_sha256
                || !view.readback_passed
            {
                return Err(invalid(
                    "VISUAL_REFERENCE_COMPARISON_VIEW_INVALID",
                    "Every candidate view must be unique, read back and belong to the exact candidate GLB.",
                ));
            }
            renderer_ids.insert(view.renderer_id.as_str());
        }
        if view_ids != required_views || renderer_ids.len() != 1 {
            return Err(invalid(
                "VISUAL_REFERENCE_COMPARISON_VIEW_INVALID",
                "Reference comparison requires the exact eight-view set from one renderer.",
            ));
        }
        Ok(())
    }
}

impl VisualReferenceComparisonReport {
    pub fn build(
        input: &VisualReferenceComparisonInput,
        graph: &VisualEvidenceGraph,
        provider: VisionEvidenceProviderProvenance,
        assessments: Vec<VisualReferenceClaimAssessment>,
    ) -> CoreResult<Self> {
        Self::build_with_budget(input, graph, provider, None, assessments)
    }

    pub fn build_with_budget(
        input: &VisualReferenceComparisonInput,
        graph: &VisualEvidenceGraph,
        provider: VisionEvidenceProviderProvenance,
        budget_evidence: Option<crate::VisualReferenceComparisonBudgetEvidence>,
        mut assessments: Vec<VisualReferenceClaimAssessment>,
    ) -> CoreResult<Self> {
        provider.validate()?;
        if let Some(evidence) = &budget_evidence {
            evidence.validate_against(input)?;
        }
        if input.evidence_graph_sha256 != semantic_sha256(graph)? {
            return Err(invalid(
                "VISUAL_REFERENCE_COMPARISON_LINEAGE_INVALID",
                "Comparison report evidence graph does not match its Rust-owned input.",
            ));
        }
        assessments.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
        let (macro_score, meso_score, micro_score, failure_codes, repair_claim_ids) =
            validate_and_score_assessments(input, graph, &assessments)?;
        let mut report = Self {
            schema_version: VISUAL_REFERENCE_COMPARISON_REPORT_SCHEMA_VERSION.into(),
            report_sha256: String::new(),
            comparison_input_sha256: semantic_sha256(input)?,
            provider,
            budget_evidence,
            assessments,
            macro_similarity_bps: macro_score,
            meso_similarity_bps: meso_score,
            micro_similarity_bps: micro_score,
            passed: failure_codes.is_empty(),
            failure_codes,
            repair_claim_ids,
        };
        report.report_sha256 = semantic_sha256(&report)?;
        Ok(report)
    }

    pub fn validate_against(
        &self,
        input: &VisualReferenceComparisonInput,
        graph: &VisualEvidenceGraph,
    ) -> CoreResult<()> {
        if self.schema_version != VISUAL_REFERENCE_COMPARISON_REPORT_SCHEMA_VERSION
            || self.comparison_input_sha256 != semantic_sha256(input)?
        {
            return Err(invalid(
                "VISUAL_REFERENCE_COMPARISON_REPORT_LINEAGE_INVALID",
                "Comparison report must match one exact Rust-owned input.",
            ));
        }
        self.provider.validate()?;
        if let Some(evidence) = &self.budget_evidence {
            evidence.validate_against(input)?;
        }
        let (macro_score, meso_score, micro_score, failure_codes, repair_claim_ids) =
            validate_and_score_assessments(input, graph, &self.assessments)?;
        if self.macro_similarity_bps != macro_score
            || self.meso_similarity_bps != meso_score
            || self.micro_similarity_bps != micro_score
            || self.failure_codes != failure_codes
            || self.repair_claim_ids != repair_claim_ids
            || self.passed != self.failure_codes.is_empty()
        {
            return Err(invalid(
                "VISUAL_REFERENCE_COMPARISON_REPORT_DERIVATION_INVALID",
                "Comparison pass/fail, scores and repair targets must be derived by Rust.",
            ));
        }
        let mut unhashed = self.clone();
        unhashed.report_sha256.clear();
        if self.report_sha256 != semantic_sha256(&unhashed)? {
            return Err(invalid(
                "VISUAL_REFERENCE_COMPARISON_REPORT_HASH_INVALID",
                "Comparison report hash does not match its semantic content.",
            ));
        }
        Ok(())
    }
}

fn validate_and_score_assessments(
    input: &VisualReferenceComparisonInput,
    graph: &VisualEvidenceGraph,
    assessments: &[VisualReferenceClaimAssessment],
) -> CoreResult<(
    Option<u16>,
    Option<u16>,
    Option<u16>,
    Vec<String>,
    Vec<String>,
)> {
    let comparable_claims = graph
        .claims
        .iter()
        .filter(|claim| {
            claim.status != VisualClaimStatus::Unknown && !claim.source_evidence_ids.is_empty()
        })
        .map(|claim| (claim.claim_id.as_str(), claim))
        .collect::<BTreeMap<_, _>>();
    let known_views = input
        .candidate_views
        .iter()
        .map(|view| view.view_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut actual_claim_ids = BTreeSet::new();
    let mut macro_scores = Vec::new();
    let mut meso_scores = Vec::new();
    let mut micro_scores = Vec::new();
    let mut failures = Vec::new();
    let mut repairs = Vec::new();

    for assessment in assessments {
        require_id(
            "comparison.assessments.claim_id",
            &assessment.claim_id,
            Some("vclaim_"),
        )?;
        require_safe_text("comparison.assessments.reason", &assessment.reason, 1, 320)?;
        if assessment.similarity_bps > 10_000
            || assessment.confidence_bps == 0
            || assessment.confidence_bps > 10_000
            || !actual_claim_ids.insert(assessment.claim_id.as_str())
        {
            return Err(invalid(
                "VISUAL_REFERENCE_COMPARISON_ASSESSMENT_INVALID",
                "Comparison assessments must be unique and use bounded similarity/confidence scores.",
            ));
        }
        let claim = comparable_claims
            .get(assessment.claim_id.as_str())
            .ok_or_else(|| {
                invalid(
                    "VISUAL_REFERENCE_COMPARISON_CLAIM_INVALID",
                    "Comparison assessment references an unknown or non-comparable claim.",
                )
            })?;
        let mut expected_sources = claim.source_evidence_ids.clone();
        expected_sources.sort();
        let mut actual_sources = assessment.source_evidence_ids.clone();
        actual_sources.sort();
        actual_sources.dedup();
        if actual_sources != expected_sources {
            return Err(invalid(
                "VISUAL_REFERENCE_COMPARISON_SOURCE_INVALID",
                "Each assessment must bind the exact source evidence used by its claim.",
            ));
        }
        let candidate_views = assessment
            .candidate_view_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if candidate_views.is_empty()
            || candidate_views.len() != assessment.candidate_view_ids.len()
            || !candidate_views.is_subset(&known_views)
        {
            return Err(invalid(
                "VISUAL_REFERENCE_COMPARISON_VIEW_INVALID",
                "Each assessment must name one or more unique views from the exact candidate set.",
            ));
        }
        let coherent = match assessment.outcome {
            VisualReferenceMatchOutcome::Matched => assessment.similarity_bps >= 7_000,
            VisualReferenceMatchOutcome::Partial => {
                (3_000..7_000).contains(&assessment.similarity_bps)
            }
            VisualReferenceMatchOutcome::Contradicted => assessment.similarity_bps < 3_000,
            VisualReferenceMatchOutcome::NotVisible => assessment.similarity_bps == 0,
        };
        if !coherent {
            return Err(invalid(
                "VISUAL_REFERENCE_COMPARISON_OUTCOME_INVALID",
                "Comparison outcome must agree with its bounded similarity score.",
            ));
        }
        match claim.level {
            VisualDetailLevel::Macro => macro_scores.push(assessment.similarity_bps),
            VisualDetailLevel::Meso => meso_scores.push(assessment.similarity_bps),
            VisualDetailLevel::Micro => micro_scores.push(assessment.similarity_bps),
        }
        let critical_minimum = input
            .acceptance_policy
            .critical_minimum_bps
            .max(input.acceptance_policy.minimum_for_level(claim.level));
        let critical_outcome_failed = (input.acceptance_policy.critical_requires_matched
            && assessment.outcome != VisualReferenceMatchOutcome::Matched)
            || (!input.acceptance_policy.critical_not_visible_allowed
                && assessment.outcome == VisualReferenceMatchOutcome::NotVisible);
        if claim.critical
            && (critical_outcome_failed || assessment.similarity_bps < critical_minimum)
        {
            failures.push("CRITICAL_REFERENCE_CLAIM_MISMATCH".into());
            repairs.push(claim.claim_id.clone());
        } else if assessment.outcome != VisualReferenceMatchOutcome::Matched {
            repairs.push(claim.claim_id.clone());
        }
    }
    if actual_claim_ids != comparable_claims.keys().copied().collect::<BTreeSet<_>>() {
        return Err(invalid(
            "VISUAL_REFERENCE_COMPARISON_COVERAGE_INVALID",
            "Every observed or evidence-backed inferred claim requires exactly one assessment.",
        ));
    }
    if graph.claims.iter().any(|claim| {
        claim.critical
            && claim.status != VisualClaimStatus::Unknown
            && claim.source_evidence_ids.is_empty()
    }) {
        failures.push("CRITICAL_REFERENCE_CLAIM_NOT_COMPARABLE".into());
    }

    let macro_score = average_bps(&macro_scores);
    let meso_score = average_bps(&meso_scores);
    let micro_score = average_bps(&micro_scores);
    if macro_score.is_none_or(|score| score < input.acceptance_policy.macro_minimum_bps) {
        failures.push("REFERENCE_MACRO_MISMATCH".into());
    }
    if meso_score.is_none_or(|score| score < input.acceptance_policy.meso_minimum_bps) {
        failures.push("REFERENCE_MESO_MISMATCH".into());
    }
    if micro_score.is_some_and(|score| score < input.acceptance_policy.micro_minimum_bps) {
        failures.push("REFERENCE_MICRO_MISMATCH".into());
    }
    failures.sort();
    failures.dedup();
    repairs.sort();
    repairs.dedup();
    Ok((macro_score, meso_score, micro_score, failures, repairs))
}

fn average_bps(values: &[u16]) -> Option<u16> {
    (!values.is_empty()).then(|| {
        let total = values.iter().map(|value| u64::from(*value)).sum::<u64>();
        u16::try_from(total / values.len() as u64).expect("basis-point average remains bounded")
    })
}

fn validate_unique_ids(field: &str, values: &[String], prefix: &str, max: usize) -> CoreResult<()> {
    if values.len() > max {
        return Err(invalid(
            "MULTIMODAL_ID_LIST_INVALID",
            format!("{field} exceeds the reviewed limit."),
        ));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        require_id(field, value, Some(prefix))?;
        if !unique.insert(value.as_str()) {
            return Err(invalid(
                "MULTIMODAL_ID_LIST_INVALID",
                format!("{field} contains a duplicate identifier."),
            ));
        }
    }
    Ok(())
}

fn require_project_id(value: &str) -> CoreResult<()> {
    if value.starts_with("prj_") {
        require_id("project_id", value, Some("prj_"))
    } else {
        require_id("project_id", value, Some("project_"))
    }
}

fn require_id(field: &str, value: &str, prefix: Option<&str>) -> CoreResult<()> {
    if value.is_empty()
        || value.len() > 160
        || prefix.is_some_and(|prefix| !value.starts_with(prefix))
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
    {
        return Err(invalid(
            "MULTIMODAL_ID_INVALID",
            format!("{field} is outside the reviewed identifier contract."),
        ));
    }
    Ok(())
}

fn require_safe_token(field: &str, value: &str, max: usize) -> CoreResult<()> {
    require_safe_text(field, value, 1, max)?;
    if !value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'/')
    }) {
        return Err(invalid(
            "MULTIMODAL_TOKEN_INVALID",
            format!("{field} contains unsupported characters."),
        ));
    }
    Ok(())
}

fn require_safe_renderer_token(field: &str, value: &str, max: usize) -> CoreResult<()> {
    require_safe_text(field, value, 1, max)?;
    if !value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'/' | b'@')
    }) {
        return Err(invalid(
            "MULTIMODAL_TOKEN_INVALID",
            format!("{field} contains unsupported characters."),
        ));
    }
    Ok(())
}

fn require_safe_text(field: &str, value: &str, min: usize, max: usize) -> CoreResult<()> {
    let lower = value.to_ascii_lowercase();
    if value.len() < min
        || value.len() > max
        || value.contains('\0')
        || value.contains("://")
        || lower.contains("data:image/")
        || lower.contains("bearer ")
        || lower.contains("api_key")
        || lower.contains("/users/")
        || lower.contains("../")
        || lower.contains("sk-")
    {
        return Err(invalid(
            "MULTIMODAL_TEXT_UNSAFE",
            format!("{field} contains an unbounded path, URL, credential marker or invalid text."),
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> CoreResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(
            "MULTIMODAL_SHA256_INVALID",
            format!("{field} must be a lowercase SHA-256 digest."),
        ));
    }
    Ok(())
}

fn invalid(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{
        ForgeVisualDesignToken, ForgeVisualExportProfile, ForgeVisualMaterialBinding,
        ForgeVisualPart, ForgeVisualProgramStage, ReferenceClass, ReferenceEvidenceKind,
        ReferenceEvidenceObservations, ReferenceImageBrightnessBucket, ReferenceImageColorBucket,
        ReferenceImageEdgeDensityBucket, ReferenceImageForegroundConfidence,
        ReferenceImageSurfaceFacts, VisualDetailBinding, VisualDetailBindingKind,
        VisualDetailInventoryItem,
    };

    fn evidence() -> ReferenceEvidence {
        ReferenceEvidence {
            schema_version: "ReferenceEvidence@1".into(),
            evidence_id: "refevid_arm_front".into(),
            project_id: "prj_multimodal".into(),
            kind: ReferenceEvidenceKind::Image,
            reference_class: ReferenceClass::SingleImage,
            domain_pack_id: "pack_robotic_arm_concept".into(),
            source_file_name: "arm-front.png".into(),
            source_media_type: "image/png".into(),
            source_object_sha256: "a".repeat(64),
            source_imported_asset_version_id: None,
            source_statement: "User supplied visual reference".into(),
            license_statement: "User confirms rights for design reference".into(),
            missing_views: vec!["back".into()],
            user_notes: "Use the visible silhouette and panel language".into(),
            observations: ReferenceEvidenceObservations {
                silhouette_summary: "Tall articulated desktop arm".into(),
                proportion_ranges: vec!["upper and lower arm have similar length".into()],
                material_zone_observations: vec!["dark shell with blue accents".into()],
                visible_part_hypotheses: vec![],
                uncertainties: vec!["back surface is not visible".into()],
                image_surface_facts: Some(ReferenceImageSurfaceFacts {
                    width: 1024,
                    height: 1024,
                    aspect_ratio_milli: 1000,
                    dominant_color_buckets: vec![ReferenceImageColorBucket::Blue],
                    brightness: ReferenceImageBrightnessBucket::Dark,
                    edge_density: ReferenceImageEdgeDensityBucket::High,
                    foreground_bbox_normalized: [100, 80, 900, 950],
                    contact_sheet_layout_evidence: false,
                    foreground_confidence: ReferenceImageForegroundConfidence::Medium,
                }),
            },
            created_at: "2026-07-26T12:00:00Z".into(),
            glb_inspection: None,
        }
    }

    fn request(evidence: &ReferenceEvidence) -> MultimodalDesignRequest {
        MultimodalDesignRequest {
            schema_version: MULTIMODAL_DESIGN_REQUEST_SCHEMA_VERSION.into(),
            request_id: "mmreq_arm_001".into(),
            project_id: evidence.project_id.clone(),
            turn_id: "turn_arm_001".into(),
            domain_pack_id: evidence.domain_pack_id.clone(),
            instruction: "Create a refined articulated arm using the visible silhouette and blue panel language".into(),
            reference_inputs: vec![MultimodalReferenceInput {
                evidence_id: evidence.evidence_id.clone(),
                evidence_sha256: semantic_sha256(evidence).unwrap(),
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
        }
    }

    fn graph(
        request: &MultimodalDesignRequest,
        evidence: &ReferenceEvidence,
    ) -> VisualEvidenceGraph {
        VisualEvidenceGraph {
            schema_version: VISUAL_EVIDENCE_GRAPH_SCHEMA_VERSION.into(),
            graph_id: "vegraph_arm_001".into(),
            request_id: request.request_id.clone(),
            request_sha256: semantic_sha256(request).unwrap(),
            project_id: request.project_id.clone(),
            domain_pack_id: request.domain_pack_id.clone(),
            provider: VisionEvidenceProviderProvenance {
                provider_id: "openai_compatible_vision".into(),
                model_id: "qwen3-vl-plus".into(),
                provider_response_sha256: "b".repeat(64),
                analyzed_at: "2026-07-26T12:01:00Z".into(),
            },
            claims: vec![
                VisualEvidenceClaim {
                    claim_id: "vclaim_silhouette".into(),
                    level: VisualDetailLevel::Macro,
                    status: VisualClaimStatus::Observed,
                    target: VisualClaimTarget::Geometry,
                    description: "Tall articulated silhouette with balanced arm segments".into(),
                    critical: true,
                    confidence_bps: 9200,
                    source_evidence_ids: vec![evidence.evidence_id.clone()],
                    source_view_id: Some("front".into()),
                    source_region: None,
                },
                VisualEvidenceClaim {
                    claim_id: "vclaim_panels".into(),
                    level: VisualDetailLevel::Meso,
                    status: VisualClaimStatus::Observed,
                    target: VisualClaimTarget::Material,
                    description: "Blue armor panels contrast with a dark structural shell".into(),
                    critical: true,
                    confidence_bps: 8700,
                    source_evidence_ids: vec![evidence.evidence_id.clone()],
                    source_view_id: Some("front".into()),
                    source_region: Some(NormalizedEvidenceRegion {
                        left: 120,
                        top: 120,
                        right: 880,
                        bottom: 820,
                    }),
                },
                VisualEvidenceClaim {
                    claim_id: "vclaim_back_surface".into(),
                    level: VisualDetailLevel::Micro,
                    status: VisualClaimStatus::Unknown,
                    target: VisualClaimTarget::Surface,
                    description: "Back-surface micro pattern is not visible".into(),
                    critical: false,
                    confidence_bps: 0,
                    source_evidence_ids: vec![],
                    source_view_id: None,
                    source_region: None,
                },
            ],
        }
    }

    fn program() -> ForgeVisualProgram {
        ForgeVisualProgram {
            schema_version: "ForgeVisualProgram@1".into(),
            program_id: "visual_arm_multimodal".into(),
            domain_pack_id: "pack_robotic_arm_concept".into(),
            title: "Multimodal arm".into(),
            stage: ForgeVisualProgramStage::Draft,
            visual_only: true,
            design_tokens: vec![ForgeVisualDesignToken {
                token_id: "style".into(),
                value: "dark blue articulated industrial".into(),
            }],
            parts: vec![ForgeVisualPart {
                part_id: "part_arm".into(),
                role: "arm".into(),
                parent_part_id: None,
                geometry_output_ids: vec!["output_arm".into()],
                material_zone_ids: vec!["zone_arm".into()],
            }],
            geometry_graph: json!({
                "schema_version":"ShapeProgram@1",
                "program_id":"shape_arm_multimodal",
                "units":"millimeter",
                "coordinate_system":"right_handed_y_up",
                "operations":[{"operation_id":"op_arm","op":"primitive_box","args":{"size":[10.0,20.0,8.0],"position":[0.0,0.0,0.0],"rotation":[0.0,0.0,0.0],"material_id":"mat_automotive_paint"}}],
                "outputs":[{"output_id":"output_arm","operation_id":"op_arm","kind":"mesh","part_role":"arm"}]
            }),
            assembly_graph: json!({"schema_version":"AssemblyGraph@1"}),
            material_graph: vec![ForgeVisualMaterialBinding {
                part_id: "part_arm".into(),
                material_zone_id: "zone_arm".into(),
                material_id: "mat_automotive_paint".into(),
            }],
            surface_graph: vec![],
            detail_inventory: vec![
                VisualDetailInventoryItem {
                    detail_id: "detail_silhouette".into(),
                    level: VisualDetailLevel::Macro,
                    description: "Articulated silhouette".into(),
                    critical: true,
                    status: VisualDetailStatus::Bound,
                    bindings: vec![VisualDetailBinding {
                        kind: VisualDetailBindingKind::GeometryOutput,
                        part_id: "part_arm".into(),
                        target_id: "output_arm".into(),
                    }],
                },
                VisualDetailInventoryItem {
                    detail_id: "detail_panels".into(),
                    level: VisualDetailLevel::Meso,
                    description: "Blue armor panel language".into(),
                    critical: true,
                    status: VisualDetailStatus::Bound,
                    bindings: vec![VisualDetailBinding {
                        kind: VisualDetailBindingKind::MaterialZone,
                        part_id: "part_arm".into(),
                        target_id: "zone_arm".into(),
                    }],
                },
                VisualDetailInventoryItem {
                    detail_id: "detail_back_surface".into(),
                    level: VisualDetailLevel::Micro,
                    description: "Back-surface pattern remains unresolved".into(),
                    critical: false,
                    status: VisualDetailStatus::Unresolved,
                    bindings: vec![],
                },
            ],
            export_profile: ForgeVisualExportProfile::ProductionConcept,
        }
    }

    fn binding(
        request: &MultimodalDesignRequest,
        graph: &VisualEvidenceGraph,
        program: &ForgeVisualProgram,
    ) -> MultimodalProgramEvidenceBinding {
        MultimodalProgramEvidenceBinding {
            schema_version: MULTIMODAL_PROGRAM_EVIDENCE_BINDING_SCHEMA_VERSION.into(),
            binding_id: "mmbind_arm_001".into(),
            request_sha256: semantic_sha256(request).unwrap(),
            evidence_graph_sha256: semantic_sha256(graph).unwrap(),
            source_program_sha256: semantic_sha256(program).unwrap(),
            project_id: request.project_id.clone(),
            domain_pack_id: request.domain_pack_id.clone(),
            program_id: program.program_id.clone(),
            dispositions: vec![
                VisualClaimDisposition {
                    claim_id: "vclaim_silhouette".into(),
                    disposition: VisualClaimDispositionKind::Bound,
                    detail_ids: vec!["detail_silhouette".into()],
                    reason: "Visible silhouette is implemented by the arm output".into(),
                },
                VisualClaimDisposition {
                    claim_id: "vclaim_panels".into(),
                    disposition: VisualClaimDispositionKind::Bound,
                    detail_ids: vec!["detail_panels".into()],
                    reason: "Visible panel color language is bound to the arm material zone".into(),
                },
                VisualClaimDisposition {
                    claim_id: "vclaim_back_surface".into(),
                    disposition: VisualClaimDispositionKind::Unresolved,
                    detail_ids: vec!["detail_back_surface".into()],
                    reason: "The source declares the back view missing".into(),
                },
            ],
        }
    }

    fn candidate_views(glb_sha256: &str) -> Vec<VisualFixedViewEvidence> {
        REQUIRED_VISUAL_VIEW_IDS
            .into_iter()
            .enumerate()
            .map(|(index, view_id)| VisualFixedViewEvidence {
                view_id: view_id.into(),
                glb_sha256: glb_sha256.into(),
                renderer_id: "forgecad_fixed_eight_view_v1".into(),
                image_sha256: format!("{:064x}", index + 32),
                readback_passed: true,
            })
            .collect()
    }

    fn turntable_views(glb_sha256: &str) -> Vec<VisualFixedViewEvidence> {
        [
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
        .enumerate()
        .map(|(index, view_id)| VisualFixedViewEvidence {
            view_id: view_id.into(),
            glb_sha256: glb_sha256.into(),
            renderer_id: "forgecad_turntable_eight_v1".into(),
            image_sha256: format!("{:064x}", index + 96),
            readback_passed: true,
        })
        .collect()
    }

    fn passing_assessments() -> Vec<VisualReferenceClaimAssessment> {
        vec![
            VisualReferenceClaimAssessment {
                claim_id: "vclaim_silhouette".into(),
                outcome: VisualReferenceMatchOutcome::Matched,
                similarity_bps: 8_400,
                confidence_bps: 9_100,
                source_evidence_ids: vec!["refevid_arm_front".into()],
                candidate_view_ids: vec!["iso".into(), "front".into()],
                reason: "The candidate preserves the visible tall articulated silhouette".into(),
            },
            VisualReferenceClaimAssessment {
                claim_id: "vclaim_panels".into(),
                outcome: VisualReferenceMatchOutcome::Matched,
                similarity_bps: 7_600,
                confidence_bps: 8_800,
                source_evidence_ids: vec!["refevid_arm_front".into()],
                candidate_view_ids: vec!["iso".into(), "front".into()],
                reason: "The dark shell and blue panel hierarchy remain visible".into(),
            },
        ]
    }

    #[test]
    fn pv006a_multimodal_request_graph_and_program_binding_preserve_exact_lineage() {
        let evidence = evidence();
        let request = request(&evidence);
        let graph = graph(&request, &evidence);
        let program = program();
        let binding = binding(&request, &graph, &program);
        binding
            .validate_against(&request, &graph, &[evidence], &program)
            .unwrap();
    }

    #[test]
    fn pv006a_rejects_stale_reference_hash_and_missing_view_observation() {
        let evidence = evidence();
        let mut bad_request = request(&evidence);
        bad_request.reference_inputs[0].evidence_sha256 = "c".repeat(64);
        assert_eq!(
            bad_request
                .validate_with_evidence(&[evidence.clone()])
                .unwrap_err()
                .code(),
            "MULTIMODAL_REFERENCE_HASH_MISMATCH"
        );

        let request = request(&evidence);
        let mut graph = graph(&request, &evidence);
        graph.claims[0].source_view_id = Some("back".into());
        assert_eq!(
            graph
                .validate_against(&request, &[evidence])
                .unwrap_err()
                .code(),
            "VISUAL_EVIDENCE_MISSING_VIEW_OBSERVED"
        );
    }

    #[test]
    fn pv006a_rejects_credential_or_url_leak_in_provider_output() {
        let evidence = evidence();
        let request = request(&evidence);
        let mut graph = graph(&request, &evidence);
        graph.claims[0].description = "Fetch https://example.invalid with sk-secret".into();
        assert_eq!(
            graph
                .validate_against(&request, &[evidence])
                .unwrap_err()
                .code(),
            "MULTIMODAL_TEXT_UNSAFE"
        );
    }

    #[test]
    fn pv006a_rejects_bound_claim_without_real_program_detail() {
        let evidence = evidence();
        let request = request(&evidence);
        let graph = graph(&request, &evidence);
        let program = program();
        let binding = MultimodalProgramEvidenceBinding {
            schema_version: MULTIMODAL_PROGRAM_EVIDENCE_BINDING_SCHEMA_VERSION.into(),
            binding_id: "mmbind_arm_bad".into(),
            request_sha256: semantic_sha256(&request).unwrap(),
            evidence_graph_sha256: semantic_sha256(&graph).unwrap(),
            source_program_sha256: semantic_sha256(&program).unwrap(),
            project_id: request.project_id.clone(),
            domain_pack_id: request.domain_pack_id.clone(),
            program_id: program.program_id.clone(),
            dispositions: vec![
                VisualClaimDisposition {
                    claim_id: "vclaim_silhouette".into(),
                    disposition: VisualClaimDispositionKind::Bound,
                    detail_ids: vec!["detail_missing".into()],
                    reason: "Invalid fixture".into(),
                },
                VisualClaimDisposition {
                    claim_id: "vclaim_panels".into(),
                    disposition: VisualClaimDispositionKind::Bound,
                    detail_ids: vec!["detail_panels".into()],
                    reason: "Visible panel language".into(),
                },
                VisualClaimDisposition {
                    claim_id: "vclaim_back_surface".into(),
                    disposition: VisualClaimDispositionKind::Unresolved,
                    detail_ids: vec!["detail_back_surface".into()],
                    reason: "Back view is missing".into(),
                },
            ],
        };
        assert_eq!(
            binding
                .validate_against(&request, &graph, &[evidence], &program)
                .unwrap_err()
                .code(),
            "MULTIMODAL_PROGRAM_DETAIL_UNKNOWN"
        );
    }

    #[test]
    fn pv006a_selection_and_locks_require_an_active_asset() {
        let evidence = evidence();
        let mut request = request(&evidence);
        request.selection = Some(MultimodalSelectionScope {
            part_ids: vec!["part_arm".into()],
            material_zone_ids: vec![],
            reference_region: None,
        });
        assert_eq!(
            request
                .validate_with_evidence(&[evidence])
                .unwrap_err()
                .code(),
            "MULTIMODAL_SELECTION_REQUIRES_ACTIVE_ASSET"
        );
    }

    #[test]
    fn pv006c_reference_comparison_binds_exact_evidence_program_glb_and_eight_views() {
        let evidence = evidence();
        let request = request(&evidence);
        let graph = graph(&request, &evidence);
        let program = program();
        let binding = binding(&request, &graph, &program);
        let glb_sha256 = "c".repeat(64);
        let input = VisualReferenceComparisonInput::build(
            &request,
            &graph,
            &binding,
            &[evidence.clone()],
            &program,
            &glb_sha256,
            &candidate_views(&glb_sha256),
        )
        .unwrap();
        let report = VisualReferenceComparisonReport::build(
            &input,
            &graph,
            VisionEvidenceProviderProvenance {
                provider_id: "openai_compatible_vision".into(),
                model_id: "qwen3.7-plus".into(),
                provider_response_sha256: "d".repeat(64),
                analyzed_at: "2026-07-26T23:50:00Z".into(),
            },
            passing_assessments(),
        )
        .unwrap();
        report.validate_against(&input, &graph).unwrap();
        assert!(report.passed);
        assert_eq!(report.macro_similarity_bps, Some(8_400));
        assert_eq!(report.meso_similarity_bps, Some(7_600));
        assert_eq!(report.micro_similarity_bps, None);
        assert!(report.failure_codes.is_empty());
        assert!(report.repair_claim_ids.is_empty());
    }

    #[test]
    fn e005_r2_reference_comparison_accepts_generic_turntable_eight_without_arm_view_names() {
        let evidence = evidence();
        let request = request(&evidence);
        let graph = graph(&request, &evidence);
        let program = program();
        let binding = binding(&request, &graph, &program);
        let glb_sha256 = "9".repeat(64);
        let mut input = VisualReferenceComparisonInput::build(
            &request,
            &graph,
            &binding,
            &[evidence.clone()],
            &program,
            &glb_sha256,
            &candidate_views(&glb_sha256),
        )
        .unwrap();
        input.candidate_view_profile = Some(VisualReferenceCandidateViewProfile::TurntableEight);
        input.candidate_views = turntable_views(&glb_sha256);
        input
            .validate_against(&request, &graph, &binding, &[evidence], &program)
            .unwrap();
    }

    #[test]
    fn e005_r2_unified_source_comparison_and_rust_sealed_proposal_have_exact_lineage() {
        let evidence = evidence();
        let request = request(&evidence);
        let graph = graph(&request, &evidence);
        let source: Value = serde_json::from_str(include_str!(
            "../../../../../../packages/concept-spec/fixtures/e005-r1-unified-service-console.json"
        ))
        .unwrap();
        let lowering = crate::lower_forge_visual_author_source_v1(&source).unwrap();
        let glb_sha256 = "9".repeat(64);
        let input = VisualReferenceComparisonInput::build_for_e005_source(
            &request,
            &graph,
            &[evidence.clone()],
            &source,
            &glb_sha256,
            &turntable_views(&glb_sha256),
            VisualReferenceAcceptancePolicy::default_policy(),
        )
        .unwrap();
        assert_eq!(input.source_program_sha256, lowering.source_program_sha256);
        assert_eq!(
            input.candidate_view_profile,
            Some(VisualReferenceCandidateViewProfile::TurntableEight)
        );

        let assessments = vec![
            VisualReferenceClaimAssessment {
                claim_id: "vclaim_silhouette".into(),
                outcome: VisualReferenceMatchOutcome::Partial,
                similarity_bps: 5_000,
                confidence_bps: 9_100,
                source_evidence_ids: vec![evidence.evidence_id.clone()],
                candidate_view_ids: vec!["turntable_000".into(), "turntable_045".into()],
                reason: "The primary silhouette requires one bounded position repair".into(),
            },
            VisualReferenceClaimAssessment {
                claim_id: "vclaim_panels".into(),
                outcome: VisualReferenceMatchOutcome::Matched,
                similarity_bps: 7_600,
                confidence_bps: 8_800,
                source_evidence_ids: vec![evidence.evidence_id.clone()],
                candidate_view_ids: vec!["turntable_000".into()],
                reason: "The visible panel hierarchy matches".into(),
            },
        ];
        let report = VisualReferenceComparisonReport::build(
            &input,
            &graph,
            VisionEvidenceProviderProvenance {
                provider_id: "vision_fixture".into(),
                model_id: "vision_fixture".into(),
                provider_response_sha256: "d".repeat(64),
                analyzed_at: "2026-07-29T12:00:00Z".into(),
            },
            assessments,
        )
        .unwrap();
        assert!(!report.passed);
        assert_eq!(report.repair_claim_ids, vec!["vclaim_silhouette"]);
        let proposal = json!({
            "schema_version":"E005VisualPatchProposal@1",
            "patch_id":"visualpatch_e005_r2_one_call",
            "decision":"typed_visual_patch",
            "expected_source_sha256":input.source_program_sha256,
            "comparison_input_sha256":semantic_sha256(&input).unwrap(),
            "repair_claim_ids":report.repair_claim_ids,
            "operations":[{
                "op":"set_instance_position",
                "instance_id":"instance_shell",
                "position":[10.0,0.0,0.0]
            }]
        });
        let sealed =
            crate::seal_e005_visual_patch_proposal_v1(&proposal, &input, &graph, &report).unwrap();
        assert_eq!(sealed.comparison_report_sha256, report.report_sha256);
        assert_eq!(
            sealed.comparison_input_sha256,
            semantic_sha256(&input).unwrap()
        );

        let mut executable_surface = proposal;
        executable_surface["operations"] = json!([{
            "op":"set_surface_tuning",
            "binding_id":"surface_shell",
            "edge_wear":0.2,
            "micro_detail":0.8
        }]);
        let surface_sealed =
            crate::seal_e005_visual_patch_proposal_v1(&executable_surface, &input, &graph, &report)
                .unwrap();
        let parent_lowering = crate::lower_forge_visual_author_source_v1(&source).unwrap();
        let surface_result = crate::apply_e005_visual_patch_v1(
            &source,
            &serde_json::to_value(surface_sealed).unwrap(),
        )
        .unwrap();
        assert_eq!(
            surface_result.lowering.shape_program_sha256,
            parent_lowering.shape_program_sha256
        );
        assert_ne!(
            surface_result.lowering.surface_plan_sha256,
            parent_lowering.surface_plan_sha256
        );

        let mut stale_source = source;
        stale_source["seed"] = json!(999);
        assert_eq!(
            input
                .validate_for_e005_source(&request, &graph, &[evidence], &stale_source)
                .unwrap_err()
                .code(),
            "E005_R2_COMPARISON_LINEAGE_INVALID"
        );
    }

    #[test]
    fn pv006c_reference_comparison_rejects_stale_views_and_incomplete_claim_coverage() {
        let evidence = evidence();
        let request = request(&evidence);
        let graph = graph(&request, &evidence);
        let program = program();
        let binding = binding(&request, &graph, &program);
        let glb_sha256 = "c".repeat(64);
        let mut views = candidate_views(&glb_sha256);
        views[0].glb_sha256 = "e".repeat(64);
        assert_eq!(
            VisualReferenceComparisonInput::build(
                &request,
                &graph,
                &binding,
                &[evidence.clone()],
                &program,
                &glb_sha256,
                &views,
            )
            .unwrap_err()
            .code(),
            "VISUAL_REFERENCE_COMPARISON_VIEW_INVALID"
        );

        let input = VisualReferenceComparisonInput::build(
            &request,
            &graph,
            &binding,
            &[evidence],
            &program,
            &glb_sha256,
            &candidate_views(&glb_sha256),
        )
        .unwrap();
        let mut assessments = passing_assessments();
        assessments.pop();
        assert_eq!(
            VisualReferenceComparisonReport::build(
                &input,
                &graph,
                VisionEvidenceProviderProvenance {
                    provider_id: "vision_fixture".into(),
                    model_id: "vision_fixture".into(),
                    provider_response_sha256: "f".repeat(64),
                    analyzed_at: "2026-07-26T23:51:00Z".into(),
                },
                assessments,
            )
            .unwrap_err()
            .code(),
            "VISUAL_REFERENCE_COMPARISON_COVERAGE_INVALID"
        );
    }

    #[test]
    fn pv006c_reference_comparison_derives_repair_targets_and_cannot_fake_pass() {
        let evidence = evidence();
        let request = request(&evidence);
        let graph = graph(&request, &evidence);
        let program = program();
        let binding = binding(&request, &graph, &program);
        let glb_sha256 = "c".repeat(64);
        let input = VisualReferenceComparisonInput::build(
            &request,
            &graph,
            &binding,
            &[evidence],
            &program,
            &glb_sha256,
            &candidate_views(&glb_sha256),
        )
        .unwrap();
        let mut assessments = passing_assessments();
        assessments[0].outcome = VisualReferenceMatchOutcome::Partial;
        assessments[0].similarity_bps = 4_200;
        let mut report = VisualReferenceComparisonReport::build(
            &input,
            &graph,
            VisionEvidenceProviderProvenance {
                provider_id: "vision_fixture".into(),
                model_id: "vision_fixture".into(),
                provider_response_sha256: "1".repeat(64),
                analyzed_at: "2026-07-26T23:52:00Z".into(),
            },
            assessments,
        )
        .unwrap();
        assert!(!report.passed);
        assert!(report
            .failure_codes
            .contains(&"CRITICAL_REFERENCE_CLAIM_MISMATCH".into()));
        assert!(report
            .failure_codes
            .contains(&"REFERENCE_MACRO_MISMATCH".into()));
        assert_eq!(report.repair_claim_ids, vec!["vclaim_silhouette"]);

        report.passed = true;
        assert_eq!(
            report.validate_against(&input, &graph).unwrap_err().code(),
            "VISUAL_REFERENCE_COMPARISON_REPORT_DERIVATION_INVALID"
        );
    }

    #[test]
    fn c111b_reference_policy_is_hash_bound_and_enforces_frozen_level_thresholds() {
        let evidence = evidence();
        let request = request(&evidence);
        let graph = graph(&request, &evidence);
        let program = program();
        let binding = binding(&request, &graph, &program);
        let glb_sha256 = "c".repeat(64);
        let reviewed_c111 = crate::reviewed_c111_draft_visual_program().unwrap();
        let policy = crate::c111b_visual_reference_acceptance_policy(&reviewed_c111)
            .unwrap()
            .unwrap();
        assert_eq!(policy.macro_minimum_bps, 7_600);
        assert_eq!(policy.meso_minimum_bps, 6_500);
        assert_eq!(policy.micro_minimum_bps, 5_000);
        assert_eq!(
            policy.source_contract_sha256.as_deref(),
            Some("6ea15677d504d59e19d26c3d6e0f8fdc4caa96882c574608426ec731db114a87")
        );
        let mut unrelated_program = reviewed_c111.clone();
        unrelated_program.program_id = "visual_program_unrelated".into();
        assert!(
            crate::c111b_visual_reference_acceptance_policy(&unrelated_program)
                .unwrap()
                .is_none()
        );

        let input = VisualReferenceComparisonInput::build_with_policy(
            &request,
            &graph,
            &binding,
            &[evidence],
            &program,
            &glb_sha256,
            &candidate_views(&glb_sha256),
            policy,
        )
        .unwrap();
        assert_eq!(
            input.schema_version,
            VISUAL_REFERENCE_COMPARISON_INPUT_SCHEMA_VERSION
        );

        let mut below_macro = passing_assessments();
        below_macro[0].similarity_bps = 7_500;
        below_macro[1].outcome = VisualReferenceMatchOutcome::Partial;
        below_macro[1].similarity_bps = 6_500;
        let failed = VisualReferenceComparisonReport::build(
            &input,
            &graph,
            VisionEvidenceProviderProvenance {
                provider_id: "vision_fixture".into(),
                model_id: "vision_fixture".into(),
                provider_response_sha256: "2".repeat(64),
                analyzed_at: "2026-07-28T13:00:00Z".into(),
            },
            below_macro,
        )
        .unwrap();
        assert!(!failed.passed);
        assert!(failed
            .failure_codes
            .contains(&"REFERENCE_MACRO_MISMATCH".into()));

        let mut exact_threshold = passing_assessments();
        exact_threshold[0].similarity_bps = 7_600;
        exact_threshold[1].outcome = VisualReferenceMatchOutcome::Partial;
        exact_threshold[1].similarity_bps = 6_500;
        let passed = VisualReferenceComparisonReport::build(
            &input,
            &graph,
            VisionEvidenceProviderProvenance {
                provider_id: "vision_fixture".into(),
                model_id: "vision_fixture".into(),
                provider_response_sha256: "3".repeat(64),
                analyzed_at: "2026-07-28T13:01:00Z".into(),
            },
            exact_threshold,
        )
        .unwrap();
        assert!(passed.passed);

        let mut tampered_input = input.clone();
        tampered_input.acceptance_policy.macro_minimum_bps = 6_000;
        assert_eq!(
            passed
                .validate_against(&tampered_input, &graph)
                .unwrap_err()
                .code(),
            "VISUAL_REFERENCE_COMPARISON_REPORT_LINEAGE_INVALID"
        );
    }
}
