use forgecad_core::{
    evaluate_native_v003_gate_profile_v2, native_v003_gate_profile_sha256,
    NativeGateEvidenceSource, NativeGenerationGateBinding, NativeGenerationGateEvaluation,
    NativeGenerationGateEvidence, VerificationOutcome,
    NATIVE_GENERATION_GATE_EVALUATION_SCHEMA_VERSION,
    NATIVE_GENERATION_GATE_EVIDENCE_SCHEMA_VERSION, NATIVE_V003_GATE_IDS,
    NATIVE_V003_GATE_PROFILE_ID, NATIVE_V003_GATE_PROFILE_SHA256, NATIVE_V003_GATE_PROFILE_VERSION,
};

fn sha(ch: char) -> String {
    std::iter::repeat_n(ch, 64).collect()
}

fn binding() -> NativeGenerationGateBinding {
    NativeGenerationGateBinding {
        gate_report_id: "gate_v2_primary".into(),
        attempt_id: "attempt_v2_primary".into(),
        glb_sha256: sha('a'),
        compile_readback_id: "readback_v2_primary".into(),
        render_fingerprint: "render_v2_primary".into(),
        summary: "Rust-owned V003 v2 gate facts.".into(),
    }
}

fn source_for(gate_id: &str) -> NativeGateEvidenceSource {
    match gate_id {
        "has_triangles"
        | "has_meshes"
        | "closed_manifold"
        | "surface_provenance_present"
        | "glb_hash_verified" => NativeGateEvidenceSource::RestrictedGeometryGlbReadback,
        "four_views_read_back" | "r006_same_source_views" => {
            NativeGateEvidenceSource::DeterministicConceptRenderReadback
        }
        "brief_coverage" => NativeGateEvidenceSource::BriefCoverageResolver,
        "semantic_proportion_bound" => NativeGateEvidenceSource::SemanticProportionResolver,
        "domain_role_coverage" => NativeGateEvidenceSource::DomainRoleResolver,
        "material_texture_provenance" => NativeGateEvidenceSource::ProductionMaterialReadback,
        "editability_evidence" => NativeGateEvidenceSource::RecipeAssemblyReadback,
        "generation_source_marked" => NativeGateEvidenceSource::GenerationExecutionTrace,
        _ => panic!("test fixture requested an unknown gate"),
    }
}

fn hash_for(source: NativeGateEvidenceSource) -> String {
    let ch = match source {
        NativeGateEvidenceSource::RestrictedGeometryGlbReadback => '1',
        NativeGateEvidenceSource::DeterministicConceptRenderReadback => '2',
        NativeGateEvidenceSource::BriefCoverageResolver => '3',
        NativeGateEvidenceSource::SemanticProportionResolver => '4',
        NativeGateEvidenceSource::DomainRoleResolver => '5',
        NativeGateEvidenceSource::ProductionMaterialReadback => '6',
        NativeGateEvidenceSource::RecipeAssemblyReadback => '7',
        NativeGateEvidenceSource::GenerationExecutionTrace => '8',
    };
    sha(ch)
}

fn passing_evidence() -> Vec<NativeGenerationGateEvidence> {
    NATIVE_V003_GATE_IDS
        .into_iter()
        .map(|gate_id| {
            let source = source_for(gate_id);
            NativeGenerationGateEvidence {
                schema_version: NATIVE_GENERATION_GATE_EVIDENCE_SCHEMA_VERSION.into(),
                gate_id: gate_id.into(),
                outcome: VerificationOutcome::Pass,
                source,
                source_sha256: hash_for(source),
                summary: format!("{gate_id} was verified by Rust-owned evidence."),
            }
        })
        .collect()
}

fn evaluate(evidence: Vec<NativeGenerationGateEvidence>) -> NativeGenerationGateEvaluation {
    evaluate_native_v003_gate_profile_v2(binding(), evidence).unwrap()
}

#[test]
fn profile_v2_id_version_hash_and_exact_thirteen_gate_order_are_frozen() {
    assert_eq!(
        native_v003_gate_profile_sha256(),
        NATIVE_V003_GATE_PROFILE_SHA256
    );
    let evaluation = evaluate(passing_evidence());
    assert_eq!(
        evaluation.schema_version,
        NATIVE_GENERATION_GATE_EVALUATION_SCHEMA_VERSION
    );
    assert_eq!(evaluation.gate_profile_id, NATIVE_V003_GATE_PROFILE_ID);
    assert_eq!(
        evaluation.gate_profile_version,
        NATIVE_V003_GATE_PROFILE_VERSION
    );
    assert_eq!(
        evaluation.gate_profile_sha256,
        NATIVE_V003_GATE_PROFILE_SHA256
    );
    assert_eq!(
        evaluation.report.gate_profile_version,
        NATIVE_V003_GATE_PROFILE_VERSION
    );
    assert_eq!(
        evaluation
            .report
            .checks
            .iter()
            .map(|check| check.gate_id.as_str())
            .collect::<Vec<_>>(),
        NATIVE_V003_GATE_IDS
    );
    assert!(evaluation.report.is_passed());
}

#[test]
fn missing_unknown_and_duplicate_gate_evidence_are_rejected() {
    let mut missing = passing_evidence();
    missing.pop();
    assert_eq!(
        evaluate_native_v003_gate_profile_v2(binding(), missing)
            .unwrap_err()
            .code(),
        "NATIVE_V003_GATE_MISSING"
    );

    let mut unknown = passing_evidence();
    unknown[0].gate_id = "human_visual_score".into();
    assert_eq!(
        evaluate_native_v003_gate_profile_v2(binding(), unknown)
            .unwrap_err()
            .code(),
        "NATIVE_V003_GATE_UNKNOWN"
    );

    let mut duplicate = passing_evidence();
    duplicate.push(duplicate[0].clone());
    assert_eq!(
        evaluate_native_v003_gate_profile_v2(binding(), duplicate)
            .unwrap_err()
            .code(),
        "NATIVE_V003_GATE_DUPLICATE"
    );
}

#[test]
fn gate_source_type_and_shared_source_fingerprint_contradictions_are_rejected() {
    let mut wrong_source = passing_evidence();
    wrong_source[0].source = NativeGateEvidenceSource::GenerationExecutionTrace;
    wrong_source[0].source_sha256 = hash_for(NativeGateEvidenceSource::GenerationExecutionTrace);
    assert_eq!(
        evaluate_native_v003_gate_profile_v2(binding(), wrong_source)
            .unwrap_err()
            .code(),
        "NATIVE_V003_GATE_SOURCE_CONTRADICTION"
    );

    let mut conflicting_hash = passing_evidence();
    let mesh = conflicting_hash
        .iter_mut()
        .find(|item| item.gate_id == "has_meshes")
        .unwrap();
    mesh.source_sha256 = sha('f');
    assert_eq!(
        evaluate_native_v003_gate_profile_v2(binding(), conflicting_hash)
            .unwrap_err()
            .code(),
        "NATIVE_V003_GATE_SOURCE_CONTRADICTION"
    );
}

#[test]
fn only_code_owned_concrete_failure_rules_can_authorize_repair() {
    let mut evidence = passing_evidence();
    evidence
        .iter_mut()
        .find(|item| item.gate_id == "closed_manifold")
        .unwrap()
        .outcome = VerificationOutcome::Fail;
    let repairable = evaluate(evidence);
    assert!(repairable.report.allows_repair());
    assert_eq!(
        repairable.report.repairable_gate_ids(),
        ["closed_manifold"].into_iter().collect()
    );

    let mut nonrepairable = passing_evidence();
    nonrepairable
        .iter_mut()
        .find(|item| item.gate_id == "brief_coverage")
        .unwrap()
        .outcome = VerificationOutcome::Fail;
    let nonrepairable = evaluate(nonrepairable);
    assert!(!nonrepairable.report.allows_repair());
    assert!(nonrepairable.report.repairable_gate_ids().is_empty());
}

#[test]
fn undetermined_is_always_blocking_and_never_repairable() {
    let mut evidence = passing_evidence();
    evidence
        .iter_mut()
        .find(|item| item.gate_id == "closed_manifold")
        .unwrap()
        .outcome = VerificationOutcome::Undetermined;
    let evaluation = evaluate(evidence);
    assert!(!evaluation.report.is_passed());
    assert!(evaluation.report.has_undetermined());
    assert!(!evaluation.report.allows_repair());
    assert!(evaluation.report.repairable_gate_ids().is_empty());
}

#[test]
fn tampered_profile_stamp_or_report_cannot_be_revalidated() {
    let mut evaluation = evaluate(passing_evidence());
    evaluation.gate_profile_sha256 = sha('0');
    assert_eq!(
        evaluation.validate().unwrap_err().code(),
        "NATIVE_V003_GATE_PROFILE_INVALID"
    );

    let mut evaluation = evaluate(passing_evidence());
    evaluation.report.checks[0].repairable = true;
    assert_eq!(
        evaluation.validate().unwrap_err().code(),
        "GENERATION_GATE_REPAIRABILITY_INVALID"
    );
}
