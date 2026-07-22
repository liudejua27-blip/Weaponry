//! Code-owned V003 native generation gate profile.
//!
//! This module turns Rust-owned readback and provenance facts into the exact
//! hard-gate set consumed by [`crate::SingleGenerationSession`]. It does not
//! accept a model-authored gate list, perform visual scoring, or persist a
//! Version/Snapshot. Missing, unknown, duplicate, or source-contradictory
//! evidence is rejected before a [`crate::GenerationGateReport`] is created.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    CoreError, CoreResult, GenerationGateCheck, GenerationGateReport, VerificationOutcome,
    GENERATION_GATE_REPORT_SCHEMA_VERSION,
};

pub const NATIVE_GENERATION_GATE_EVIDENCE_SCHEMA_VERSION: &str = "NativeGenerationGateEvidence@2";
pub const NATIVE_GENERATION_GATE_EVALUATION_SCHEMA_VERSION: &str =
    "NativeGenerationGateEvaluation@2";
pub const NATIVE_V003_GATE_PROFILE_ID: &str = "forgecad.v003.native_generation_gate";
pub const NATIVE_V003_GATE_PROFILE_VERSION: &str = "native_v003_gate_v2";

/// SHA-256 of [`NATIVE_V003_GATE_PROFILE_CANONICAL`]. Any rule, ordering,
/// source, or repairability change requires a new profile version and digest.
pub const NATIVE_V003_GATE_PROFILE_SHA256: &str =
    "c59c57167aba5a567abcd8090fc898fb68d0d298c20e5c8714c3abe77e87807b";

pub const NATIVE_V003_GATE_IDS: [&str; 13] = [
    "has_triangles",
    "has_meshes",
    "four_views_read_back",
    "closed_manifold",
    "surface_provenance_present",
    "glb_hash_verified",
    "brief_coverage",
    "semantic_proportion_bound",
    "domain_role_coverage",
    "material_texture_provenance",
    "editability_evidence",
    "r006_same_source_views",
    "generation_source_marked",
];

/// Frozen, deliberately plain-text profile definition used only for its
/// version hash. The line order is also the canonical output check order.
pub const NATIVE_V003_GATE_PROFILE_CANONICAL: &str = concat!(
    "profile_id=forgecad.v003.native_generation_gate\n",
    "profile_version=native_v003_gate_v2\n",
    "has_triangles|restricted_geometry_glb_readback|not_repairable\n",
    "has_meshes|restricted_geometry_glb_readback|not_repairable\n",
    "four_views_read_back|deterministic_concept_render_readback|not_repairable\n",
    "closed_manifold|restricted_geometry_glb_readback|repairable\n",
    "surface_provenance_present|restricted_geometry_glb_readback|repairable\n",
    "glb_hash_verified|restricted_geometry_glb_readback|not_repairable\n",
    "brief_coverage|brief_coverage_resolver|not_repairable\n",
    "semantic_proportion_bound|semantic_proportion_resolver|not_repairable\n",
    "domain_role_coverage|domain_role_resolver|not_repairable\n",
    "material_texture_provenance|production_material_readback|not_repairable\n",
    "editability_evidence|recipe_assembly_readback|not_repairable\n",
    "r006_same_source_views|deterministic_concept_render_readback|not_repairable\n",
    "generation_source_marked|generation_execution_trace|not_repairable\n",
);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum NativeGateEvidenceSource {
    RestrictedGeometryGlbReadback,
    DeterministicConceptRenderReadback,
    BriefCoverageResolver,
    SemanticProportionResolver,
    DomainRoleResolver,
    ProductionMaterialReadback,
    RecipeAssemblyReadback,
    GenerationExecutionTrace,
}

/// One native fact. Product-tool arguments and Provider output must never be
/// deserialized directly into this DTO; the native executor constructs it
/// only after validating the named Rust-owned source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NativeGenerationGateEvidence {
    pub schema_version: String,
    pub gate_id: String,
    pub outcome: VerificationOutcome,
    pub source: NativeGateEvidenceSource,
    /// Hash of the source record or artifact, not of model-authored prose.
    pub source_sha256: String,
    pub summary: String,
}

/// Report identities supplied by the trusted native runtime. Profile identity
/// and repairability are intentionally absent: this module owns both.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NativeGenerationGateBinding {
    pub gate_report_id: String,
    pub attempt_id: String,
    pub glb_sha256: String,
    pub compile_readback_id: String,
    pub render_fingerprint: String,
    pub summary: String,
}

/// Internal evidence envelope. The result card consumes only the downstream
/// `SingleResultDecision@1`; this evidence is for native validation/audit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NativeGenerationGateEvaluation {
    pub schema_version: String,
    pub gate_profile_id: String,
    pub gate_profile_version: String,
    pub gate_profile_sha256: String,
    pub evidence: Vec<NativeGenerationGateEvidence>,
    pub report: GenerationGateReport,
}

#[derive(Debug, Clone, Copy)]
struct GateRule {
    gate_id: &'static str,
    source: NativeGateEvidenceSource,
    repairable_on_fail: bool,
}

const GATE_RULES: [GateRule; 13] = [
    rule(
        "has_triangles",
        NativeGateEvidenceSource::RestrictedGeometryGlbReadback,
        false,
    ),
    rule(
        "has_meshes",
        NativeGateEvidenceSource::RestrictedGeometryGlbReadback,
        false,
    ),
    rule(
        "four_views_read_back",
        NativeGateEvidenceSource::DeterministicConceptRenderReadback,
        false,
    ),
    rule(
        "closed_manifold",
        NativeGateEvidenceSource::RestrictedGeometryGlbReadback,
        true,
    ),
    rule(
        "surface_provenance_present",
        NativeGateEvidenceSource::RestrictedGeometryGlbReadback,
        true,
    ),
    rule(
        "glb_hash_verified",
        NativeGateEvidenceSource::RestrictedGeometryGlbReadback,
        false,
    ),
    rule(
        "brief_coverage",
        NativeGateEvidenceSource::BriefCoverageResolver,
        false,
    ),
    rule(
        "semantic_proportion_bound",
        NativeGateEvidenceSource::SemanticProportionResolver,
        false,
    ),
    rule(
        "domain_role_coverage",
        NativeGateEvidenceSource::DomainRoleResolver,
        false,
    ),
    rule(
        "material_texture_provenance",
        NativeGateEvidenceSource::ProductionMaterialReadback,
        false,
    ),
    rule(
        "editability_evidence",
        NativeGateEvidenceSource::RecipeAssemblyReadback,
        false,
    ),
    rule(
        "r006_same_source_views",
        NativeGateEvidenceSource::DeterministicConceptRenderReadback,
        false,
    ),
    rule(
        "generation_source_marked",
        NativeGateEvidenceSource::GenerationExecutionTrace,
        false,
    ),
];

const fn rule(
    gate_id: &'static str,
    source: NativeGateEvidenceSource,
    repairable_on_fail: bool,
) -> GateRule {
    GateRule {
        gate_id,
        source,
        repairable_on_fail,
    }
}

impl NativeGenerationGateEvaluation {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != NATIVE_GENERATION_GATE_EVALUATION_SCHEMA_VERSION
            || self.gate_profile_id != NATIVE_V003_GATE_PROFILE_ID
            || self.gate_profile_version != NATIVE_V003_GATE_PROFILE_VERSION
            || self.gate_profile_sha256 != NATIVE_V003_GATE_PROFILE_SHA256
            || native_v003_gate_profile_sha256() != NATIVE_V003_GATE_PROFILE_SHA256
        {
            return Err(invalid(
                "NATIVE_V003_GATE_PROFILE_INVALID",
                "Native generation evidence must use the exact code-owned V003 gate profile v2 stamp.",
            ));
        }
        self.report.validate()?;
        if self.report.gate_profile_version != NATIVE_V003_GATE_PROFILE_VERSION {
            return Err(invalid(
                "NATIVE_V003_GATE_PROFILE_INVALID",
                "Generation gate report profile version does not match the native v2 profile.",
            ));
        }
        let expected_checks = validate_evidence(&self.evidence)?;
        if self.report.checks != expected_checks {
            return Err(invalid(
                "NATIVE_V003_GATE_REPORT_EVIDENCE_MISMATCH",
                "Generation gate checks must be derived exactly from their validated native evidence.",
            ));
        }
        Ok(())
    }
}

/// Builds the only accepted native V003 v2 report. The caller supplies stable
/// artifact/attempt bindings plus Rust-owned facts; it cannot choose the gate
/// set, source mapping, ordering, profile stamp, or repairability.
pub fn evaluate_native_v003_gate_profile_v2(
    binding: NativeGenerationGateBinding,
    evidence: Vec<NativeGenerationGateEvidence>,
) -> CoreResult<NativeGenerationGateEvaluation> {
    let checks = validate_evidence(&evidence)?;
    let report = GenerationGateReport {
        schema_version: GENERATION_GATE_REPORT_SCHEMA_VERSION.into(),
        gate_report_id: binding.gate_report_id,
        attempt_id: binding.attempt_id,
        glb_sha256: binding.glb_sha256,
        compile_readback_id: binding.compile_readback_id,
        render_fingerprint: binding.render_fingerprint,
        gate_profile_version: NATIVE_V003_GATE_PROFILE_VERSION.into(),
        checks,
        summary: binding.summary,
    };
    let evaluation = NativeGenerationGateEvaluation {
        schema_version: NATIVE_GENERATION_GATE_EVALUATION_SCHEMA_VERSION.into(),
        gate_profile_id: NATIVE_V003_GATE_PROFILE_ID.into(),
        gate_profile_version: NATIVE_V003_GATE_PROFILE_VERSION.into(),
        gate_profile_sha256: NATIVE_V003_GATE_PROFILE_SHA256.into(),
        evidence,
        report,
    };
    evaluation.validate()?;
    Ok(evaluation)
}

pub fn native_v003_gate_profile_sha256() -> String {
    let mut hasher = Sha256::new();
    hasher.update(NATIVE_V003_GATE_PROFILE_CANONICAL.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn validate_evidence(
    evidence: &[NativeGenerationGateEvidence],
) -> CoreResult<Vec<GenerationGateCheck>> {
    let known_ids = NATIVE_V003_GATE_IDS.into_iter().collect::<BTreeSet<_>>();
    let mut by_gate = BTreeMap::new();
    let mut source_hashes = BTreeMap::new();

    for item in evidence {
        if item.schema_version != NATIVE_GENERATION_GATE_EVIDENCE_SCHEMA_VERSION {
            return Err(invalid(
                "NATIVE_V003_GATE_EVIDENCE_SCHEMA_INVALID",
                "Native gate evidence must use its exact v2 schema.",
            ));
        }
        if !known_ids.contains(item.gate_id.as_str()) {
            return Err(invalid(
                "NATIVE_V003_GATE_UNKNOWN",
                "Native gate evidence contains a gate outside the frozen v2 profile.",
            ));
        }
        require_sha256("source_sha256", &item.source_sha256)?;
        require_summary(&item.summary)?;
        if by_gate.insert(item.gate_id.as_str(), item).is_some() {
            return Err(invalid(
                "NATIVE_V003_GATE_DUPLICATE",
                "Native gate evidence contains a duplicate gate id.",
            ));
        }
        if let Some(existing) = source_hashes.insert(item.source, item.source_sha256.as_str()) {
            if existing != item.source_sha256 {
                return Err(invalid(
                    "NATIVE_V003_GATE_SOURCE_CONTRADICTION",
                    "One Rust-owned evidence source cannot have conflicting fingerprints in the same evaluation.",
                ));
            }
        }
    }

    if by_gate.len() != GATE_RULES.len() {
        return Err(invalid(
            "NATIVE_V003_GATE_MISSING",
            "Native gate evidence must contain every gate in the frozen v2 profile exactly once.",
        ));
    }

    GATE_RULES
        .iter()
        .map(|rule| {
            let item = by_gate.get(rule.gate_id).ok_or_else(|| {
                invalid(
                    "NATIVE_V003_GATE_MISSING",
                    "Native gate evidence is missing a required v2 gate.",
                )
            })?;
            if item.source != rule.source {
                return Err(invalid(
                    "NATIVE_V003_GATE_SOURCE_CONTRADICTION",
                    "Native gate evidence source does not match the code-owned source for its gate.",
                ));
            }
            Ok(GenerationGateCheck {
                gate_id: rule.gate_id.into(),
                outcome: item.outcome,
                repairable: item.outcome == VerificationOutcome::Fail
                    && rule.repairable_on_fail,
                summary: item.summary.clone(),
            })
        })
        .collect()
}

fn invalid(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message)
}

fn require_sha256(field: &str, value: &str) -> CoreResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid(
            "NATIVE_V003_GATE_SHA256_INVALID",
            format!("{field} must be a lowercase SHA-256 digest."),
        ));
    }
    Ok(())
}

fn require_summary(value: &str) -> CoreResult<()> {
    if value.trim().is_empty() || value.len() > 1_024 {
        return Err(invalid(
            "NATIVE_V003_GATE_SUMMARY_INVALID",
            "Native gate evidence summary must be bounded and non-empty.",
        ));
    }
    Ok(())
}
