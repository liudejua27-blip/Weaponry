//! Deterministic PV004 build ledger and eight-view convergence contract.
//!
//! This module does not render or compile geometry. It accepts only hashes and
//! readback facts produced by those existing restricted runtimes, then decides
//! whether one ForgeVisualProgram revision is eligible for a single preview.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{semantic_sha256, CoreError, CoreResult};

pub const DESIGN_BUILD_LEDGER_SCHEMA_VERSION: &str = "DesignBuildLedger@1";
pub const VISUAL_CONVERGENCE_INPUT_SCHEMA_VERSION: &str = "VisualConvergenceInput@1";
pub const VISUAL_CONVERGENCE_REPORT_SCHEMA_VERSION: &str = "VisualConvergenceReport@1";
pub const MAX_VISUAL_REPAIR_ATTEMPTS: u8 = 2;
pub const REQUIRED_VISUAL_VIEW_IDS: [&str; 8] = [
    "iso",
    "front",
    "back",
    "left",
    "right",
    "top",
    "gripper_iso",
    "gripper_front",
];

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualBuildStage {
    Silhouette,
    Structure,
    Form,
    Material,
    Surface,
    Lighting,
    Optimization,
}

impl VisualBuildStage {
    pub const ORDERED: [Self; 7] = [
        Self::Silhouette,
        Self::Structure,
        Self::Form,
        Self::Material,
        Self::Surface,
        Self::Lighting,
        Self::Optimization,
    ];
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualBuildPass {
    pub stage: VisualBuildStage,
    pub input_sha256: String,
    pub output_sha256: String,
    pub completed: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DesignBuildLedger {
    pub schema_version: String,
    pub source_program_sha256: String,
    pub source_revision: u64,
    pub passes: Vec<VisualBuildPass>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualGlbReadbackEvidence {
    pub glb_sha256: String,
    pub shape_program_sha256: String,
    pub triangle_count: u64,
    pub primitive_count: u64,
    pub material_zone_count: u64,
    pub closed_manifold: bool,
    pub surface_provenance_present: bool,
    pub pbr_channels_complete: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualFixedViewEvidence {
    pub view_id: String,
    pub glb_sha256: String,
    pub renderer_id: String,
    pub image_sha256: String,
    pub readback_passed: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualDetailCoverage {
    pub macro_bound: u32,
    pub meso_bound: u32,
    pub micro_bound: u32,
    pub critical_unresolved: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualRepairEvidence {
    pub repair_number: u8,
    pub parent_program_sha256: String,
    pub result_program_sha256: String,
    pub changed_domains: Vec<String>,
    pub same_intent: bool,
}

/// Hash-only bridge from the separately validated multimodal reference
/// comparison into the deterministic convergence decision. The full report
/// remains a distinct typed artifact so Provider observations cannot mutate
/// this summary or directly decide pass/fail.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualReferenceConvergenceEvidence {
    pub comparison_input_sha256: String,
    pub comparison_report_sha256: String,
    pub passed: bool,
    pub failure_codes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualConvergenceInput {
    pub schema_version: String,
    pub ledger: DesignBuildLedger,
    pub readback: VisualGlbReadbackEvidence,
    pub fixed_views: Vec<VisualFixedViewEvidence>,
    pub detail_coverage: VisualDetailCoverage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_comparison: Option<VisualReferenceConvergenceEvidence>,
    pub repairs: Vec<VisualRepairEvidence>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualConvergenceReport {
    pub schema_version: String,
    pub report_sha256: String,
    pub source_program_sha256: String,
    pub source_revision: u64,
    pub glb_sha256: String,
    pub passed: bool,
    pub completed_stage_count: u8,
    pub fixed_view_count: u8,
    pub repair_attempt_count: u8,
    pub failure_codes: Vec<String>,
}

impl VisualConvergenceInput {
    pub fn evaluate(&self) -> CoreResult<VisualConvergenceReport> {
        self.validate_envelope()?;
        let mut failures = Vec::new();
        self.evaluate_ledger(&mut failures);
        self.evaluate_readback(&mut failures);
        self.evaluate_views(&mut failures);
        self.evaluate_details(&mut failures);
        self.evaluate_reference_comparison(&mut failures);
        self.evaluate_repairs(&mut failures);
        failures.sort();
        failures.dedup();

        let mut report = VisualConvergenceReport {
            schema_version: VISUAL_CONVERGENCE_REPORT_SCHEMA_VERSION.into(),
            report_sha256: String::new(),
            source_program_sha256: self.ledger.source_program_sha256.clone(),
            source_revision: self.ledger.source_revision,
            glb_sha256: self.readback.glb_sha256.clone(),
            passed: failures.is_empty(),
            completed_stage_count: self
                .ledger
                .passes
                .iter()
                .filter(|pass| pass.completed)
                .count() as u8,
            fixed_view_count: self.fixed_views.len() as u8,
            repair_attempt_count: self.repairs.len() as u8,
            failure_codes: failures,
        };
        report.report_sha256 = semantic_sha256(&report)?;
        Ok(report)
    }

    fn validate_envelope(&self) -> CoreResult<()> {
        if self.schema_version != VISUAL_CONVERGENCE_INPUT_SCHEMA_VERSION
            || self.ledger.schema_version != DESIGN_BUILD_LEDGER_SCHEMA_VERSION
            || self.ledger.source_revision == 0
            || !is_sha256(&self.ledger.source_program_sha256)
            || !is_sha256(&self.readback.glb_sha256)
            || !is_sha256(&self.readback.shape_program_sha256)
        {
            return Err(invalid("visual convergence envelope is invalid"));
        }
        Ok(())
    }

    fn evaluate_ledger(&self, failures: &mut Vec<String>) {
        if self.ledger.passes.len() != VisualBuildStage::ORDERED.len() {
            failures.push("BUILD_STAGE_SET_INCOMPLETE".into());
            return;
        }
        let mut expected_input = self.ledger.source_program_sha256.as_str();
        for (pass, expected_stage) in self.ledger.passes.iter().zip(VisualBuildStage::ORDERED) {
            if pass.stage != expected_stage {
                failures.push("BUILD_STAGE_ORDER_INVALID".into());
            }
            if !pass.completed || !is_sha256(&pass.output_sha256) {
                failures.push("BUILD_STAGE_INCOMPLETE".into());
            }
            if pass.input_sha256 != expected_input {
                failures.push("BUILD_STAGE_LINEAGE_MISMATCH".into());
            }
            expected_input = &pass.output_sha256;
        }
        if expected_input != self.readback.glb_sha256 {
            failures.push("BUILD_GLB_LINEAGE_MISMATCH".into());
        }
    }

    fn evaluate_readback(&self, failures: &mut Vec<String>) {
        if self.readback.triangle_count == 0 || self.readback.primitive_count == 0 {
            failures.push("GLB_GEOMETRY_EMPTY".into());
        }
        if self.readback.material_zone_count == 0 || !self.readback.pbr_channels_complete {
            failures.push("PBR_MATERIAL_INCOMPLETE".into());
        }
        if !self.readback.closed_manifold {
            failures.push("GLB_NOT_CLOSED_MANIFOLD".into());
        }
        if !self.readback.surface_provenance_present {
            failures.push("SURFACE_PROVENANCE_MISSING".into());
        }
    }

    fn evaluate_views(&self, failures: &mut Vec<String>) {
        let required = REQUIRED_VISUAL_VIEW_IDS
            .into_iter()
            .collect::<BTreeSet<_>>();
        let actual = self
            .fixed_views
            .iter()
            .map(|view| view.view_id.as_str())
            .collect::<BTreeSet<_>>();
        if self.fixed_views.len() != required.len() || actual != required {
            failures.push("EIGHT_VIEW_SET_INCOMPLETE".into());
        }
        let renderer_ids = self
            .fixed_views
            .iter()
            .map(|view| view.renderer_id.as_str())
            .collect::<BTreeSet<_>>();
        if renderer_ids.len() != 1
            || renderer_ids.contains("")
            || self.fixed_views.iter().any(|view| {
                view.glb_sha256 != self.readback.glb_sha256
                    || !is_sha256(&view.image_sha256)
                    || !view.readback_passed
            })
        {
            failures.push("EIGHT_VIEW_LINEAGE_INVALID".into());
        }
    }

    fn evaluate_details(&self, failures: &mut Vec<String>) {
        if self.detail_coverage.macro_bound == 0
            || self.detail_coverage.meso_bound == 0
            || self.detail_coverage.micro_bound == 0
        {
            failures.push("DETAIL_LEVEL_COVERAGE_INCOMPLETE".into());
        }
        if self.detail_coverage.critical_unresolved != 0 {
            failures.push("CRITICAL_DETAIL_UNRESOLVED".into());
        }
    }

    fn evaluate_repairs(&self, failures: &mut Vec<String>) {
        if self.repairs.len() > usize::from(MAX_VISUAL_REPAIR_ATTEMPTS) {
            failures.push("VISUAL_REPAIR_LIMIT_EXCEEDED".into());
            return;
        }
        let Some(first) = self.repairs.first() else {
            return;
        };
        let mut expected_parent = first.parent_program_sha256.as_str();
        if !is_sha256(expected_parent) {
            failures.push("VISUAL_REPAIR_LINEAGE_INVALID".into());
        }
        for (index, repair) in self.repairs.iter().enumerate() {
            if repair.repair_number != (index + 1) as u8
                || repair.parent_program_sha256 != expected_parent
                || !is_sha256(&repair.result_program_sha256)
                || repair.changed_domains.is_empty()
                || !repair.same_intent
            {
                failures.push("VISUAL_REPAIR_LINEAGE_INVALID".into());
            }
            expected_parent = &repair.result_program_sha256;
        }
        // The ledger must compile the final repaired revision, never the
        // original failed source or an intermediate patch result.
        if expected_parent != self.ledger.source_program_sha256 {
            failures.push("VISUAL_REPAIR_LINEAGE_INVALID".into());
        }
    }

    fn evaluate_reference_comparison(&self, failures: &mut Vec<String>) {
        let Some(reference) = &self.reference_comparison else {
            return;
        };
        if !is_sha256(&reference.comparison_input_sha256)
            || !is_sha256(&reference.comparison_report_sha256)
            || reference.failure_codes.len() > 32
            || reference
                .failure_codes
                .iter()
                .any(|code| code.is_empty() || code.len() > 120)
            || (reference.passed && !reference.failure_codes.is_empty())
            || (!reference.passed && reference.failure_codes.is_empty())
        {
            failures.push("REFERENCE_COMPARISON_EVIDENCE_INVALID".into());
            return;
        }
        if !reference.passed {
            failures.push("REFERENCE_COMPARISON_FAILED".into());
            failures.extend(reference.failure_codes.iter().cloned());
        }
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid(message: impl Into<String>) -> CoreError {
    CoreError::invalid_data("VISUAL_CONVERGENCE_INVALID", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(byte: char) -> String {
        byte.to_string().repeat(64)
    }

    fn passing_input() -> VisualConvergenceInput {
        let source = hash('a');
        let outputs = ['b', 'c', 'd', 'e', 'f', '1', '2']
            .into_iter()
            .map(hash)
            .collect::<Vec<_>>();
        let mut input = source.clone();
        let passes = VisualBuildStage::ORDERED
            .into_iter()
            .zip(outputs.iter())
            .map(|(stage, output)| {
                let pass = VisualBuildPass {
                    stage,
                    input_sha256: input.clone(),
                    output_sha256: output.clone(),
                    completed: true,
                };
                input = output.clone();
                pass
            })
            .collect();
        VisualConvergenceInput {
            schema_version: VISUAL_CONVERGENCE_INPUT_SCHEMA_VERSION.into(),
            ledger: DesignBuildLedger {
                schema_version: DESIGN_BUILD_LEDGER_SCHEMA_VERSION.into(),
                source_program_sha256: source,
                source_revision: 1,
                passes,
            },
            readback: VisualGlbReadbackEvidence {
                glb_sha256: outputs.last().unwrap().clone(),
                shape_program_sha256: hash('3'),
                triangle_count: 120_000,
                primitive_count: 150,
                material_zone_count: 12,
                closed_manifold: true,
                surface_provenance_present: true,
                pbr_channels_complete: true,
            },
            fixed_views: REQUIRED_VISUAL_VIEW_IDS
                .into_iter()
                .enumerate()
                .map(|(index, view_id)| VisualFixedViewEvidence {
                    view_id: view_id.into(),
                    glb_sha256: outputs.last().unwrap().clone(),
                    renderer_id: "forgecad_fixed_eight_view_v1".into(),
                    image_sha256: format!("{:064x}", index + 16),
                    readback_passed: true,
                })
                .collect(),
            detail_coverage: VisualDetailCoverage {
                macro_bound: 3,
                meso_bound: 12,
                micro_bound: 9,
                critical_unresolved: 0,
            },
            reference_comparison: None,
            repairs: Vec::new(),
        }
    }

    #[test]
    fn pv004_fixed_ledger_and_eight_views_pass_one_result_gate() {
        let report = passing_input().evaluate().unwrap();
        assert!(report.passed);
        assert_eq!(report.completed_stage_count, 7);
        assert_eq!(report.fixed_view_count, 8);
        assert!(is_sha256(&report.report_sha256));
    }

    #[test]
    fn pv004_wrong_stage_or_view_lineage_fails_closed() {
        let mut input = passing_input();
        input.ledger.passes.swap(1, 2);
        input.fixed_views[0].glb_sha256 = hash('9');
        let report = input.evaluate().unwrap();
        assert!(!report.passed);
        assert!(report
            .failure_codes
            .contains(&"BUILD_STAGE_ORDER_INVALID".into()));
        assert!(report
            .failure_codes
            .contains(&"EIGHT_VIEW_LINEAGE_INVALID".into()));
    }

    #[test]
    fn pv004_critical_detail_or_third_repair_cannot_pass() {
        let mut input = passing_input();
        input.detail_coverage.critical_unresolved = 1;
        let source = input.ledger.source_program_sha256.clone();
        input.repairs = (1..=3)
            .map(|number| VisualRepairEvidence {
                repair_number: number,
                parent_program_sha256: source.clone(),
                result_program_sha256: hash(char::from(b'3' + number)),
                changed_domains: vec!["surface".into()],
                same_intent: true,
            })
            .collect();
        let report = input.evaluate().unwrap();
        assert!(!report.passed);
        assert!(report
            .failure_codes
            .contains(&"CRITICAL_DETAIL_UNRESOLVED".into()));
        assert!(report
            .failure_codes
            .contains(&"VISUAL_REPAIR_LIMIT_EXCEEDED".into()));
    }

    #[test]
    fn pv004_two_same_intent_repairs_must_end_at_the_compiled_revision() {
        let mut input = passing_input();
        let final_source = input.ledger.source_program_sha256.clone();
        input.repairs = vec![
            VisualRepairEvidence {
                repair_number: 1,
                parent_program_sha256: hash('8'),
                result_program_sha256: hash('9'),
                changed_domains: vec!["geometry".into()],
                same_intent: true,
            },
            VisualRepairEvidence {
                repair_number: 2,
                parent_program_sha256: hash('9'),
                result_program_sha256: final_source.clone(),
                changed_domains: vec!["surface".into()],
                same_intent: true,
            },
        ];
        assert!(input.evaluate().unwrap().passed);
        input.repairs[1].result_program_sha256 = hash('7');
        let report = input.evaluate().unwrap();
        assert!(!report.passed);
        assert!(report
            .failure_codes
            .contains(&"VISUAL_REPAIR_LINEAGE_INVALID".into()));
    }

    #[test]
    fn pv006c_reference_comparison_failure_blocks_visual_convergence() {
        let mut input = passing_input();
        input.reference_comparison = Some(VisualReferenceConvergenceEvidence {
            comparison_input_sha256: hash('8'),
            comparison_report_sha256: hash('9'),
            passed: false,
            failure_codes: vec!["REFERENCE_MACRO_MISMATCH".into()],
        });
        let report = input.evaluate().unwrap();
        assert!(!report.passed);
        assert!(report
            .failure_codes
            .contains(&"REFERENCE_COMPARISON_FAILED".to_string()));
        assert!(report
            .failure_codes
            .contains(&"REFERENCE_MACRO_MISMATCH".to_string()));
    }

    #[test]
    fn pv006c_reference_comparison_summary_cannot_fake_a_pass() {
        let mut input = passing_input();
        input.reference_comparison = Some(VisualReferenceConvergenceEvidence {
            comparison_input_sha256: hash('8'),
            comparison_report_sha256: hash('9'),
            passed: true,
            failure_codes: vec!["REFERENCE_MESO_MISMATCH".into()],
        });
        let report = input.evaluate().unwrap();
        assert!(!report.passed);
        assert!(report
            .failure_codes
            .contains(&"REFERENCE_COMPARISON_EVIDENCE_INVALID".to_string()));
    }
}
