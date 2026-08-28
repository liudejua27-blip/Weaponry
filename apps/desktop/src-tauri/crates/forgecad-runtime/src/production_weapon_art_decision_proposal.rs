//! Read-only assembly-level art decision projection for FPS weapon form work.
//!
//! This module intentionally stops before geometry search. It binds the exact
//! candidate, authoring documents, CameraLock, FormEvidence and FormArt
//! receipts, then reports which closed assembly groups have a stable parameter
//! sink and which artistic evidence axes remain blocked. It never writes CAS or
//! SQLite, never invokes a Worker, and never creates or promotes a candidate.

use super::{canonical_json_hash, is_opaque_id, is_sha256, Runtime, RuntimeError};
use crate::production_weapon_assembly_parameter_mutator::production_weapon_assembly_parameter_descriptors;
use forgecad_contracts::{
    ProductionWeaponArtDecisionProposalAssemblyGroupDecision,
    ProductionWeaponArtDecisionProposalBlocker, ProductionWeaponArtDecisionProposalGateResult,
    ProductionWeaponArtDecisionProposalGetRequest, ProductionWeaponArtDecisionProposalGetResult,
    ProductionWeaponArtDecisionProposalViewBinding, ProductionWeaponAssemblyDecisionRegistry,
    ProductionWeaponAssemblyDecisionRegistryGroup, ProductionWeaponFormArtEvidenceNegativeSpaceRow,
    PRODUCTION_WEAPON_ART_DECISION_PROPOSAL_GATE_IDS,
    PRODUCTION_WEAPON_ART_DECISION_PROPOSAL_GET_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_ART_DECISION_PROPOSAL_GET_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_ART_DECISION_PROPOSAL_OBJECTIVE_POLICY,
    PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_POLICY,
    PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_PROFILE_ID,
    PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_SCHEMA_VERSION,
    PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_VIEW_KINDS,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const REGISTRY_ID: &str = "fps-weapon-assembly-decision-registry";
const REQUIRED_VIEWS: [&str; 6] = [
    "front",
    "back",
    "left",
    "right",
    "top",
    "rear-three-quarter",
];
const REQUEST_FIELDS: [&str; 22] = [
    "schema_version",
    "session_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "artifact_id",
    "artifact_sha256",
    "geometry_program_sha256",
    "geometry_program_canonical_sha256",
    "operator_catalog_sha256",
    "reference_canvas_canonical_sha256",
    "design_spec_canonical_sha256",
    "camera_lock_id",
    "camera_lock_canonical_sha256",
    "form_evidence_id",
    "form_evidence_object_sha256",
    "form_evidence_canonical_sha256",
    "form_art_evidence_id",
    "form_art_evidence_object_sha256",
    "form_art_evidence_canonical_sha256",
    "first_person_profile_id",
    "first_person_profile_sha256",
];
const ALLOWED_OPERATOR_IDS: [&str; 28] = [
    "forgecad.geometry.primitive@2",
    "forgecad.geometry.profile-extrude@1",
    "forgecad.geometry.profile-loft@1",
    "forgecad.geometry.profile-loft@2",
    "forgecad.geometry.multi-loop-profile-loft@1",
    "forgecad.geometry.longitudinal-section-loft@1",
    "forgecad.geometry.subd-cage@1",
    "forgecad.geometry.subd-cage@2",
    "forgecad.geometry.authoring-mesh@1",
    "forgecad.geometry.surface-patch@1",
    "forgecad.geometry.surface-shell@1",
    "forgecad.geometry.revolve@1",
    "forgecad.geometry.tube-sweep@1",
    "forgecad.geometry.transform@2",
    "forgecad.geometry.mirror@1",
    "forgecad.geometry.array@1",
    "forgecad.geometry.bevel@1",
    "forgecad.geometry.bevel@2",
    "forgecad.geometry.normal-policy@1",
    "forgecad.geometry.panel@1",
    "forgecad.geometry.panel@2",
    "forgecad.geometry.vent-array@1",
    "forgecad.geometry.vent-array@2",
    "forgecad.geometry.recessed-channel@1",
    "forgecad.geometry.energy-core@1",
    "forgecad.geometry.joint-stack@1",
    "forgecad.geometry.part-output@1",
    "forgecad.geometry.boolean@1",
];

#[derive(Clone, Copy)]
struct GroupSpec {
    group_id: &'static str,
    part_ids: &'static [&'static str],
    parameter_ids: &'static [&'static str],
    invariants: &'static [&'static str],
    priority: u64,
}

const GROUPS: [GroupSpec; 5] = [
    GroupSpec {
        group_id: "receiver-envelope",
        part_ids: &["receiver-main", "receiver-upper", "receiver-lower"],
        parameter_ids: &[
            "receiver-envelope-width",
            "receiver-envelope-height",
            "receiver-envelope-shoulder",
        ],
        invariants: &["shared-axis", "clearance-min"],
        priority: 1,
    },
    GroupSpec {
        group_id: "muzzle-axis",
        part_ids: &[
            "muzzle-shroud",
            "muzzle-emitter",
            "muzzle-core",
            "energy-ring",
            "energy-core",
            "core-housing",
        ],
        parameter_ids: &[
            "muzzle-axis-shroud-envelope",
            "muzzle-axis-emitter-envelope",
            "muzzle-axis-core-aperture",
        ],
        invariants: &["shared-axis", "coaxial", "clearance-min"],
        priority: 2,
    },
    GroupSpec {
        group_id: "stock-open-frame",
        part_ids: &["rear-stock", "rear-cap", "underbrace"],
        parameter_ids: &["stock-open-frame-clearance", "stock-open-frame-angle"],
        invariants: &["enclosed-void", "clearance-min"],
        priority: 3,
    },
    GroupSpec {
        group_id: "trigger-void",
        part_ids: &["trigger-guard", "grip", "magazine"],
        parameter_ids: &["trigger-void-clearance", "trigger-void-centroid"],
        invariants: &["enclosed-void", "clearance-min"],
        priority: 4,
    },
    GroupSpec {
        group_id: "rail-spine",
        part_ids: &["top-fin", "top-rail", "bottom-rail"],
        parameter_ids: &["rail-spine-continuity", "rail-spine-offset"],
        invariants: &["continuous-spine", "shared-axis"],
        priority: 5,
    },
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(message.into())
}

fn parse_request(
    value: &Value,
) -> Result<ProductionWeaponArtDecisionProposalGetRequest, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("art decision request must be an object"))?;
    if object.len() != REQUEST_FIELDS.len()
        || object
            .keys()
            .any(|field| !REQUEST_FIELDS.contains(&field.as_str()))
        || REQUEST_FIELDS
            .iter()
            .any(|field| !object.contains_key(*field))
    {
        return Err(invalid("ART_DECISION_REQUEST_FIELDS_INVALID"));
    }
    let request: ProductionWeaponArtDecisionProposalGetRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("ART_DECISION_REQUEST_INVALID: {error}")))?;
    if request.schema_version != PRODUCTION_WEAPON_ART_DECISION_PROPOSAL_GET_REQUEST_SCHEMA_VERSION
    {
        return Err(invalid("ART_DECISION_REQUEST_SCHEMA_INVALID"));
    }
    for id in [
        &request.session_id,
        &request.project_id,
        &request.candidate_id,
        &request.artifact_id,
        &request.camera_lock_id,
        &request.form_evidence_id,
        &request.form_art_evidence_id,
    ] {
        if !is_opaque_id(id) {
            return Err(invalid("ART_DECISION_REQUEST_ID_INVALID"));
        }
    }
    for hash in [
        &request.candidate_state_sha256,
        &request.artifact_sha256,
        &request.geometry_program_sha256,
        &request.geometry_program_canonical_sha256,
        &request.operator_catalog_sha256,
        &request.reference_canvas_canonical_sha256,
        &request.design_spec_canonical_sha256,
        &request.camera_lock_canonical_sha256,
        &request.form_evidence_object_sha256,
        &request.form_evidence_canonical_sha256,
        &request.form_art_evidence_object_sha256,
        &request.form_art_evidence_canonical_sha256,
    ] {
        if !is_sha256(hash) {
            return Err(invalid("ART_DECISION_REQUEST_HASH_INVALID"));
        }
    }
    match (
        &request.first_person_profile_id,
        &request.first_person_profile_sha256,
    ) {
        (None, None) => {}
        (Some(id), Some(hash)) if is_opaque_id(id) && is_sha256(hash) => {}
        _ => return Err(invalid("ART_DECISION_FIRST_PERSON_BINDING_INVALID")),
    }
    Ok(request)
}

fn canonical_document(document: &Value, schema: &str) -> Result<String, RuntimeError> {
    if document.get("schema_version").and_then(Value::as_str) != Some(schema) {
        return Err(invalid("ART_DECISION_AUTHORING_SCHEMA_INVALID"));
    }
    let actual = document
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| invalid("ART_DECISION_AUTHORING_CANONICAL_MISSING"))?;
    let mut normalized = document.clone();
    normalized["canonical_sha256"] = Value::String(String::new());
    let expected = canonical_json_hash(&normalized);
    if actual != expected {
        return Err(invalid("ART_DECISION_AUTHORING_CANONICAL_INVALID"));
    }
    Ok(expected)
}

fn program_parts(program: &Value) -> Result<BTreeMap<String, Vec<String>>, RuntimeError> {
    let outputs = program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("ART_DECISION_PROGRAM_PART_OUTPUTS_MISSING"))?;
    let mut parts = BTreeMap::new();
    for output in outputs {
        let part_id = output
            .get("part_id")
            .and_then(Value::as_str)
            .filter(|id| is_opaque_id(id))
            .ok_or_else(|| invalid("ART_DECISION_PROGRAM_PART_ID_INVALID"))?;
        let nodes = output
            .get("input_node_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("ART_DECISION_PROGRAM_PART_NODES_INVALID"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|id| is_opaque_id(id))
                    .map(str::to_owned)
                    .ok_or_else(|| invalid("ART_DECISION_PROGRAM_NODE_ID_INVALID"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if nodes.is_empty() || parts.insert(part_id.to_owned(), nodes).is_some() {
            return Err(invalid("ART_DECISION_PROGRAM_PART_BINDING_INVALID"));
        }
    }
    Ok(parts)
}

fn node_operators(program: &Value) -> Result<BTreeMap<String, String>, RuntimeError> {
    let nodes = program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("ART_DECISION_PROGRAM_NODES_MISSING"))?;
    let mut result = BTreeMap::new();
    for node in nodes {
        let node_id = node
            .get("node_id")
            .and_then(Value::as_str)
            .filter(|id| is_opaque_id(id))
            .ok_or_else(|| invalid("ART_DECISION_PROGRAM_NODE_ID_INVALID"))?;
        let operator_id = node
            .get("operator_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("ART_DECISION_PROGRAM_OPERATOR_MISSING"))?;
        if !ALLOWED_OPERATOR_IDS.contains(&operator_id) {
            return Err(invalid("ART_DECISION_PROGRAM_OPERATOR_OUTSIDE_REGISTRY"));
        }
        if result
            .insert(node_id.to_owned(), operator_id.to_owned())
            .is_some()
        {
            return Err(invalid("ART_DECISION_PROGRAM_NODE_DUPLICATE"));
        }
    }
    Ok(result)
}

pub(crate) fn build_registry(
    operator_catalog_sha256: &str,
    program: &Value,
) -> Result<
    (
        ProductionWeaponAssemblyDecisionRegistry,
        Vec<ProductionWeaponArtDecisionProposalAssemblyGroupDecision>,
    ),
    RuntimeError,
> {
    let parts = program_parts(program)?;
    let operators = node_operators(program)?;
    let mut groups = Vec::with_capacity(GROUPS.len());
    let mut decisions = Vec::with_capacity(GROUPS.len());
    for spec in GROUPS {
        let mut source_node_ids = BTreeSet::new();
        let mut allowed_operator_ids = BTreeSet::new();
        let mut complete = true;
        for part_id in spec.part_ids {
            let Some(node_ids) = parts.get(*part_id) else {
                complete = false;
                continue;
            };
            for node_id in node_ids {
                source_node_ids.insert(node_id.clone());
                if let Some(operator_id) = operators.get(node_id) {
                    allowed_operator_ids.insert(operator_id.clone());
                } else {
                    complete = false;
                }
            }
        }
        if source_node_ids.is_empty() || allowed_operator_ids.is_empty() {
            return Err(invalid("ART_DECISION_ASSEMBLY_GROUP_EMPTY"));
        }
        let source_node_ids = source_node_ids.into_iter().collect::<Vec<_>>();
        let allowed_operator_ids = allowed_operator_ids.into_iter().collect::<Vec<_>>();
        let group = ProductionWeaponAssemblyDecisionRegistryGroup {
            group_id: spec.group_id.to_owned(),
            intent_kind: spec.group_id.to_owned(),
            part_ids: spec
                .part_ids
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            source_node_ids: source_node_ids.clone(),
            parameter_ids: spec
                .parameter_ids
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            allowed_operator_ids: allowed_operator_ids.clone(),
            coupling_mode: "linked".to_owned(),
            invariants: spec
                .invariants
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            affected_view_kinds: PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_VIEW_KINDS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            priority: spec.priority,
        };
        let blocker_codes = if complete {
            vec!["BLOCKED_PARAMETER_SINK".to_owned()]
        } else {
            vec!["BLOCKED_ASSEMBLY_REGISTRY".to_owned()]
        };
        decisions.push(ProductionWeaponArtDecisionProposalAssemblyGroupDecision {
            group_id: group.group_id.clone(),
            status: if complete {
                "BLOCKED_PARAMETER_SINK".to_owned()
            } else {
                "BLOCKED_ASSEMBLY_REGISTRY".to_owned()
            },
            part_ids: group.part_ids.clone(),
            source_node_ids: source_node_ids.clone(),
            parameter_ids: group.parameter_ids.clone(),
            allowed_operator_ids: allowed_operator_ids.clone(),
            coupling_mode: group.coupling_mode.clone(),
            invariants: group.invariants.clone(),
            affected_view_kinds: group.affected_view_kinds.clone(),
            blocker_codes,
        });
        groups.push(group);
    }
    let mut registry = ProductionWeaponAssemblyDecisionRegistry {
        schema_version: PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_SCHEMA_VERSION.to_owned(),
        registry_id: REGISTRY_ID.to_owned(),
        profile_id: PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_PROFILE_ID.to_owned(),
        operator_catalog_sha256: operator_catalog_sha256.to_owned(),
        registry_policy: PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_POLICY.to_owned(),
        groups,
        canonical_sha256: String::new(),
    };
    registry.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&registry).map_err(|error| invalid(error.to_string()))?,
    );
    Ok((registry, decisions))
}

fn apply_parameter_sink_availability(
    decisions: &mut [ProductionWeaponArtDecisionProposalAssemblyGroupDecision],
    program: &Value,
    geometry_program_canonical_sha256: &str,
) -> Result<(), RuntimeError> {
    let sink_descriptors = production_weapon_assembly_parameter_descriptors(
        program,
        geometry_program_canonical_sha256,
    )?;
    let available_parameter_ids = sink_descriptors
        .available
        .iter()
        .map(|descriptor| descriptor.parameter_id.as_str())
        .collect::<BTreeSet<_>>();
    for decision in decisions {
        if decision.status != "BLOCKED_ASSEMBLY_REGISTRY"
            && decision
                .parameter_ids
                .iter()
                .all(|parameter_id| available_parameter_ids.contains(parameter_id.as_str()))
        {
            decision.status = "READY_FOR_SEARCH".to_owned();
            decision.blocker_codes.clear();
        }
    }
    Ok(())
}

fn add_blocker(
    blockers: &mut Vec<ProductionWeaponArtDecisionProposalBlocker>,
    blocker_code: &str,
    scope: &str,
    group_id: Option<&str>,
    view_kind: Option<&str>,
    evidence_sha256: Option<&str>,
) {
    blockers.push(ProductionWeaponArtDecisionProposalBlocker {
        blocker_code: blocker_code.to_owned(),
        scope: scope.to_owned(),
        group_id: group_id.map(str::to_owned),
        view_kind: view_kind.map(str::to_owned),
        evidence_sha256: evidence_sha256.map(str::to_owned),
    });
}

fn negative_space_gate_pass(
    view_kind: &str,
    visual_structure_review_status: &str,
    negative_space_status: &str,
    rows: &[ProductionWeaponFormArtEvidenceNegativeSpaceRow],
) -> bool {
    if visual_structure_review_status != "user_confirmed" {
        return false;
    }
    let required_structure_ids: &[&str] = match view_kind {
        "left" => &["left.trigger-void", "left.open-stock-void"],
        "right" => &["right.trigger-void", "right.open-stock-void"],
        "rear-three-quarter" => &["rear3q.trigger-void", "rear3q.open-stock-void"],
        "front" | "back" | "top" => {
            return negative_space_status == "not-applicable" && rows.is_empty();
        }
        _ => return false,
    };
    if negative_space_status != "observed" || rows.len() != required_structure_ids.len() {
        return false;
    }
    let observed_ids = rows
        .iter()
        .map(|row| row.structure_id.as_str())
        .collect::<BTreeSet<_>>();
    if observed_ids.len() != rows.len()
        || required_structure_ids
            .iter()
            .any(|required| !observed_ids.contains(required))
    {
        return false;
    }
    rows.iter().all(|row| {
        row.status == "observed"
            && !row.sealed
            && !row.missing
            && row.iou_milli >= 850
            && row.boundary_f1_milli >= 800
            && (850..=1150).contains(&row.area_ratio_milli)
            && row.centroid_error_milli <= 3000
    })
}

fn gate(
    gate_id: &str,
    status: &str,
    evidence_sha256: Option<&str>,
    blocker_codes: &[&str],
) -> ProductionWeaponArtDecisionProposalGateResult {
    ProductionWeaponArtDecisionProposalGateResult {
        gate_id: gate_id.to_owned(),
        status: status.to_owned(),
        evidence_sha256: evidence_sha256.map(str::to_owned),
        blocker_codes: blocker_codes
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    }
}

fn status_from_blockers(blockers: &[ProductionWeaponArtDecisionProposalBlocker]) -> String {
    for code in [
        "BLOCKED_LINEAGE",
        "BLOCKED_REFERENCE_ANNOTATION",
        "BLOCKED_CAMERA",
        "BLOCKED_ASSEMBLY_REGISTRY",
        "BLOCKED_PARAMETER_SINK",
        "BLOCKED_NEGATIVE_SPACE",
        "BLOCKED_LINE_FLOW",
        "BLOCKED_FIRST_PERSON_PROFILE",
        "NO_STRICT_MULTI_VIEW_IMPROVEMENT",
    ] {
        if blockers.iter().any(|blocker| blocker.blocker_code == code) {
            return code.to_owned();
        }
    }
    "READY_ASSEMBLY_FORM_SEARCH".to_owned()
}

impl Runtime {
    /// Return a deterministic, read-only assembly art-decision projection.
    pub fn production_weapon_art_decision_proposal_get(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let request = parse_request(&value)?;
        let session = self
            .store
            .get_agentic_session(&request.session_id)?
            .ok_or_else(|| invalid("ART_DECISION_SESSION_NOT_FOUND"))?;
        if session.project_id != request.project_id
            || session.candidate_id != request.candidate_id
            || session.candidate_state_sha256 != request.candidate_state_sha256
        {
            return Err(invalid("ART_DECISION_SESSION_LINEAGE_MISMATCH"));
        }
        let candidate = self
            .candidate(&request.candidate_id)?
            .ok_or_else(|| invalid("ART_DECISION_CANDIDATE_NOT_FOUND"))?;
        let candidate_artifact_sha256 = candidate
            .manifest_hash
            .as_deref()
            .or(candidate.prepared_object_sha256.as_deref());
        if candidate.project_id != request.project_id
            || candidate.canonical_sha256 != request.candidate_state_sha256
            || candidate.prepared_object_id.as_deref() != Some(request.artifact_id.as_str())
            || candidate_artifact_sha256 != Some(request.artifact_sha256.as_str())
        {
            return Err(invalid("ART_DECISION_CANDIDATE_LINEAGE_MISMATCH"));
        }
        let geometry = self
            .store
            .get_geometry_candidate_evidence(&request.candidate_id)?
            .ok_or_else(|| invalid("ART_DECISION_GEOMETRY_EVIDENCE_NOT_FOUND"))?;
        if geometry.project_id != request.project_id
            || geometry.artifact_object_sha256 != request.artifact_sha256
            || geometry.geometry_program_object_sha256 != request.geometry_program_sha256
            || geometry.geometry_program_sha256 != request.geometry_program_canonical_sha256
            || geometry.operator_catalog_sha256 != request.operator_catalog_sha256
        {
            return Err(invalid("ART_DECISION_GEOMETRY_LINEAGE_MISMATCH"));
        }
        let program_bytes = self.cas_read(&request.geometry_program_sha256)?;
        let program: Value = serde_json::from_slice(&program_bytes)
            .map_err(|error| invalid(format!("ART_DECISION_GEOMETRY_PROGRAM_INVALID: {error}")))?;
        if program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2") {
            return Err(invalid("ART_DECISION_GEOMETRY_PROGRAM_BINDING_MISMATCH"));
        }
        let mut program_normalized = program.clone();
        if let Some(object) = program_normalized.as_object_mut() {
            if let Some(declared) = object.remove("canonical_sha256") {
                if declared.as_str() != Some(request.geometry_program_canonical_sha256.as_str()) {
                    return Err(invalid("ART_DECISION_GEOMETRY_PROGRAM_BINDING_MISMATCH"));
                }
            }
        }
        if canonical_json_hash(&program_normalized) != request.geometry_program_canonical_sha256 {
            return Err(invalid("ART_DECISION_GEOMETRY_PROGRAM_CANONICAL_MISMATCH"));
        }

        // Read the immutable authoring documents from the durable session
        // instead of asking session_get to recompute the mutable latest visual
        // observation. CrossView/FormArt evidence may be produced after the
        // session snapshot; that must not invalidate its original Canvas and
        // DesignSpec object bindings.
        let durable_session = self
            .store
            .get_agentic_session(&request.session_id)?
            .ok_or_else(|| invalid("ART_DECISION_SESSION_NOT_FOUND"))?;
        if durable_session.project_id != request.project_id
            || durable_session.candidate_id != request.candidate_id
            || durable_session.candidate_state_sha256 != request.candidate_state_sha256
            || !is_sha256(&durable_session.reference_canvas_sha256)
            || !is_sha256(&durable_session.design_spec_sha256)
        {
            return Err(invalid("ART_DECISION_SESSION_LINEAGE_MISMATCH"));
        }
        let canvas_bytes = self.cas_read(&durable_session.reference_canvas_sha256)?;
        let canvas: Value = serde_json::from_slice(&canvas_bytes)
            .map_err(|error| invalid(format!("ART_DECISION_REFERENCE_CANVAS_INVALID: {error}")))?;
        let spec_bytes = self.cas_read(&durable_session.design_spec_sha256)?;
        let spec: Value = serde_json::from_slice(&spec_bytes)
            .map_err(|error| invalid(format!("ART_DECISION_DESIGN_SPEC_INVALID: {error}")))?;
        if canonical_document(&canvas, "ReferenceCanvas@1")?
            != request.reference_canvas_canonical_sha256
            || canonical_document(&spec, "DesignSpec@1")? != request.design_spec_canonical_sha256
        {
            return Err(invalid("ART_DECISION_AUTHORING_LINEAGE_MISMATCH"));
        }

        let camera_result = self.production_camera_lock_get(json!({
            "schema_version":"ProductionCameraLockGetRequest@1",
            "camera_lock_id":request.camera_lock_id,
            "session_id":request.session_id,
            "project_id":request.project_id,
            "candidate_id":request.candidate_id
        }))?;
        let camera_lock = camera_result
            .get("camera_lock")
            .ok_or_else(|| invalid("ART_DECISION_CAMERA_LOCK_MISSING"))?;
        if camera_lock.get("canonical_sha256").and_then(Value::as_str)
            != Some(request.camera_lock_canonical_sha256.as_str())
        {
            return Err(invalid("ART_DECISION_CAMERA_LINEAGE_MISMATCH"));
        }

        self.production_weapon_form_evidence_get(json!({
            "schema_version":"ProductionWeaponFormEvidenceGetRequest@1",
            "form_evidence_id":request.form_evidence_id,
            "session_id":request.session_id,
            "project_id":request.project_id,
            "candidate_id":request.candidate_id
        }))?;
        let form = self
            .store
            .get_production_weapon_form_evidence(&request.form_evidence_id)?
            .ok_or_else(|| invalid("ART_DECISION_FORM_EVIDENCE_NOT_FOUND"))?;
        if form.receipt_object_sha256 != request.form_evidence_object_sha256
            || form.canonical_sha256 != request.form_evidence_canonical_sha256
            || form.camera_lock_id != request.camera_lock_id
            || form.camera_lock_canonical_sha256 != request.camera_lock_canonical_sha256
            || form.reference_canvas_canonical_sha256 != request.reference_canvas_canonical_sha256
            || form.design_spec_canonical_sha256 != request.design_spec_canonical_sha256
        {
            return Err(invalid("ART_DECISION_FORM_EVIDENCE_LINEAGE_MISMATCH"));
        }
        self.production_weapon_form_art_evidence_get(json!({
            "schema_version":"ProductionWeaponFormArtEvidenceGetRequest@1",
            "art_evidence_id":request.form_art_evidence_id,
            "session_id":request.session_id,
            "project_id":request.project_id,
            "candidate_id":request.candidate_id
        }))?;
        let art = self
            .store
            .get_production_weapon_form_art_evidence(&request.form_art_evidence_id)?
            .ok_or_else(|| invalid("ART_DECISION_FORM_ART_EVIDENCE_NOT_FOUND"))?;
        if art.receipt_object_sha256 != request.form_art_evidence_object_sha256
            || art.canonical_sha256 != request.form_art_evidence_canonical_sha256
            || art.form_evidence_object_sha256 != request.form_evidence_object_sha256
            || art.form_evidence_canonical_sha256 != request.form_evidence_canonical_sha256
            || art.camera_lock_id != request.camera_lock_id
            || art.camera_lock_canonical_sha256 != request.camera_lock_canonical_sha256
        {
            return Err(invalid("ART_DECISION_FORM_ART_LINEAGE_MISMATCH"));
        }

        let (registry, mut assembly_group_decisions) =
            build_registry(&request.operator_catalog_sha256, &program)?;
        // A registry row is only search-ready when the same product-owned
        // resolver used by the pure mutator can bind every parameter in the
        // group.  This keeps the art decision aligned with executable typed
        // semantics without exposing a JSON path or claiming that the still
        // unavailable stock/trigger/rail controls exist.
        apply_parameter_sink_availability(
            &mut assembly_group_decisions,
            &program,
            &request.geometry_program_canonical_sha256,
        )?;
        let mut blockers = Vec::new();
        let mut view_bindings = Vec::with_capacity(REQUIRED_VIEWS.len());
        let mut annotations_pass = true;
        let mut negative_pass = true;
        let mut line_pass = true;
        for view_kind in REQUIRED_VIEWS {
            let form_view = form
                .views
                .iter()
                .find(|view| view.view_kind == view_kind)
                .ok_or_else(|| invalid("ART_DECISION_FORM_VIEW_MISSING"))?;
            let art_view = art
                .views
                .iter()
                .find(|view| view.view_kind == view_kind)
                .ok_or_else(|| invalid("ART_DECISION_FORM_ART_VIEW_MISSING"))?;
            if form_view.view_id != art_view.view_id
                || form_view.reference_id != art_view.reference_id
                || form_view.reference_sha256 != art_view.reference_sha256
                || form_view.camera_hash != art_view.camera_hash
                || form_view.camera_canonical_sha256 != art_view.camera_canonical_sha256
                || form_view.receipt_object_sha256
                    != art_view.form_evidence_view_receipt_object_sha256
                || form_view.canonical_sha256
                    != art_view.form_evidence_view_receipt_canonical_sha256
            {
                return Err(invalid("ART_DECISION_VIEW_LINEAGE_MISMATCH"));
            }
            if art_view.visual_structure_review_status != "user_confirmed" {
                annotations_pass = false;
                add_blocker(
                    &mut blockers,
                    "BLOCKED_REFERENCE_ANNOTATION",
                    "view",
                    None,
                    Some(view_kind),
                    Some(&art_view.target_object_sha256),
                );
            }
            if !negative_space_gate_pass(
                view_kind,
                &art_view.visual_structure_review_status,
                &art_view.negative_space_status,
                &art_view.negative_space_rows,
            ) {
                negative_pass = false;
                add_blocker(
                    &mut blockers,
                    "BLOCKED_NEGATIVE_SPACE",
                    "view",
                    None,
                    Some(view_kind),
                    Some(&art_view.receipt_object_sha256),
                );
            }
            if art_view.line_flow_status != "observed" {
                line_pass = false;
                add_blocker(
                    &mut blockers,
                    "BLOCKED_LINE_FLOW",
                    "view",
                    None,
                    Some(view_kind),
                    Some(&art_view.receipt_object_sha256),
                );
            }
            view_bindings.push(ProductionWeaponArtDecisionProposalViewBinding {
                view_kind: view_kind.to_owned(),
                view_id: form_view.view_id.clone(),
                reference_id: form_view.reference_id.clone(),
                reference_sha256: form_view.reference_sha256.clone(),
                camera_hash: form_view.camera_hash.clone(),
                camera_canonical_sha256: form_view.camera_canonical_sha256.clone(),
                render_set_object_sha256: form_view.render_set_object_sha256.clone(),
                render_set_canonical_sha256: form_view.render_set_canonical_sha256.clone(),
                form_evidence_view_receipt_object_sha256: form_view.receipt_object_sha256.clone(),
                form_evidence_view_receipt_canonical_sha256: form_view.canonical_sha256.clone(),
                form_art_evidence_view_receipt_object_sha256: art_view
                    .receipt_object_sha256
                    .clone(),
                form_art_evidence_view_receipt_canonical_sha256: art_view.canonical_sha256.clone(),
                target_sha256: art_view.target_object_sha256.clone(),
                visual_structure_canonical_sha256: art_view
                    .visual_structure_canonical_sha256
                    .clone(),
                part_id_status: art_view.part_id_status.clone(),
                negative_space_status: art_view.negative_space_status.clone(),
                line_flow_status: art_view.line_flow_status.clone(),
                view_observation_status: art_view.view_observation_status.clone(),
            });
        }

        for decision in &assembly_group_decisions {
            for code in &decision.blocker_codes {
                add_blocker(
                    &mut blockers,
                    code,
                    "assembly",
                    Some(&decision.group_id),
                    None,
                    Some(&registry.canonical_sha256),
                );
            }
        }
        let first_person_pass = request.first_person_profile_id.is_some();
        if !first_person_pass {
            add_blocker(
                &mut blockers,
                "BLOCKED_FIRST_PERSON_PROFILE",
                "global",
                None,
                None,
                None,
            );
        }
        let registry_complete = assembly_group_decisions
            .iter()
            .all(|decision| decision.status != "BLOCKED_ASSEMBLY_REGISTRY");
        let parameter_sinks_ready = assembly_group_decisions
            .iter()
            .all(|decision| decision.status == "READY_FOR_SEARCH");
        let gate_results = vec![
            gate("lineage", "PASS", Some(&geometry.canonical_sha256), &[]),
            gate(
                "reference-annotation",
                if annotations_pass { "PASS" } else { "BLOCKED" },
                Some(&art.canonical_sha256),
                if annotations_pass {
                    &[]
                } else {
                    &["BLOCKED_REFERENCE_ANNOTATION"]
                },
            ),
            gate(
                "camera",
                "PASS",
                Some(&request.camera_lock_canonical_sha256),
                &[],
            ),
            gate(
                "assembly-registry",
                if registry_complete { "PASS" } else { "BLOCKED" },
                Some(&registry.canonical_sha256),
                if registry_complete {
                    &[]
                } else {
                    &["BLOCKED_ASSEMBLY_REGISTRY"]
                },
            ),
            gate(
                "parameter-sink",
                if parameter_sinks_ready {
                    "PASS"
                } else {
                    "BLOCKED"
                },
                Some(&registry.canonical_sha256),
                if parameter_sinks_ready {
                    &[]
                } else {
                    &["BLOCKED_PARAMETER_SINK"]
                },
            ),
            gate(
                "negative-space",
                if negative_pass { "PASS" } else { "BLOCKED" },
                Some(&art.canonical_sha256),
                if negative_pass {
                    &[]
                } else {
                    &["BLOCKED_NEGATIVE_SPACE"]
                },
            ),
            gate(
                "line-flow",
                if line_pass { "PASS" } else { "BLOCKED" },
                Some(&art.canonical_sha256),
                if line_pass {
                    &[]
                } else {
                    &["BLOCKED_LINE_FLOW"]
                },
            ),
            gate(
                "first-person-readability",
                if first_person_pass { "PASS" } else { "BLOCKED" },
                request.first_person_profile_sha256.as_deref(),
                if first_person_pass {
                    &[]
                } else {
                    &["BLOCKED_FIRST_PERSON_PROFILE"]
                },
            ),
            gate("candidate-search-critic", "NOT_RUN", None, &[]),
            gate("surface-scope", "LOCKED", None, &[]),
        ];
        debug_assert_eq!(
            gate_results
                .iter()
                .map(|gate| gate.gate_id.as_str())
                .collect::<Vec<_>>(),
            PRODUCTION_WEAPON_ART_DECISION_PROPOSAL_GATE_IDS
        );

        let proposal_status = status_from_blockers(&blockers);
        let mut result = ProductionWeaponArtDecisionProposalGetResult {
            schema_version: PRODUCTION_WEAPON_ART_DECISION_PROPOSAL_GET_RESULT_SCHEMA_VERSION
                .to_owned(),
            proposal_projection_id: format!(
                "art-decision-{}",
                &request.candidate_id[..request.candidate_id.len().min(96)]
            ),
            session_id: request.session_id,
            project_id: request.project_id,
            candidate_id: request.candidate_id,
            candidate_state_sha256: request.candidate_state_sha256,
            artifact_id: request.artifact_id,
            artifact_sha256: request.artifact_sha256,
            geometry_program_sha256: request.geometry_program_sha256,
            geometry_program_canonical_sha256: request.geometry_program_canonical_sha256,
            operator_catalog_sha256: request.operator_catalog_sha256,
            assembly_registry_id: registry.registry_id,
            assembly_registry_canonical_sha256: registry.canonical_sha256,
            reference_canvas_canonical_sha256: request.reference_canvas_canonical_sha256,
            design_spec_canonical_sha256: request.design_spec_canonical_sha256,
            camera_lock_id: request.camera_lock_id,
            camera_lock_canonical_sha256: request.camera_lock_canonical_sha256,
            form_evidence_id: request.form_evidence_id,
            form_evidence_object_sha256: request.form_evidence_object_sha256,
            form_evidence_canonical_sha256: request.form_evidence_canonical_sha256,
            form_art_evidence_id: request.form_art_evidence_id,
            form_art_evidence_object_sha256: request.form_art_evidence_object_sha256,
            form_art_evidence_canonical_sha256: request.form_art_evidence_canonical_sha256,
            first_person_profile_id: request.first_person_profile_id,
            first_person_profile_sha256: request.first_person_profile_sha256,
            view_bindings,
            assembly_group_decisions,
            objective_policy: PRODUCTION_WEAPON_ART_DECISION_PROPOSAL_OBJECTIVE_POLICY.to_owned(),
            gate_results,
            blockers,
            proposal_status,
            read_only: true,
            runtime_write_performed: false,
            worker_invoked: false,
            candidate_generated: false,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
            replayed: true,
            restart_hash_verified: true,
            canonical_sha256: String::new(),
        };
        result.canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&result).map_err(|error| invalid(error.to_string()))?,
        );
        serde_json::to_value(result).map_err(|error| invalid(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn negative_row(structure_id: &str) -> ProductionWeaponFormArtEvidenceNegativeSpaceRow {
        ProductionWeaponFormArtEvidenceNegativeSpaceRow {
            structure_id: structure_id.to_owned(),
            expected_region_canonical_sha256: "f".repeat(64),
            iou_milli: 900,
            boundary_f1_milli: 850,
            area_ratio_milli: 1000,
            centroid_error_milli: 1200,
            sealed: false,
            missing: false,
            status: "observed".to_owned(),
        }
    }

    #[test]
    fn art_decision_request_is_closed_and_first_person_binding_is_paired() {
        let mut request = json!({
            "schema_version":"ProductionWeaponArtDecisionProposalGetRequest@1",
            "session_id":"session-a","project_id":"project-a","candidate_id":"candidate-a",
            "candidate_state_sha256":"1".repeat(64),"artifact_id":"artifact-a","artifact_sha256":"2".repeat(64),
            "geometry_program_sha256":"3".repeat(64),"geometry_program_canonical_sha256":"4".repeat(64),
            "operator_catalog_sha256":"5".repeat(64),"reference_canvas_canonical_sha256":"7".repeat(64),
            "design_spec_canonical_sha256":"8".repeat(64),"camera_lock_id":"camera-lock-a",
            "camera_lock_canonical_sha256":"9".repeat(64),"form_evidence_id":"form-a",
            "form_evidence_object_sha256":"a".repeat(64),"form_evidence_canonical_sha256":"b".repeat(64),
            "form_art_evidence_id":"art-a","form_art_evidence_object_sha256":"c".repeat(64),
            "form_art_evidence_canonical_sha256":"d".repeat(64),"first_person_profile_id":null,
            "first_person_profile_sha256":null
        });
        assert!(parse_request(&request).is_ok());
        request["path"] = Value::String("/tmp/forbidden".to_owned());
        assert!(parse_request(&request).is_err());
        request.as_object_mut().unwrap().remove("path");
        request["first_person_profile_id"] = Value::String("ads-profile".to_owned());
        assert!(parse_request(&request).is_err());
    }

    #[test]
    fn assembly_registry_is_closed_and_reports_missing_parameter_sinks() {
        let mut nodes = Vec::new();
        let mut outputs = Vec::new();
        for spec in GROUPS {
            for part_id in spec.part_ids {
                nodes.push(json!({"node_id":part_id,"operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box"}}));
                outputs.push(json!({"part_id":part_id,"input_node_ids":[part_id]}));
            }
        }
        let program = json!({"nodes":nodes,"part_outputs":outputs});
        let (registry, decisions) = build_registry(&"e".repeat(64), &program).unwrap();
        assert_eq!(registry.groups.len(), 5);
        assert_eq!(decisions.len(), 5);
        assert!(decisions
            .iter()
            .all(|decision| decision.status == "BLOCKED_PARAMETER_SINK"));
        assert_eq!(
            status_from_blockers(&[ProductionWeaponArtDecisionProposalBlocker {
                blocker_code: "BLOCKED_PARAMETER_SINK".to_owned(),
                scope: "global".to_owned(),
                group_id: None,
                view_kind: None,
                evidence_sha256: None
            }]),
            "BLOCKED_PARAMETER_SINK"
        );
    }

    #[test]
    fn executable_receiver_and_muzzle_sinks_are_ready_while_unimplemented_groups_stay_blocked() {
        let mut program = crate::production_weapon_assembly_parameter_mutator::production_weapon_assembly_parameter_test_fixture();
        for part_id in [
            "energy-ring",
            "energy-core",
            "core-housing",
            "rear-stock",
            "rear-cap",
            "underbrace",
            "trigger-guard",
            "grip",
            "magazine",
            "top-fin",
            "top-rail",
            "bottom-rail",
        ] {
            let node_id = format!("{part_id}-node");
            program["nodes"].as_array_mut().unwrap().push(json!({
                "node_id":node_id,
                "operator_id":"forgecad.geometry.primitive@2",
                "inputs":[],
                "parameters":{
                    "shape":"box",
                    "size_m":[0.2,0.2,0.2],
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            }));
            program["part_outputs"].as_array_mut().unwrap().push(json!({
                "part_id":part_id,
                "input_node_ids":[format!("{part_id}-node")],
                "material_zone_id":"zone-mechanical",
                "solid":true
            }));
        }
        program.as_object_mut().unwrap().remove("canonical_sha256");
        let canonical = canonical_json_hash(&program);
        program["canonical_sha256"] = Value::String(canonical.clone());
        let (registry, mut decisions) = build_registry(
            &forgecad_worker_protocol::operator_catalog_sha256(),
            &program,
        )
        .expect("assembly registry");
        apply_parameter_sink_availability(&mut decisions, &program, &canonical)
            .expect("typed sink availability");

        assert_eq!(registry.groups.len(), 5);
        for group_id in ["receiver-envelope", "muzzle-axis"] {
            let decision = decisions
                .iter()
                .find(|decision| decision.group_id == group_id)
                .expect("supported assembly group");
            assert_eq!(decision.status, "READY_FOR_SEARCH");
            assert!(decision.blocker_codes.is_empty());
        }
        for group_id in ["stock-open-frame", "trigger-void", "rail-spine"] {
            let decision = decisions
                .iter()
                .find(|decision| decision.group_id == group_id)
                .expect("unsupported assembly group");
            assert_eq!(decision.status, "BLOCKED_PARAMETER_SINK");
            assert_eq!(decision.blocker_codes, ["BLOCKED_PARAMETER_SINK"]);
        }
    }

    #[test]
    fn negative_space_gate_requires_three_exact_two_void_views_and_three_confirmed_na_views() {
        for (view_kind, structure_ids) in [
            ("left", ["left.trigger-void", "left.open-stock-void"]),
            ("right", ["right.trigger-void", "right.open-stock-void"]),
            (
                "rear-three-quarter",
                ["rear3q.trigger-void", "rear3q.open-stock-void"],
            ),
        ] {
            let mut rows = structure_ids.map(negative_row).to_vec();
            assert!(negative_space_gate_pass(
                view_kind,
                "user_confirmed",
                "observed",
                &rows
            ));
            rows[0].boundary_f1_milli = 799;
            assert!(!negative_space_gate_pass(
                view_kind,
                "user_confirmed",
                "observed",
                &rows
            ));
        }
        for view_kind in ["front", "back", "top"] {
            assert!(negative_space_gate_pass(
                view_kind,
                "user_confirmed",
                "not-applicable",
                &[]
            ));
            assert!(!negative_space_gate_pass(
                view_kind,
                "unreviewed",
                "not-applicable",
                &[]
            ));
        }
        let duplicate = vec![
            negative_row("left.trigger-void"),
            negative_row("left.trigger-void"),
        ];
        assert!(!negative_space_gate_pass(
            "left",
            "user_confirmed",
            "observed",
            &duplicate
        ));
    }
}
