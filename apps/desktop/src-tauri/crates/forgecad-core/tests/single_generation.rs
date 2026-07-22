use forgecad_core::{
    GenerationAttemptKind, GenerationGateCheck, GenerationGateReport, GenerationPreview,
    RepairAttempt, SingleGenerationAttempt, SingleGenerationSession, SingleGenerationSessionState,
    VerificationOutcome, GENERATION_GATE_REPORT_SCHEMA_VERSION, REPAIR_ATTEMPT_SCHEMA_VERSION,
    SINGLE_GENERATION_ATTEMPT_SCHEMA_VERSION,
};

fn sha(label: &str) -> String {
    let mut value = String::with_capacity(64);
    while value.len() < 64 {
        value.push_str(label);
    }
    value[..64].to_string()
}

fn initial() -> SingleGenerationAttempt {
    SingleGenerationAttempt {
        schema_version: SINGLE_GENERATION_ATTEMPT_SCHEMA_VERSION.into(),
        attempt_id: "attempt_initial".into(),
        turn_id: "turn_single".into(),
        project_id: "project_single".into(),
        attempt_kind: GenerationAttemptKind::Initial,
        parent_attempt_id: None,
        brief_sha256: sha("a"),
        domain_pack_id: "pack_robotic_arm_concept".into(),
        domain_pack_sha256: sha("b"),
        core_recipe_or_profile_sha256: sha("c"),
        runtime_manifest_sha256: sha("d"),
        shape_program_sha256: sha("e"),
        recipe_provenance_sha256: Some(sha("f")),
    }
}

fn report(attempt_id: &str, checks: Vec<GenerationGateCheck>) -> GenerationGateReport {
    GenerationGateReport {
        schema_version: GENERATION_GATE_REPORT_SCHEMA_VERSION.into(),
        gate_report_id: format!("gate_{attempt_id}"),
        attempt_id: attempt_id.into(),
        glb_sha256: sha("1"),
        compile_readback_id: "readback_primary".into(),
        render_fingerprint: "render_primary".into(),
        gate_profile_version: "v003_gate_profile_v1".into(),
        checks,
        summary: "Hard-gate evidence.".into(),
    }
}

fn check(id: &str, outcome: VerificationOutcome, repairable: bool) -> GenerationGateCheck {
    GenerationGateCheck {
        gate_id: id.into(),
        outcome,
        repairable,
        summary: format!("{id} check"),
    }
}

fn preview() -> GenerationPreview {
    GenerationPreview {
        preview_id: "preview_single".into(),
        artifact_sha256: sha("1"),
        artifact_profile_id: "interactive_preview".into(),
        expires_at: Some("2026-07-18T12:00:00Z".into()),
    }
}

fn repair(
    parent: &SingleGenerationAttempt,
    report: &GenerationGateReport,
    attempt_id: &str,
    repaired_gate_ids: Vec<&str>,
) -> RepairAttempt {
    let mut repaired = parent.clone();
    repaired.attempt_id = attempt_id.into();
    repaired.attempt_kind = GenerationAttemptKind::Repair;
    repaired.parent_attempt_id = Some(parent.attempt_id.clone());
    repaired.shape_program_sha256 = sha("7");
    RepairAttempt {
        schema_version: REPAIR_ATTEMPT_SCHEMA_VERSION.into(),
        repair_id: format!("repair_{attempt_id}"),
        parent_attempt_id: parent.attempt_id.clone(),
        parent_gate_report_id: report.gate_report_id.clone(),
        repaired_gate_ids: repaired_gate_ids.into_iter().map(str::to_owned).collect(),
        repaired_attempt: repaired,
    }
}

#[test]
fn passing_every_hard_gate_yields_one_transient_preview_not_a_version() {
    let initial = initial();
    let mut session = SingleGenerationSession::begin(initial.clone()).unwrap();
    let report = report(
        &initial.attempt_id,
        vec![
            check("scope", VerificationOutcome::Pass, false),
            check("compile_readback", VerificationOutcome::Pass, false),
            check("render", VerificationOutcome::Pass, false),
        ],
    );

    session
        .record_gate_report(
            report.clone(),
            "decision_single".into(),
            "Ready for review.".into(),
            Some(preview()),
        )
        .unwrap();

    let SingleGenerationSessionState::ReadyForPreview(decision) = session.state() else {
        panic!("expected ready preview")
    };
    assert_eq!(decision.attempt_id, initial.attempt_id);
    assert_eq!(decision.gate_report_id, report.gate_report_id);
    assert!(decision.preview.is_some());
    assert_eq!(session.repair_attempts().len(), 0);
    // Core's V003 session owns no repository write handle. It only returns
    // this transient descriptor; promotion remains a separate user confirm.
}

#[test]
fn undetermined_gate_is_never_aggregated_as_a_passing_result_or_repair() {
    let initial = initial();
    let mut session = SingleGenerationSession::begin(initial.clone()).unwrap();
    let report = report(
        &initial.attempt_id,
        vec![
            check("compile_readback", VerificationOutcome::Pass, false),
            check("render", VerificationOutcome::Undetermined, false),
        ],
    );
    assert!(!report.is_passed());
    assert!(!report.allows_repair());

    session
        .record_gate_report(
            report,
            "decision_undetermined".into(),
            "Unable to verify result.".into(),
            Some(preview()),
        )
        .unwrap();
    let SingleGenerationSessionState::Failed(decision) = session.state() else {
        panic!("undetermined must fail")
    };
    assert!(decision.preview.is_none());
    assert_eq!(
        decision.failure.as_ref().unwrap().code,
        "GENERATION_GATE_UNDETERMINED"
    );
}

#[test]
fn a_nonrepairable_failure_cannot_be_hidden_by_another_repairable_failure() {
    let initial = initial();
    let mut session = SingleGenerationSession::begin(initial.clone()).unwrap();
    let report = report(
        &initial.attempt_id,
        vec![
            check("material_coverage", VerificationOutcome::Fail, true),
            check("closed_manifold", VerificationOutcome::Fail, false),
        ],
    );
    assert!(!report.allows_repair());

    session
        .record_gate_report(
            report,
            "decision_nonrepairable".into(),
            "The hard gate failed.".into(),
            None,
        )
        .unwrap();
    let SingleGenerationSessionState::Failed(decision) = session.state() else {
        panic!("a nonrepairable failure must terminate the attempt")
    };
    assert!(decision.preview.is_none());
    assert_eq!(decision.failure.as_ref().unwrap().repair_attempts_used, 0);
}

#[test]
fn repairs_are_same_intent_limited_to_two_and_cannot_escape_failed_field_scope() {
    let initial = initial();
    let mut session = SingleGenerationSession::begin(initial.clone()).unwrap();
    let first_report = report(
        &initial.attempt_id,
        vec![check("material_coverage", VerificationOutcome::Fail, true)],
    );
    session
        .record_gate_report(
            first_report.clone(),
            "decision_unused_1".into(),
            "Repair needed.".into(),
            None,
        )
        .unwrap();
    assert!(matches!(
        session.state(),
        SingleGenerationSessionState::AwaitingRepair { .. }
    ));

    let invalid_scope = repair(
        &initial,
        &first_report,
        "attempt_repair_invalid",
        vec!["render"],
    );
    assert_eq!(
        session
            .apply_repair(invalid_scope, &first_report)
            .unwrap_err()
            .code(),
        "REPAIR_ATTEMPT_SCOPE_INVALID"
    );

    let first_repair = repair(
        &initial,
        &first_report,
        "attempt_repair_1",
        vec!["material_coverage"],
    );
    session.apply_repair(first_repair, &first_report).unwrap();
    let second_report = report(
        session.current_attempt().attempt_id.as_str(),
        vec![check("material_coverage", VerificationOutcome::Fail, true)],
    );
    session
        .record_gate_report(
            second_report.clone(),
            "decision_unused_2".into(),
            "Repair needed.".into(),
            None,
        )
        .unwrap();
    let second_repair = repair(
        session.current_attempt(),
        &second_report,
        "attempt_repair_2",
        vec!["material_coverage"],
    );
    session.apply_repair(second_repair, &second_report).unwrap();

    let third_report = report(
        session.current_attempt().attempt_id.as_str(),
        vec![check("material_coverage", VerificationOutcome::Fail, true)],
    );
    session
        .record_gate_report(
            third_report,
            "decision_failed".into(),
            "Could not complete the single result.".into(),
            None,
        )
        .unwrap();
    let SingleGenerationSessionState::Failed(decision) = session.state() else {
        panic!("two repairs is the hard limit")
    };
    assert_eq!(decision.failure.as_ref().unwrap().repair_attempts_used, 2);
    assert_eq!(session.repair_attempts().len(), 2);
}

#[test]
fn repair_rejects_brief_or_recipe_intent_drift() {
    let initial = initial();
    let mut session = SingleGenerationSession::begin(initial.clone()).unwrap();
    let gate = report(
        &initial.attempt_id,
        vec![check("render", VerificationOutcome::Fail, true)],
    );
    session
        .record_gate_report(
            gate.clone(),
            "decision_unused".into(),
            "Repair needed.".into(),
            None,
        )
        .unwrap();
    let mut repair = repair(&initial, &gate, "attempt_drift", vec!["render"]);
    repair.repaired_attempt.brief_sha256 = sha("0");
    assert_eq!(
        session.apply_repair(repair, &gate).unwrap_err().code(),
        "REPAIR_ATTEMPT_INTENT_DRIFT"
    );
}

#[test]
fn third_repair_is_rejected_after_the_two_repair_budget_is_consumed() {
    let initial = initial();
    let mut session = SingleGenerationSession::begin(initial.clone()).unwrap();
    let first_gate = report(
        &initial.attempt_id,
        vec![check("render", VerificationOutcome::Fail, true)],
    );
    session
        .record_gate_report(
            first_gate.clone(),
            "decision_1".into(),
            "Repair.".into(),
            None,
        )
        .unwrap();
    session
        .apply_repair(
            repair(
                &initial,
                &first_gate,
                "attempt_repair_first",
                vec!["render"],
            ),
            &first_gate,
        )
        .unwrap();

    let second_gate = report(
        session.current_attempt().attempt_id.as_str(),
        vec![check("render", VerificationOutcome::Fail, true)],
    );
    session
        .record_gate_report(
            second_gate.clone(),
            "decision_2".into(),
            "Repair.".into(),
            None,
        )
        .unwrap();
    let second_parent = session.current_attempt().clone();
    session
        .apply_repair(
            repair(
                &second_parent,
                &second_gate,
                "attempt_repair_second",
                vec!["render"],
            ),
            &second_gate,
        )
        .unwrap();

    // A subsequent failed report terminally fails the session instead of
    // opening a third repair slot; it also cannot expose a preview.
    let exhausted_gate = report(
        session.current_attempt().attempt_id.as_str(),
        vec![check("render", VerificationOutcome::Fail, true)],
    );
    session
        .record_gate_report(
            exhausted_gate,
            "decision_terminal".into(),
            "Repair budget exhausted.".into(),
            None,
        )
        .unwrap();
    assert!(matches!(
        session.state(),
        SingleGenerationSessionState::Failed(_)
    ));
    let rejected = repair(
        session.current_attempt(),
        &first_gate,
        "attempt_repair_third",
        vec!["render"],
    );
    assert_eq!(
        session
            .apply_repair(rejected, &first_gate)
            .unwrap_err()
            .code(),
        "REPAIR_ATTEMPT_STATE_INVALID"
    );
}

#[test]
fn cancellation_has_no_preview_and_is_terminal() {
    let initial = initial();
    let mut session = SingleGenerationSession::begin(initial).unwrap();
    session
        .cancel(
            "decision_cancelled".into(),
            "gate_cancelled".into(),
            "Generation cancelled.".into(),
            forgecad_core::GenerationCancel {
                code: "USER_CANCELLED".into(),
                message: "The user cancelled this generation.".into(),
            },
        )
        .unwrap();
    let SingleGenerationSessionState::Cancelled(decision) = session.state() else {
        panic!("expected cancelled")
    };
    assert!(decision.preview.is_none());
    assert!(decision.cancel.is_some());
    assert_eq!(
        session
            .cancel(
                "decision_again".into(),
                "gate_again".into(),
                "Again.".into(),
                forgecad_core::GenerationCancel {
                    code: "USER_CANCELLED".into(),
                    message: "Again.".into()
                }
            )
            .unwrap_err()
            .code(),
        "SINGLE_GENERATION_SESSION_TERMINAL"
    );
}
