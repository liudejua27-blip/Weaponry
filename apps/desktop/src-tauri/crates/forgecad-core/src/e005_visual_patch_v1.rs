//! E005-R2 hash-bound visual decision and typed patch for the unified source.
//!
//! The comparison Provider never returns a replacement object. Rust accepts
//! only `accept` or at most eight bounded edits against an exact R1 source and
//! an exact Rust-derived visual comparison report.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    lower_forge_visual_author_source_v1, semantic_sha256, AuthorScalarV1, CoreError, CoreResult,
    ForgeVisualAuthorLoweringV1, ForgeVisualAuthorSourceV1, VisualEvidenceGraph,
    VisualReferenceComparisonInput, VisualReferenceComparisonReport,
};

pub const E005_VISUAL_PATCH_SCHEMA_VERSION: &str = "E005VisualPatch@1";
pub const E005_VISUAL_PATCH_PROPOSAL_SCHEMA_VERSION: &str = "E005VisualPatchProposal@1";
pub const E005_VISUAL_PATCH_RESULT_SCHEMA_VERSION: &str = "E005VisualPatchResult@1";

fn invalid(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message.into())
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum E005VisualDecisionKindV1 {
    Accept,
    TypedVisualPatch,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum E005VisualPatchOperationV1 {
    SetParameterDefault {
        parameter_id: String,
        value: Value,
    },
    SetInstancePosition {
        instance_id: String,
        position: [f64; 3],
    },
    SetInstanceRotation {
        instance_id: String,
        rotation: [f64; 3],
    },
    SetRepeatStep {
        instance_id: String,
        step: [f64; 3],
    },
    SetSurfaceTuning {
        binding_id: String,
        edge_wear: f64,
        micro_detail: f64,
    },
    SetTemplateNodePosition {
        node_id: String,
        position: [f64; 3],
    },
}

impl E005VisualPatchOperationV1 {
    fn target(&self) -> String {
        match self {
            Self::SetParameterDefault { parameter_id, .. } => format!("parameter:{parameter_id}"),
            Self::SetInstancePosition { instance_id, .. } => {
                format!("instance-position:{instance_id}")
            }
            Self::SetInstanceRotation { instance_id, .. } => {
                format!("instance-rotation:{instance_id}")
            }
            Self::SetRepeatStep { instance_id, .. } => format!("instance-repeat:{instance_id}"),
            Self::SetSurfaceTuning { binding_id, .. } => format!("surface:{binding_id}"),
            Self::SetTemplateNodePosition { node_id, .. } => format!("template-node:{node_id}"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct E005VisualPatchV1 {
    pub schema_version: String,
    pub patch_id: String,
    pub decision: E005VisualDecisionKindV1,
    pub expected_source_sha256: String,
    pub comparison_input_sha256: String,
    pub comparison_report_sha256: String,
    pub repair_claim_ids: Vec<String>,
    pub operations: Vec<E005VisualPatchOperationV1>,
}

/// Ephemeral Provider output from the one permitted visual-review call.
///
/// It deliberately cannot author `comparison_report_sha256`: that report is
/// derived by Rust only after the Provider response and budget evidence are
/// known. Rust seals this proposal into `E005VisualPatch@1` before any source
/// mutation is permitted.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct E005VisualPatchProposalV1 {
    pub schema_version: String,
    pub patch_id: String,
    pub decision: E005VisualDecisionKindV1,
    pub expected_source_sha256: String,
    pub comparison_input_sha256: String,
    pub repair_claim_ids: Vec<String>,
    pub operations: Vec<E005VisualPatchOperationV1>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct E005VisualPatchResultV1 {
    pub schema_version: String,
    pub decision: E005VisualDecisionKindV1,
    pub parent_source_sha256: String,
    pub patch_sha256: String,
    pub final_source_sha256: String,
    pub final_source: Value,
    pub lowering: ForgeVisualAuthorLoweringV1,
}

pub fn seal_e005_visual_patch_proposal_v1(
    proposal_value: &Value,
    input: &VisualReferenceComparisonInput,
    graph: &VisualEvidenceGraph,
    report: &VisualReferenceComparisonReport,
) -> CoreResult<E005VisualPatchV1> {
    let proposal: E005VisualPatchProposalV1 = serde_json::from_value(proposal_value.clone())
        .map_err(|error| invalid("E005_R2_PROPOSAL_SCHEMA_INVALID", error.to_string()))?;
    if proposal.schema_version != E005_VISUAL_PATCH_PROPOSAL_SCHEMA_VERSION
        || !proposal.patch_id.starts_with("visualpatch_")
        || proposal.patch_id.len() > 128
        || !valid_hash(&proposal.expected_source_sha256)
        || proposal.comparison_input_sha256 != semantic_sha256(input)?
        || proposal.expected_source_sha256 != input.source_program_sha256
        || proposal.operations.len() > 8
    {
        return Err(invalid(
            "E005_R2_PROPOSAL_INVALID",
            "visual proposal identity, source, comparison input or operation budget is invalid",
        ));
    }
    let patch = E005VisualPatchV1 {
        schema_version: E005_VISUAL_PATCH_SCHEMA_VERSION.into(),
        patch_id: proposal.patch_id,
        decision: proposal.decision,
        expected_source_sha256: proposal.expected_source_sha256,
        comparison_input_sha256: proposal.comparison_input_sha256,
        comparison_report_sha256: report.report_sha256.clone(),
        repair_claim_ids: proposal.repair_claim_ids,
        operations: proposal.operations,
    };
    validate_e005_visual_patch_against_comparison_v1(&patch, input, graph, report)?;
    Ok(patch)
}

pub fn validate_e005_visual_patch_against_comparison_v1(
    patch: &E005VisualPatchV1,
    input: &VisualReferenceComparisonInput,
    graph: &VisualEvidenceGraph,
    report: &VisualReferenceComparisonReport,
) -> CoreResult<()> {
    report.validate_against(input, graph)?;
    if patch.comparison_input_sha256 != semantic_sha256(input)?
        || patch.comparison_report_sha256 != report.report_sha256
        || patch.repair_claim_ids != report.repair_claim_ids
    {
        return Err(invalid(
            "E005_R2_COMPARISON_LINEAGE_INVALID",
            "visual decision must bind the exact Rust-derived comparison input, report and repair claims",
        ));
    }
    match (report.passed, patch.decision) {
        (true, E005VisualDecisionKindV1::Accept)
            if patch.operations.is_empty() && patch.repair_claim_ids.is_empty() => Ok(()),
        (false, E005VisualDecisionKindV1::TypedVisualPatch)
            if !patch.operations.is_empty() && !patch.repair_claim_ids.is_empty() => Ok(()),
        _ => Err(invalid(
            "E005_R2_VISUAL_DECISION_INVALID",
            "passed comparison requires accept; failed comparison requires one non-empty typed patch",
        )),
    }
}

pub fn apply_e005_visual_patch_v1(
    source_value: &Value,
    patch_value: &Value,
) -> CoreResult<E005VisualPatchResultV1> {
    let mut source: ForgeVisualAuthorSourceV1 = serde_json::from_value(source_value.clone())
        .map_err(|error| invalid("E005_R2_SOURCE_INVALID", error.to_string()))?;
    let patch: E005VisualPatchV1 = serde_json::from_value(patch_value.clone())
        .map_err(|error| invalid("E005_R2_PATCH_SCHEMA_INVALID", error.to_string()))?;
    if patch.schema_version != E005_VISUAL_PATCH_SCHEMA_VERSION
        || !patch.patch_id.starts_with("visualpatch_")
        || patch.patch_id.len() > 128
        || !valid_hash(&patch.expected_source_sha256)
        || !valid_hash(&patch.comparison_input_sha256)
        || !valid_hash(&patch.comparison_report_sha256)
        || patch.operations.len() > 8
    {
        return Err(invalid(
            "E005_R2_PATCH_INVALID",
            "visual patch identity, hashes or operation budget are invalid",
        ));
    }
    let parent = lower_forge_visual_author_source_v1(source_value)?;
    if parent.source_program_sha256 != patch.expected_source_sha256 {
        return Err(CoreError::conflict(
            "E005_R2_PATCH_STALE",
            "visual patch does not bind the active unified author source",
        ));
    }
    if patch.decision == E005VisualDecisionKindV1::Accept {
        if !patch.operations.is_empty() || !patch.repair_claim_ids.is_empty() {
            return Err(invalid(
                "E005_R2_ACCEPT_MUTATION_FORBIDDEN",
                "accept cannot carry repair claims or mutations",
            ));
        }
        return Ok(E005VisualPatchResultV1 {
            schema_version: E005_VISUAL_PATCH_RESULT_SCHEMA_VERSION.into(),
            decision: patch.decision,
            parent_source_sha256: parent.source_program_sha256.clone(),
            patch_sha256: semantic_sha256(&patch)?,
            final_source_sha256: parent.source_program_sha256.clone(),
            final_source: source_value.clone(),
            lowering: parent,
        });
    }
    if patch.operations.is_empty() || patch.repair_claim_ids.is_empty() {
        return Err(invalid(
            "E005_R2_PATCH_EMPTY",
            "typed_visual_patch requires repair claims and at least one bounded operation",
        ));
    }
    let mut repair_claims = BTreeSet::new();
    if patch
        .repair_claim_ids
        .iter()
        .any(|claim| !claim.starts_with("vclaim_") || !repair_claims.insert(claim.as_str()))
    {
        return Err(invalid(
            "E005_R2_REPAIR_CLAIMS_INVALID",
            "repair claim IDs must be unique visual claims",
        ));
    }
    let mut targets = BTreeSet::new();
    for operation in &patch.operations {
        if !targets.insert(operation.target()) {
            return Err(invalid(
                "E005_R2_PATCH_TARGET_DUPLICATE",
                "a visual patch may modify each typed target at most once",
            ));
        }
        match operation {
            E005VisualPatchOperationV1::SetParameterDefault {
                parameter_id,
                value,
            } => {
                let parameter = source
                    .parameters
                    .iter_mut()
                    .find(|item| item.parameter_id == *parameter_id)
                    .ok_or_else(|| {
                        invalid(
                            "E005_R2_PATCH_TARGET_MISSING",
                            "parameter target is missing",
                        )
                    })?;
                parameter.default = value.clone();
            }
            E005VisualPatchOperationV1::SetInstancePosition {
                instance_id,
                position,
            } => {
                finite_vec(position, 100_000.0)?;
                let instance = source
                    .instances
                    .iter_mut()
                    .find(|item| item.instance_id == *instance_id)
                    .ok_or_else(|| {
                        invalid("E005_R2_PATCH_TARGET_MISSING", "instance target is missing")
                    })?;
                instance.transform.position = position.map(AuthorScalarV1::Literal);
            }
            E005VisualPatchOperationV1::SetInstanceRotation {
                instance_id,
                rotation,
            } => {
                finite_vec(rotation, std::f64::consts::PI)?;
                let instance = source
                    .instances
                    .iter_mut()
                    .find(|item| item.instance_id == *instance_id)
                    .ok_or_else(|| {
                        invalid("E005_R2_PATCH_TARGET_MISSING", "instance target is missing")
                    })?;
                instance.transform.rotation = rotation.map(AuthorScalarV1::Literal);
            }
            E005VisualPatchOperationV1::SetRepeatStep { instance_id, step } => {
                finite_vec(step, 100_000.0)?;
                let instance = source
                    .instances
                    .iter_mut()
                    .find(|item| item.instance_id == *instance_id)
                    .ok_or_else(|| {
                        invalid("E005_R2_PATCH_TARGET_MISSING", "instance target is missing")
                    })?;
                instance.repeat.step = step.map(AuthorScalarV1::Literal);
            }
            E005VisualPatchOperationV1::SetSurfaceTuning {
                binding_id,
                edge_wear,
                micro_detail,
            } => {
                if !edge_wear.is_finite()
                    || !micro_detail.is_finite()
                    || !(0.0..=1.0).contains(edge_wear)
                    || !(0.0..=1.0).contains(micro_detail)
                {
                    return Err(invalid(
                        "E005_R2_PATCH_VALUE_INVALID",
                        "surface tuning must be within 0..=1",
                    ));
                }
                let binding = source
                    .surface_bindings
                    .iter_mut()
                    .find(|item| item.binding_id == *binding_id)
                    .ok_or_else(|| {
                        invalid("E005_R2_PATCH_TARGET_MISSING", "surface target is missing")
                    })?;
                binding.edge_wear = *edge_wear;
                binding.micro_detail = *micro_detail;
            }
            E005VisualPatchOperationV1::SetTemplateNodePosition { node_id, position } => {
                finite_vec(position, 100_000.0)?;
                let nodes = source
                    .geometry_templates
                    .get_mut("nodes")
                    .and_then(Value::as_array_mut)
                    .ok_or_else(|| {
                        invalid(
                            "E005_R2_SOURCE_INVALID",
                            "geometry template nodes are missing",
                        )
                    })?;
                let node = nodes
                    .iter_mut()
                    .find(|item| item.get("node_id").and_then(Value::as_str) == Some(node_id))
                    .ok_or_else(|| {
                        invalid(
                            "E005_R2_PATCH_TARGET_MISSING",
                            "template node target is missing",
                        )
                    })?;
                let kind = node.get("kind").and_then(Value::as_str).unwrap_or_default();
                if !matches!(kind, "box" | "extrude" | "revolve" | "loft" | "sweep") {
                    return Err(invalid(
                        "E005_R2_PATCH_TARGET_INVALID",
                        "only positioned source geometry may move",
                    ));
                }
                node["position"] = json!(position);
            }
        }
    }
    let final_source = serde_json::to_value(&source)
        .map_err(|error| invalid("E005_R2_SOURCE_INVALID", error.to_string()))?;
    let lowering = lower_forge_visual_author_source_v1(&final_source)?;
    Ok(E005VisualPatchResultV1 {
        schema_version: E005_VISUAL_PATCH_RESULT_SCHEMA_VERSION.into(),
        decision: patch.decision,
        parent_source_sha256: parent.source_program_sha256,
        patch_sha256: semantic_sha256(&patch)?,
        final_source_sha256: lowering.source_program_sha256.clone(),
        final_source,
        lowering,
    })
}

fn finite_vec(values: &[f64; 3], limit: f64) -> CoreResult<()> {
    if values
        .iter()
        .all(|value| value.is_finite() && value.abs() <= limit)
    {
        Ok(())
    } else {
        Err(invalid(
            "E005_R2_PATCH_VALUE_INVALID",
            "visual patch vector is non-finite or outside its bounded range",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> Value {
        serde_json::from_str(include_str!(
            "../../../../../../packages/concept-spec/fixtures/e005-r1-unified-service-console.json"
        ))
        .unwrap()
    }

    fn patch(source: &Value) -> Value {
        let source_sha256 = lower_forge_visual_author_source_v1(source)
            .unwrap()
            .source_program_sha256;
        json!({
            "schema_version":"E005VisualPatch@1",
            "patch_id":"visualpatch_e005_r2_fixture",
            "decision":"typed_visual_patch",
            "expected_source_sha256":source_sha256,
            "comparison_input_sha256":"a".repeat(64),
            "comparison_report_sha256":"b".repeat(64),
            "repair_claim_ids":["vclaim_meso_fastener_rhythm"],
            "operations":[
                {"op":"set_parameter_default","parameter_id":"param_fastener_count","value":8},
                {"op":"set_surface_tuning","binding_id":"surface_shell","edge_wear":0.2,"micro_detail":0.55}
            ]
        })
    }

    #[test]
    fn e005_r2_typed_visual_patch_changes_exact_source_and_relowers() {
        let source = source();
        let parent = lower_forge_visual_author_source_v1(&source).unwrap();
        let result = apply_e005_visual_patch_v1(&source, &patch(&source)).unwrap();
        assert_eq!(result.parent_source_sha256, parent.source_program_sha256);
        assert_ne!(result.final_source_sha256, result.parent_source_sha256);
        assert_ne!(
            result.lowering.shape_program_sha256,
            parent.shape_program_sha256
        );
        assert_eq!(result.lowering.semantic_density.expanded_output_count, 13);
    }

    #[test]
    fn e005_r2_rejects_stale_duplicate_or_replacement_patch() {
        let source = source();
        let mut stale = patch(&source);
        stale["expected_source_sha256"] = json!("c".repeat(64));
        assert_eq!(
            apply_e005_visual_patch_v1(&source, &stale)
                .unwrap_err()
                .code(),
            "E005_R2_PATCH_STALE"
        );
        let mut duplicate = patch(&source);
        duplicate["operations"].as_array_mut().unwrap().push(json!({
            "op":"set_parameter_default","parameter_id":"param_fastener_count","value":10
        }));
        assert_eq!(
            apply_e005_visual_patch_v1(&source, &duplicate)
                .unwrap_err()
                .code(),
            "E005_R2_PATCH_TARGET_DUPLICATE"
        );
        let mut replacement = patch(&source);
        replacement["replacement_source"] = source.clone();
        assert_eq!(
            apply_e005_visual_patch_v1(&source, &replacement)
                .unwrap_err()
                .code(),
            "E005_R2_PATCH_SCHEMA_INVALID"
        );
    }

    #[test]
    fn e005_r2_accept_is_identity_and_cannot_hide_mutation() {
        let source = source();
        let mut accept = patch(&source);
        accept["decision"] = json!("accept");
        accept["repair_claim_ids"] = json!([]);
        accept["operations"] = json!([]);
        let result = apply_e005_visual_patch_v1(&source, &accept).unwrap();
        assert_eq!(result.parent_source_sha256, result.final_source_sha256);
        accept["operations"] = json!([{"op":"set_surface_tuning","binding_id":"surface_shell","edge_wear":0.1,"micro_detail":0.1}]);
        assert_eq!(
            apply_e005_visual_patch_v1(&source, &accept)
                .unwrap_err()
                .code(),
            "E005_R2_ACCEPT_MUTATION_FORBIDDEN"
        );
    }
}
