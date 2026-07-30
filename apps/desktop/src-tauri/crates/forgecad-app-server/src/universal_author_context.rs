//! Rust-validated U002 author context shared by lifecycle, Action Loop and the
//! native Product Tool executor.

use forgecad_core::{
    semantic_sha256, ReferenceEvidence, UniversalAuthorRequest, VisualEvidenceGraphV2,
};
use serde_json::{json, Value};

use crate::canonical::{canonical_json, sha256_hex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniversalAuthorContextError {
    pub code: String,
    pub message: String,
}

#[derive(Clone)]
pub struct ValidatedUniversalAuthorContext {
    request: UniversalAuthorRequest,
    evidence: Vec<ReferenceEvidence>,
    visual_evidence_graph: Option<Value>,
    context_digest: String,
}

impl std::fmt::Debug for ValidatedUniversalAuthorContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ValidatedUniversalAuthorContext")
            .field("request_id", &self.request.request_id)
            .field("input_mode", &self.request.input_mode)
            .field("reference_count", &self.evidence.len())
            .field("has_active_asset", &self.request.active_asset.is_some())
            .field("context_digest", &self.context_digest)
            .finish()
    }
}

impl ValidatedUniversalAuthorContext {
    pub fn new(
        request: UniversalAuthorRequest,
        evidence: &[ReferenceEvidence],
        visual_evidence_graph: Option<Value>,
    ) -> Result<Self, UniversalAuthorContextError> {
        request
            .validate_with_evidence(evidence)
            .map_err(|error| UniversalAuthorContextError {
                code: error.code().into(),
                message: format!("Rust rejected UniversalAuthorRequest@1: {error}"),
            })?;
        let request_sha256 =
            semantic_sha256(&request).map_err(|error| UniversalAuthorContextError {
                code: error.code().into(),
                message: error.to_string(),
            })?;
        let context_digest = sha256_hex(
            canonical_json(&json!({
                "schema_version": "ValidatedUniversalAuthorContext@1",
                "request_sha256": request_sha256,
                "visual_evidence_graph": visual_evidence_graph,
            }))
            .as_bytes(),
        );
        Ok(Self {
            request,
            evidence: evidence.to_vec(),
            visual_evidence_graph,
            context_digest,
        })
    }

    pub fn request(&self) -> &UniversalAuthorRequest {
        &self.request
    }

    pub fn evidence(&self) -> &[ReferenceEvidence] {
        &self.evidence
    }

    pub fn context_digest(&self) -> &str {
        &self.context_digest
    }

    /// The category-open evidence graph remains the product truth.  Callers
    /// that need to project it into a bounded comparison wire format must
    /// decode it here and validate it against the exact authored profile;
    /// they may not substitute a legacy Domain Pack graph.
    pub fn visual_evidence_graph_v2(
        &self,
    ) -> Result<Option<VisualEvidenceGraphV2>, UniversalAuthorContextError> {
        self.visual_evidence_graph
            .clone()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| UniversalAuthorContextError {
                code: "VISUAL_EVIDENCE_GRAPH_V2_INVALID".into(),
                message: format!("VisualEvidenceGraph@2 could not be decoded: {error}"),
            })
    }

    pub fn provider_projection(&self) -> Value {
        json!({
            "schema_version": "UniversalAuthorContext@1",
            "request": self.request,
            "visual_evidence_graph": self.visual_evidence_graph,
            "rules": [
                "Identify the actual subject without converting it to a known template.",
                "Return SubjectProfile@1, VisualFeatureContract@1 and RepresentationPlan@1 in author_universal_asset.",
                "Category is open text. Domain Packs are optional knowledge hints only.",
                "Only code-owned capability IDs in the exact manifest may be selected.",
                "Unavailable representation is a normal typed limitation and must not include geometry.",
                "Observed features require sealed evidence; hidden or inferred content must keep that status."
            ]
        })
    }
}
