//! Rust-validated U002 author context shared by lifecycle, Action Loop and the
//! native Product Tool executor.

use forgecad_core::{
    representation_capability_manifest, semantic_sha256, EvidenceStatus, ReferenceEvidence,
    SubjectProfile, UniversalAuthorRequest, UniversalEvidenceClaim, VisualClaimStatus,
    VisualDetailLevel, VisualEvidenceGraph, VisualEvidenceGraphV2, VisualFeatureEvidenceRegion,
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
    pub fn visual_evidence_graph_v2_for_profile(
        &self,
        profile: &SubjectProfile,
    ) -> Result<Option<VisualEvidenceGraphV2>, UniversalAuthorContextError> {
        let Some(value) = self.visual_evidence_graph.clone() else {
            return Ok(None);
        };
        let schema_version = value
            .get("schema_version")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| UniversalAuthorContextError {
                code: "VISUAL_EVIDENCE_GRAPH_SCHEMA_INVALID".into(),
                message: "Visual evidence graph must declare a schema version.".into(),
            })?;
        let graph = if schema_version == "VisualEvidenceGraph@2" {
            serde_json::from_value(value).map_err(|error| UniversalAuthorContextError {
                code: "VISUAL_EVIDENCE_GRAPH_V2_INVALID".into(),
                message: format!("VisualEvidenceGraph@2 could not be decoded: {error}"),
            })?
        } else if schema_version == "VisualEvidenceGraph@1" {
            project_legacy_visual_evidence_graph(&value, &self.request, profile, &self.evidence)?
        } else {
            return Err(UniversalAuthorContextError {
                code: "VISUAL_EVIDENCE_GRAPH_SCHEMA_INVALID".into(),
                message: format!("Unsupported visual evidence graph schema: {schema_version}"),
            });
        };
        graph
            .validate_against(&self.request, profile)
            .map_err(|error| UniversalAuthorContextError {
                code: error.code().into(),
                message: format!(
                    "VisualEvidenceGraph@2 failed universal lineage validation: {error}"
                ),
            })?;
        Ok(Some(graph))
    }

    pub fn provider_projection(&self) -> Value {
        let reference_evidence_ledger = self
            .request
            .reference_inputs
            .iter()
            .map(|reference| {
                json!({
                    "evidence_id": reference.evidence_id,
                    "role": reference.role,
                    "view_hint": reference.view_hint,
                })
            })
            .collect::<Vec<_>>();
        let geometry_authoring_playbook = json!({
            "purpose": "给出可组合的几何语法，不是完整对象模板；部件名称、数量、比例、轮廓和材质必须根据当前 SubjectProfile 重写。",
            "canonical_chains": [
                "box|cylinder|capsule|wedge -> optional bevel_approx -> part -> material_zone -> output",
                "box|bevel_approx -> optional surface_panel or groove -> part -> material_zone -> output",
                "profile + extrude|revolve -> part -> material_zone -> output",
                "profile + sweep or section_set + loft -> part -> material_zone -> output",
                "geometry -> array|radial_array|mirror -> part -> material_zone -> output"
            ],
            "graph_rules": [
                "Every reference must point to an earlier node or an earlier declared profile/section set.",
                "Every output graph must be disjoint: a node, ancestor, part, or material_zone may belong to only one output. Duplicate a bounded branch with fresh node IDs when two parts need similar geometry.",
                "Each output should terminate at material_zone wrapping part wrapping geometry; output.node_id must point to the material_zone.",
                "Primitive nodes have no input_node_id; transforms/details have exactly one earlier geometry input; union/subtract have 2..=8 earlier geometry inputs."
            ],
            "local_face_rules": [
                "surface_panel and groove accept only a direct box or bevel_approx source.",
                "Their position is local to that source; the coordinate along the selected face normal must be exactly 0, and the other two coordinates plus half the detail size must stay within the source half-size.",
                "When the local face placement is uncertain, use position [0,0,0] and a detail size smaller than the source face; do not use world-space coordinates or a numeric axis vector."
            ],
            "profile_rules": [
                "extrude/revolve/sweep require a declared profile_id; loft requires a declared section_set_id whose sections reference declared profiles.",
                "Profiles are normalized counter-clockwise point loops; use 3..=32 non-self-intersecting points and resample_count 8..=256.",
                "Use revolve only with non-negative profile x coordinates; use sweep with 2..=32 non-zero, non-self-intersecting path points and valid cap flags.",
                "For generic_visual_exterior organic, character, animal or plant subjects, prefer capsule, box, cylinder and bounded bevel branches over profile/loft. If a loft is necessary, every profile referenced by one section_set must use one identical resample_count (choose 16 or 24), section positions must be strictly increasing, and cap_policy must be start/none/end."
            ],
            "visual_quality_priorities": [
                "Spend geometry budget first on the subject's macro silhouette, proportions, negative space and identity-bearing parts.",
                "Then express meso structure with separated shells, panels, recesses, rings, repetitions and controlled bevels; do not fill the budget with unrelated primitives.",
                "Use micro appearance features only when the SubjectProfile or user brief supports them; bind them to real parts/material zones so Appearance Compiler can produce reviewed PBR layers.",
                "Do not claim hidden, functional, manufacturing or physically correct structure from a single view; mark it inferred/hidden and keep the exterior visually coherent."
            ],
            "minimum_valid_branch": {
                "nodes": [
                    {"kind":"box","node_id":"node_subject_base","size":[400.0,240.0,180.0],"position":[0.0,0.0,0.0]},
                    {"kind":"part","node_id":"node_subject_part","input_node_id":"node_subject_base","part_id":"part_subject_body","role":"primary_subject_body"},
                    {"kind":"material_zone","node_id":"node_subject_zone","input_node_id":"node_subject_part","zone_id":"zone_subject_body","material_id":"mat_subject_body"}
                ],
                "output":{"output_id":"output_subject_body","node_id":"node_subject_zone"}
            }
        });
        json!({
            "schema_version": "UniversalAuthorContext@1",
            "request": self.request,
            "visual_evidence_graph": self.visual_evidence_graph,
            // Keep a short, copy-safe ledger next to the large sealed request.
            // DeepSeek must bind observed feature regions to these exact IDs;
            // aliases such as image_1/reference_1 are not product evidence.
            "reference_evidence_ledger": reference_evidence_ledger,
            // The hash in UniversalAuthorRequest binds the exact registry;
            // the read-only manifest below gives DeepSeek enough information
            // to choose a real capability instead of guessing an ID. Rust
            // still validates the returned RepresentationPlan against this
            // same registry, so this projection never grants execution.
            "capability_manifest_sha256": self.request.capability_manifest_sha256,
            "capability_manifest": representation_capability_manifest(),
            "geometry_authoring_playbook": geometry_authoring_playbook,
            "rules": [
                "Identify the actual subject without converting it to a known template.",
                "Return SubjectProfile@1, VisualFeatureContract@1 and RepresentationPlan@1 in author_universal_asset.",
                "Cross-contract checklist before submitting: SubjectProfile.parts is the exact closed part set of part_id values; do not mention a part_id anywhere unless it is in that array. SubjectProfile.features is a flat array of feature_id, part_id, level and description only: feature.part_id must copy one declared SubjectProfile.parts[].part_id byte-for-byte, feature_id must be unique, and SubjectProfile.features must never contain affected_part_ids, covered_feature_ids or nested VFC requirement objects. A mirrored concept is still two rows with two IDs: use suffixes such as feat_ear_shape__part_ear_left and feat_ear_shape__part_ear_right; never reuse one feature_id for left/right, paired eyes, limbs or repeated parts. For the smallest valid single-part subject, use one declared part and exactly three distinct features at levels macro, meso and micro, all pointing to that part. VisualFeatureContract.requirements must contain exactly one requirement for every SubjectProfile.features[].feature_id, with no extra or missing feature IDs; every affected_part_ids entry must be a declared part_id. RepresentationPlan.parts must contain exactly one row for every SubjectProfile.parts[].part_id, with no extra or missing part IDs; every covered_feature_ids entry must be a declared feature whose affected_part_ids includes that same part_id. Copy IDs exactly; attach inner, hidden or uncertain visual detail to an existing visible parent part instead of inventing a new part. Prefer one primary feature per visible part when the decomposition does not require finer mapping.",
                "Category is open text. Domain Packs are optional knowledge hints only.",
                "Only code-owned capability IDs in the exact manifest may be selected.",
                "For hard-surface parts use procedural.generic_hard_surface_v1; for any other visible non-functional exterior part use procedural.generic_visual_exterior_v1. This category-open capability is the default executable proxy for organic, character, animal, plant, furniture, building and unknown subjects when specialist deformable/mesh_seed is unavailable; do not return quality_limited only for that absence. Mark soft, back-side or unobserved micro detail as inferred/uncertain and explain the proxy boundary in part rationale. This capability does not require a visual_exterior category tag: keep the real subject identity in category/category_tags and let Rust validate the capability. Distinct procedural parts may combine both capabilities in one program; when mixed, set domain=generic_visual_exterior and keep each capability on the part it actually represents. Do not downgrade a non-robot subject to an arm or C111 template.",
                "ForgeVisualGeometryProgram@2 materials contain only material_id and base_material_id. GeometryProgramBudget@1 must contain exactly schema_version, max_profiles, max_section_sets, max_nodes, max_parts, max_materials, max_outputs, max_operations and triangle_budget; never add texture or target-triangle fields.",
                "Describe appearance semantically in SubjectProfile.materials.appearance_traits and feature descriptions. For skin, fur, fabric, wood, bark, foliage, stone, concrete or clay, preserve those words exactly enough for Rust Appearance Compiler to choose its reviewed non-mechanical PBR token; do not substitute metal, glass or rubber merely because the object is unfamiliar. For reflective, emissive, rubber or glass parts, keep those explicit material semantics on the affected part only.",
                "Return limitation only when no reviewed proxy can express the requested visible exterior, evidence is insufficient, the Provider is unavailable, or the target is contradictory; limitation must not include geometry.",
                "Observed features require sealed evidence; hidden or inferred content must keep that status.",
                "Every VisualFeatureContract requirement with evidence_status=observed must contain at least one evidence_regions entry; unproven or occluded content must not be observed.",
                "VisualFeatureContract evidence_regions.evidence_id must be copied byte-for-byte from request.reference_inputs[].evidence_id or the reference_evidence_ledger; never invent image_1, reference_1 or another alias.",
                "If the request has no reference_inputs, do not mark any feature observed and keep evidence_regions empty.",
                "Geometry kind is a closed Rust vocabulary: box, cylinder, capsule, wedge, extrude, revolve, loft, sweep, mirror, array, radial_array, bevel_approx, surface_panel, groove, shell, lattice_deform, local_mesh_patch, union, subtract, part and material_zone. There is no sphere, ellipsoid, torus, mesh, script or arbitrary geometry kind; use capsule, revolve or bounded box/bevel branches for rounded visible masses."
            ]
        })
    }
}

/// The read-only vision panel still returns the legacy claim envelope because
/// it does not know the Provider-authored SubjectProfile yet. Once the
/// universal author has produced that profile, Rust can compile the legacy
/// claims into the category-open graph without allowing the client or vision
/// model to invent feature IDs. Claims are matched by macro/meso/micro level;
/// every authored feature receives the same-level evidence, and unrepresented
/// features remain hidden rather than being marked observed.
fn project_legacy_visual_evidence_graph(
    value: &serde_json::Value,
    request: &UniversalAuthorRequest,
    profile: &SubjectProfile,
    evidence: &[ReferenceEvidence],
) -> Result<VisualEvidenceGraphV2, UniversalAuthorContextError> {
    let legacy: VisualEvidenceGraph =
        serde_json::from_value(value.clone()).map_err(|error| UniversalAuthorContextError {
            code: "VISUAL_EVIDENCE_GRAPH_LEGACY_INVALID".into(),
            message: format!("VisualEvidenceGraph@1 could not be decoded: {error}"),
        })?;
    let request_evidence_ids = request
        .reference_inputs
        .iter()
        .map(|reference| reference.evidence_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let sealed_evidence_ids = evidence
        .iter()
        .map(|reference| reference.evidence_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if legacy.project_id != request.project_id {
        return Err(UniversalAuthorContextError {
            code: "VISUAL_EVIDENCE_GRAPH_PROJECT_MISMATCH".into(),
            message: "Legacy visual evidence graph belongs to another Project.".into(),
        });
    }
    if legacy.claims.iter().any(|claim| {
        claim.source_evidence_ids.iter().any(|evidence_id| {
            !request_evidence_ids.contains(evidence_id.as_str())
                || !sealed_evidence_ids.contains(evidence_id.as_str())
        })
    }) {
        return Err(UniversalAuthorContextError {
            code: "VISUAL_EVIDENCE_GRAPH_EVIDENCE_MISMATCH".into(),
            message: "Legacy visual evidence graph references evidence outside the sealed request."
                .into(),
        });
    }

    let claims = profile
        .features
        .iter()
        .enumerate()
        .map(|(index, feature)| {
            let source = legacy
                .claims
                .iter()
                .find(|claim| match (claim.level, feature.level) {
                    (VisualDetailLevel::Macro, forgecad_core::VisualFeatureLevel::Macro)
                    | (VisualDetailLevel::Meso, forgecad_core::VisualFeatureLevel::Meso)
                    | (VisualDetailLevel::Micro, forgecad_core::VisualFeatureLevel::Micro) => true,
                    _ => false,
                });
            let (status, evidence_regions) = match source {
                Some(claim) if claim.status == VisualClaimStatus::Observed => {
                    let regions = claim
                        .source_evidence_ids
                        .iter()
                        .map(|evidence_id| VisualFeatureEvidenceRegion {
                            evidence_id: evidence_id.clone(),
                            view_id: claim.source_view_id.clone(),
                            region_per_mille: claim.source_region.map(|region| {
                                [region.left, region.top, region.right, region.bottom]
                            }),
                        })
                        .collect::<Vec<_>>();
                    if regions.is_empty() {
                        (EvidenceStatus::Conflicting, Vec::new())
                    } else {
                        (EvidenceStatus::Observed, regions)
                    }
                }
                Some(claim) if matches!(claim.status, VisualClaimStatus::Inferred) => {
                    (EvidenceStatus::Inferred, Vec::new())
                }
                Some(_) | None => (EvidenceStatus::Hidden, Vec::new()),
            };
            UniversalEvidenceClaim {
                claim_id: format!(
                    "v2claim_{}",
                    &sha256_hex(format!("{}:{index}", feature.feature_id).as_bytes())[..24]
                ),
                feature_id: feature.feature_id.clone(),
                status,
                evidence_regions,
                description: source
                    .map(|claim| claim.description.clone())
                    .unwrap_or_else(|| feature.description.clone()),
            }
        })
        .collect();

    Ok(VisualEvidenceGraphV2 {
        schema_version: "VisualEvidenceGraph@2".into(),
        graph_id: format!(
            "vegraph_v2_{}",
            &sha256_hex(
                format!(
                    "{}:{}",
                    legacy.graph_id,
                    semantic_sha256(profile).map_err(|error| UniversalAuthorContextError {
                        code: error.code().into(),
                        message: error.to_string(),
                    })?
                )
                .as_bytes(),
            )[..24]
        ),
        universal_request_sha256: semantic_sha256(request).map_err(|error| {
            UniversalAuthorContextError {
                code: error.code().into(),
                message: error.to_string(),
            }
        })?,
        subject_profile_sha256: semantic_sha256(profile).map_err(|error| {
            UniversalAuthorContextError {
                code: error.code().into(),
                message: error.to_string(),
            }
        })?,
        claims,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_core::{representation_capability_manifest_sha256, UniversalInputMode};
    use serde_json::json;

    fn text_request() -> UniversalAuthorRequest {
        UniversalAuthorRequest {
            schema_version: "UniversalAuthorRequest@1".into(),
            request_id: "u002_context_projection_test".into(),
            project_id: "project_context_projection_test".into(),
            turn_id: "turn_context_projection_test".into(),
            instruction: "生成一个银白色科幻装甲外壳".into(),
            input_mode: UniversalInputMode::Text,
            reference_inputs: Vec::new(),
            active_asset: None,
            selection: Default::default(),
            locks: Default::default(),
            capability_manifest_sha256: representation_capability_manifest_sha256().unwrap(),
        }
    }

    #[test]
    fn provider_projection_includes_exact_capability_manifest_and_unavailable_branches() {
        let request = text_request();
        let context = ValidatedUniversalAuthorContext::new(request.clone(), &[], None).unwrap();
        let projection = context.provider_projection();

        assert_eq!(
            projection
                .get("capability_manifest_sha256")
                .and_then(Value::as_str),
            Some(request.capability_manifest_sha256.as_str())
        );
        assert_eq!(
            projection
                .get("reference_evidence_ledger")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        assert!(projection
            .get("rules")
            .and_then(Value::as_array)
            .is_some_and(|rules| rules.iter().any(|rule| {
                rule.as_str()
                    .is_some_and(|text| text.contains("byte-for-byte"))
            })));
        assert!(projection
            .get("rules")
            .and_then(Value::as_array)
            .is_some_and(|rules| rules.iter().any(|rule| {
                rule.as_str()
                    .is_some_and(|text| text.contains("exact closed part set"))
            })));
        assert!(projection
            .get("rules")
            .and_then(Value::as_array)
            .is_some_and(|rules| rules.iter().any(|rule| {
                rule.as_str()
                    .is_some_and(|text| text.contains("no sphere"))
            })));
        assert_eq!(
            projection
                .pointer("/capability_manifest/schema_version")
                .and_then(Value::as_str),
            Some("RepresentationCapabilityManifest@1")
        );
        let capabilities = projection
            .pointer("/capability_manifest/capabilities")
            .and_then(Value::as_array)
            .expect("provider projection must include the code-owned registry");
        assert!(capabilities.iter().any(|capability| {
            capability.get("capability_id").and_then(Value::as_str)
                == Some("procedural.generic_hard_surface_v1")
                && capability.get("availability").and_then(Value::as_str) == Some("available")
        }));
        assert!(capabilities.iter().any(|capability| {
            capability.get("capability_id").and_then(Value::as_str) == Some("mesh_seed.generic_v1")
                && capability.get("availability").and_then(Value::as_str) == Some("unavailable")
        }));
        assert_eq!(
            projection
                .pointer("/geometry_authoring_playbook/minimum_valid_branch/output/node_id")
                .and_then(Value::as_str),
            Some("node_subject_zone")
        );
        assert!(projection
            .pointer("/geometry_authoring_playbook/graph_rules")
            .and_then(Value::as_array)
            .is_some_and(|rules| rules.iter().any(|rule| {
                rule.as_str()
                    .is_some_and(|text| text.contains("output graph must be disjoint"))
            })));
        assert!(projection
            .pointer("/geometry_authoring_playbook/canonical_chains")
            .and_then(Value::as_array)
            .is_some_and(|chains| chains.iter().any(|chain| {
                chain
                    .as_str()
                    .is_some_and(|text| text.contains("profile + sweep"))
            })));
        assert!(projection
            .pointer("/geometry_authoring_playbook/visual_quality_priorities")
            .and_then(Value::as_array)
            .is_some_and(|priorities| priorities.iter().any(|priority| {
                priority
                    .as_str()
                    .is_some_and(|text| text.contains("macro silhouette"))
            })));
    }

    #[test]
    fn provider_projection_keeps_request_and_graph_as_read_only_context() {
        let request = text_request();
        let graph = json!({
            "schema_version": "VisualEvidenceGraph@2",
            "graph_id": "graph_context_projection_test",
            "universal_request_sha256": semantic_sha256(&request).unwrap(),
            "subject_profile_sha256": "a".repeat(64),
            "claims": []
        });
        let context =
            ValidatedUniversalAuthorContext::new(request.clone(), &[], Some(graph.clone()))
                .unwrap();
        let projection = context.provider_projection();

        assert_eq!(
            projection.get("request"),
            Some(&serde_json::to_value(request).unwrap())
        );
        assert_eq!(projection.get("visual_evidence_graph"), Some(&graph));
        assert!(projection.get("provider_key").is_none());
        assert!(projection.get("database_path").is_none());
        assert!(projection.get("object_store_root").is_none());
    }

    #[test]
    fn legacy_visual_graph_is_projected_after_profile_authoring_without_marking_hidden_features_observed(
    ) {
        let mut request = text_request();
        request.input_mode = forgecad_core::UniversalInputMode::SingleImage;
        request.reference_inputs = vec![forgecad_core::UniversalReferenceInput {
            evidence_id: "refevid_projection_test".into(),
            evidence_sha256: "a".repeat(64),
            role: "primary_silhouette".into(),
            view_hint: Some("front".into()),
        }];
        let profile: SubjectProfile = serde_json::from_value(json!({
            "schema_version":"SubjectProfile@1",
            "profile_id":"subject_projection_test",
            "request_sha256":semantic_sha256(&request).unwrap(),
            "identity_label":"测试对象",
            "category":"open category subject",
            "category_tags":["hard_surface"],
            "silhouette":"bounded silhouette",
            "negative_space":"visible recess",
            "pose":"static",
            "visible_views":["front"],
            "occlusions":["rear hidden"],
            "uncertainties":["rear material"],
            "parts":[{"part_id":"part_body","label":"主体","semantic_role":"primary_mass","traits":["hard_surface"],"uncertainty_bps":1000}],
            "features":[
                {"feature_id":"feature_macro","part_id":"part_body","level":"macro","description":"主体轮廓"},
                {"feature_id":"feature_meso","part_id":"part_body","level":"meso","description":"中频分件"},
                {"feature_id":"feature_micro","part_id":"part_body","level":"micro","description":"表面细节"}
            ],
            "materials":[]
        })).unwrap();
        let legacy_graph = json!({
            "schema_version":"VisualEvidenceGraph@1",
            "graph_id":"legacy_projection_graph",
            "request_id":"legacy_request",
            "request_sha256":"b".repeat(64),
            "project_id":request.project_id,
            "domain_pack_id":"pack_unclassified",
            "provider":{"provider_id":"qwen","model_id":"qwen-test","provider_response_sha256":"c".repeat(64),"analyzed_at":"test"},
            "claims":[]
        });
        let context =
            ValidatedUniversalAuthorContext::new(request.clone(), &[], Some(legacy_graph))
                .unwrap_err();
        assert_eq!(context.code, "UNIVERSAL_REFERENCE_NOT_FOUND");

        let request_without_evidence = forgecad_core::UniversalAuthorRequest {
            reference_inputs: Vec::new(),
            input_mode: forgecad_core::UniversalInputMode::Text,
            ..request
        };
        let graph = json!({
            "schema_version":"VisualEvidenceGraph@1",
            "graph_id":"legacy_projection_graph",
            "request_id":"legacy_request",
            "request_sha256":"b".repeat(64),
            "project_id":request_without_evidence.project_id,
            "domain_pack_id":"pack_unclassified",
            "provider":{"provider_id":"qwen","model_id":"qwen-test","provider_response_sha256":"c".repeat(64),"analyzed_at":"test"},
            "claims":[]
        });
        let context = ValidatedUniversalAuthorContext::new(
            request_without_evidence.clone(),
            &[],
            Some(graph),
        )
        .unwrap();
        let projected = context
            .visual_evidence_graph_v2_for_profile(&profile)
            .unwrap()
            .unwrap();
        assert_eq!(projected.schema_version, "VisualEvidenceGraph@2");
        assert_eq!(
            projected.universal_request_sha256,
            semantic_sha256(&request_without_evidence).unwrap()
        );
        assert!(projected.claims.iter().all(|claim| {
            claim.status == EvidenceStatus::Hidden && claim.evidence_regions.is_empty()
        }));
    }
}
