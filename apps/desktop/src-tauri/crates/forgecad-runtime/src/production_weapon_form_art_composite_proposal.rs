//! Closed composition seam for cumulative production-weapon FormArt edits.
//!
//! The existing single-source proposal remains authoritative for its historical
//! receipts. This module introduces the non-lossy composition boundary needed
//! by the next contract version: the original visual baseline and the current
//! proposal base are distinct bindings, while every additional operation is a
//! registered product-owned transformation over one stable Part/source node.
//! No caller-provided profile points, raw GeometryProgram patch, script, path or
//! operator ID can enter this seam.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, now_string, Runtime,
    RuntimeError,
};
use crate::production_weapon_assembly_parameter_mutator::{
    production_weapon_rear_stock_owner_void_half_y_flat_z_mutate,
    production_weapon_rear_stock_owner_void_half_y_flat_z_profile_id,
    production_weapon_receiver_upper_aperture_profile_ids,
    production_weapon_receiver_upper_aperture_trial_mutate,
    production_weapon_receiver_upper_u_topology_profile_ids,
    production_weapon_side_panel_a_aperture_profile_ids,
    production_weapon_side_panel_a_aperture_trial_mutate,
    production_weapon_trigger_guard_aperture_profile_id,
    production_weapon_trigger_guard_aperture_trial_mutate,
};
use forgecad_store::{
    production_weapon_form_art_composite_proposal_record_canonical_sha256,
    ProductionWeaponFormArtCompositeProposalStoreRecord,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const COMPOSITE_PLAN_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtCompositeProposalPlan@1";
const COMPOSITE_OPERATION_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtCompositeProposalOperation@1";
const COMPOSITION_POLICY: &str =
    "runtime-owned-original-baseline-current-base-registered-disjoint-replacements@1";
const REGISTERED_PROFILE_REPLACE: &str = "registered_profile_replace";
const MAX_OPERATIONS: usize = 8;
const PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtCompositeProposalPrepareRequest@1";
const GET_REQUEST_SCHEMA_VERSION: &str = "ProductionWeaponFormArtCompositeProposalGetRequest@1";
const PREPARE_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtCompositeProposalPrepareResult@1";
const GET_RESULT_SCHEMA_VERSION: &str = "ProductionWeaponFormArtCompositeProposalGetResult@1";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const REQUEST_CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-input-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CompositePrepareRequest {
    schema_version: String,
    proposal_id: String,
    session_id: String,
    project_id: String,
    original_fresh_baseline_id: String,
    plan: CompositeProposalPlan,
    idempotency_key: String,
    max_response_bytes: u64,
    runtime_write_performed: bool,
    writer_policy: String,
    canonicalization_policy: String,
    input_sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CompositeGetRequest {
    schema_version: String,
    project_id: String,
    proposal_id: String,
    max_response_bytes: u64,
    runtime_write_performed: bool,
    writer_policy: String,
    canonicalization_policy: String,
    input_sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompositeProposalOperation {
    pub schema_version: String,
    pub sequence_index: u8,
    pub operation_id: String,
    pub operation_kind: String,
    pub source_node_id: String,
    pub part_id: String,
    pub registered_profile_id: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompositeProposalPlan {
    pub schema_version: String,
    pub project_id: String,
    pub original_source_candidate_id: String,
    pub original_source_candidate_state_sha256: String,
    pub original_source_artifact_sha256: String,
    pub original_fresh_baseline_canonical_sha256: String,
    pub current_base_candidate_id: String,
    pub current_base_candidate_state_sha256: String,
    pub current_base_artifact_sha256: String,
    pub current_base_geometry_program_sha256: String,
    pub current_base_proposal_evidence_sha256: String,
    pub operations: Vec<CompositeProposalOperation>,
    pub composition_policy: String,
    pub canonical_sha256: String,
}

fn invalid(reason: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_PROPOSAL_INVALID: {}",
        reason.into()
    ))
}

fn canonical_without_declared_hash<T: Serialize>(value: &T) -> Result<String, RuntimeError> {
    let mut value = serde_json::to_value(value).map_err(|error| invalid(error.to_string()))?;
    value
        .as_object_mut()
        .ok_or_else(|| invalid("canonical object is unavailable"))?
        .remove("canonical_sha256");
    Ok(canonical_json_hash(&value))
}

fn validate_identifier(value: &str, label: &str) -> Result<(), RuntimeError> {
    if !is_opaque_id(value) {
        return Err(invalid(format!("{label} is invalid")));
    }
    Ok(())
}

fn validate_sha(value: &str, label: &str) -> Result<(), RuntimeError> {
    if !is_sha256(value) {
        return Err(invalid(format!("{label} is invalid")));
    }
    Ok(())
}

fn request_input_sha256<T: Serialize>(request: &T) -> Result<String, RuntimeError> {
    let mut value = serde_json::to_value(request).map_err(|error| invalid(error.to_string()))?;
    value
        .as_object_mut()
        .ok_or_else(|| invalid("request object is unavailable"))?
        .remove("input_sha256");
    Ok(canonical_json_hash(&value))
}

fn validate_transport_fields(
    max_response_bytes: u64,
    runtime_write_performed: bool,
    writer_policy: &str,
    canonicalization_policy: &str,
) -> Result<(), RuntimeError> {
    if max_response_bytes != MAX_RESPONSE_BYTES
        || runtime_write_performed
        || writer_policy != WRITER_POLICY
        || canonicalization_policy != REQUEST_CANONICALIZATION_POLICY
    {
        return Err(invalid("transport policy differs"));
    }
    Ok(())
}

fn parse_prepare_request(request: &Value) -> Result<CompositePrepareRequest, RuntimeError> {
    let parsed: CompositePrepareRequest =
        serde_json::from_value(request.clone()).map_err(|error| invalid(error.to_string()))?;
    if parsed.schema_version != PREPARE_REQUEST_SCHEMA_VERSION {
        return Err(invalid("prepare request schema differs"));
    }
    for (value, label) in [
        (parsed.proposal_id.as_str(), "proposal_id"),
        (parsed.session_id.as_str(), "session_id"),
        (parsed.project_id.as_str(), "project_id"),
        (
            parsed.original_fresh_baseline_id.as_str(),
            "original_fresh_baseline_id",
        ),
        (parsed.idempotency_key.as_str(), "idempotency_key"),
    ] {
        validate_identifier(value, label)?;
    }
    validate_transport_fields(
        parsed.max_response_bytes,
        parsed.runtime_write_performed,
        &parsed.writer_policy,
        &parsed.canonicalization_policy,
    )?;
    validate_composite_proposal_plan(&parsed.plan)?;
    if parsed.project_id != parsed.plan.project_id
        || parsed.input_sha256 != request_input_sha256(&parsed)?
    {
        return Err(invalid("prepare request scope or input hash differs"));
    }
    Ok(parsed)
}

fn parse_get_request(request: &Value) -> Result<CompositeGetRequest, RuntimeError> {
    let parsed: CompositeGetRequest =
        serde_json::from_value(request.clone()).map_err(|error| invalid(error.to_string()))?;
    if parsed.schema_version != GET_REQUEST_SCHEMA_VERSION {
        return Err(invalid("get request schema differs"));
    }
    validate_identifier(&parsed.project_id, "project_id")?;
    validate_identifier(&parsed.proposal_id, "proposal_id")?;
    validate_transport_fields(
        parsed.max_response_bytes,
        parsed.runtime_write_performed,
        &parsed.writer_policy,
        &parsed.canonicalization_policy,
    )?;
    if parsed.input_sha256 != request_input_sha256(&parsed)? {
        return Err(invalid("get request input hash differs"));
    }
    Ok(parsed)
}

pub(crate) fn validate_composite_proposal_plan(
    plan: &CompositeProposalPlan,
) -> Result<(), RuntimeError> {
    if plan.schema_version != COMPOSITE_PLAN_SCHEMA_VERSION
        || plan.composition_policy != COMPOSITION_POLICY
    {
        return Err(invalid("schema or composition policy differs"));
    }
    validate_identifier(&plan.project_id, "project_id")?;
    validate_identifier(
        &plan.original_source_candidate_id,
        "original_source_candidate_id",
    )?;
    validate_identifier(&plan.current_base_candidate_id, "current_base_candidate_id")?;
    if plan.original_source_candidate_id == plan.current_base_candidate_id {
        return Err(invalid(
            "current base must be a proposal descendant, not the original source",
        ));
    }
    for (value, label) in [
        (
            plan.original_source_candidate_state_sha256.as_str(),
            "original_source_candidate_state_sha256",
        ),
        (
            plan.original_source_artifact_sha256.as_str(),
            "original_source_artifact_sha256",
        ),
        (
            plan.original_fresh_baseline_canonical_sha256.as_str(),
            "original_fresh_baseline_canonical_sha256",
        ),
        (
            plan.current_base_candidate_state_sha256.as_str(),
            "current_base_candidate_state_sha256",
        ),
        (
            plan.current_base_artifact_sha256.as_str(),
            "current_base_artifact_sha256",
        ),
        (
            plan.current_base_geometry_program_sha256.as_str(),
            "current_base_geometry_program_sha256",
        ),
        (
            plan.current_base_proposal_evidence_sha256.as_str(),
            "current_base_proposal_evidence_sha256",
        ),
    ] {
        validate_sha(value, label)?;
    }
    if plan.operations.is_empty() || plan.operations.len() > MAX_OPERATIONS {
        return Err(invalid("operation count is outside 1..=8"));
    }
    let mut operation_ids = BTreeSet::new();
    let mut source_nodes = BTreeSet::new();
    let mut parts = BTreeSet::new();
    for (index, operation) in plan.operations.iter().enumerate() {
        if operation.schema_version != COMPOSITE_OPERATION_SCHEMA_VERSION
            || operation.sequence_index as usize != index
            || operation.operation_kind != REGISTERED_PROFILE_REPLACE
        {
            return Err(invalid("operation schema, order or kind differs"));
        }
        validate_identifier(&operation.operation_id, "operation_id")?;
        validate_identifier(&operation.source_node_id, "source_node_id")?;
        validate_identifier(&operation.part_id, "part_id")?;
        if !operation_ids.insert(operation.operation_id.as_str())
            || !source_nodes.insert(operation.source_node_id.as_str())
            || !parts.insert(operation.part_id.as_str())
        {
            return Err(invalid(
                "operation IDs, source nodes and Parts must be disjoint",
            ));
        }
        let registered = (
            operation.source_node_id.as_str(),
            operation.part_id.as_str(),
            operation.registered_profile_id.as_str(),
        );
        if !matches!(
            registered,
            ("trigger-guard", "trigger-guard", profile)
                if profile == production_weapon_trigger_guard_aperture_profile_id()
        ) && !matches!(
            registered,
            ("rear-stock", "rear-stock", profile)
                if profile == production_weapon_rear_stock_owner_void_half_y_flat_z_profile_id()
        ) && !matches!(
            registered,
            ("side-panel-a", "side-panel-a", profile)
                if production_weapon_side_panel_a_aperture_profile_ids().contains(&profile)
        ) && !matches!(
            registered,
            ("receiver-upper", "receiver-upper", profile)
                if production_weapon_receiver_upper_aperture_profile_ids().contains(&profile)
        ) {
            return Err(invalid("registered profile binding is unavailable"));
        }
        if operation.canonical_sha256 != canonical_without_declared_hash(operation)? {
            return Err(invalid("operation canonical hash differs"));
        }
    }
    if plan.canonical_sha256 != canonical_without_declared_hash(plan)? {
        return Err(invalid("plan canonical hash differs"));
    }
    Ok(())
}

fn geometry_program_nodes(
    program: &Value,
    label: &str,
) -> Result<BTreeMap<String, Value>, RuntimeError> {
    let nodes = program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("{label} nodes are unavailable")))?;
    let mut by_id = BTreeMap::new();
    for node in nodes {
        let node_id = node
            .get("node_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid(format!("{label} node_id is unavailable")))?;
        validate_identifier(node_id, &format!("{label} node_id"))?;
        if by_id.insert(node_id.to_owned(), node.clone()).is_some() {
            return Err(invalid(format!("{label} has duplicate node_id")));
        }
    }
    if by_id.is_empty() {
        return Err(invalid(format!("{label} nodes are empty")));
    }
    Ok(by_id)
}

fn geometry_program_part_roots(
    program: &Value,
    label: &str,
) -> Result<BTreeMap<String, Vec<String>>, RuntimeError> {
    let outputs = program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("{label} part_outputs are unavailable")))?;
    let mut by_part = BTreeMap::new();
    for output in outputs {
        let part_id = output
            .get("part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid(format!("{label} part_id is unavailable")))?;
        validate_identifier(part_id, &format!("{label} part_id"))?;
        let roots = output
            .get("input_node_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid(format!("{label} input_node_ids are unavailable")))?;
        if roots.is_empty() {
            return Err(invalid(format!("{label} part closure has no roots")));
        }
        let mut root_ids = Vec::with_capacity(roots.len());
        for root in roots {
            let root_id = root
                .as_str()
                .ok_or_else(|| invalid(format!("{label} input_node_id is invalid")))?;
            validate_identifier(root_id, &format!("{label} input_node_id"))?;
            if root_ids.iter().any(|existing| existing == root_id) {
                return Err(invalid(format!("{label} part closure has duplicate roots")));
            }
            root_ids.push(root_id.to_owned());
        }
        if by_part.insert(part_id.to_owned(), root_ids).is_some() {
            return Err(invalid(format!("{label} has duplicate part_id")));
        }
    }
    if by_part.is_empty() {
        return Err(invalid(format!("{label} part_outputs are empty")));
    }
    Ok(by_part)
}

fn geometry_node_closure(
    nodes: &BTreeMap<String, Value>,
    roots: &[String],
    label: &str,
) -> Result<BTreeSet<String>, RuntimeError> {
    let mut closure = BTreeSet::new();
    let mut pending = roots.to_vec();
    while let Some(node_id) = pending.pop() {
        if !closure.insert(node_id.clone()) {
            continue;
        }
        let node = nodes
            .get(&node_id)
            .ok_or_else(|| invalid(format!("{label} references unknown node")))?;
        let inputs = node
            .get("inputs")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid(format!("{label} node inputs are unavailable")))?;
        for input in inputs {
            let input_id = input
                .as_str()
                .ok_or_else(|| invalid(format!("{label} input node is invalid")))?;
            validate_identifier(input_id, &format!("{label} input node"))?;
            pending.push(input_id.to_owned());
        }
    }
    Ok(closure)
}

/// Enforce the closed composite edit boundary before any candidate materialization.
/// The only permitted node deltas are the registered operation source nodes;
/// PartOutputs and every other node must remain exactly equal to the current base.
pub(crate) fn validate_exact_composite_delta(
    plan: &CompositeProposalPlan,
    current_base_program: &Value,
    composed_program: &Value,
) -> Result<(), RuntimeError> {
    validate_composite_proposal_plan(plan)?;
    let base_nodes = geometry_program_nodes(current_base_program, "current-base")?;
    let composed_nodes = geometry_program_nodes(composed_program, "composed")?;
    let u_topology_operation = plan.operations.iter().find(|operation| {
        production_weapon_receiver_upper_u_topology_profile_ids()
            .contains(&operation.registered_profile_id.as_str())
    });
    let base_node_ids = base_nodes.keys().cloned().collect::<BTreeSet<_>>();
    let composed_node_ids = composed_nodes.keys().cloned().collect::<BTreeSet<_>>();
    let node_ids_valid = if u_topology_operation.is_some() {
        let mut expected = base_node_ids.clone();
        expected.insert("receiver-upper-right".to_owned());
        expected.insert("receiver-upper-bridge".to_owned());
        composed_node_ids == expected
    } else {
        composed_node_ids == base_node_ids
    };
    if !node_ids_valid {
        return Err(invalid(
            "composite node ID set differs from closed registered edit",
        ));
    }

    let changed_nodes = base_nodes
        .iter()
        .filter_map(|(node_id, base_node)| {
            (composed_nodes.get(node_id) != Some(base_node)).then_some(node_id.clone())
        })
        .collect::<BTreeSet<_>>();
    let expected_nodes = plan
        .operations
        .iter()
        .map(|operation| operation.source_node_id.clone())
        .collect::<BTreeSet<_>>();
    if changed_nodes != expected_nodes {
        return Err(invalid(
            "changed node set differs from registered replacement source nodes",
        ));
    }

    let part_roots = geometry_program_part_roots(current_base_program, "current-base")?;
    let composed_part_roots = geometry_program_part_roots(composed_program, "composed")?;
    let part_outputs_valid = if let Some(operation) = u_topology_operation {
        let expected_receiver_upper = vec![
            operation.source_node_id.clone(),
            "receiver-upper-right".to_owned(),
            "receiver-upper-bridge".to_owned(),
        ];
        let base_outputs = current_base_program
            .get("part_outputs")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("current-base part_outputs are unavailable"))?;
        let composed_outputs = composed_program
            .get("part_outputs")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("composed part_outputs are unavailable"))?;
        let target_positions = base_outputs
            .iter()
            .enumerate()
            .filter_map(|(position, output)| {
                (output.get("part_id").and_then(Value::as_str) == Some(operation.part_id.as_str()))
                    .then_some(position)
            })
            .collect::<Vec<_>>();
        if target_positions.len() != 1 || base_outputs.len() != composed_outputs.len() {
            false
        } else {
            let target_position = target_positions[0];
            let mut expected_target = base_outputs[target_position].clone();
            expected_target["input_node_ids"] = Value::Array(
                expected_receiver_upper
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            );
            base_outputs
                .iter()
                .enumerate()
                .all(|(position, base_output)| {
                    let composed_output = &composed_outputs[position];
                    if position == target_position {
                        composed_output == &expected_target
                    } else {
                        // A U-topology edit may replace only the registered
                        // receiver-upper roots. Metadata, ordering, and every
                        // other PartOutput remain byte-for-byte bound to the
                        // current base.
                        composed_output == base_output
                    }
                })
                && part_roots.keys().collect::<Vec<_>>()
                    == composed_part_roots.keys().collect::<Vec<_>>()
                && part_roots.iter().all(|(part_id, roots)| {
                    if part_id == &operation.part_id {
                        composed_part_roots.get(part_id) == Some(&expected_receiver_upper)
                    } else {
                        composed_part_roots.get(part_id) == Some(roots)
                    }
                })
        }
    } else {
        current_base_program.get("part_outputs") == composed_program.get("part_outputs")
    };
    if !part_outputs_valid {
        return Err(invalid("part_outputs differ from closed registered edit"));
    }
    let mut owners_by_node: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (part_id, roots) in &part_roots {
        let closure = geometry_node_closure(&base_nodes, roots, "current-base")?;
        for node_id in &changed_nodes {
            if closure.contains(node_id) {
                owners_by_node
                    .entry(node_id.clone())
                    .or_default()
                    .insert(part_id.clone());
            }
        }
    }
    for operation in &plan.operations {
        let owners = owners_by_node
            .get(&operation.source_node_id)
            .ok_or_else(|| invalid("registered source node is outside its Part closure"))?;
        if owners.len() != 1 || !owners.contains(&operation.part_id) {
            return Err(invalid(
                "registered source node is not uniquely owned by its declared Part closure",
            ));
        }
    }
    for node_id in &changed_nodes {
        if owners_by_node
            .get(node_id)
            .map_or(true, |owners| owners.len() != 1)
        {
            return Err(invalid(
                "changed node does not have exactly one owning Part closure",
            ));
        }
    }
    Ok(())
}

/// Apply only the registered, ordered operations to the exact current-base
/// GeometryProgram. The original visual source is deliberately not used as the
/// geometry base; it remains a separate comparison/registration binding.
pub(crate) fn compose_current_base_geometry_program(
    plan: &CompositeProposalPlan,
    current_base_program: &Value,
) -> Result<Value, RuntimeError> {
    validate_composite_proposal_plan(plan)?;
    if current_base_program
        .get("project_id")
        .and_then(Value::as_str)
        != Some(plan.project_id.as_str())
        || current_base_program
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(plan.current_base_geometry_program_sha256.as_str())
    {
        return Err(invalid("current-base GeometryProgram binding differs"));
    }
    let mut composed = current_base_program.clone();
    for operation in &plan.operations {
        match operation.registered_profile_id.as_str() {
            value if value == production_weapon_trigger_guard_aperture_profile_id() => {
                composed = production_weapon_trigger_guard_aperture_trial_mutate(&composed)?;
            }
            value
                if value == production_weapon_rear_stock_owner_void_half_y_flat_z_profile_id() =>
            {
                composed = production_weapon_rear_stock_owner_void_half_y_flat_z_mutate(&composed)?;
            }
            value if production_weapon_side_panel_a_aperture_profile_ids().contains(&value) => {
                composed = production_weapon_side_panel_a_aperture_trial_mutate(&composed, value)?;
            }
            value if production_weapon_receiver_upper_aperture_profile_ids().contains(&value) => {
                composed =
                    production_weapon_receiver_upper_aperture_trial_mutate(&composed, value)?;
            }
            _ => return Err(invalid("registered profile dispatch is unavailable")),
        }
    }
    if composed.get("canonical_sha256") == current_base_program.get("canonical_sha256") {
        return Err(invalid("composition did not change current-base geometry"));
    }
    validate_exact_composite_delta(plan, current_base_program, &composed)?;
    Ok(composed)
}

pub(crate) fn composite_source_receipt(
    plan: &CompositeProposalPlan,
    composed_program: &Value,
) -> Result<Value, RuntimeError> {
    validate_composite_proposal_plan(plan)?;
    let composed_sha256 = composed_program
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("composed GeometryProgram hash is unavailable"))?;
    let mut value = json!({
        "schema_version":"ProductionWeaponFormArtCompositeProposalSourceReceipt@1",
        "project_id":plan.project_id,
        "original_source_candidate_id":plan.original_source_candidate_id,
        "original_source_candidate_state_sha256":plan.original_source_candidate_state_sha256,
        "original_fresh_baseline_canonical_sha256":plan.original_fresh_baseline_canonical_sha256,
        "current_base_candidate_id":plan.current_base_candidate_id,
        "current_base_candidate_state_sha256":plan.current_base_candidate_state_sha256,
        "current_base_geometry_program_sha256":plan.current_base_geometry_program_sha256,
        "current_base_proposal_evidence_sha256":plan.current_base_proposal_evidence_sha256,
        "composed_geometry_program_sha256":composed_sha256,
        "operation_count":plan.operations.len(),
        "operation_ids":plan.operations.iter().map(|operation| operation.operation_id.clone()).collect::<Vec<_>>(),
        "source_node_ids":plan.operations.iter().map(|operation| operation.source_node_id.clone()).collect::<Vec<_>>(),
        "part_ids":plan.operations.iter().map(|operation| operation.part_id.clone()).collect::<Vec<_>>(),
        "composition_policy":COMPOSITION_POLICY,
        "runtime_write_performed":false,
        "candidate_created":false,
        "six_view_evaluation":"NOT_RUN_SOURCE_COMPOSITION_ONLY",
        "quality_status":"QUALITY_TARGET_NOT_MET",
        "canonical_sha256":""
    });
    value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
    Ok(value)
}

fn record_projection(
    record: &ProductionWeaponFormArtCompositeProposalStoreRecord,
    schema_version: &str,
    replayed: bool,
    runtime_write_performed: bool,
) -> Value {
    json!({
        "schema_version":schema_version,
        "proposal_id":record.proposal_id,
        "project_id":record.project_id,
        "session_id":record.session_id,
        "plan_object_sha256":record.plan_object_sha256,
        "plan_canonical_sha256":record.plan_canonical_sha256,
        "original_current_final_lineage":{
            "current_base_candidate_id":record.current_base_candidate_id,
            "current_base_candidate_state_sha256":record.current_base_candidate_state_sha256,
            "current_base_artifact_sha256":record.current_base_artifact_sha256,
            "current_base_geometry_program_sha256":record.current_base_geometry_program_sha256,
            "current_base_proposal_evidence_receipt_object_sha256":record.current_base_proposal_evidence_receipt_object_sha256,
            "composed_geometry_program_sha256":record.composed_geometry_program_sha256,
            "proposal_candidate_id":record.proposal_candidate_id,
            "proposal_candidate_state_sha256":record.proposal_candidate_state_sha256,
            "proposal_artifact_sha256":record.proposal_artifact_sha256,
            "proposal_artifact_readback_object_sha256":record.proposal_artifact_readback_object_sha256,
        },
        "reviewable_candidate":{
            "candidate_id":record.proposal_candidate_id,
            "candidate_state_sha256":record.proposal_candidate_state_sha256,
            "artifact_sha256":record.proposal_artifact_sha256,
            "artifact_readback_object_sha256":record.proposal_artifact_readback_object_sha256,
            "geometry_program_sha256":record.composed_geometry_program_sha256,
            "status":record.status,
        },
        "six_view_evaluation":{
            "status":if record.cross_view_evidence_bundle_sha256.is_some() {"DURABLE"} else {"NOT_RUN_AWAITING_EXACT_6X9_AOV"},
            "view_order":["front","back","left","right","top","rear-three-quarter"],
            "aov_count_expected":54,
            "cross_view_evidence_bundle_sha256":record.cross_view_evidence_bundle_sha256,
        },
        "proposal_form_art_evidence":{
            "status":if record.proposal_form_art_evidence_receipt_object_sha256.is_some() {"DURABLE"} else {"NOT_RUN"},
            "receipt_object_sha256":record.proposal_form_art_evidence_receipt_object_sha256,
        },
        "receipt_object_sha256":record.receipt_object_sha256,
        "record_canonical_sha256":record.canonical_sha256,
        "replayed":replayed,
        "restart_hash_verified":!runtime_write_performed,
        "runtime_write_performed":runtime_write_performed,
        "persistent_user_data_touched":runtime_write_performed,
        "candidate_confirm_allowed":false,
        "promotion_eligible":false,
        "secondary_form_approved":"NOT_CREATED",
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "quality_status":"QUALITY_TARGET_NOT_MET",
        "visual_review_status":"NOT_RUN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "aov_bytes_in_summary":false,
        "limitations":[
            "Exact six-view 9-AOV evaluation has not run for this composite candidate.",
            "No secondary-form approval, confirmation, immutable version, export, human review or commercial-engine validation is implied."
        ],
        "canonical_sha256":record.canonical_sha256,
    })
}

pub(crate) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let request = parse_prepare_request(request).map_err(|error| {
        eprintln!("FORGECAD_FORM_ART_COMPOSITE_STAGE=parse-prepare error={error}");
        error
    })?;
    if let Some(existing) = runtime
        .store
        .get_production_weapon_form_art_composite_proposal_by_idempotency(
            &request.project_id,
            &request.idempotency_key,
        )?
    {
        if existing.proposal_id != request.proposal_id
            || existing.input_sha256 != request.input_sha256
            || existing.plan_canonical_sha256 != request.plan.canonical_sha256
        {
            return Err(invalid("idempotency replay binding differs"));
        }
        return Ok(record_projection(
            &existing,
            PREPARE_RESULT_SCHEMA_VERSION,
            true,
            false,
        ));
    }

    let session = runtime
        .store
        .get_agentic_session(&request.session_id)?
        .ok_or_else(|| invalid("DesignSession is unavailable"))?;
    if session.project_id != request.project_id
        || session.candidate_id != request.plan.original_source_candidate_id
        || session.candidate_state_sha256 != request.plan.original_source_candidate_state_sha256
    {
        return Err(invalid("original DesignSession scope differs"));
    }
    let original_candidate = runtime
        .candidate(&request.plan.original_source_candidate_id)?
        .ok_or_else(|| invalid("original source candidate is unavailable"))?;
    let current_base_candidate = runtime
        .candidate(&request.plan.current_base_candidate_id)?
        .ok_or_else(|| invalid("current-base candidate is unavailable"))?;
    if original_candidate.project_id != request.project_id
        || original_candidate.canonical_sha256
            != request.plan.original_source_candidate_state_sha256
        || current_base_candidate.project_id != request.project_id
        || current_base_candidate.canonical_sha256
            != request.plan.current_base_candidate_state_sha256
    {
        return Err(invalid("candidate state binding differs"));
    }
    let original_geometry = super::agentic_action::load_geometry_bindings(
        runtime,
        &original_candidate,
        &request.project_id,
        &session,
    )?;
    let current_geometry = super::agentic_action::load_geometry_bindings(
        runtime,
        &current_base_candidate,
        &request.project_id,
        &session,
    )?;
    // The immutable current base may have been authored by an older build
    // cohort. Recompiling it under the current Worker changes embedded cohort
    // metadata and therefore the GLB hash even when geometry is identical.
    // Validate the persisted source artifact and lineage exactly here; the new
    // composed child below is still compiled and read back by the current
    // cohort through prepare_geometry_candidate_exact.
    let current_inspection =
        super::agentic_action::inspect_persisted_candidate_for_cohort_transition(
            runtime,
            &current_geometry,
        )?;
    let current_readback = super::agentic_action::verify_artifact_readback(
        runtime,
        &current_base_candidate,
        &current_geometry,
        &current_inspection,
    )?;
    if original_geometry.artifact_sha256 != request.plan.original_source_artifact_sha256
        || current_geometry.artifact_sha256 != request.plan.current_base_artifact_sha256
        || current_geometry.evidence.geometry_program_sha256
            != request.plan.current_base_geometry_program_sha256
    {
        return Err(invalid("original/current geometry evidence differs"));
    }
    let baseline = runtime
        .store
        .get_production_weapon_form_art_baseline_by_id(&request.original_fresh_baseline_id)?
        .ok_or_else(|| invalid("original fresh FormArt baseline is unavailable"))?;
    if baseline.project_id != request.project_id
        || baseline.session_id != request.session_id
        || baseline.candidate_id != request.plan.original_source_candidate_id
        || baseline.candidate_state_sha256 != request.plan.original_source_candidate_state_sha256
        || baseline.artifact_sha256 != request.plan.original_source_artifact_sha256
        || baseline.canonical_sha256 != request.plan.original_fresh_baseline_canonical_sha256
    {
        return Err(invalid("original fresh baseline binding differs"));
    }
    let base_proof = runtime
        .store
        .get_production_weapon_form_art_proposal_evidence(
            &request.plan.current_base_proposal_evidence_sha256,
        )?
        .ok_or_else(|| invalid("current-base proposal evidence is unavailable"))?;
    if base_proof.project_id != request.project_id
        || base_proof.session_id != request.session_id
        || base_proof.source_candidate_id != request.plan.original_source_candidate_id
        || base_proof.source_candidate_state_sha256
            != request.plan.original_source_candidate_state_sha256
        || base_proof.source_artifact_sha256 != request.plan.original_source_artifact_sha256
        || base_proof.proposal_candidate_id != request.plan.current_base_candidate_id
        || base_proof.proposal_candidate_state_sha256
            != request.plan.current_base_candidate_state_sha256
        || base_proof.proposal_artifact_sha256 != request.plan.current_base_artifact_sha256
    {
        return Err(invalid("current-base proposal evidence binding differs"));
    }

    let mut composed =
        compose_current_base_geometry_program(&request.plan, &current_geometry.program).map_err(
            |error| {
                eprintln!("FORGECAD_FORM_ART_COMPOSITE_STAGE=compose-current-base error={error}");
                error
            },
        )?;
    // Seal the final composition at this write boundary. Individual registered
    // mutators also hash their output, but the orchestrator owns the final
    // ordered aggregate and must not rely on an intermediate operation hash.
    composed
        .as_object_mut()
        .ok_or_else(|| invalid("composed GeometryProgram is not an object"))?
        .remove("canonical_sha256");
    composed["canonical_sha256"] = Value::String(canonical_json_hash(&composed));
    let composed_program_sha256 = composed
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("composed GeometryProgram hash is unavailable"))?
        .to_owned();
    let base_version_id = runtime
        .store
        .latest_version_for_project(&request.project_id)?
        .map(|version| version.version_id);
    let candidate_key = format!(
        "form-art-composite-candidate-{}",
        &canonical_json_hash(&json!({
            "proposal_id":request.proposal_id,
            "input_sha256":request.input_sha256,
            "plan_canonical_sha256":request.plan.canonical_sha256,
            "composed_geometry_program_sha256":composed_program_sha256,
        }))[..48]
    );
    let prepared = runtime
        .prepare_geometry_candidate_exact(
            &request.project_id,
            base_version_id.as_deref(),
            &candidate_key,
            json!({
                "typed":"geometry",
                "reference_id":session.reference_id,
                "geometry_program":composed,
            }),
        )
        .map_err(|error| {
            eprintln!(
                "FORGECAD_FORM_ART_COMPOSITE_STAGE=current-cohort-child-prepare error={error}"
            );
            error
        })?;
    let candidate_value = prepared
        .get("candidate")
        .ok_or_else(|| invalid("prepared candidate is missing"))?;
    let proposal_candidate_id = candidate_value
        .get("candidate_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| invalid("prepared candidate ID is invalid"))?
        .to_owned();
    let proposal_candidate_state_sha256 = candidate_value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("prepared candidate state hash is invalid"))?
        .to_owned();
    let artifact = prepared
        .get("artifact")
        .ok_or_else(|| invalid("prepared ArtifactReadback is missing"))?;
    let proposal_artifact_readback_sha256 = artifact
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("prepared ArtifactReadback canonical hash is invalid"))?
        .to_owned();
    let proposal_evidence = runtime
        .store
        .get_geometry_candidate_evidence(&proposal_candidate_id)?
        .ok_or_else(|| invalid("prepared GeometryCandidateEvidence is unavailable"))?;
    if proposal_evidence.project_id != request.project_id
        || proposal_evidence.geometry_program_sha256 != composed_program_sha256
        || proposal_evidence.artifact_object_sha256
            != artifact
                .get("artifact_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
    {
        return Err(invalid("prepared candidate evidence binding differs"));
    }

    let plan_value =
        serde_json::to_value(&request.plan).map_err(|error| invalid(error.to_string()))?;
    let plan_bytes =
        canonical_json_bytes(&plan_value).map_err(|error| invalid(error.to_string()))?;
    let plan_object = runtime.put_object(
        &plan_bytes,
        None,
        "application/json",
        "production-weapon-form-art-composite-proposal-plan",
    )?;
    let timestamp = now_string();
    let request_value =
        serde_json::to_value(&request).map_err(|error| invalid(error.to_string()))?;
    let mut record = ProductionWeaponFormArtCompositeProposalStoreRecord {
        schema_version: "ProductionWeaponFormArtCompositeProposalStoreRecord@1".to_owned(),
        project_id: request.project_id.clone(),
        proposal_id: request.proposal_id.clone(),
        session_id: request.session_id.clone(),
        idempotency_key: request.idempotency_key.clone(),
        plan_object_sha256: plan_object.record.sha256,
        plan_canonical_sha256: request.plan.canonical_sha256.clone(),
        current_base_candidate_id: request.plan.current_base_candidate_id.clone(),
        current_base_candidate_state_sha256: request
            .plan
            .current_base_candidate_state_sha256
            .clone(),
        current_base_artifact_sha256: request.plan.current_base_artifact_sha256.clone(),
        current_base_geometry_program_sha256: request
            .plan
            .current_base_geometry_program_sha256
            .clone(),
        current_base_geometry_program_object_sha256: current_geometry
            .evidence
            .geometry_program_object_sha256,
        current_base_proposal_evidence_receipt_object_sha256: request
            .plan
            .current_base_proposal_evidence_sha256
            .clone(),
        composed_geometry_program_sha256: composed_program_sha256,
        composed_geometry_program_object_sha256: proposal_evidence.geometry_program_object_sha256,
        proposal_candidate_id,
        proposal_candidate_state_sha256,
        proposal_artifact_sha256: proposal_evidence.artifact_object_sha256,
        proposal_artifact_readback_object_sha256: proposal_evidence.artifact_readback_object_sha256,
        proposal_artifact_readback_sha256,
        cross_view_evidence_bundle_sha256: None,
        proposal_form_art_evidence_receipt_object_sha256: None,
        receipt_object_sha256: "0".repeat(64),
        request_sha256: canonical_json_hash(&request_value),
        input_sha256: request.input_sha256,
        status: "PREPARED_REVIEWABLE_CANDIDATE_AWAITING_SIX_VIEW".to_owned(),
        quality_status: "QUALITY_TARGET_NOT_MET".to_owned(),
        candidate_confirm_allowed: false,
        secondary_form_approved: "NOT_CREATED".to_owned(),
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        canonical_sha256: String::new(),
        created_at: timestamp,
    };
    record.canonical_sha256 =
        production_weapon_form_art_composite_proposal_record_canonical_sha256(&record)?;
    let receipt = json!({
        "schema_version":"ProductionWeaponFormArtCompositeProposalReceipt@1",
        "project_id":record.project_id,
        "proposal_id":record.proposal_id,
        "session_id":record.session_id,
        "plan_object_sha256":record.plan_object_sha256,
        "plan_canonical_sha256":record.plan_canonical_sha256,
        "original_source_candidate_id":request.plan.original_source_candidate_id,
        "original_source_candidate_state_sha256":request.plan.original_source_candidate_state_sha256,
        "original_source_artifact_sha256":request.plan.original_source_artifact_sha256,
        "original_fresh_baseline_id":request.original_fresh_baseline_id,
        "original_fresh_baseline_canonical_sha256":request.plan.original_fresh_baseline_canonical_sha256,
        "current_base_candidate_id":record.current_base_candidate_id,
        "current_base_candidate_state_sha256":record.current_base_candidate_state_sha256,
        "current_base_artifact_sha256":record.current_base_artifact_sha256,
        "current_base_geometry_program_sha256":record.current_base_geometry_program_sha256,
        "current_base_proposal_evidence_receipt_object_sha256":record.current_base_proposal_evidence_receipt_object_sha256,
        "composed_geometry_program_sha256":record.composed_geometry_program_sha256,
        "proposal_candidate_id":record.proposal_candidate_id,
        "proposal_candidate_state_sha256":record.proposal_candidate_state_sha256,
        "proposal_artifact_sha256":record.proposal_artifact_sha256,
        "proposal_artifact_readback_object_sha256":record.proposal_artifact_readback_object_sha256,
        "current_base_artifact_readback_object_sha256":current_readback,
        "record_canonical_sha256":record.canonical_sha256,
        "status":record.status,
        "six_view_evaluation":"NOT_RUN_AWAITING_EXACT_6X9_AOV",
        "quality_status":"QUALITY_TARGET_NOT_MET",
        "candidate_confirm_allowed":false,
        "secondary_form_approved":"NOT_CREATED",
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    });
    let receipt_bytes =
        canonical_json_bytes(&receipt).map_err(|error| invalid(error.to_string()))?;
    let receipt_object = runtime.put_object(
        &receipt_bytes,
        None,
        "application/json",
        "production-weapon-form-art-composite-proposal-receipt",
    )?;
    record.receipt_object_sha256 = receipt_object.record.sha256.clone();
    let (stored, replayed) = runtime
        .store
        .record_production_weapon_form_art_composite_proposal_with_replay(
            &record,
            &receipt_object.record,
        )?;
    Ok(record_projection(
        &stored,
        PREPARE_RESULT_SCHEMA_VERSION,
        replayed,
        !replayed,
    ))
}

pub(crate) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let request = parse_get_request(request)?;
    let record = runtime
        .store
        .get_production_weapon_form_art_composite_proposal(
            &request.project_id,
            &request.proposal_id,
        )?
        .ok_or_else(|| invalid("durable composite proposal is unavailable"))?;
    Ok(record_projection(
        &record,
        GET_RESULT_SCHEMA_VERSION,
        true,
        false,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::production_weapon_assembly_parameter_mutator::production_weapon_stock_upper_profile_04z_station_isolation_trial_mutate;
    use crate::production_weapon_d1_seed;

    fn sha(seed: u8) -> String {
        format!("{:064x}", seed)
    }

    fn operation() -> CompositeProposalOperation {
        let mut operation = CompositeProposalOperation {
            schema_version: COMPOSITE_OPERATION_SCHEMA_VERSION.to_owned(),
            sequence_index: 0,
            operation_id: "operation-trigger-guard-aperture".to_owned(),
            operation_kind: REGISTERED_PROFILE_REPLACE.to_owned(),
            source_node_id: "trigger-guard".to_owned(),
            part_id: "trigger-guard".to_owned(),
            registered_profile_id: production_weapon_trigger_guard_aperture_profile_id().to_owned(),
            canonical_sha256: String::new(),
        };
        operation.canonical_sha256 = canonical_without_declared_hash(&operation).unwrap();
        operation
    }

    #[test]
    fn composite_plan_preserves_current_base_edit_and_adds_disjoint_trigger_aperture() {
        let original = production_weapon_d1_seed::materialize("project-composite").unwrap();
        let current_base =
            production_weapon_stock_upper_profile_04z_station_isolation_trial_mutate(&original, 0)
                .unwrap();
        let node = |program: &Value, node_id: &str| {
            program["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|node| node["node_id"] == node_id)
                .unwrap()
                .clone()
        };
        assert_ne!(
            node(&original, "rear-stock"),
            node(&current_base, "rear-stock")
        );
        let current_base_sha = current_base["canonical_sha256"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut plan = CompositeProposalPlan {
            schema_version: COMPOSITE_PLAN_SCHEMA_VERSION.to_owned(),
            project_id: "project-composite".to_owned(),
            original_source_candidate_id: "candidate-original".to_owned(),
            original_source_candidate_state_sha256: sha(1),
            original_source_artifact_sha256: sha(2),
            original_fresh_baseline_canonical_sha256: sha(3),
            current_base_candidate_id: "candidate-04bb".to_owned(),
            current_base_candidate_state_sha256: sha(4),
            current_base_artifact_sha256: sha(5),
            current_base_geometry_program_sha256: current_base_sha,
            current_base_proposal_evidence_sha256: sha(6),
            operations: vec![operation()],
            composition_policy: COMPOSITION_POLICY.to_owned(),
            canonical_sha256: String::new(),
        };
        plan.canonical_sha256 = canonical_without_declared_hash(&plan).unwrap();
        let composed = compose_current_base_geometry_program(&plan, &current_base).unwrap();
        assert!(validate_exact_composite_delta(&plan, &current_base, &composed).is_ok());
        assert_eq!(
            node(&composed, "rear-stock"),
            node(&current_base, "rear-stock")
        );
        let trigger = composed["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["node_id"] == "trigger-guard")
            .unwrap();
        assert_eq!(
            trigger["operator_id"],
            "forgecad.geometry.profile-extrude@1"
        );
        let compiled = forgecad_geometry_worker::compile_geometry_program(&composed).unwrap();
        assert_eq!(compiled.part_ids.len(), 23);
        let receipt = composite_source_receipt(&plan, &composed).unwrap();
        assert_eq!(receipt["current_base_candidate_id"], "candidate-04bb");
        assert_eq!(receipt["operation_count"], 1);
        assert_eq!(receipt["candidate_created"], false);
        assert_eq!(receipt["quality_status"], "QUALITY_TARGET_NOT_MET");
    }

    #[test]
    fn composite_plan_rejects_duplicate_or_unregistered_operations() {
        let mut first = operation();
        let mut second = operation();
        second.sequence_index = 1;
        second.operation_id = "operation-trigger-guard-aperture-second".to_owned();
        second.canonical_sha256 = canonical_without_declared_hash(&second).unwrap();
        let mut plan = CompositeProposalPlan {
            schema_version: COMPOSITE_PLAN_SCHEMA_VERSION.to_owned(),
            project_id: "project-composite".to_owned(),
            original_source_candidate_id: "candidate-original".to_owned(),
            original_source_candidate_state_sha256: sha(1),
            original_source_artifact_sha256: sha(2),
            original_fresh_baseline_canonical_sha256: sha(3),
            current_base_candidate_id: "candidate-04bb".to_owned(),
            current_base_candidate_state_sha256: sha(4),
            current_base_artifact_sha256: sha(5),
            current_base_geometry_program_sha256: sha(6),
            current_base_proposal_evidence_sha256: sha(7),
            operations: vec![first.clone(), second],
            composition_policy: COMPOSITION_POLICY.to_owned(),
            canonical_sha256: String::new(),
        };
        plan.canonical_sha256 = canonical_without_declared_hash(&plan).unwrap();
        assert!(validate_composite_proposal_plan(&plan).is_err());

        first.registered_profile_id = "caller-profile@1".to_owned();
        first.canonical_sha256 = canonical_without_declared_hash(&first).unwrap();
        plan.operations = vec![first];
        plan.canonical_sha256 = canonical_without_declared_hash(&plan).unwrap();
        assert!(validate_composite_proposal_plan(&plan).is_err());
    }

    #[test]
    fn composite_delta_rejects_unregistered_node_and_part_output_drift() {
        let original = production_weapon_d1_seed::materialize("project-composite").unwrap();
        let current_base =
            production_weapon_stock_upper_profile_04z_station_isolation_trial_mutate(&original, 0)
                .unwrap();
        let mut plan = CompositeProposalPlan {
            schema_version: COMPOSITE_PLAN_SCHEMA_VERSION.to_owned(),
            project_id: "project-composite".to_owned(),
            original_source_candidate_id: "candidate-original".to_owned(),
            original_source_candidate_state_sha256: sha(1),
            original_source_artifact_sha256: sha(2),
            original_fresh_baseline_canonical_sha256: sha(3),
            current_base_candidate_id: "candidate-04bb".to_owned(),
            current_base_candidate_state_sha256: sha(4),
            current_base_artifact_sha256: sha(5),
            current_base_geometry_program_sha256: current_base["canonical_sha256"]
                .as_str()
                .unwrap()
                .to_owned(),
            current_base_proposal_evidence_sha256: sha(6),
            operations: vec![operation()],
            composition_policy: COMPOSITION_POLICY.to_owned(),
            canonical_sha256: String::new(),
        };
        plan.canonical_sha256 = canonical_without_declared_hash(&plan).unwrap();
        let composed = compose_current_base_geometry_program(&plan, &current_base).unwrap();

        let mut unregistered_node_change = composed.clone();
        let rear_stock = unregistered_node_change["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["node_id"] == "rear-stock")
            .unwrap();
        rear_stock["operator_id"] = json!("forgecad.geometry.primitive@2");
        assert!(
            validate_exact_composite_delta(&plan, &current_base, &unregistered_node_change)
                .is_err()
        );

        let mut part_output_drift = composed.clone();
        part_output_drift["part_outputs"]
            .as_array_mut()
            .unwrap()
            .first_mut()
            .unwrap()["solid"] = Value::Bool(false);
        assert!(validate_exact_composite_delta(&plan, &current_base, &part_output_drift).is_err());
    }
}
