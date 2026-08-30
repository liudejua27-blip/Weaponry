//! Runtime-owned single-source-node FormArt authoring proposal.
//!
//! This is the Runtime-owned seam between real D1 FormArt evidence and the
//! durable `AuthoringMesh@2` typed authoring kernel. The read path revalidates an
//! existing source revision and returns a deterministic typed proposal. The
//! explicit prepare path materializes the child revision, replaces one bound
//! GeometryProgram source node, prepares a new worker-validated candidate and
//! evaluates the exact six FormArt views. Neither path confirms the candidate,
//! approves secondary form, advances production Stage, versions, or exports.

use super::{
    authoring_mesh_v2::{validate_source_binding, AuthoringMeshV2Revision},
    authoring_mesh_v2_geometry::authoring_mesh_v2_geometry_parameters,
    canonical_json_bytes, canonical_json_hash, compile_geometry_with_runtime_worker,
    hash_geometry_program_with_runtime_worker, is_opaque_id, is_sha256, strict_glb_inspection,
    validate_worker_metadata, Runtime, RuntimeError,
};
use forgecad_contracts::{
    build_cohort_sha256, AuthoringMeshMoveVerticesRequest, AuthoringMeshOpenFrameNotchRequest,
    AuthoringMeshRearStockVoidBoundaryBridgeRequest, AuthoringMeshRearStockVoidRailBowRequest,
    AuthoringMeshRevision, AuthoringMeshRevisionId, AuthoringMeshVertexId, CandidateRecord,
    GeometryCandidateEvidenceRecord, ProductionCameraLockRegistrationLineageRecord,
    ProductionWeaponFormArtBaselineRecord, ProductionWeaponFormArtEvidenceRecord,
    AUTHORING_MESH_V2_DURABLE_GET_REQUEST_SCHEMA_VERSION,
    AUTHORING_MESH_V2_DURABLE_PREPARE_REQUEST_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const REQUEST_SCHEMA_VERSION: &str = "ProductionWeaponFormArtMeshProposalGetRequest@1";
const RESULT_SCHEMA_VERSION: &str = "ProductionWeaponFormArtMeshProposalGetResult@1";
const PREPARE_RESULT_SCHEMA_VERSION: &str = "ProductionWeaponFormArtMeshProposalPrepareResult@1";
const MOVE_VERTICES_SCHEMA_VERSION: &str = "AuthoringMeshMoveVertices@1";
const OPEN_FRAME_NOTCH_SCHEMA_VERSION: &str = "AuthoringMeshOpenFrameNotch@1";
const REAR_STOCK_VOID_RAIL_BOW_SCHEMA_VERSION: &str = "AuthoringMeshRearStockVoidRailBow@1";
const REAR_STOCK_VOID_BOUNDARY_BRIDGE_SCHEMA_VERSION: &str =
    "AuthoringMeshRearStockVoidBoundaryBridge@1";
const CHILD_PROPOSAL_SCHEMA_VERSION: &str = "AuthoringMeshMoveVerticesChildRevisionProposal@1";
const OPEN_FRAME_NOTCH_CHILD_PROPOSAL_SCHEMA_VERSION: &str =
    "AuthoringMeshOpenFrameNotchChildRevisionProposal@1";
const REAR_STOCK_VOID_RAIL_BOW_CHILD_PROPOSAL_SCHEMA_VERSION: &str =
    "AuthoringMeshRearStockVoidRailBowChildRevisionProposal@1";
const REAR_STOCK_VOID_BOUNDARY_BRIDGE_CHILD_PROPOSAL_SCHEMA_VERSION: &str =
    "AuthoringMeshRearStockVoidBoundaryBridgeChildRevisionProposal@1";
const POLICY: &str = "production-weapon-form-art-single-source-node-move-vertices-six-view@1";
const MOVE_POLICY: &str = "runtime-owned-single-source-node-move-vertices-compat@1";
const OPEN_FRAME_NOTCH_POLICY: &str =
    "production-weapon-form-art-single-source-node-open-frame-notch-six-view@1";
const REAR_STOCK_VOID_RAIL_BOW_POLICY: &str =
    "production-weapon-form-art-single-source-node-rear-stock-void-rail-bow-six-view@1";
const REAR_STOCK_VOID_BOUNDARY_BRIDGE_POLICY: &str =
    "production-weapon-form-art-single-source-node-rear-stock-void-boundary-bridge-six-view@1";
const OPEN_FRAME_NOTCH_EDIT_POLICY: &str = "runtime-derived-box-open-frame@1";
const OPEN_FRAME_NOTCH_DURABLE_POLICY: &str = "runtime-owned-single-source-node-open-frame-notch@1";
const REAR_STOCK_VOID_RAIL_BOW_EDIT_POLICY: &str = "runtime-derived-rear-stock-void-rail-bow@1";
const REAR_STOCK_VOID_RAIL_BOW_DURABLE_POLICY: &str =
    "runtime-owned-single-source-node-rear-stock-void-rail-bow@1";
const REAR_STOCK_VOID_BOUNDARY_BRIDGE_EDIT_POLICY: &str =
    "runtime-derived-rear-stock-void-boundary-bridge@1";
const REAR_STOCK_VOID_BOUNDARY_BRIDGE_DURABLE_POLICY: &str =
    "runtime-owned-single-source-node-rear-stock-void-boundary-bridge@1";
const REAR_STOCK_VOID_BOUNDARY_BRIDGE_PROFILE_ID: &str = "registered-void-boundary-depth-wedge-5@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const MAX_RESPONSE_BYTES: usize = 1_048_576;
const MAX_VERTEX_MOVES: usize = 32;
const MAX_COORDINATE_M: f64 = 10.0;
const MAX_DISPLACEMENT_M: f64 = 1.0;
const POSITION_TOLERANCE_M: f64 = 1.0e-9;
const MAX_JSON_BYTES: u64 = 1_048_576;
const DURABLE_MOVE_IDEMPOTENCY_PREFIX: &str = "form-art-move-vertices-durable";
const DURABLE_OPEN_FRAME_NOTCH_IDEMPOTENCY_PREFIX: &str = "form-art-open-frame-notch-durable";
const DURABLE_REAR_STOCK_VOID_RAIL_BOW_IDEMPOTENCY_PREFIX: &str =
    "form-art-rear-stock-void-rail-bow-durable";
const DURABLE_REAR_STOCK_VOID_BOUNDARY_BRIDGE_IDEMPOTENCY_PREFIX: &str =
    "form-art-rear-stock-void-boundary-bridge-durable";
const CANDIDATE_PREPARE_IDEMPOTENCY_PREFIX: &str = "form-art-move-vertices-candidate";
const OPEN_FRAME_NOTCH_CANDIDATE_PREPARE_IDEMPOTENCY_PREFIX: &str =
    "form-art-open-frame-notch-candidate";
const REAR_STOCK_VOID_RAIL_BOW_CANDIDATE_PREPARE_IDEMPOTENCY_PREFIX: &str =
    "form-art-rear-stock-void-rail-bow-candidate";
const REAR_STOCK_VOID_BOUNDARY_BRIDGE_CANDIDATE_PREPARE_IDEMPOTENCY_PREFIX: &str =
    "form-art-rear-stock-void-boundary-bridge-candidate";
const SECONDARY_FORM_GATE_POLICY: &str = "form-art-secondary-pareto-review@2";
const SECONDARY_FORM_METRIC_POLICY: &str = "core-raster-absolute-tradeoff-0.01-semantic-exact@1";
const PROPOSAL_FORM_ART_EVIDENCE_SCHEMA_VERSION: &str = "ProductionWeaponFormArtProposalEvidence@1";
const PROPOSAL_FORM_ART_EVIDENCE_POLICY: &str =
    "proposal-candidate-six-view-form-art-part-owner-negative-line@1";
const FIXED_RASTER_SIZE_PX: u64 = 512;
const METRIC_PPM_SCALE: f64 = 1_000_000.0;
// User-authorized review tolerance for 512px raster-sensitive Form metrics.
// This is deliberately isolated from semantic, topology, hash, UV overlap,
// bake miss/cross-hit and approval gates, which remain exact/fail closed.
const MAX_CORE_TRADEOFF_PPM: i64 = 10_000;
const MIN_CORE_IMPROVEMENT_PPM: i64 = 1_000;
const MIN_AGGREGATE_IMPROVEMENT_PPM: i64 = 1;
const COMPOSITE_EVALUATION_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtCompositeCandidateEvaluation@1";
const COMPOSITE_EVALUATION_POLICY: &str =
    "existing-composite-candidate-original-fresh-baseline-candidate-only-six-view@1";
const COMPOSITE_BASELINE_AOV_KINDS: [&str; 9] = [
    "beauty",
    "silhouette",
    "depth",
    "normal",
    "ao",
    "part-id",
    "material-id",
    "wireframe",
    "uv-stretch",
];

const REQUIRED_VIEW_KINDS: [&str; 6] = [
    "front",
    "back",
    "left",
    "right",
    "top",
    "rear-three-quarter",
];

const REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "mesh_id",
    "lineage_id",
    "parent_revision_id",
    "parent_revision_sha256",
    "parent_revision_object_sha256",
    "source_node_id",
    "part_id",
    "source_binding_sha256",
    "form_art_evidence_id",
    "form_art_evidence_object_sha256",
    "form_art_evidence_canonical_sha256",
    "edit",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MoveVertexInput {
    vertex_id: String,
    before_position_m: [f64; 3],
    after_position_m: [f64; 3],
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MoveVerticesInput {
    schema_version: String,
    operation: String,
    source_node_id: String,
    part_id: String,
    coordinate_space: String,
    selection_policy: String,
    vertex_moves: Vec<MoveVertexInput>,
    canonical_sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OpenFrameNotchInput {
    schema_version: String,
    operation: String,
    source_node_id: String,
    part_id: String,
    coordinate_space: String,
    selection_policy: String,
    opening_width_milli: u32,
    opening_height_milli: u32,
    canonical_sha256: String,
}

/// The public edit intentionally contains no geometric selection or
/// orientation values.  Runtime derives those values from the bound
/// `GeometryProgram` relationship between the rear-stock and its lower beam.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RearStockVoidRailBowInput {
    schema_version: String,
    operation: String,
    source_node_id: String,
    part_id: String,
    coordinate_space: String,
    selection_policy: String,
    canonical_sha256: String,
}

/// The public edit intentionally contains only a registered profile ID. The
/// Runtime derives all boundary selection and bridge topology from that
/// profile; callers cannot provide vertices, mesh data, camera state or
/// scalar transforms.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RearStockVoidBoundaryBridgeInput {
    schema_version: String,
    operation: String,
    source_node_id: String,
    part_id: String,
    coordinate_space: String,
    selection_policy: String,
    profile_id: String,
    canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
struct RearStockVoidRailBowOrientation {
    derivation_policy: String,
    source_space: String,
    upper_node_id: String,
    lower_node_id: String,
    source_local_relation_m: [f64; 3],
    void_centroid_m: [f64; 3],
    void_face_normal_m: [f64; 3],
    void_axis: String,
    longitudinal_axis: String,
    depth_axis: String,
}

#[derive(Debug, Clone, Serialize)]
struct MoveVertexOutput {
    vertex_id: String,
    before_position_m: [f64; 3],
    after_position_m: [f64; 3],
    delta_m: [f64; 3],
}

#[derive(Debug, Clone, Serialize)]
struct MoveVerticesOutput {
    schema_version: String,
    operation: String,
    source_node_id: String,
    part_id: String,
    coordinate_space: String,
    selection_policy: String,
    vertex_moves: Vec<MoveVertexOutput>,
    canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
struct OpenFrameNotchOutput {
    schema_version: String,
    operation: String,
    source_node_id: String,
    part_id: String,
    coordinate_space: String,
    selection_policy: String,
    opening_width_milli: u32,
    opening_height_milli: u32,
    canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
struct RearStockVoidRailBowOutput {
    schema_version: String,
    operation: String,
    source_node_id: String,
    part_id: String,
    coordinate_space: String,
    selection_policy: String,
    semantic_orientation: RearStockVoidRailBowOrientation,
    canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
struct RearStockVoidBoundaryBridgeOutput {
    schema_version: String,
    operation: String,
    source_node_id: String,
    part_id: String,
    coordinate_space: String,
    selection_policy: String,
    profile_id: String,
    canonical_sha256: String,
}

#[derive(Debug, Clone)]
enum ParsedTypedEdit {
    MoveVertices(MoveVerticesInput),
    OpenFrameNotch(OpenFrameNotchInput),
    RearStockVoidRailBow(RearStockVoidRailBowInput),
    RearStockVoidBoundaryBridge(RearStockVoidBoundaryBridgeInput),
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum TypedEditOutput {
    MoveVertices(MoveVerticesOutput),
    OpenFrameNotch(OpenFrameNotchOutput),
    RearStockVoidRailBow(RearStockVoidRailBowOutput),
    RearStockVoidBoundaryBridge(RearStockVoidBoundaryBridgeOutput),
}

impl TypedEditOutput {
    fn operation(&self) -> &'static str {
        match self {
            Self::MoveVertices(_) => "move_vertices",
            Self::OpenFrameNotch(_) => "open_frame_notch",
            Self::RearStockVoidRailBow(_) => "rear_stock_void_rail_bow",
            Self::RearStockVoidBoundaryBridge(_) => "rear_stock_void_boundary_bridge",
        }
    }

    fn child_proposal_schema_version(&self) -> &'static str {
        match self {
            Self::MoveVertices(_) => CHILD_PROPOSAL_SCHEMA_VERSION,
            Self::OpenFrameNotch(_) => OPEN_FRAME_NOTCH_CHILD_PROPOSAL_SCHEMA_VERSION,
            Self::RearStockVoidRailBow(_) => REAR_STOCK_VOID_RAIL_BOW_CHILD_PROPOSAL_SCHEMA_VERSION,
            Self::RearStockVoidBoundaryBridge(_) => {
                REAR_STOCK_VOID_BOUNDARY_BRIDGE_CHILD_PROPOSAL_SCHEMA_VERSION
            }
        }
    }

    fn policy(&self) -> &'static str {
        match self {
            Self::MoveVertices(_) => POLICY,
            Self::OpenFrameNotch(_) => OPEN_FRAME_NOTCH_POLICY,
            Self::RearStockVoidRailBow(_) => REAR_STOCK_VOID_RAIL_BOW_POLICY,
            Self::RearStockVoidBoundaryBridge(_) => REAR_STOCK_VOID_BOUNDARY_BRIDGE_POLICY,
        }
    }

    fn edit_policy(&self) -> &'static str {
        match self {
            Self::MoveVertices(_) => MOVE_POLICY,
            Self::OpenFrameNotch(_) => OPEN_FRAME_NOTCH_DURABLE_POLICY,
            Self::RearStockVoidRailBow(_) => REAR_STOCK_VOID_RAIL_BOW_DURABLE_POLICY,
            Self::RearStockVoidBoundaryBridge(_) => REAR_STOCK_VOID_BOUNDARY_BRIDGE_DURABLE_POLICY,
        }
    }

    fn durable_idempotency_prefix(&self) -> &'static str {
        match self {
            Self::MoveVertices(_) => DURABLE_MOVE_IDEMPOTENCY_PREFIX,
            Self::OpenFrameNotch(_) => DURABLE_OPEN_FRAME_NOTCH_IDEMPOTENCY_PREFIX,
            Self::RearStockVoidRailBow(_) => DURABLE_REAR_STOCK_VOID_RAIL_BOW_IDEMPOTENCY_PREFIX,
            Self::RearStockVoidBoundaryBridge(_) => {
                DURABLE_REAR_STOCK_VOID_BOUNDARY_BRIDGE_IDEMPOTENCY_PREFIX
            }
        }
    }

    fn candidate_prepare_idempotency_prefix(&self) -> &'static str {
        match self {
            Self::MoveVertices(_) => CANDIDATE_PREPARE_IDEMPOTENCY_PREFIX,
            Self::OpenFrameNotch(_) => OPEN_FRAME_NOTCH_CANDIDATE_PREPARE_IDEMPOTENCY_PREFIX,
            Self::RearStockVoidRailBow(_) => {
                REAR_STOCK_VOID_RAIL_BOW_CANDIDATE_PREPARE_IDEMPOTENCY_PREFIX
            }
            Self::RearStockVoidBoundaryBridge(_) => {
                REAR_STOCK_VOID_BOUNDARY_BRIDGE_CANDIDATE_PREPARE_IDEMPOTENCY_PREFIX
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ProposalContext {
    request_input_sha256: String,
    project_id: String,
    candidate_id: String,
    candidate_state_sha256: String,
    mesh_id: String,
    lineage_id: String,
    parent_revision_object_sha256: String,
    source_node_id: String,
    part_id: String,
    source_binding_sha256: String,
    form_art_evidence_id: String,
    form_art_evidence_object_sha256: String,
    form_art_evidence_canonical_sha256: String,
    idempotency_key: String,
    edit_sha256: String,
    parent: AuthoringMeshRevision,
    art: ProductionWeaponFormArtEvidenceRecord,
    fresh_baseline: Option<ProductionWeaponFormArtBaselineRecord>,
    fresh_registration_lineage: Option<ProductionCameraLockRegistrationLineageRecord>,
    source_form_art_worker_cohorts: BTreeSet<Option<String>>,
    source_form_art_current_cohort_compatible: bool,
    source_binding: forgecad_contracts::AuthoringMeshV2SourceBinding,
    six_view_requirement: Value,
    typed_edit: TypedEditOutput,
    operation_lineage_sha256: String,
    operation_id: String,
}

/// The candidate-independent portion of the FormArt producer.  A composite
/// proposal has no `ProposalContext` (and, deliberately, does not create a
/// child revision), so the six-view/FormArt/secondary gates consume this
/// narrow scope instead of a fabricated authoring-mesh context.
struct FormArtEvaluationScope<'a> {
    project_id: &'a str,
    session_id: &'a str,
    source_candidate_id: &'a str,
    source_candidate_state_sha256: &'a str,
    source_artifact_sha256: &'a str,
    source_artifact_readback_sha256: &'a str,
    source_form_art_evidence_id: &'a str,
    source_form_art_evidence_object_sha256: &'a str,
    source_form_art_evidence_canonical_sha256: &'a str,
    art: &'a ProductionWeaponFormArtEvidenceRecord,
    fresh_baseline: Option<&'a ProductionWeaponFormArtBaselineRecord>,
    fresh_registration_lineage: Option<&'a ProductionCameraLockRegistrationLineageRecord>,
}

impl<'a> FormArtEvaluationScope<'a> {
    fn from_proposal_context(context: &'a ProposalContext) -> Self {
        Self {
            project_id: &context.project_id,
            session_id: &context.art.session_id,
            source_candidate_id: &context.candidate_id,
            source_candidate_state_sha256: &context.candidate_state_sha256,
            source_artifact_sha256: &context.source_binding.artifact_sha256,
            source_artifact_readback_sha256: &context.source_binding.artifact_readback_sha256,
            source_form_art_evidence_id: &context.form_art_evidence_id,
            source_form_art_evidence_object_sha256: &context.form_art_evidence_object_sha256,
            source_form_art_evidence_canonical_sha256: &context.form_art_evidence_canonical_sha256,
            art: &context.art,
            fresh_baseline: context.fresh_baseline.as_ref(),
            fresh_registration_lineage: context.fresh_registration_lineage.as_ref(),
        }
    }
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "PRODUCTION_WEAPON_FORM_ART_MESH_PROPOSAL_INVALID: {}",
        message.into()
    ))
}

fn exact_object<'a>(value: &'a Value) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("request must be an object"))?;
    let expected = REQUEST_FIELDS.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if expected != actual {
        return Err(invalid("request fields differ from the closed contract"));
    }
    Ok(object)
}

fn text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{field} must be a string")))
}

fn identifier<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!("{field} must be an opaque ID")));
    }
    Ok(value)
}

fn sha<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{field} must be a SHA-256")));
    }
    Ok(value)
}

fn finite_position(position: [f64; 3], field: &str) -> Result<(), RuntimeError> {
    if position
        .iter()
        .any(|value| !value.is_finite() || value.abs() > MAX_COORDINATE_M)
    {
        return Err(invalid(format!("{field} is not finite or bounded")));
    }
    Ok(())
}

fn input_hash(request: &Value, object: &Map<String, Value>) -> Result<String, RuntimeError> {
    let supplied = sha(object, "input_sha256")?;
    let mut preimage = request.clone();
    preimage["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != supplied {
        return Err(invalid("input_sha256 differs from the closed request"));
    }
    Ok(supplied.to_owned())
}

fn parse_move_vertices_edit(value: &Value) -> Result<(MoveVerticesInput, String), RuntimeError> {
    let edit: MoveVerticesInput = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("typed MoveVertices edit is invalid: {error}")))?;
    if edit.schema_version != MOVE_VERTICES_SCHEMA_VERSION
        || edit.operation != "move_vertices"
        || edit.coordinate_space != "source-local"
        || edit.selection_policy != "explicit-stable-vertex-ids@1"
    {
        return Err(invalid("typed MoveVertices edit policy differs"));
    }
    for (field, value) in [
        ("edit.source_node_id", edit.source_node_id.as_str()),
        ("edit.part_id", edit.part_id.as_str()),
    ] {
        if !is_opaque_id(value) {
            return Err(invalid(format!("{field} is not an opaque ID")));
        }
    }
    if edit.vertex_moves.is_empty() || edit.vertex_moves.len() > MAX_VERTEX_MOVES {
        return Err(invalid("typed MoveVertices edit vertex budget is invalid"));
    }
    if !is_sha256(&edit.canonical_sha256) {
        return Err(invalid(
            "typed MoveVertices edit canonical_sha256 is invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for (index, move_vertex) in edit.vertex_moves.iter().enumerate() {
        if !is_opaque_id(&move_vertex.vertex_id) || !ids.insert(move_vertex.vertex_id.as_str()) {
            return Err(invalid(format!(
                "typed MoveVertices edit vertex_moves[{index}] has a duplicate/invalid vertex_id"
            )));
        }
        finite_position(
            move_vertex.before_position_m,
            &format!("edit.vertex_moves[{index}].before_position_m"),
        )?;
        finite_position(
            move_vertex.after_position_m,
            &format!("edit.vertex_moves[{index}].after_position_m"),
        )?;
        let delta = [
            move_vertex.after_position_m[0] - move_vertex.before_position_m[0],
            move_vertex.after_position_m[1] - move_vertex.before_position_m[1],
            move_vertex.after_position_m[2] - move_vertex.before_position_m[2],
        ];
        let displacement = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
        if !displacement.is_finite() || displacement > MAX_DISPLACEMENT_M {
            return Err(invalid(format!(
                "typed MoveVertices edit vertex_moves[{index}] displacement is outside bounds"
            )));
        }
    }
    let mut normalized = serde_json::to_value(&edit).map_err(|error| invalid(error.to_string()))?;
    normalized["canonical_sha256"] = Value::String(String::new());
    let expected = canonical_json_hash(&normalized);
    if edit.canonical_sha256 != expected {
        return Err(invalid("typed MoveVertices edit canonical_sha256 differs"));
    }
    Ok((edit, expected))
}

fn parse_open_frame_notch_edit(
    value: &Value,
) -> Result<(OpenFrameNotchInput, String), RuntimeError> {
    let edit: OpenFrameNotchInput = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("typed OpenFrameNotch edit is invalid: {error}")))?;
    if edit.schema_version != OPEN_FRAME_NOTCH_SCHEMA_VERSION
        || edit.operation != "open_frame_notch"
        || edit.coordinate_space != "source-local"
        || edit.selection_policy != OPEN_FRAME_NOTCH_EDIT_POLICY
    {
        return Err(invalid("typed OpenFrameNotch edit policy differs"));
    }
    for (field, value) in [
        ("edit.source_node_id", edit.source_node_id.as_str()),
        ("edit.part_id", edit.part_id.as_str()),
    ] {
        if !is_opaque_id(value) {
            return Err(invalid(format!("{field} is not an opaque ID")));
        }
    }
    if !(1..=999).contains(&edit.opening_width_milli)
        || !(1..=999).contains(&edit.opening_height_milli)
    {
        return Err(invalid(
            "typed OpenFrameNotch normalized width/height must be between 1 and 999 milli",
        ));
    }
    if !is_sha256(&edit.canonical_sha256) {
        return Err(invalid(
            "typed OpenFrameNotch edit canonical_sha256 is invalid",
        ));
    }
    let mut normalized = serde_json::to_value(&edit).map_err(|error| invalid(error.to_string()))?;
    normalized["canonical_sha256"] = Value::String(String::new());
    let expected = canonical_json_hash(&normalized);
    if edit.canonical_sha256 != expected {
        return Err(invalid(
            "typed OpenFrameNotch edit canonical_sha256 differs",
        ));
    }
    Ok((edit, expected))
}

fn parse_rear_stock_void_rail_bow_edit(
    value: &Value,
) -> Result<(RearStockVoidRailBowInput, String), RuntimeError> {
    let edit: RearStockVoidRailBowInput =
        serde_json::from_value(value.clone()).map_err(|error| {
            invalid(format!(
                "typed RearStockVoidRailBow edit is invalid: {error}"
            ))
        })?;
    if edit.schema_version != REAR_STOCK_VOID_RAIL_BOW_SCHEMA_VERSION
        || edit.operation != "rear_stock_void_rail_bow"
        || edit.coordinate_space != "source-local"
        || edit.selection_policy != REAR_STOCK_VOID_RAIL_BOW_EDIT_POLICY
    {
        return Err(invalid("typed RearStockVoidRailBow edit policy differs"));
    }
    if edit.source_node_id != "rear-stock" || edit.part_id != "rear-stock" {
        return Err(invalid(
            "typed RearStockVoidRailBow edit is bound to rear-stock only",
        ));
    }
    if !is_sha256(&edit.canonical_sha256) {
        return Err(invalid(
            "typed RearStockVoidRailBow edit canonical_sha256 is invalid",
        ));
    }
    let mut normalized = serde_json::to_value(&edit).map_err(|error| invalid(error.to_string()))?;
    normalized["canonical_sha256"] = Value::String(String::new());
    let expected = canonical_json_hash(&normalized);
    if edit.canonical_sha256 != expected {
        return Err(invalid(
            "typed RearStockVoidRailBow edit canonical_sha256 differs",
        ));
    }
    Ok((edit, expected))
}

fn parse_rear_stock_void_boundary_bridge_edit(
    value: &Value,
) -> Result<(RearStockVoidBoundaryBridgeInput, String), RuntimeError> {
    let edit: RearStockVoidBoundaryBridgeInput =
        serde_json::from_value(value.clone()).map_err(|error| {
            invalid(format!(
                "typed RearStockVoidBoundaryBridge edit is invalid: {error}"
            ))
        })?;
    if edit.schema_version != REAR_STOCK_VOID_BOUNDARY_BRIDGE_SCHEMA_VERSION
        || edit.operation != "rear_stock_void_boundary_bridge"
        || edit.coordinate_space != "source-local"
        || edit.selection_policy != REAR_STOCK_VOID_BOUNDARY_BRIDGE_EDIT_POLICY
        || edit.profile_id != REAR_STOCK_VOID_BOUNDARY_BRIDGE_PROFILE_ID
    {
        return Err(invalid(
            "typed RearStockVoidBoundaryBridge edit policy or profile differs",
        ));
    }
    if edit.source_node_id != "rear-stock" || edit.part_id != "rear-stock" {
        return Err(invalid(
            "typed RearStockVoidBoundaryBridge edit is bound to rear-stock only",
        ));
    }
    if !is_sha256(&edit.canonical_sha256) {
        return Err(invalid(
            "typed RearStockVoidBoundaryBridge edit canonical_sha256 is invalid",
        ));
    }
    let mut normalized = serde_json::to_value(&edit).map_err(|error| invalid(error.to_string()))?;
    normalized["canonical_sha256"] = Value::String(String::new());
    let expected = canonical_json_hash(&normalized);
    if edit.canonical_sha256 != expected {
        return Err(invalid(
            "typed RearStockVoidBoundaryBridge edit canonical_sha256 differs",
        ));
    }
    Ok((edit, expected))
}

fn parse_edit(value: &Value) -> Result<(ParsedTypedEdit, String), RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("typed edit must be an object"))?;
    let schema_version = object
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("typed edit schema_version must be a string"))?;
    let operation = object
        .get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("typed edit operation must be a string"))?;
    match (schema_version, operation) {
        (MOVE_VERTICES_SCHEMA_VERSION, "move_vertices") => {
            let (edit, hash) = parse_move_vertices_edit(value)?;
            Ok((ParsedTypedEdit::MoveVertices(edit), hash))
        }
        (OPEN_FRAME_NOTCH_SCHEMA_VERSION, "open_frame_notch") => {
            let (edit, hash) = parse_open_frame_notch_edit(value)?;
            Ok((ParsedTypedEdit::OpenFrameNotch(edit), hash))
        }
        (REAR_STOCK_VOID_RAIL_BOW_SCHEMA_VERSION, "rear_stock_void_rail_bow") => {
            let (edit, hash) = parse_rear_stock_void_rail_bow_edit(value)?;
            Ok((ParsedTypedEdit::RearStockVoidRailBow(edit), hash))
        }
        (REAR_STOCK_VOID_BOUNDARY_BRIDGE_SCHEMA_VERSION, "rear_stock_void_boundary_bridge") => {
            let (edit, hash) = parse_rear_stock_void_boundary_bridge_edit(value)?;
            Ok((ParsedTypedEdit::RearStockVoidBoundaryBridge(edit), hash))
        }
        _ => Err(invalid(
            "typed edit schema_version and operation are not supported",
        )),
    }
}

fn load_parent_revision(
    runtime: &Runtime,
    project_id: &str,
    mesh_id: &str,
    parent_revision_id: &str,
    parent_revision_sha256: &str,
    parent_revision_object_sha256: &str,
) -> Result<AuthoringMeshRevision, RuntimeError> {
    let mut request = json!({
        "schema_version": AUTHORING_MESH_V2_DURABLE_GET_REQUEST_SCHEMA_VERSION,
        "project_id": project_id,
        "mesh_id": mesh_id,
        "revision_id": parent_revision_id,
        "revision_sha256": parent_revision_sha256,
        "revision_object_sha256": parent_revision_object_sha256,
        "writer_policy": WRITER_POLICY,
        "runtime_write_performed": false,
        "persistent_user_data_touched": false,
        "input_sha256": ""
    });
    request["input_sha256"] = Value::String(canonical_json_hash(&request));
    let result = runtime.authoring_mesh_v2_durable_get(&request)?;
    let revision: AuthoringMeshRevision = serde_json::from_value(
        result
            .get("revision")
            .cloned()
            .ok_or_else(|| invalid("durable parent revision payload is missing"))?,
    )
    .map_err(|error| invalid(format!("durable parent revision is malformed: {error}")))?;
    AuthoringMeshV2Revision::from_record(revision.clone())?;
    Ok(revision)
}

fn load_form_art(
    runtime: &Runtime,
    art_evidence_id: &str,
    project_id: &str,
    candidate_id: &str,
    object_sha256: &str,
    canonical_sha256: &str,
) -> Result<
    (
        ProductionWeaponFormArtEvidenceRecord,
        BTreeSet<Option<String>>,
    ),
    RuntimeError,
> {
    super::production_weapon_form_art_evidence::read_persisted_form_art_for_projection(
        runtime,
        art_evidence_id,
        project_id,
        candidate_id,
        object_sha256,
        canonical_sha256,
    )
}

fn require_six_views(
    record: &ProductionWeaponFormArtEvidenceRecord,
    proposal_policy: &str,
) -> Result<Value, RuntimeError> {
    if record.view_kinds
        != REQUIRED_VIEW_KINDS
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
        || record.views.len() != REQUIRED_VIEW_KINDS.len()
    {
        return Err(invalid(
            "FormArt evidence does not contain the exact six required views",
        ));
    }
    let mut view_ids = Vec::with_capacity(REQUIRED_VIEW_KINDS.len());
    let mut reference_ids = Vec::with_capacity(REQUIRED_VIEW_KINDS.len());
    let mut camera_hashes = Vec::with_capacity(REQUIRED_VIEW_KINDS.len());
    for (index, view) in record.views.iter().enumerate() {
        if view.view_kind != REQUIRED_VIEW_KINDS[index]
            || !is_opaque_id(&view.view_id)
            || !is_opaque_id(&view.reference_id)
            || !is_sha256(&view.reference_sha256)
            || !is_sha256(&view.camera_hash)
            || !is_sha256(&view.camera_canonical_sha256)
        {
            return Err(invalid("FormArt evidence six-view binding is invalid"));
        }
        view_ids.push(view.view_id.clone());
        reference_ids.push(view.reference_id.clone());
        camera_hashes.push(view.camera_hash.clone());
    }
    Ok(json!({
        "policy": proposal_policy,
        "view_order": REQUIRED_VIEW_KINDS,
        "required_view_kinds": REQUIRED_VIEW_KINDS,
        "required_view_count": REQUIRED_VIEW_KINDS.len(),
        "bound_view_count": record.views.len(),
        "view_ids": view_ids,
        "reference_ids": reference_ids,
        "camera_hashes": camera_hashes,
        "require_non_regression": true,
        "require_strict_improvement": false,
        "evaluation_status": "REQUIRED_NOT_EVALUATED",
        "gate_status": "PENDING_SIX_VIEW_FORMART_REVIEW"
    }))
}

fn materialize_move_payload(
    edit: &MoveVerticesInput,
    revision: &AuthoringMeshRevision,
    source_node_id: &str,
    part_id: &str,
) -> Result<MoveVerticesOutput, RuntimeError> {
    if edit.source_node_id != source_node_id || edit.part_id != part_id {
        return Err(invalid("typed edit source node or Part differs"));
    }
    let positions = revision
        .original
        .vertices
        .iter()
        .map(|vertex| (vertex.vertex_id.0.as_str(), vertex.position_m))
        .collect::<BTreeMap<_, _>>();
    let mut moves = edit.vertex_moves.clone();
    moves.sort_by(|left, right| left.vertex_id.cmp(&right.vertex_id));
    let mut output = Vec::with_capacity(moves.len());
    let mut changed = false;
    for move_vertex in moves {
        let current = positions
            .get(move_vertex.vertex_id.as_str())
            .copied()
            .ok_or_else(|| invalid("typed edit references a vertex outside the parent revision"))?;
        if current
            .iter()
            .zip(move_vertex.before_position_m.iter())
            .any(|(left, right)| (left - right).abs() > POSITION_TOLERANCE_M)
        {
            return Err(invalid("typed edit before_position_m is stale"));
        }
        let delta = [
            move_vertex.after_position_m[0] - move_vertex.before_position_m[0],
            move_vertex.after_position_m[1] - move_vertex.before_position_m[1],
            move_vertex.after_position_m[2] - move_vertex.before_position_m[2],
        ];
        changed |= delta.iter().any(|value| value.abs() > POSITION_TOLERANCE_M);
        output.push(MoveVertexOutput {
            vertex_id: move_vertex.vertex_id,
            before_position_m: move_vertex.before_position_m,
            after_position_m: move_vertex.after_position_m,
            delta_m: delta,
        });
    }
    if !changed {
        return Err(invalid("typed edit is a no-op"));
    }
    let mut result = MoveVerticesOutput {
        schema_version: MOVE_VERTICES_SCHEMA_VERSION.to_owned(),
        operation: "move_vertices".to_owned(),
        source_node_id: source_node_id.to_owned(),
        part_id: part_id.to_owned(),
        coordinate_space: "source-local".to_owned(),
        selection_policy: "explicit-stable-vertex-ids@1".to_owned(),
        vertex_moves: output,
        canonical_sha256: String::new(),
    };
    result.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&result).map_err(|error| invalid(error.to_string()))?,
    );
    Ok(result)
}

fn materialize_open_frame_notch_payload(
    edit: &OpenFrameNotchInput,
    source_node_id: &str,
    part_id: &str,
) -> Result<OpenFrameNotchOutput, RuntimeError> {
    if edit.source_node_id != source_node_id || edit.part_id != part_id {
        return Err(invalid("typed edit source node or Part differs"));
    }
    let mut result = OpenFrameNotchOutput {
        schema_version: OPEN_FRAME_NOTCH_SCHEMA_VERSION.to_owned(),
        operation: "open_frame_notch".to_owned(),
        source_node_id: source_node_id.to_owned(),
        part_id: part_id.to_owned(),
        coordinate_space: "source-local".to_owned(),
        selection_policy: OPEN_FRAME_NOTCH_EDIT_POLICY.to_owned(),
        opening_width_milli: edit.opening_width_milli,
        opening_height_milli: edit.opening_height_milli,
        canonical_sha256: String::new(),
    };
    result.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&result).map_err(|error| invalid(error.to_string()))?,
    );
    Ok(result)
}

fn materialize_rear_stock_void_rail_bow_payload(
    edit: &RearStockVoidRailBowInput,
    source_node_id: &str,
    part_id: &str,
    semantic_orientation: RearStockVoidRailBowOrientation,
) -> Result<RearStockVoidRailBowOutput, RuntimeError> {
    if edit.source_node_id != source_node_id || edit.part_id != part_id {
        return Err(invalid("typed edit source node or Part differs"));
    }
    let mut result = RearStockVoidRailBowOutput {
        schema_version: REAR_STOCK_VOID_RAIL_BOW_SCHEMA_VERSION.to_owned(),
        operation: "rear_stock_void_rail_bow".to_owned(),
        source_node_id: source_node_id.to_owned(),
        part_id: part_id.to_owned(),
        coordinate_space: "source-local".to_owned(),
        selection_policy: REAR_STOCK_VOID_RAIL_BOW_EDIT_POLICY.to_owned(),
        semantic_orientation,
        canonical_sha256: String::new(),
    };
    result.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&result).map_err(|error| invalid(error.to_string()))?,
    );
    Ok(result)
}

fn materialize_rear_stock_void_boundary_bridge_payload(
    edit: &RearStockVoidBoundaryBridgeInput,
    source_node_id: &str,
    part_id: &str,
) -> Result<RearStockVoidBoundaryBridgeOutput, RuntimeError> {
    if edit.source_node_id != source_node_id || edit.part_id != part_id {
        return Err(invalid("typed edit source node or Part differs"));
    }
    let mut result = RearStockVoidBoundaryBridgeOutput {
        schema_version: REAR_STOCK_VOID_BOUNDARY_BRIDGE_SCHEMA_VERSION.to_owned(),
        operation: "rear_stock_void_boundary_bridge".to_owned(),
        source_node_id: source_node_id.to_owned(),
        part_id: part_id.to_owned(),
        coordinate_space: "source-local".to_owned(),
        selection_policy: REAR_STOCK_VOID_BOUNDARY_BRIDGE_EDIT_POLICY.to_owned(),
        profile_id: edit.profile_id.clone(),
        canonical_sha256: String::new(),
    };
    result.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&result).map_err(|error| invalid(error.to_string()))?,
    );
    Ok(result)
}

fn load_context(runtime: &Runtime, request: &Value) -> Result<ProposalContext, RuntimeError> {
    let object = exact_object(request)?;
    if text(object, "schema_version")? != REQUEST_SCHEMA_VERSION
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
        || text(object, "writer_policy")? != WRITER_POLICY
        || text(object, "canonicalization_policy")? != CANONICALIZATION_POLICY
        || object.get("max_response_bytes").and_then(Value::as_u64)
            != Some(MAX_RESPONSE_BYTES as u64)
    {
        return Err(invalid("request policy or response budget differs"));
    }
    let request_input_sha256 = input_hash(request, object)?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let candidate_id = identifier(object, "candidate_id")?.to_owned();
    let candidate_state_sha256 = sha(object, "candidate_state_sha256")?.to_owned();
    let mesh_id = identifier(object, "mesh_id")?.to_owned();
    let lineage_id = identifier(object, "lineage_id")?.to_owned();
    let parent_revision_id = identifier(object, "parent_revision_id")?.to_owned();
    let parent_revision_sha256 = sha(object, "parent_revision_sha256")?.to_owned();
    let parent_revision_object_sha256 = sha(object, "parent_revision_object_sha256")?.to_owned();
    let source_node_id = identifier(object, "source_node_id")?.to_owned();
    let part_id = identifier(object, "part_id")?.to_owned();
    let source_binding_sha256 = sha(object, "source_binding_sha256")?.to_owned();
    let form_art_evidence_id = identifier(object, "form_art_evidence_id")?.to_owned();
    let form_art_evidence_object_sha256 =
        sha(object, "form_art_evidence_object_sha256")?.to_owned();
    let form_art_evidence_canonical_sha256 =
        sha(object, "form_art_evidence_canonical_sha256")?.to_owned();
    let idempotency_key = identifier(object, "idempotency_key")?.to_owned();
    let (edit, edit_sha256) = parse_edit(
        object
            .get("edit")
            .ok_or_else(|| invalid("typed edit is missing"))?,
    )?;

    let candidate = runtime
        .candidate(&candidate_id)?
        .ok_or_else(|| invalid("source candidate is unavailable"))?;
    if candidate.project_id != project_id || candidate.canonical_sha256 != candidate_state_sha256 {
        return Err(invalid("candidate binding differs"));
    }
    let parent = load_parent_revision(
        runtime,
        &project_id,
        &mesh_id,
        &parent_revision_id,
        &parent_revision_sha256,
        &parent_revision_object_sha256,
    )?;
    if parent.mesh_id.0 != mesh_id
        || parent.lineage_id.0 != lineage_id
        || parent.revision_id.0 != parent_revision_id
        || parent.canonical_sha256 != parent_revision_sha256
    {
        return Err(invalid("durable parent revision identity differs"));
    }
    let binding = parent
        .source_binding
        .as_ref()
        .ok_or_else(|| invalid("durable parent revision has no source binding"))?
        .clone();
    validate_source_binding(&binding)?;
    if binding.project_id != project_id
        || binding.candidate_id != candidate_id
        || binding.candidate_state_sha256 != candidate_state_sha256
        || binding.source_node_id != source_node_id
        || binding.part_id != part_id
        || binding.canonical_sha256 != source_binding_sha256
    {
        return Err(invalid("source candidate/node/Part binding differs"));
    }
    let (art, historical_form_art_worker_cohorts) = load_form_art(
        runtime,
        &form_art_evidence_id,
        &project_id,
        &candidate_id,
        &form_art_evidence_object_sha256,
        &form_art_evidence_canonical_sha256,
    )?;
    let runtime_cohort = build_cohort_sha256();
    if art.candidate_state_sha256 != candidate_state_sha256
        || art.artifact_sha256 != binding.artifact_sha256
        || art.artifact_id != binding.artifact_id
    {
        return Err(invalid("FormArt candidate/artifact binding differs"));
    }
    let fresh_baseline = runtime_cohort
        .as_deref()
        .map(|cohort| {
            runtime
                .store
                .get_production_weapon_form_art_baseline_for_current_source(
                    &project_id,
                    &candidate_id,
                    &binding.artifact_sha256,
                    cohort,
                )
                .map_err(RuntimeError::from)
        })
        .transpose()?
        .flatten();
    if let Some(baseline) = fresh_baseline.as_ref() {
        let expected_views = REQUIRED_VIEW_KINDS
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        if baseline.session_id != art.session_id
            || baseline.project_id != project_id
            || baseline.candidate_id != candidate_id
            || baseline.candidate_state_sha256 != candidate_state_sha256
            || baseline.artifact_id != binding.artifact_id
            || baseline.artifact_sha256 != binding.artifact_sha256
            || baseline.view_kinds != expected_views
            || baseline.views.len() != REQUIRED_VIEW_KINDS.len()
            || baseline.historical_form_art_reused
            || !baseline.worker_started
            || !baseline.worker_cohort_verified
            || baseline.runtime_build_cohort_sha256 != runtime_cohort.as_deref().unwrap_or_default()
            || baseline.views.iter().any(|view| {
                view.render_worker_build_cohort_sha256 != baseline.runtime_build_cohort_sha256
            })
            || baseline.candidate_confirmed
            || baseline.version_created
            || baseline.export_performed
        {
            return Err(invalid(
                "fresh FormArt baseline does not match the exact current source scope",
            ));
        }
    }
    let fresh_registration_lineage = fresh_baseline
        .as_ref()
        .map(|baseline| {
            let lineage = runtime
                .store
                .get_production_camera_lock_registration_lineage(&baseline.registration_lineage_id)?
                .ok_or_else(|| invalid("fresh baseline registration lineage is unavailable"))?;
            super::agentic_session::validate_production_camera_lock_registration_lineage_runtime(
                runtime, &lineage,
            )?;
            if lineage.registration_lineage_id != baseline.registration_lineage_id
                || lineage.canonical_sha256 != baseline.registration_lineage_canonical_sha256
                || lineage.receipt_object_sha256
                    != baseline.registration_lineage_receipt_object_sha256
                || lineage.registered_rig_v2_object_sha256
                    != baseline.registered_rig_v2_object_sha256
                || lineage.registered_rig_v2_canonical_sha256
                    != baseline.registered_rig_v2_canonical_sha256
                || lineage.session_id != baseline.session_id
                || lineage.project_id != baseline.project_id
                || lineage.candidate_id != baseline.candidate_id
                || lineage.candidate_state_sha256 != baseline.candidate_state_sha256
                || lineage.artifact_id != baseline.artifact_id
                || lineage.artifact_sha256 != baseline.artifact_sha256
                || !lineage.promotable
            {
                return Err(invalid(
                    "fresh baseline registration lineage or RigV2 binding differs",
                ));
            }
            Ok(lineage)
        })
        .transpose()?;
    let source_form_art_current_cohort_compatible = fresh_baseline.is_some()
        || (historical_form_art_worker_cohorts.len() == 1
            && historical_form_art_worker_cohorts.contains(&runtime_cohort));
    let source_form_art_worker_cohorts = if fresh_baseline.is_some() {
        [runtime_cohort.clone()].into_iter().collect()
    } else {
        historical_form_art_worker_cohorts
    };
    let rail_bow_orientation = if matches!(&edit, ParsedTypedEdit::RearStockVoidRailBow(_)) {
        let source = load_source_program_for_binding(
            runtime,
            &project_id,
            &candidate_id,
            &binding,
            &source_node_id,
            &part_id,
        )?;
        Some(derive_rear_stock_void_rail_bow_orientation(
            &source.program,
            &source_node_id,
            &part_id,
        )?)
    } else {
        None
    };
    let typed_edit = match edit {
        ParsedTypedEdit::MoveVertices(edit) => TypedEditOutput::MoveVertices(
            materialize_move_payload(&edit, &parent, &source_node_id, &part_id)?,
        ),
        ParsedTypedEdit::OpenFrameNotch(edit) => TypedEditOutput::OpenFrameNotch(
            materialize_open_frame_notch_payload(&edit, &source_node_id, &part_id)?,
        ),
        ParsedTypedEdit::RearStockVoidRailBow(edit) => {
            TypedEditOutput::RearStockVoidRailBow(materialize_rear_stock_void_rail_bow_payload(
                &edit,
                &source_node_id,
                &part_id,
                rail_bow_orientation
                    .ok_or_else(|| invalid("rear-stock rail-bow orientation is unavailable"))?,
            )?)
        }
        ParsedTypedEdit::RearStockVoidBoundaryBridge(edit) => {
            TypedEditOutput::RearStockVoidBoundaryBridge(
                materialize_rear_stock_void_boundary_bridge_payload(
                    &edit,
                    &source_node_id,
                    &part_id,
                )?,
            )
        }
    };
    let six_view_requirement = require_six_views(&art, typed_edit.policy())?;
    let typed_edit_value =
        serde_json::to_value(&typed_edit).map_err(|error| invalid(error.to_string()))?;
    // Keep the MoveVertices preimage byte-compatible with the existing D1
    // proposal while giving the additive topology operation its own typed
    // payload hash and child schema identity.
    let mut operation_lineage = json!({
        "schema_version": typed_edit.child_proposal_schema_version(),
        "policy": typed_edit.policy(),
        "mesh_id": mesh_id,
        "lineage_id": lineage_id,
        "parent_revision_id": parent_revision_id,
        "parent_revision_sha256": parent_revision_sha256,
        "source_node_id": source_node_id,
        "part_id": part_id,
        "typed_edit_sha256": edit_sha256,
    });
    let operation_hash_field = match typed_edit.operation() {
        "move_vertices" => "move_vertices_sha256",
        "open_frame_notch" => "open_frame_notch_sha256",
        "rear_stock_void_rail_bow" => "rear_stock_void_rail_bow_sha256",
        "rear_stock_void_boundary_bridge" => "rear_stock_void_boundary_bridge_sha256",
        _ => unreachable!("typed edit operation is closed"),
    };
    operation_lineage[operation_hash_field] = Value::String(canonical_json_hash(&typed_edit_value));
    let operation_lineage_sha256 = canonical_json_hash(&operation_lineage);
    let operation_id = format!(
        "amop-{}",
        &canonical_json_hash(&json!({
            "operation_lineage_sha256":operation_lineage_sha256,
            "parent_revision_id":parent_revision_id,
            "source_node_id":source_node_id,
            "part_id":part_id,
        }))[..48]
    );
    Ok(ProposalContext {
        request_input_sha256,
        project_id,
        candidate_id,
        candidate_state_sha256,
        mesh_id,
        lineage_id,
        parent_revision_object_sha256,
        source_node_id,
        part_id,
        source_binding_sha256,
        form_art_evidence_id,
        form_art_evidence_object_sha256,
        form_art_evidence_canonical_sha256,
        idempotency_key,
        edit_sha256,
        parent,
        art,
        fresh_baseline,
        fresh_registration_lineage,
        source_form_art_worker_cohorts,
        source_form_art_current_cohort_compatible,
        source_binding: binding,
        six_view_requirement,
        typed_edit,
        operation_lineage_sha256,
        operation_id,
    })
}

fn get_result(context: &ProposalContext) -> Result<Value, RuntimeError> {
    let parent = &context.parent;
    let operation = context.typed_edit.operation();
    let child_schema_version = context.typed_edit.child_proposal_schema_version();
    let proposal_policy = context.typed_edit.policy();
    let edit_policy = context.typed_edit.edit_policy();
    let typed_edit_value =
        serde_json::to_value(&context.typed_edit).map_err(|error| invalid(error.to_string()))?;
    let child_seed = canonical_json_hash(&json!({
        "schema_version": child_schema_version,
        "mesh_id": context.mesh_id,
        "lineage_id": context.lineage_id,
        "parent_revision_id": parent.revision_id.0,
        "parent_revision_sha256": parent.canonical_sha256,
        "revision_index": parent.revision_index.saturating_add(1),
        "operation_lineage_sha256": context.operation_lineage_sha256,
    }));
    let proposed_child_revision_id = format!("amrev-proposal-{}", &child_seed[..56]);
    let proposed_child_revision_sha256 = canonical_json_hash(&json!({
        "schema_version": child_schema_version,
        "mesh_id": context.mesh_id,
        "lineage_id": context.lineage_id,
        "parent_revision_ids": [parent.revision_id.0],
        "revision_id": proposed_child_revision_id,
        "revision_index": parent.revision_index.saturating_add(1),
        "operation": operation,
        "operation_lineage_sha256": context.operation_lineage_sha256,
        "typed_edit_sha256": context.edit_sha256,
        "materialization_status": "KERNEL_CHILD_NOT_PERSISTED_IN_READ_ONLY_PROPOSAL",
    }));
    let proposal_id = format!(
        "form-art-{operation}-proposal-{}",
        &canonical_json_hash(&json!({
            "request_input_sha256": context.request_input_sha256,
            "idempotency_key": context.idempotency_key,
            "operation_lineage_sha256": context.operation_lineage_sha256,
        }))[..32]
    );
    let source_form_art_cohort_status = if context.fresh_baseline.is_some() {
        "CURRENT_COHORT_FRESH_BASELINE_COMPATIBLE"
    } else if context.source_form_art_current_cohort_compatible {
        "CURRENT_COHORT_COMPATIBLE"
    } else {
        "HISTORICAL_COHORT_REFRESH_REQUIRED"
    };
    let proposal_status = if context.source_form_art_current_cohort_compatible {
        "PROPOSAL_ONLY_REQUIRES_EXPLICIT_PREPARE_AND_SIX_VIEW_REVIEW"
    } else {
        "PROPOSAL_READY_BASELINE_COHORT_REFRESH_REQUIRED_BEFORE_PREPARE"
    };
    let mut result = json!({
        "schema_version": RESULT_SCHEMA_VERSION,
        "proposal_id": proposal_id,
        "policy": proposal_policy,
        "project_id": context.project_id,
        "candidate_id": context.candidate_id,
        "candidate_state_sha256": context.candidate_state_sha256,
        "mesh_id": context.mesh_id,
        "lineage_id": context.lineage_id,
        "source_node_id": context.source_node_id,
        "part_id": context.part_id,
        "source_binding_sha256": context.source_binding_sha256,
        "form_art_evidence_id": context.form_art_evidence_id,
        "form_art_evidence_object_sha256": context.form_art_evidence_object_sha256,
        "form_art_evidence_canonical_sha256": context.form_art_evidence_canonical_sha256,
        "fresh_baseline_id": context.fresh_baseline.as_ref().map(|baseline| baseline.baseline_id.clone()),
        "fresh_baseline_canonical_sha256": context.fresh_baseline.as_ref().map(|baseline| baseline.canonical_sha256.clone()),
        "fresh_baseline_registration_lineage_id": context.fresh_baseline.as_ref().map(|baseline| baseline.registration_lineage_id.clone()),
        "source_form_art_worker_cohorts": context.source_form_art_worker_cohorts.iter().cloned().collect::<Vec<_>>(),
        "runtime_build_cohort_sha256": build_cohort_sha256(),
        "source_form_art_cohort_status": source_form_art_cohort_status,
        "prepare_eligible_by_form_art_cohort": context.source_form_art_current_cohort_compatible,
        "blocking_reasons": if context.source_form_art_current_cohort_compatible {
            Vec::<String>::new()
        } else {
            vec!["BASELINE_FORM_ART_COHORT_REFRESH_REQUIRED".to_owned()]
        },
        "parent_revision": {
            "revision_id": parent.revision_id.0,
            "revision_index": parent.revision_index,
            "revision_sha256": parent.canonical_sha256,
            "revision_object_sha256": context.parent_revision_object_sha256,
            "parent_revision_ids": parent.parent_revision_ids,
        },
        "child_revision": {
            "schema_version": child_schema_version,
            "revision_id": proposed_child_revision_id,
            "revision_index": parent.revision_index.saturating_add(1),
            "parent_revision_ids": [parent.revision_id.0],
            "revision_sha256": proposed_child_revision_sha256,
            "operation": operation,
            "operation_id": context.operation_id,
            "operation_lineage_sha256": context.operation_lineage_sha256,
            "materialization_status": "KERNEL_CHILD_NOT_PERSISTED_IN_READ_ONLY_PROPOSAL",
            "durable": false,
        },
        "typed_edit": typed_edit_value,
        "six_view_requirement": context.six_view_requirement,
        "proposal_status": proposal_status,
        "quality_status": "QUALITY_TARGET_NOT_MET",
        "secondary_form_approved": "NOT_CREATED",
        "stage": "camera-calibrated",
        "runtime_write_performed": false,
        "persistent_user_data_touched": false,
        "stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "request_input_sha256": context.request_input_sha256,
        "idempotency_key": context.idempotency_key,
        "limitations": [
            "REAL_D1_DURABLE_AUTHORING_MESH_PARENT_BOUND",
            "SINGLE_SOURCE_NODE_AND_PART_BOUND",
            match operation {
                "move_vertices" => "TYPED_MOVE_VERTICES_KERNEL_PAYLOAD_VALIDATED",
                "open_frame_notch" => "TYPED_OPEN_FRAME_NOTCH_KERNEL_PAYLOAD_VALIDATED",
                "rear_stock_void_rail_bow" => "TYPED_REAR_STOCK_VOID_RAIL_BOW_KERNEL_PAYLOAD_VALIDATED",
                "rear_stock_void_boundary_bridge" => {
                    "TYPED_REAR_STOCK_VOID_BOUNDARY_BRIDGE_KERNEL_PAYLOAD_VALIDATED"
                }
                _ => unreachable!("typed edit operation is closed"),
            },
            match operation {
                "move_vertices" => "MOVE_VERTICES_KERNEL_CHILD_NOT_PERSISTED_IN_READ_ONLY_PROPOSAL",
                "open_frame_notch" => "OPEN_FRAME_NOTCH_KERNEL_CHILD_NOT_PERSISTED_IN_READ_ONLY_PROPOSAL",
                "rear_stock_void_rail_bow" => "REAR_STOCK_VOID_RAIL_BOW_KERNEL_CHILD_NOT_PERSISTED_IN_READ_ONLY_PROPOSAL",
                "rear_stock_void_boundary_bridge" => {
                    "REAR_STOCK_VOID_BOUNDARY_BRIDGE_KERNEL_CHILD_NOT_PERSISTED_IN_READ_ONLY_PROPOSAL"
                }
                _ => unreachable!("typed edit operation is closed"),
            },
            "SIX_VIEW_NON_REGRESSION_REQUIRED_NOT_EVALUATED",
            if context.source_form_art_current_cohort_compatible {
                "SOURCE_FORM_ART_CURRENT_COHORT_VERIFIED"
            } else {
                "SOURCE_FORM_ART_HISTORICAL_COHORT_READABLE_BUT_NOT_PREPARE_ELIGIBLE"
            },
            "NO_SECONDARY_FORM_APPROVAL",
            "NO_STAGE_ADVANCEMENT",
            "NO_CANDIDATE_CONFIRM",
            "NO_VERSION_CREATED",
            "NO_EXPORT",
            "NO_VISUAL_QUALITY_CLAIM"
        ],
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "canonical_sha256": ""
    });
    if let Some(child) = result
        .get_mut("child_revision")
        .and_then(Value::as_object_mut)
    {
        child.insert(
            format!("{operation}_policy"),
            Value::String(edit_policy.to_owned()),
        );
    }
    if let Some(result_object) = result.as_object_mut() {
        result_object.insert(
            format!("{operation}_policy"),
            Value::String(edit_policy.to_owned()),
        );
    }
    if serde_json::to_vec(&result)
        .map_err(|error| invalid(error.to_string()))?
        .len()
        > MAX_RESPONSE_BYTES
    {
        return Err(invalid("proposal result exceeds max_response_bytes"));
    }
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    Ok(result)
}

pub(crate) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    get_result(&load_context(runtime, request)?)
}

#[derive(Debug, Clone)]
struct SourceProgramContext {
    program: Value,
    reference_id: Option<String>,
    evidence: GeometryCandidateEvidenceRecord,
}

fn load_source_program(
    runtime: &Runtime,
    context: &ProposalContext,
) -> Result<SourceProgramContext, RuntimeError> {
    load_source_program_for_binding(
        runtime,
        &context.project_id,
        &context.candidate_id,
        &context.source_binding,
        &context.source_node_id,
        &context.part_id,
    )
}

fn load_source_program_for_binding(
    runtime: &Runtime,
    project_id: &str,
    candidate_id: &str,
    source_binding: &forgecad_contracts::AuthoringMeshV2SourceBinding,
    source_node_id: &str,
    part_id: &str,
) -> Result<SourceProgramContext, RuntimeError> {
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(candidate_id)?
        .ok_or_else(|| invalid("source candidate geometry evidence is unavailable"))?;
    if evidence.project_id != project_id
        || evidence.candidate_id != candidate_id
        || evidence.geometry_program_sha256 != source_binding.geometry_program_sha256
        || evidence.artifact_object_sha256 != source_binding.artifact_sha256
    {
        return Err(invalid(
            "source candidate GeometryProgram/artifact binding differs",
        ));
    }
    let bytes =
        runtime.cas_read_bounded(&evidence.geometry_program_object_sha256, MAX_JSON_BYTES)?;
    let mut program: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("source GeometryProgram is invalid JSON: {error}")))?;
    let object = program
        .as_object()
        .ok_or_else(|| invalid("source GeometryProgram is not an object"))?;
    if object.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
        || object.get("project_id").and_then(Value::as_str) != Some(project_id)
        || object.contains_key("canonical_sha256")
    {
        return Err(invalid("source GeometryProgram draft shape differs"));
    }
    let hash = hash_geometry_program_with_runtime_worker(&program).map_err(|error| {
        invalid(format!(
            "source GeometryProgram Worker hash failed: {error}"
        ))
    })?;
    if hash.get("canonical_sha256").and_then(Value::as_str)
        != Some(source_binding.geometry_program_sha256.as_str())
    {
        return Err(invalid("source GeometryProgram hash differs from binding"));
    }
    program["canonical_sha256"] = Value::String(source_binding.geometry_program_sha256.clone());

    let nodes = program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("source GeometryProgram nodes are unavailable"))?;
    let matching_nodes = nodes
        .iter()
        .filter(|node| node.get("node_id").and_then(Value::as_str) == Some(source_node_id))
        .collect::<Vec<_>>();
    if matching_nodes.len() != 1 {
        return Err(invalid(
            "source GeometryProgram node is absent or ambiguous",
        ));
    }
    let outputs = program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("source GeometryProgram PartOutputs are unavailable"))?;
    let matching_parts = outputs
        .iter()
        .filter(|output| output.get("part_id").and_then(Value::as_str) == Some(part_id))
        .collect::<Vec<_>>();
    if matching_parts.len() != 1
        || matching_parts[0]
            .get("input_node_ids")
            .and_then(Value::as_array)
            .is_none_or(|ids| {
                ids.iter()
                    .filter(|value| value.as_str() == Some(source_node_id))
                    .count()
                    != 1
            })
        || outputs
            .iter()
            .filter(|output| {
                output
                    .get("input_node_ids")
                    .and_then(Value::as_array)
                    .is_some_and(|ids| {
                        ids.iter()
                            .any(|value| value.as_str() == Some(source_node_id))
                    })
            })
            .count()
            != 1
    {
        return Err(invalid(
            "source node is not uniquely owned by the requested Part",
        ));
    }
    Ok(SourceProgramContext {
        program,
        reference_id: evidence.reference_id.clone(),
        evidence,
    })
}

fn source_program_vec3(node: &Value, field: &str, node_id: &str) -> Result<[f64; 3], RuntimeError> {
    let values = node
        .pointer(&format!("/parameters/{field}"))
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("{node_id} {field} is unavailable")))?;
    if values.len() != 3 {
        return Err(invalid(format!("{node_id} {field} must have three values")));
    }
    let vector = values
        .iter()
        .map(|value| {
            value
                .as_f64()
                .ok_or_else(|| invalid(format!("{node_id} {field} contains a non-number")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let vector = [vector[0], vector[1], vector[2]];
    finite_position(vector, &format!("{node_id} {field}"))?;
    Ok(vector)
}

fn axis_label(axis: usize) -> &'static str {
    match axis {
        0 => "X",
        1 => "Y",
        2 => "Z",
        _ => unreachable!("source axis index is bounded"),
    }
}

/// Derive the rail-bow orientation from the source GeometryProgram rather
/// than accepting caller-selected vertices or normals.  The two source node
/// centers identify the local void-facing axis; the upper box dimensions
/// identify the longitudinal/depth axes used by the typed kernel.
fn derive_rear_stock_void_rail_bow_orientation(
    source_program: &Value,
    source_node_id: &str,
    part_id: &str,
) -> Result<RearStockVoidRailBowOrientation, RuntimeError> {
    if source_node_id != "rear-stock" || part_id != "rear-stock" {
        return Err(invalid(
            "rear-stock rail-bow semantic derivation requires the rear-stock source Part",
        ));
    }
    let nodes = source_program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("source GeometryProgram nodes are unavailable"))?;
    let node = |node_id: &str| -> Result<&Value, RuntimeError> {
        let matches = nodes
            .iter()
            .filter(|value| value.get("node_id").and_then(Value::as_str) == Some(node_id))
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(invalid(format!(
                "source GeometryProgram {node_id} node is absent or ambiguous"
            )));
        }
        Ok(matches[0])
    };
    let upper = node("rear-stock")?;
    let lower = node("rear-stock-lower-beam")?;
    let upper_position = source_program_vec3(upper, "position_m", "rear-stock")?;
    let lower_position = source_program_vec3(lower, "position_m", "rear-stock-lower-beam")?;
    let upper_size = source_program_vec3(upper, "size_m", "rear-stock")?;
    let lower_size = source_program_vec3(lower, "size_m", "rear-stock-lower-beam")?;
    if upper_size
        .iter()
        .any(|value| *value <= POSITION_TOLERANCE_M)
        || lower_size
            .iter()
            .any(|value| *value <= POSITION_TOLERANCE_M)
    {
        return Err(invalid("rear-stock rail-bow source box sizes are invalid"));
    }

    let relation = [
        lower_position[0] - upper_position[0],
        lower_position[1] - upper_position[1],
        lower_position[2] - upper_position[2],
    ];
    finite_position(relation, "rear-stock source-local relation")?;
    let void_axis = (0..3)
        .max_by(|left, right| relation[*left].abs().total_cmp(&relation[*right].abs()))
        .expect("three source axes");
    let relation_magnitude = relation[void_axis].abs();
    if relation_magnitude <= upper_size[void_axis] * 0.5 + POSITION_TOLERANCE_M
        || (0..3).any(|axis| {
            axis != void_axis
                && (relation[axis].abs()
                    > (upper_size[axis] + lower_size[axis]) * 0.5 + POSITION_TOLERANCE_M)
        })
    {
        return Err(invalid(
            "rear-stock lower beam does not define one source-local void axis",
        ));
    }
    let second_axis = (0..3)
        .filter(|axis| *axis != void_axis)
        .max_by(|left, right| upper_size[*left].total_cmp(&upper_size[*right]))
        .expect("two non-void source axes");
    let third_axis = (0..3)
        .find(|axis| *axis != void_axis && *axis != second_axis)
        .expect("three distinct source axes");
    if (upper_size[second_axis] - upper_size[third_axis]).abs() <= POSITION_TOLERANCE_M {
        return Err(invalid(
            "rear-stock source longitudinal/depth axes are ambiguous",
        ));
    }
    let mut normal = [0.0; 3];
    normal[void_axis] = relation[void_axis].signum();
    Ok(RearStockVoidRailBowOrientation {
        derivation_policy: REAR_STOCK_VOID_RAIL_BOW_EDIT_POLICY.to_owned(),
        source_space: "source-local".to_owned(),
        upper_node_id: "rear-stock".to_owned(),
        lower_node_id: "rear-stock-lower-beam".to_owned(),
        source_local_relation_m: relation,
        void_centroid_m: relation,
        void_face_normal_m: normal,
        void_axis: axis_label(void_axis).to_owned(),
        longitudinal_axis: axis_label(second_axis).to_owned(),
        depth_axis: axis_label(third_axis).to_owned(),
    })
}

fn replace_source_node(
    source_program: &Value,
    source_node_id: &str,
    part_id: &str,
    parameters: Value,
) -> Result<Value, RuntimeError> {
    let mut derived = source_program.clone();
    let nodes = derived
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("source GeometryProgram nodes are unavailable"))?;
    let indices = nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.get("node_id").and_then(Value::as_str) == Some(source_node_id))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if indices.len() != 1 {
        return Err(invalid("source node replacement is not unique"));
    }
    let node = nodes
        .get_mut(indices[0])
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("source node is not an object"))?;
    node.insert(
        "operator_id".to_owned(),
        Value::String("forgecad.geometry.authoring-mesh@1".to_owned()),
    );
    node.insert("inputs".to_owned(), json!([]));
    node.insert("parameters".to_owned(), parameters);

    let outputs = derived
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("derived GeometryProgram PartOutputs are unavailable"))?;
    if outputs
        .iter()
        .filter(|output| output.get("part_id").and_then(Value::as_str) == Some(part_id))
        .count()
        != 1
        || outputs
            .iter()
            .filter(|output| {
                output
                    .get("input_node_ids")
                    .and_then(Value::as_array)
                    .is_some_and(|ids| {
                        ids.iter()
                            .any(|value| value.as_str() == Some(source_node_id))
                    })
            })
            .count()
            != 1
    {
        return Err(invalid("derived Part output ownership is ambiguous"));
    }
    derived
        .as_object_mut()
        .expect("derived GeometryProgram is an object")
        .remove("canonical_sha256");
    Ok(derived)
}

#[derive(Debug, Clone)]
struct DerivedGeometry {
    program: Value,
    program_sha256: String,
    part_ids: Vec<String>,
    worker_build_cohort_sha256: Option<String>,
    triangle_count: u64,
    source_program_sha256: String,
    source_program_object_sha256: String,
    source_artifact_sha256: String,
    source_artifact_readback_sha256: String,
}

fn materialize_geometry_program(
    runtime: &Runtime,
    context: &ProposalContext,
    child_revision: &AuthoringMeshRevision,
) -> Result<DerivedGeometry, RuntimeError> {
    let source = load_source_program(runtime, context)?;
    let parameters = authoring_mesh_v2_geometry_parameters(
        child_revision,
        context.source_binding.position_m,
        context.source_binding.rotation_rad,
    )?;
    let mut program = replace_source_node(
        &source.program,
        &context.source_node_id,
        &context.part_id,
        parameters,
    )?;
    let hash = hash_geometry_program_with_runtime_worker(&program).map_err(|error| {
        invalid(format!(
            "derived GeometryProgram Worker hash failed: {error}"
        ))
    })?;
    let program_sha256 = hash
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("derived GeometryProgram hash is unavailable"))?
        .to_owned();
    if program_sha256 == context.source_binding.geometry_program_sha256 {
        return Err(invalid(
            "typed authoring edit did not change the GeometryProgram",
        ));
    }
    program["canonical_sha256"] = Value::String(program_sha256.clone());
    let first = compile_geometry_with_runtime_worker(&program, None)
        .map_err(|error| invalid(format!("derived Geometry Worker compile failed: {error}")))?;
    let repeat = compile_geometry_with_runtime_worker(&program, None)
        .map_err(|error| invalid(format!("derived Geometry Worker replay failed: {error}")))?;
    if first.glb != repeat.glb
        || first.program_sha256 != program_sha256
        || repeat.program_sha256 != program_sha256
        || first.part_ids != repeat.part_ids
        || first.material_zone_ids != repeat.material_zone_ids
        || first.build_cohort_sha256 != repeat.build_cohort_sha256
    {
        return Err(invalid("derived Geometry Worker replay is not byte exact"));
    }
    let inspection = strict_glb_inspection(&first.glb)?;
    validate_worker_metadata(&first, &inspection)?;
    if !inspection.hard_gate_passed
        || !inspection
            .part_ids
            .iter()
            .any(|value| value == &context.part_id)
    {
        return Err(invalid("derived Geometry Worker strict readback failed"));
    }
    Ok(DerivedGeometry {
        program,
        program_sha256,
        part_ids: inspection.part_ids,
        worker_build_cohort_sha256: first.build_cohort_sha256,
        triangle_count: inspection.triangle_count,
        source_program_sha256: context.source_binding.geometry_program_sha256.clone(),
        source_program_object_sha256: source.evidence.geometry_program_object_sha256,
        source_artifact_sha256: context.source_binding.artifact_sha256.clone(),
        source_artifact_readback_sha256: context.source_binding.artifact_readback_sha256.clone(),
    })
}

/// Build the immutable child in memory before touching the durable revision
/// index.  The prepare pipeline has several later failure points (the
/// Geometry Worker, candidate transaction, six-view FormArt replay and the
/// proposal-side receipt).  Keeping this step write-free means those errors
/// cannot leave a reachable AuthoringMesh child that has no corresponding
/// proposal result.  The child is persisted only after all of those stages
/// have succeeded (or produced the normal reviewable/rejected result).
fn materialize_child_revision(
    context: &ProposalContext,
) -> Result<AuthoringMeshRevision, RuntimeError> {
    let child = match &context.typed_edit {
        TypedEditOutput::MoveVertices(move_vertices) => {
            let vertex_ids = move_vertices
                .vertex_moves
                .iter()
                .map(|value| AuthoringMeshVertexId(value.vertex_id.clone()))
                .collect::<Vec<_>>();
            let delta_m = move_vertices
                .vertex_moves
                .iter()
                .map(|value| value.delta_m)
                .collect::<Vec<_>>();
            AuthoringMeshV2Revision::from_record(context.parent.clone())?
                .move_vertices(AuthoringMeshMoveVerticesRequest {
                    operation_id: context.operation_id.clone(),
                    parent_revision_id: AuthoringMeshRevisionId(
                        context.parent.revision_id.0.clone(),
                    ),
                    vertex_ids,
                    delta_m,
                    operation_lineage_sha256: context.operation_lineage_sha256.clone(),
                })?
                .child_revision
        }
        TypedEditOutput::OpenFrameNotch(edit) => {
            AuthoringMeshV2Revision::from_record(context.parent.clone())?
                .open_frame_notch(AuthoringMeshOpenFrameNotchRequest {
                    operation_id: context.operation_id.clone(),
                    parent_revision_id: AuthoringMeshRevisionId(
                        context.parent.revision_id.0.clone(),
                    ),
                    opening_width_milli: edit.opening_width_milli,
                    opening_height_milli: edit.opening_height_milli,
                    operation_lineage_sha256: context.operation_lineage_sha256.clone(),
                })?
                .child_revision
        }
        TypedEditOutput::RearStockVoidRailBow(edit) => {
            let orientation = &edit.semantic_orientation;
            AuthoringMeshV2Revision::from_record(context.parent.clone())?
                .rear_stock_void_rail_bow(AuthoringMeshRearStockVoidRailBowRequest {
                    operation_id: context.operation_id.clone(),
                    parent_revision_id: AuthoringMeshRevisionId(
                        context.parent.revision_id.0.clone(),
                    ),
                    expected_void_centroid_m: orientation.void_centroid_m,
                    expected_void_face_normal_m: orientation.void_face_normal_m,
                    operation_lineage_sha256: context.operation_lineage_sha256.clone(),
                })?
                .child_revision
        }
        TypedEditOutput::RearStockVoidBoundaryBridge(_) => {
            AuthoringMeshV2Revision::from_record(context.parent.clone())?
                .rear_stock_void_boundary_bridge(AuthoringMeshRearStockVoidBoundaryBridgeRequest {
                    operation_id: context.operation_id.clone(),
                    parent_revision_id: AuthoringMeshRevisionId(
                        context.parent.revision_id.0.clone(),
                    ),
                    operation_lineage_sha256: context.operation_lineage_sha256.clone(),
                })?
                .child_revision
        }
    };
    AuthoringMeshV2Revision::from_record(child.clone())?;
    validate_child_revision_binding(context, &child)?;
    Ok(child)
}

fn validate_child_revision_binding(
    context: &ProposalContext,
    child: &AuthoringMeshRevision,
) -> Result<(), RuntimeError> {
    if child.mesh_id.0 != context.mesh_id
        || child.lineage_id.0 != context.lineage_id
        || child.parent_revision_ids
            != vec![AuthoringMeshRevisionId(
                context.parent.revision_id.0.clone(),
            )]
        || child
            .operation
            .as_ref()
            .map(|value| value.operation_lineage_sha256.as_str())
            != Some(context.operation_lineage_sha256.as_str())
    {
        return Err(invalid("derived AuthoringMesh child binding differs"));
    }
    Ok(())
}

/// Persist the already-validated child at the commit boundary of the
/// multi-stage proposal.  The Store operation remains immutable and exactly
/// idempotent; this function deliberately does not mutate an existing child
/// or attempt compensating deletion from the revision DAG.
fn persist_durable_child(
    runtime: &Runtime,
    context: &ProposalContext,
    child_revision: AuthoringMeshRevision,
) -> Result<(Value, AuthoringMeshRevision), RuntimeError> {
    match &context.typed_edit {
        TypedEditOutput::MoveVertices(_) => {
            persist_durable_move_vertices_child(runtime, context, child_revision)
        }
        TypedEditOutput::OpenFrameNotch(_) => {
            persist_durable_open_frame_notch_child(runtime, context, child_revision)
        }
        TypedEditOutput::RearStockVoidRailBow(_) => {
            persist_durable_rear_stock_void_rail_bow_child(runtime, context, child_revision)
        }
        TypedEditOutput::RearStockVoidBoundaryBridge(_) => {
            persist_durable_rear_stock_void_boundary_bridge_child(runtime, context, child_revision)
        }
    }
}

fn persist_durable_move_vertices_child(
    runtime: &Runtime,
    context: &ProposalContext,
    child_revision: AuthoringMeshRevision,
) -> Result<(Value, AuthoringMeshRevision), RuntimeError> {
    let TypedEditOutput::MoveVertices(move_vertices) = &context.typed_edit else {
        return Err(invalid(
            "move_vertices durable child received another typed edit",
        ));
    };
    let vertex_ids = move_vertices
        .vertex_moves
        .iter()
        .map(|value| AuthoringMeshVertexId(value.vertex_id.clone()))
        .collect::<Vec<_>>();
    let delta_m = move_vertices
        .vertex_moves
        .iter()
        .map(|value| value.delta_m)
        .collect::<Vec<_>>();
    validate_child_revision_binding(context, &child_revision)?;
    let durable_idempotency_key = format!(
        "{}-{}",
        context.typed_edit.durable_idempotency_prefix(),
        &canonical_json_hash(&json!({
            "request_input_sha256":context.request_input_sha256,
            "operation_lineage_sha256":context.operation_lineage_sha256,
        }))[..48]
    );
    let mut request = json!({
        "schema_version": AUTHORING_MESH_V2_DURABLE_PREPARE_REQUEST_SCHEMA_VERSION,
        "project_id": context.project_id,
        "operation": "move_vertices",
        "mesh_id": context.mesh_id,
        "lineage_id": context.lineage_id,
        "parent_revision_id": context.parent.revision_id.0,
        "operation_id": context.operation_id,
        "edge_id": null,
        "split_ratio_milli": null,
        "vertex_ids": vertex_ids.iter().map(|value| value.0.clone()).collect::<Vec<_>>(),
        "delta_m": delta_m,
        "operation_lineage_sha256": context.operation_lineage_sha256,
        "positions_m": null,
        "faces": null,
        "evaluated": null,
        "idempotency_key": durable_idempotency_key,
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": false,
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "input_sha256": ""
    });
    request["input_sha256"] = Value::String(canonical_json_hash(&request));
    let durable = runtime.authoring_mesh_v2_durable_prepare(&request)?;
    let persisted_child: AuthoringMeshRevision = serde_json::from_value(
        durable
            .get("revision")
            .cloned()
            .ok_or_else(|| invalid("durable MoveVertices child revision is missing"))?,
    )
    .map_err(|error| {
        invalid(format!(
            "durable MoveVertices child revision is malformed: {error}"
        ))
    })?;
    AuthoringMeshV2Revision::from_record(persisted_child.clone())?;
    validate_child_revision_binding(context, &persisted_child)?;
    if persisted_child != child_revision {
        return Err(invalid(
            "durable MoveVertices child differs from the preflight child",
        ));
    }
    Ok((durable, persisted_child))
}

fn persist_durable_open_frame_notch_child(
    runtime: &Runtime,
    context: &ProposalContext,
    child_revision: AuthoringMeshRevision,
) -> Result<(Value, AuthoringMeshRevision), RuntimeError> {
    validate_child_revision_binding(context, &child_revision)?;
    let durable_idempotency_key = format!(
        "{}-{}",
        context.typed_edit.durable_idempotency_prefix(),
        &canonical_json_hash(&json!({
            "request_input_sha256": context.request_input_sha256,
            "operation_lineage_sha256": context.operation_lineage_sha256,
        }))[..48]
    );
    let durable = super::authoring_mesh_v2_durable::persist_runtime_derived_source_child(
        runtime,
        &context.project_id,
        &context.request_input_sha256,
        &durable_idempotency_key,
        child_revision.clone(),
    )?;
    let persisted_child: AuthoringMeshRevision = serde_json::from_value(
        durable
            .get("revision")
            .cloned()
            .ok_or_else(|| invalid("durable OpenFrameNotch child revision is missing"))?,
    )
    .map_err(|error| {
        invalid(format!(
            "durable OpenFrameNotch child revision is malformed: {error}"
        ))
    })?;
    AuthoringMeshV2Revision::from_record(persisted_child.clone())?;
    validate_child_revision_binding(context, &persisted_child)?;
    if persisted_child != child_revision {
        return Err(invalid(
            "durable OpenFrameNotch child differs from the preflight child",
        ));
    }
    Ok((durable, persisted_child))
}

fn persist_durable_rear_stock_void_rail_bow_child(
    runtime: &Runtime,
    context: &ProposalContext,
    child_revision: AuthoringMeshRevision,
) -> Result<(Value, AuthoringMeshRevision), RuntimeError> {
    let TypedEditOutput::RearStockVoidRailBow(_) = &context.typed_edit else {
        return Err(invalid(
            "rear_stock_void_rail_bow durable child received another typed edit",
        ));
    };
    validate_child_revision_binding(context, &child_revision)?;
    let durable_idempotency_key = format!(
        "{}-{}",
        DURABLE_REAR_STOCK_VOID_RAIL_BOW_IDEMPOTENCY_PREFIX,
        &canonical_json_hash(&json!({
            "request_input_sha256": context.request_input_sha256,
            "operation_lineage_sha256": context.operation_lineage_sha256,
        }))[..48]
    );
    let durable = super::authoring_mesh_v2_durable::persist_runtime_derived_source_child(
        runtime,
        &context.project_id,
        &context.request_input_sha256,
        &durable_idempotency_key,
        child_revision.clone(),
    )?;
    let persisted_child: AuthoringMeshRevision = serde_json::from_value(
        durable
            .get("revision")
            .cloned()
            .ok_or_else(|| invalid("durable RearStockVoidRailBow child revision is missing"))?,
    )
    .map_err(|error| {
        invalid(format!(
            "durable RearStockVoidRailBow child revision is malformed: {error}"
        ))
    })?;
    AuthoringMeshV2Revision::from_record(persisted_child.clone())?;
    validate_child_revision_binding(context, &persisted_child)?;
    if persisted_child != child_revision {
        return Err(invalid(
            "durable RearStockVoidRailBow child differs from the preflight child",
        ));
    }
    Ok((durable, persisted_child))
}

fn persist_durable_rear_stock_void_boundary_bridge_child(
    runtime: &Runtime,
    context: &ProposalContext,
    child_revision: AuthoringMeshRevision,
) -> Result<(Value, AuthoringMeshRevision), RuntimeError> {
    let TypedEditOutput::RearStockVoidBoundaryBridge(_) = &context.typed_edit else {
        return Err(invalid(
            "rear_stock_void_boundary_bridge durable child received another typed edit",
        ));
    };
    validate_child_revision_binding(context, &child_revision)?;
    let durable_idempotency_key = format!(
        "{}-{}",
        DURABLE_REAR_STOCK_VOID_BOUNDARY_BRIDGE_IDEMPOTENCY_PREFIX,
        &canonical_json_hash(&json!({
            "request_input_sha256": context.request_input_sha256,
            "operation_lineage_sha256": context.operation_lineage_sha256,
        }))[..48]
    );
    let durable = super::authoring_mesh_v2_durable::persist_runtime_derived_source_child(
        runtime,
        &context.project_id,
        &context.request_input_sha256,
        &durable_idempotency_key,
        child_revision.clone(),
    )?;
    let persisted_child: AuthoringMeshRevision =
        serde_json::from_value(durable.get("revision").cloned().ok_or_else(|| {
            invalid("durable RearStockVoidBoundaryBridge child revision is missing")
        })?)
        .map_err(|error| {
            invalid(format!(
                "durable RearStockVoidBoundaryBridge child is malformed: {error}"
            ))
        })?;
    AuthoringMeshV2Revision::from_record(persisted_child.clone())?;
    validate_child_revision_binding(context, &persisted_child)?;
    if persisted_child != child_revision {
        return Err(invalid(
            "durable RearStockVoidBoundaryBridge child differs from the preflight child",
        ));
    }
    Ok((durable, persisted_child))
}

/// Materialize proposal-side owner evidence from the exact Part-ID AOVs that
/// were rendered for the six-view proposal comparison.  The source FormArt
/// receipt supplies only the reviewed reference contour and registered camera
/// coordinate frame; every candidate-dependent pixel and artifact binding in
/// this receipt belongs to the derived proposal candidate.
fn source_artifact_part_ids(
    runtime: &Runtime,
    scope: &FormArtEvaluationScope<'_>,
) -> Result<Vec<String>, RuntimeError> {
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(scope.source_candidate_id)?
        .ok_or_else(|| invalid("source GeometryCandidateEvidence is unavailable"))?;
    if evidence.project_id != scope.project_id
        || evidence.candidate_id != scope.source_candidate_id
        || evidence.artifact_object_sha256 != scope.source_artifact_sha256
    {
        return Err(invalid(
            "source GeometryCandidateEvidence ArtifactReadback binding differs",
        ));
    }
    let readback = read_bound_json(
        runtime,
        &evidence.artifact_readback_object_sha256,
        "source ArtifactReadback",
    )?;
    if readback.get("canonical_sha256").and_then(Value::as_str)
        != Some(scope.source_artifact_readback_sha256)
    {
        return Err(invalid("source ArtifactReadback canonical hash differs"));
    }
    let part_ids = readback
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("source ArtifactReadback Part vocabulary is unavailable"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| is_opaque_id(value))
                .map(str::to_owned)
                .ok_or_else(|| invalid("source ArtifactReadback Part ID is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if part_ids.is_empty() || part_ids.iter().collect::<BTreeSet<_>>().len() != part_ids.len() {
        return Err(invalid(
            "source ArtifactReadback Part vocabulary is empty or ambiguous",
        ));
    }
    Ok(part_ids)
}

fn materialize_proposal_form_art_evidence(
    runtime: &Runtime,
    scope: &FormArtEvaluationScope<'_>,
    proposal_candidate_id: &str,
    proposal_candidate_state_sha256: &str,
    source_artifact_readback_sha256: &str,
    proposal_artifact_sha256: &str,
    proposal_artifact_readback_sha256: &str,
    worker_build_cohort_sha256: Option<&str>,
    proposal_part_ids: &[String],
    bundle_sha256: &str,
    view_evaluations: &[super::agentic_action::ViewEvaluation],
) -> Result<Value, RuntimeError> {
    if !proposal_part_ids
        .iter()
        .any(|part_id| part_id == "rear-stock")
        || proposal_part_ids.iter().collect::<BTreeSet<_>>().len() != proposal_part_ids.len()
    {
        return Err(invalid(
            "proposal ArtifactReadback does not contain one exact rear-stock Part",
        ));
    }
    let bundle = read_bound_json(runtime, bundle_sha256, "CrossViewEvidenceBundle")?;
    super::validate_cross_view_evidence_bundle(&bundle)?;
    if bundle.get("candidate_id").and_then(Value::as_str) != Some(proposal_candidate_id)
        || bundle.get("candidate_state_sha256").and_then(Value::as_str)
            != Some(proposal_candidate_state_sha256)
        || bundle.get("artifact_sha256").and_then(Value::as_str) != Some(proposal_artifact_sha256)
    {
        return Err(invalid(
            "proposal owner evidence CrossViewEvidenceBundle binding differs",
        ));
    }
    let bundle_views = bundle
        .get("view_evaluations")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("proposal owner evidence bundle views are unavailable"))?;
    let thresholds =
        super::production_weapon_form_art_evidence::ReviewedRegionPartBindingThresholds {
            min_owner_region_pixels: Some(128),
            min_boundary_adjacency_pixels: Some(32),
            max_owner_expected_void_overlap_milli: Some(0),
        };
    let source_part_ids = scope
        .fresh_baseline
        .as_ref()
        .map(|_| source_artifact_part_ids(runtime, scope))
        .transpose()?;
    let mut rows = Vec::with_capacity(REQUIRED_VIEW_KINDS.len());
    let mut owner_views_passed = true;
    for kind in REQUIRED_VIEW_KINDS {
        let view = view_evaluations
            .iter()
            .find(|view| view.kind == kind)
            .ok_or_else(|| invalid(format!("proposal owner evidence {kind} view is missing")))?;
        let art_view = scope
            .art
            .views
            .iter()
            .find(|view| view.view_kind == kind)
            .ok_or_else(|| invalid(format!("source FormArt {kind} view is missing")))?;
        let baseline_view = scope
            .fresh_baseline
            .as_ref()
            .map(|baseline| {
                baseline
                    .views
                    .iter()
                    .find(|candidate| candidate.view_kind == kind)
                    .ok_or_else(|| invalid(format!("fresh baseline {kind} view is missing")))
            })
            .transpose()?;
        if view.view_id != art_view.view_id
            || view.reference_id != art_view.reference_id
            || view.reference_sha256 != art_view.reference_sha256
        {
            return Err(invalid(format!(
                "proposal owner evidence {kind} reviewed semantic view differs"
            )));
        }
        let (camera_hash, camera_canonical_sha256, camera_source) =
            if let Some(baseline_view) = baseline_view {
                if baseline_view.reference_id != view.reference_id
                    || baseline_view.reference_sha256 != view.reference_sha256
                    || view.camera.get("camera_hash").and_then(Value::as_str)
                        != Some(baseline_view.camera_hash.as_str())
                    || view.camera.get("canonical_sha256").and_then(Value::as_str)
                        != Some(baseline_view.camera_canonical_sha256.as_str())
                {
                    return Err(invalid(format!(
                        "proposal owner evidence {kind} fresh baseline camera lineage differs"
                    )));
                }
                (
                    baseline_view.camera_hash.as_str(),
                    baseline_view.camera_canonical_sha256.as_str(),
                    "fresh-same-cohort-baseline-rig-v2",
                )
            } else {
                if view.camera.get("camera_hash").and_then(Value::as_str)
                    != Some(art_view.camera_hash.as_str())
                    || view.camera.get("canonical_sha256").and_then(Value::as_str)
                        != Some(art_view.camera_canonical_sha256.as_str())
                {
                    return Err(invalid(format!(
                        "proposal owner evidence {kind} historical camera lineage differs"
                    )));
                }
                (
                    art_view.camera_hash.as_str(),
                    art_view.camera_canonical_sha256.as_str(),
                    "historical-form-art-camera",
                )
            };
        let bundle_view = bundle_views
            .iter()
            .find(|entry| entry.get("kind").and_then(Value::as_str) == Some(kind))
            .ok_or_else(|| {
                invalid(format!(
                    "proposal owner evidence {kind} bundle row is missing"
                ))
            })?;
        let render_set_object_sha256 = bundle_view
            .get("proposal_render_set_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| {
                invalid(format!(
                    "proposal owner evidence {kind} RenderSet is missing"
                ))
            })?;
        let render_set = read_bound_json(runtime, render_set_object_sha256, "proposal RenderSet")?;
        super::validate_render_set_v2_output(&render_set)?;
        let render_set_canonical_sha256 = render_set
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| {
                invalid(format!(
                    "proposal owner evidence {kind} RenderSet canonical hash is missing"
                ))
            })?;
        let mut render_set_preimage = render_set.clone();
        render_set_preimage["canonical_sha256"] = Value::String(String::new());
        if render_set.get("candidate_id").and_then(Value::as_str) != Some(proposal_candidate_id)
            || render_set.get("artifact_sha256").and_then(Value::as_str)
                != Some(proposal_artifact_sha256)
            || render_set.get("view_id").and_then(Value::as_str) != Some(view.view_id.as_str())
            || render_set.get("camera_hash").and_then(Value::as_str) != Some(camera_hash)
            || render_set.get("reference_id").and_then(Value::as_str)
                != Some(view.reference_id.as_str())
            || canonical_json_hash(&render_set_preimage) != render_set_canonical_sha256
        {
            return Err(invalid(format!(
                "proposal owner evidence {kind} RenderSet binding differs"
            )));
        }
        let pass_hash = |name: &str| -> Result<&str, RuntimeError> {
            render_set
                .get("pass_artifacts")
                .and_then(|passes| passes.get(name))
                .and_then(|pass| pass.get("sha256"))
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| {
                    invalid(format!(
                        "proposal owner evidence {kind} {name} pass is missing"
                    ))
                })
        };
        let silhouette_pass_sha256 = pass_hash("silhouette")?;
        let part_id_pass_sha256 = pass_hash("part-id")?;
        let depth_pass_sha256 = pass_hash("depth")?;
        let normal_pass_sha256 = pass_hash("normal")?;
        let silhouette_png = runtime.cas_read_bounded(silhouette_pass_sha256, 16 * 1024 * 1024)?;
        let part_png = runtime.cas_read_bounded(part_id_pass_sha256, 16 * 1024 * 1024)?;
        let depth_png = runtime.cas_read_bounded(depth_pass_sha256, 16 * 1024 * 1024)?;
        let normal_png = runtime.cas_read_bounded(normal_pass_sha256, 16 * 1024 * 1024)?;
        let target = runtime.read_silhouette_target(&art_view.target_object_sha256)?;
        if target.get("canonical_sha256").and_then(Value::as_str)
            != Some(art_view.target_canonical_sha256.as_str())
            || target.get("reference_id").and_then(Value::as_str)
                != Some(art_view.reference_id.as_str())
            || target.get("reference_sha256").and_then(Value::as_str)
                != Some(art_view.reference_sha256.as_str())
        {
            return Err(invalid(format!(
                "proposal owner evidence {kind} target binding differs"
            )));
        }
        let target_annotation_confirmed = target.get("source").and_then(Value::as_str)
            == Some("user_refined")
            && target.get("annotation_status").and_then(Value::as_str) == Some("user_confirmed");
        let visual_structure = target.get("visual_structure").ok_or_else(|| {
            invalid(format!(
                "proposal owner evidence {kind} visual structure is missing"
            ))
        })?;
        if visual_structure
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(art_view.visual_structure_canonical_sha256.as_str())
            || art_view.visual_structure_review_status != "user_confirmed"
            || !target_annotation_confirmed
            || visual_structure
                .get("review_status")
                .and_then(Value::as_str)
                != Some("user_confirmed")
        {
            return Err(invalid(format!(
                "proposal owner evidence {kind} reviewed visual structure differs"
            )));
        }
        let crop = super::reference_view_crop(&view.view_spec)?;
        let rotation_degrees = super::reference_view_rotation_degrees(&view.view_spec)?;
        let target_mask = super::project_reference_mask_to_view(
            &runtime
                .target_mask(&art_view.target_object_sha256, &target)?
                .mask,
            &view.view_spec,
            true,
        )?;
        let (
            expected_visible_part_ids,
            source_form_evidence_view_receipt_object_sha256,
            source_form_evidence_view_receipt_canonical_sha256,
            source_baseline_view_receipt_object_sha256,
        ) = if let Some(baseline_view) = baseline_view {
            let baseline_render_set = read_bound_json(
                runtime,
                &baseline_view.render_set_object_sha256,
                "fresh baseline source RenderSet",
            )?;
            let baseline_part_id_sha256 = baseline_render_set
                .get("pass_artifacts")
                .and_then(|passes| passes.get("part-id"))
                .and_then(|pass| pass.get("sha256"))
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| {
                    invalid(format!(
                        "proposal owner evidence {kind} fresh baseline Part-ID pass is missing"
                    ))
                })?;
            if baseline_view
                .pass_artifact_object_sha256
                .get(5)
                .map(String::as_str)
                != Some(baseline_part_id_sha256)
            {
                return Err(invalid(format!(
                    "proposal owner evidence {kind} fresh baseline Part-ID pass binding differs"
                )));
            }
            let baseline_part_png =
                runtime.cas_read_bounded(baseline_part_id_sha256, 16 * 1024 * 1024)?;
            let source_part_ids = source_part_ids
                .as_ref()
                .ok_or_else(|| invalid("fresh baseline source Part vocabulary is unavailable"))?;
            (
                super::production_weapon_form_art_evidence::visible_part_ids(
                    &baseline_part_png,
                    source_part_ids,
                )?,
                None,
                None,
                Some(baseline_view.receipt_object_sha256.clone()),
            )
        } else {
            let source_form_receipt = read_bound_json(
                runtime,
                &art_view.form_evidence_view_receipt_object_sha256,
                "source FormEvidence view receipt",
            )?;
            if source_form_receipt
                .get("canonical_sha256")
                .and_then(Value::as_str)
                != Some(
                    art_view
                        .form_evidence_view_receipt_canonical_sha256
                        .as_str(),
                )
                || source_form_receipt
                    .get("candidate_id")
                    .and_then(Value::as_str)
                    != Some(scope.source_candidate_id)
                || source_form_receipt.get("view_id").and_then(Value::as_str)
                    != Some(view.view_id.as_str())
            {
                return Err(invalid(format!(
                    "proposal owner evidence {kind} source FormEvidence binding differs"
                )));
            }
            let expected = source_form_receipt
                .pointer("/part_id_evidence/observed_part_ids")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    invalid(format!(
                        "proposal owner evidence {kind} expected Part inventory is missing"
                    ))
                })?
                .iter()
                .map(|value| {
                    value.as_str().map(str::to_owned).ok_or_else(|| {
                        invalid(format!(
                            "proposal owner evidence {kind} expected Part ID is invalid"
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            (
                expected,
                Some(art_view.form_evidence_view_receipt_object_sha256.clone()),
                Some(art_view.form_evidence_view_receipt_canonical_sha256.clone()),
                None,
            )
        };
        let observation =
            super::production_weapon_form_art_evidence::derive_proposal_form_art_observation(
                Some(visual_structure),
                true,
                &target_mask,
                &silhouette_png,
                &part_png,
                &depth_png,
                &normal_png,
                proposal_part_ids,
                &expected_visible_part_ids,
                crop,
                rotation_degrees,
            )?;
        let owner_evidence = match kind {
            "left" | "right" | "rear-three-quarter" => {
                let structure_id = match kind {
                    "left" => "left.open-stock-void",
                    "right" => "right.open-stock-void",
                    _ => "rear3q.open-stock-void",
                };
                let diagnostic = super::production_weapon_form_art_evidence::diagnose_reviewed_region_part_binding_with_rotation(
                    visual_structure,
                    &target_mask,
                    &part_png,
                    proposal_part_ids,
                    crop,
                    rotation_degrees,
                    structure_id,
                    &thresholds,
                );
                let assessment = super::production_weapon_form_art_evidence::calibrate_reviewed_region_part_binding_with_rotation(
                    visual_structure,
                    &target_mask,
                    &part_png,
                    proposal_part_ids,
                    crop,
                    rotation_degrees,
                    structure_id,
                    Some(super::production_weapon_form_art_evidence::ReviewedRegionPartBindingTransform::Identity),
                    &thresholds,
                ).and_then(|calibration| {
                    super::production_weapon_form_art_evidence::strict_reviewed_region_part_binding_assessment(
                        &calibration,
                        true,
                    )
                });
                match (diagnostic, assessment) {
                    (_, Ok(assessment)) => json!({
                        "structure_id":assessment.structure_id,
                        "owner_part_id":assessment.owner_part_id,
                        "policy":assessment.policy,
                        "expected_region_canonical_sha256":assessment.expected_region_canonical_sha256,
                        "expected_void_pixel_count":assessment.expected_void_pixel_count,
                        "expected_boundary_pixel_count":assessment.expected_boundary_pixel_count,
                        "owner_region_pixel_count":assessment.owner_region_pixel_count,
                        "owner_boundary_adjacency_pixel_count":assessment.owner_boundary_adjacency_pixel_count,
                        "owner_boundary_adjacency_milli":assessment.owner_boundary_adjacency_milli,
                        "owner_expected_void_overlap_pixel_count":assessment.owner_expected_void_overlap_pixel_count,
                        "owner_expected_void_overlap_milli":assessment.owner_expected_void_overlap_milli,
                        "registered_camera_lineage_verified":true,
                        "strict_owner_void_passed":true,
                        "status":"READY_PROPOSAL_OWNER_VOID_BINDING",
                        "quality_status":"NOT_PROVEN",
                        "depth_status":"UNKNOWN"
                    }),
                    (Ok(diagnostic), Err(error)) => {
                        owner_views_passed = false;
                        let identity = diagnostic.candidates.iter().find(|candidate| {
                            candidate.transform == super::production_weapon_form_art_evidence::ReviewedRegionPartBindingTransform::Identity
                        }).expect("identity is part of the closed transform set");
                        json!({
                            "structure_id":structure_id,
                            "owner_part_id":"rear-stock",
                            "policy":super::production_weapon_form_art_evidence::STRICT_OWNER_VOID_POLICY,
                            "expected_region_canonical_sha256":diagnostic.expected_region_canonical_sha256,
                            "expected_void_pixel_count":diagnostic.expected_void_pixel_count,
                            "expected_boundary_pixel_count":diagnostic.expected_boundary_pixel_count,
                            "expected_void_bbox_px":identity.expected_void_bbox_px,
                            "owner_bbox_px":identity.owner_bbox_px,
                            "owner_minus_expected_void_bbox_edge_delta_px":identity.owner_minus_expected_void_bbox_edge_delta_px,
                            "owner_minus_expected_void_centroid_delta_milli_px":identity.owner_minus_expected_void_centroid_delta_milli_px,
                            "owner_region_pixel_count":identity.owner_region_pixel_count,
                            "owner_boundary_adjacency_pixel_count":identity.owner_boundary_adjacency_pixel_count,
                            "owner_boundary_adjacency_milli":identity.owner_boundary_adjacency_milli,
                            "owner_expected_void_overlap_pixel_count":identity.owner_expected_void_overlap_pixel_count,
                            "owner_expected_void_overlap_milli":identity.owner_expected_void_overlap_milli,
                            "identity_passes_thresholds":identity.passes_thresholds,
                            "ranked_transform":super::production_weapon_form_art_evidence::reviewed_region_part_binding_transform_name(diagnostic.ranked_transform),
                            "ranked_transform_unique":diagnostic.ranked_transform_unique,
                            "eligible_transform_count":diagnostic.eligible_transform_count,
                            "diagnostic_error":error.to_string(),
                            "registered_camera_lineage_verified":true,
                            "strict_owner_void_passed":false,
                            "blocker_code":"STRICT_REGISTERED_CAMERA_OWNER_VOID_NOT_ELIGIBLE",
                            "status":"BLOCKED_PROPOSAL_OWNER_VOID_BINDING",
                            "quality_status":"NOT_PROVEN",
                            "depth_status":"UNKNOWN"
                        })
                    }
                    (Err(error), Err(_)) => return Err(error),
                }
            }
            _ => Value::Null,
        };
        let camera_object_sha256 = render_set
            .get("camera_object_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| {
                invalid(format!(
                    "proposal owner evidence {kind} camera object is missing"
                ))
            })?;
        let camera_readback = read_bound_json(runtime, camera_object_sha256, "proposal camera")?;
        if camera_readback.get("camera_hash").and_then(Value::as_str) != Some(camera_hash)
            || camera_readback
                .get("canonical_sha256")
                .and_then(Value::as_str)
                != Some(camera_canonical_sha256)
        {
            return Err(invalid(format!(
                "proposal owner evidence {kind} camera object binding differs"
            )));
        }
        let mut row = json!({
            "view_kind":kind,
            "view_id":view.view_id,
            "reference_id":view.reference_id,
            "reference_sha256":view.reference_sha256,
            "view_spec_canonical_sha256":view.view_spec.get("canonical_sha256"),
            "crop":crop,
            "rotation_degrees":rotation_degrees,
            "camera_hash":camera_hash,
            "camera_canonical_sha256":camera_canonical_sha256,
            "camera_source":camera_source,
            "camera_object_sha256":camera_object_sha256,
            "render_set_object_sha256":render_set_object_sha256,
            "render_set_canonical_sha256":render_set_canonical_sha256,
            "silhouette_pass_object_sha256":silhouette_pass_sha256,
            "part_id_pass_object_sha256":part_id_pass_sha256,
            "depth_pass_object_sha256":depth_pass_sha256,
            "normal_pass_object_sha256":normal_pass_sha256,
            "target_object_sha256":art_view.target_object_sha256,
            "target_canonical_sha256":art_view.target_canonical_sha256,
            "visual_structure_canonical_sha256":art_view.visual_structure_canonical_sha256,
            "visual_structure_review_status":"user_confirmed",
            "source_form_evidence_view_receipt_object_sha256":source_form_evidence_view_receipt_object_sha256,
            "source_form_evidence_view_receipt_canonical_sha256":source_form_evidence_view_receipt_canonical_sha256,
            "source_fresh_baseline_view_receipt_object_sha256":source_baseline_view_receipt_object_sha256,
            "proposal_candidate_id":proposal_candidate_id,
            "proposal_candidate_state_sha256":proposal_candidate_state_sha256,
            "proposal_artifact_sha256":proposal_artifact_sha256,
            "owner_evidence":owner_evidence
        });
        let row_object = row
            .as_object_mut()
            .ok_or_else(|| invalid("proposal FormArt view row is invalid"))?;
        row_object.extend(
            observation
                .as_object()
                .ok_or_else(|| invalid("proposal FormArt observation is invalid"))?
                .clone(),
        );
        rows.push(row);
    }
    let part_id_all_views_observed = rows
        .iter()
        .all(|row| row.get("part_id_status").and_then(Value::as_str) == Some("observed"));
    let negative_space_all_views_resolved =
        rows.iter().all(
            |row| match row.get("negative_space_status").and_then(Value::as_str) {
                Some("not-applicable") => row
                    .get("negative_space_rows")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty),
                Some("observed") => row
                    .get("negative_space_rows")
                    .and_then(Value::as_array)
                    .is_some_and(|evidence| {
                        !evidence.is_empty()
                            && evidence.iter().all(|item| {
                                item.get("status").and_then(Value::as_str) == Some("observed")
                                    && item
                                        .get("iou_milli")
                                        .and_then(Value::as_u64)
                                        .is_some_and(|value| value >= 850)
                                    && item
                                        .get("boundary_f1_milli")
                                        .and_then(Value::as_u64)
                                        .is_some_and(|value| value >= 800)
                                    && item
                                        .get("area_ratio_milli")
                                        .and_then(Value::as_u64)
                                        .is_some_and(|value| (850..=1150).contains(&value))
                                    && item
                                        .get("centroid_error_milli")
                                        .and_then(Value::as_u64)
                                        .is_some_and(|value| value <= 3000)
                                    && item.get("sealed").and_then(Value::as_bool) == Some(false)
                                    && item.get("missing").and_then(Value::as_bool) == Some(false)
                            })
                    }),
                _ => false,
            },
        );
    let line_flow_all_views_resolved =
        rows.iter().all(
            |row| match row.get("line_flow_status").and_then(Value::as_str) {
                Some("not-applicable") => row
                    .get("line_flow_rows")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty),
                Some("observed") => row
                    .get("line_flow_rows")
                    .and_then(Value::as_array)
                    .is_some_and(|evidence| {
                        !evidence.is_empty()
                            && evidence.iter().all(|item| {
                                item.get("status").and_then(Value::as_str) == Some("observed")
                                    && item
                                        .get("coverage_milli")
                                        .and_then(Value::as_u64)
                                        .is_some_and(|value| value >= 900)
                                    && item
                                        .get("continuity_milli")
                                        .and_then(Value::as_u64)
                                        .is_some_and(|value| value >= 900)
                                    && item
                                        .get("symmetric_chamfer_milli")
                                        .and_then(Value::as_u64)
                                        .is_some_and(|value| value <= 3000)
                                    && item
                                        .get("max_deviation_milli")
                                        .and_then(Value::as_u64)
                                        .is_some_and(|value| value <= 5000)
                                    && item
                                        .get("direction_order_milli")
                                        .and_then(Value::as_u64)
                                        .is_some_and(|value| value >= 950)
                                    && item.get("duplicate_crossing_count").and_then(Value::as_u64)
                                        == Some(0)
                            })
                    }),
                _ => false,
            },
        );
    let proposal_form_art_evidence_ready = owner_views_passed
        && part_id_all_views_observed
        && negative_space_all_views_resolved
        && line_flow_all_views_resolved;
    let policy_definition = json!({
        "policy":PROPOSAL_FORM_ART_EVIDENCE_POLICY,
        "owner_part_id":"rear-stock",
        "required_view_kinds":REQUIRED_VIEW_KINDS,
        "owner_view_kinds":["left","right","rear-three-quarter"],
        "registered_camera_transform":"identity",
        "fixed_raster_size_px":FIXED_RASTER_SIZE_PX,
        "min_expected_void_pixels":256,
        "min_expected_boundary_pixels":64,
        "min_owner_region_pixels":128,
        "min_boundary_adjacency_pixels":32,
        "min_boundary_adjacency_milli":250,
        "max_owner_expected_void_overlap_pixels":0,
        "max_owner_expected_void_overlap_milli":0,
        "negative_space_thresholds":{
            "iou_milli_min":850,
            "boundary_f1_milli_min":800,
            "area_ratio_milli_min":850,
            "area_ratio_milli_max":1150,
            "centroid_error_milli_max":3000
        },
        "line_flow_thresholds":{
            "coverage_milli_min":900,
            "continuity_milli_min":900,
            "symmetric_chamfer_milli_max":3000,
            "max_deviation_milli_max":5000,
            "direction_order_milli_min":950,
            "duplicate_crossing_count_max":0
        }
    });
    let (
        camera_lock_id,
        camera_lock_canonical_sha256,
        camera_lock_receipt_object_sha256,
        camera_lock_source_transition_id,
        camera_lock_source_transition_sha256,
        camera_lock_source_head_canonical_sha256,
        camera_rig_object_sha256,
        camera_rig_canonical_sha256,
    ) = if let Some(lineage) = scope.fresh_registration_lineage.as_ref() {
        (
            lineage.camera_lock_id.as_str(),
            lineage.camera_lock_canonical_sha256.as_str(),
            lineage.camera_lock_receipt_object_sha256.as_str(),
            lineage.source_transition_id.as_str(),
            lineage.source_transition_sha256.as_str(),
            lineage.source_head_canonical_sha256.as_str(),
            lineage.registered_rig_v2_object_sha256.as_str(),
            lineage.registered_rig_v2_canonical_sha256.as_str(),
        )
    } else {
        (
            scope.art.camera_lock_id.as_str(),
            scope.art.camera_lock_canonical_sha256.as_str(),
            scope.art.camera_lock_receipt_object_sha256.as_str(),
            scope.art.camera_lock_source_transition_id.as_str(),
            scope.art.camera_lock_source_transition_sha256.as_str(),
            scope.art.camera_lock_source_head_canonical_sha256.as_str(),
            scope.art.camera_rig_object_sha256.as_str(),
            scope.art.camera_rig_canonical_sha256.as_str(),
        )
    };
    let mut receipt = json!({
        "schema_version":PROPOSAL_FORM_ART_EVIDENCE_SCHEMA_VERSION,
        "policy":PROPOSAL_FORM_ART_EVIDENCE_POLICY,
        "policy_sha256":canonical_json_hash(&policy_definition),
        "policy_definition":policy_definition,
        "project_id":scope.project_id,
        "session_id":scope.session_id,
        "source_candidate_id":scope.source_candidate_id,
        "source_candidate_state_sha256":scope.source_candidate_state_sha256,
        "source_artifact_sha256":scope.source_artifact_sha256,
        "source_artifact_readback_sha256":source_artifact_readback_sha256,
        "source_form_art_evidence_id":scope.source_form_art_evidence_id,
        "source_form_art_evidence_object_sha256":scope.source_form_art_evidence_object_sha256,
        "source_form_art_evidence_canonical_sha256":scope.source_form_art_evidence_canonical_sha256,
        "source_form_art_role":"reviewed-semantic-targets-only",
        "source_camera_evidence_kind":if scope.fresh_baseline.is_some() {"fresh-same-cohort-baseline-rig-v2"} else {"historical-form-art-camera"},
        "source_fresh_baseline_id":scope.fresh_baseline.as_ref().map(|baseline| baseline.baseline_id.clone()),
        "source_fresh_baseline_canonical_sha256":scope.fresh_baseline.as_ref().map(|baseline| baseline.canonical_sha256.clone()),
        "source_fresh_baseline_receipt_object_sha256":scope.fresh_baseline.as_ref().map(|baseline| baseline.receipt_object_sha256.clone()),
        "source_registration_lineage_id":scope.fresh_registration_lineage.as_ref().map(|lineage| lineage.registration_lineage_id.clone()),
        "source_registration_lineage_canonical_sha256":scope.fresh_registration_lineage.as_ref().map(|lineage| lineage.canonical_sha256.clone()),
        "source_registered_rig_v2_id":scope.fresh_baseline.as_ref().map(|baseline| baseline.registered_rig_v2_id.clone()),
        "proposal_candidate_id":proposal_candidate_id,
        "proposal_candidate_state_sha256":proposal_candidate_state_sha256,
        "proposal_artifact_sha256":proposal_artifact_sha256,
        "proposal_artifact_readback_sha256":proposal_artifact_readback_sha256,
        "worker_build_cohort_sha256":worker_build_cohort_sha256,
        "cross_view_evidence_bundle_sha256":bundle_sha256,
        "proposal_part_id_vocabulary_sha256":canonical_json_hash(&json!(proposal_part_ids)),
        "owner_part_id":"rear-stock",
        "reference_canvas_object_sha256":scope.art.reference_canvas_object_sha256,
        "reference_canvas_canonical_sha256":scope.art.reference_canvas_canonical_sha256,
        "design_spec_object_sha256":scope.art.design_spec_object_sha256,
        "design_spec_canonical_sha256":scope.art.design_spec_canonical_sha256,
        "camera_lock_id":camera_lock_id,
        "camera_lock_canonical_sha256":camera_lock_canonical_sha256,
        "camera_rig_object_sha256":camera_rig_object_sha256,
        "camera_rig_canonical_sha256":camera_rig_canonical_sha256,
        "camera_lock_receipt_object_sha256":camera_lock_receipt_object_sha256,
        "camera_lock_source_transition_id":camera_lock_source_transition_id,
        "camera_lock_source_transition_sha256":camera_lock_source_transition_sha256,
        "camera_lock_source_head_canonical_sha256":camera_lock_source_head_canonical_sha256,
        "views":rows,
        "part_id_all_views_observed":part_id_all_views_observed,
        "negative_space_all_views_resolved":negative_space_all_views_resolved,
        "line_flow_all_views_resolved":line_flow_all_views_resolved,
        "strict_owner_void_all_views_passed":owner_views_passed,
        "proposal_form_art_evidence_ready":proposal_form_art_evidence_ready,
        "status":if proposal_form_art_evidence_ready {"READY_PROPOSAL_FORM_ART_EVIDENCE"} else {"BLOCKED_PROPOSAL_FORM_ART_EVIDENCE"},
        "candidate_confirm_allowed":false,
        "secondary_form_approved":"NOT_CREATED",
        "production_stage_advanced":false,
        "quality_status":"QUALITY_TARGET_NOT_MET",
        "canonical_sha256":""
    });
    receipt["canonical_sha256"] = Value::String(canonical_json_hash(&receipt));
    let bytes = canonical_json_bytes(&receipt).map_err(|error| invalid(error.to_string()))?;
    let reservation = runtime.store.begin_cas_reservation();
    let object = runtime.store.put_object_reserved(
        &reservation,
        &bytes,
        None,
        "application/json",
        "production-weapon-form-art-proposal-evidence",
        &super::now_string(),
    )?;
    let operation = (|| -> Result<Value, RuntimeError> {
        if runtime.cas_read_bounded(&object.record.sha256, MAX_JSON_BYTES)? != bytes {
            return Err(invalid("proposal owner evidence CAS readback differs"));
        }
        let (durable_record, replayed) = runtime
            .store
            .record_production_weapon_form_art_proposal_evidence_with_replay(&object.record)?;
        let durable_readback = runtime
            .store
            .get_production_weapon_form_art_proposal_evidence(&object.record.sha256)?
            .ok_or_else(|| invalid("proposal FormArt durable evidence disappeared after write"))?;
        if durable_record != durable_readback
            || durable_record.receipt_object_sha256 != object.record.sha256
            || durable_record.canonical_sha256
                != receipt["canonical_sha256"].as_str().unwrap_or_default()
            || durable_record.proposal_candidate_id != proposal_candidate_id
            || durable_record.proposal_candidate_state_sha256 != proposal_candidate_state_sha256
            || durable_record.proposal_artifact_sha256 != proposal_artifact_sha256
            || durable_record.cross_view_evidence_bundle_sha256 != bundle_sha256
        {
            return Err(invalid(
                "proposal FormArt durable evidence Store readback differs",
            ));
        }
        Ok(json!({
            "receipt_object_sha256":object.record.sha256,
            "receipt_canonical_sha256":receipt["canonical_sha256"],
            "cas_readback_verified":true,
            "store_indexed":true,
            "store_readback_verified":true,
            "replayed":replayed,
            "durable_identity_sha256":durable_record.identity_sha256,
            "receipt":receipt
        }))
    })();
    match operation {
        Ok(value) => {
            runtime
                .store
                .release_cas_reservation_object(&reservation, &object, false)?;
            Ok(value)
        }
        Err(operation_error) => {
            match runtime
                .store
                .release_cas_reservation_object(&reservation, &object, true)
            {
                Ok(_) => Err(operation_error),
                Err(cleanup_error) => Err(invalid(format!(
                    "proposal FormArt evidence failed ({operation_error}); CAS rollback failed ({cleanup_error})"
                ))),
            }
        }
    }
}

/// Secondary-form selection is an art-direction review, not the final visual
/// quality gate. The legacy strict comparator remains untouched and is emitted
/// as a diagnostic. This policy quantizes direction-normalized deltas to parts
/// per million, records every negative delta as a real tradeoff, requires the
/// semantic region/landmark group not to regress, bounds each core tradeoff,
/// and requires both aggregate and material core improvement. It deliberately
/// does not call a bounded negative delta a tie or an exact pixel change.
///
/// The proposal also needs fresh proposal-side owner/void/Part-ID evidence
/// before it can become approval-eligible. The evidence is rendered from the
/// proposal artifact in the source FormArt registered review frame and is
/// bound here independently; source-candidate pixels are never promoted.
fn assess_secondary_form_gate(
    runtime: &Runtime,
    bundle_sha256: &str,
    scope: &FormArtEvaluationScope<'_>,
    proposal_candidate_id: &str,
    proposal_candidate_state_sha256: &str,
    proposal_artifact_sha256: &str,
    proposal_form_art_evidence: &Value,
) -> Result<Value, RuntimeError> {
    let bundle = read_bound_json(runtime, bundle_sha256, "CrossViewEvidenceBundle")?;
    super::validate_cross_view_evidence_bundle(&bundle)?;
    if bundle.get("schema_version").and_then(Value::as_str) != Some("CrossViewEvidenceBundle@1")
        || bundle.get("project_id").and_then(Value::as_str) != Some(scope.project_id)
        || bundle.get("session_id").and_then(Value::as_str) != Some(scope.session_id)
        || bundle.get("candidate_id").and_then(Value::as_str) != Some(proposal_candidate_id)
    {
        return Err(invalid("CrossViewEvidenceBundle proposal binding differs"));
    }
    let evidence_receipt_object_sha256 = proposal_form_art_evidence
        .get("receipt_object_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("proposal FormArt evidence receipt object hash is unavailable"))?;
    let evidence_receipt = proposal_form_art_evidence
        .get("receipt")
        .ok_or_else(|| invalid("proposal FormArt evidence receipt is unavailable"))?;
    let evidence_receipt_readback = read_bound_json(
        runtime,
        evidence_receipt_object_sha256,
        "proposal FormArt evidence receipt",
    )?;
    if &evidence_receipt_readback != evidence_receipt
        || evidence_receipt
            .get("schema_version")
            .and_then(Value::as_str)
            != Some(PROPOSAL_FORM_ART_EVIDENCE_SCHEMA_VERSION)
        || evidence_receipt.get("project_id").and_then(Value::as_str) != Some(scope.project_id)
        || evidence_receipt.get("session_id").and_then(Value::as_str) != Some(scope.session_id)
        || evidence_receipt
            .get("proposal_candidate_id")
            .and_then(Value::as_str)
            != Some(proposal_candidate_id)
        || evidence_receipt
            .get("proposal_candidate_state_sha256")
            .and_then(Value::as_str)
            != Some(proposal_candidate_state_sha256)
        || evidence_receipt
            .get("proposal_artifact_sha256")
            .and_then(Value::as_str)
            != Some(proposal_artifact_sha256)
        || evidence_receipt
            .get("cross_view_evidence_bundle_sha256")
            .and_then(Value::as_str)
            != Some(bundle_sha256)
    {
        return Err(invalid("proposal owner evidence receipt binding differs"));
    }
    let evidence_receipt_canonical_sha256 = evidence_receipt
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("proposal owner evidence canonical hash is unavailable"))?;
    let mut evidence_receipt_preimage = evidence_receipt.clone();
    evidence_receipt_preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&evidence_receipt_preimage) != evidence_receipt_canonical_sha256 {
        return Err(invalid("proposal FormArt evidence canonical hash differs"));
    }
    let evidence_views = evidence_receipt
        .get("views")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("proposal FormArt evidence views are unavailable"))?;
    let evidence_view_kinds = evidence_views
        .iter()
        .filter_map(|view| view.get("view_kind").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let evidence_view_order = evidence_views
        .iter()
        .filter_map(|view| view.get("view_kind").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let owner_view_kinds = BTreeSet::from(["left", "right", "rear-three-quarter"]);
    let proposal_form_art_evidence_ready = evidence_views.len() == REQUIRED_VIEW_KINDS.len()
        && evidence_view_kinds == REQUIRED_VIEW_KINDS.into_iter().collect::<BTreeSet<_>>()
        && evidence_view_order == REQUIRED_VIEW_KINDS
        && evidence_views.iter().all(|view| {
            view.get("part_id_status").and_then(Value::as_str) == Some("observed")
                && matches!(
                    view.get("negative_space_status").and_then(Value::as_str),
                    Some("observed") | Some("not-applicable")
                )
                && matches!(
                    view.get("line_flow_status").and_then(Value::as_str),
                    Some("observed") | Some("not-applicable")
                )
        })
        && evidence_views
            .iter()
            .filter(|view| {
                view.get("view_kind")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| owner_view_kinds.contains(kind))
            })
            .all(|view| {
                let owner = view.get("owner_evidence").unwrap_or(&Value::Null);
                owner.get("owner_part_id").and_then(Value::as_str) == Some("rear-stock")
                    && owner.get("registered_camera_lineage_verified") == Some(&Value::Bool(true))
                    && owner.get("strict_owner_void_passed") == Some(&Value::Bool(true))
            })
        && evidence_receipt.get("strict_owner_void_all_views_passed") == Some(&Value::Bool(true))
        && evidence_receipt.get("proposal_form_art_evidence_ready") == Some(&Value::Bool(true))
        && evidence_receipt.get("status").and_then(Value::as_str)
            == Some("READY_PROPOSAL_FORM_ART_EVIDENCE");
    let baseline_score = bundle
        .get("baseline_score")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid("CrossViewEvidenceBundle baseline score is invalid"))?;
    let proposal_score = bundle
        .get("proposal_score")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid("CrossViewEvidenceBundle proposal score is invalid"))?;
    let aggregate_delta = proposal_score - baseline_score;
    let aggregate_improvement_ppm = (aggregate_delta * METRIC_PPM_SCALE).round() as i64;
    let aggregate_improved = aggregate_improvement_ppm >= MIN_AGGREGATE_IMPROVEMENT_PPM;
    let evaluations = bundle
        .get("view_evaluations")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("CrossViewEvidenceBundle view evaluations are unavailable"))?;
    if evaluations.len() != REQUIRED_VIEW_KINDS.len() {
        return Err(invalid("CrossViewEvidenceBundle view count differs"));
    }

    let metrics = [
        ("silhouette_iou", true, true),
        ("boundary_f1_4px", true, true),
        ("bbox_edge_error", false, true),
        ("centroid_error", false, true),
        ("landmark_coverage", true, false),
        ("landmark_nme", false, false),
        ("region_median_iou", true, false),
        ("critical_region_min_iou", true, false),
    ];
    let mut semantic_non_regressing = true;
    let mut bounded_core_tradeoff = true;
    let mut strict_primary_improvement = false;
    let mut core_improvement_winners = Vec::new();
    let mut seen_views = BTreeSet::new();
    let mut per_view = Vec::with_capacity(evaluations.len());
    for evaluation in evaluations {
        let kind = evaluation
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("CrossViewEvidenceBundle view kind is unavailable"))?;
        if !REQUIRED_VIEW_KINDS.contains(&kind) || !seen_views.insert(kind.to_owned()) {
            return Err(invalid("CrossViewEvidenceBundle view identity differs"));
        }
        let baseline = evaluation
            .get("baseline_metrics")
            .ok_or_else(|| invalid(format!("{kind} baseline metrics are unavailable")))?;
        let proposal = evaluation
            .get("proposal_metrics")
            .ok_or_else(|| invalid(format!("{kind} proposal metrics are unavailable")))?;
        let mut deltas = Map::new();
        let mut view_semantic_non_regressing = true;
        let mut view_core_tradeoff_bounded = true;
        for (metric, higher_is_better, raster_sensitive) in metrics {
            let before = baseline
                .get(metric)
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite())
                .ok_or_else(|| invalid(format!("{kind} baseline {metric} is invalid")))?;
            let after = proposal
                .get(metric)
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite())
                .ok_or_else(|| invalid(format!("{kind} proposal {metric} is invalid")))?;
            let improvement_delta = if higher_is_better {
                after - before
            } else {
                before - after
            };
            let improvement_ppm = (improvement_delta * METRIC_PPM_SCALE).round() as i64;
            let classification = if improvement_ppm > 0 {
                "IMPROVED"
            } else if improvement_ppm < 0 {
                "REGRESSED"
            } else {
                "UNCHANGED_AT_PPM_PRECISION"
            };
            if raster_sensitive {
                let bounded = improvement_ppm >= -MAX_CORE_TRADEOFF_PPM;
                view_core_tradeoff_bounded &= bounded;
                bounded_core_tradeoff &= bounded;
            } else {
                let non_regressing = improvement_ppm >= 0;
                view_semantic_non_regressing &= non_regressing;
                semantic_non_regressing &= non_regressing;
            }
            if raster_sensitive && improvement_ppm >= MIN_CORE_IMPROVEMENT_PPM {
                strict_primary_improvement = true;
                core_improvement_winners.push(format!("{kind}:{metric}"));
            }
            deltas.insert(
                metric.to_owned(),
                json!({
                    "baseline":before,
                    "proposal":after,
                    "improvement_delta":improvement_delta,
                    "improvement_ppm":improvement_ppm,
                    "classification":classification,
                    "higher_is_better":higher_is_better,
                    "group":if raster_sensitive { "core" } else { "semantic" },
                }),
            );
        }
        per_view.push(json!({
            "kind":kind,
            "view_id":evaluation.get("view_id"),
            "camera_hash":evaluation.get("camera_hash"),
            "baseline_comparison_report_sha256":evaluation.get("baseline_comparison_report_sha256"),
            "proposal_comparison_report_sha256":evaluation.get("proposal_comparison_report_sha256"),
            "semantic_non_regressing":view_semantic_non_regressing,
            "core_tradeoff_bounded":view_core_tradeoff_bounded,
            "metric_deltas":deltas,
        }));
    }
    if seen_views.len() != REQUIRED_VIEW_KINDS.len() {
        return Err(invalid("CrossViewEvidenceBundle six-view coverage differs"));
    }
    let policy_definition = json!({
        "policy":SECONDARY_FORM_GATE_POLICY,
        "metric_policy":SECONDARY_FORM_METRIC_POLICY,
        "fixed_raster_size_px":FIXED_RASTER_SIZE_PX,
        "metric_delta_quantization":"direction-normalized-parts-per-million@1",
        "metric_ppm_scale":METRIC_PPM_SCALE,
        "core_metrics":[
            "silhouette_iou",
            "boundary_f1_4px",
            "bbox_edge_error",
            "centroid_error"
        ],
        "semantic_metrics":[
            "landmark_coverage",
            "landmark_nme",
            "region_median_iou",
            "critical_region_min_iou"
        ],
        "max_core_tradeoff_ppm":MAX_CORE_TRADEOFF_PPM,
        "min_core_improvement_ppm":MIN_CORE_IMPROVEMENT_PPM,
        "min_aggregate_improvement_ppm":MIN_AGGREGATE_IMPROVEMENT_PPM,
        "requirements":[
            "all_six_views_present_once",
            "all_negative_deltas_remain_explicit_regressions",
            "each_core_metric_tradeoff_is_bounded_in_ppm",
            "semantic_metrics_do_not_regress_at_ppm_precision",
            "legacy_eight_metric_aggregate_strictly_improves",
            "at_least_one_core_metric_materially_improves",
            "fresh_proposal_side_six_view_formart_part_id_owner_void_negative_space_line_flow_evidence_required_for_eligibility",
            "user_confirmation_still_required"
        ]
    });
    let reviewable_tradeoff = semantic_non_regressing
        && bounded_core_tradeoff
        && aggregate_improved
        && strict_primary_improvement;
    let eligibility_ready = reviewable_tradeoff && proposal_form_art_evidence_ready;
    let status = if eligibility_ready {
        "ELIGIBLE_AWAITING_USER_CONFIRMATION"
    } else if reviewable_tradeoff {
        "BLOCKED_PROPOSAL_FORM_ART_EVIDENCE"
    } else if !aggregate_improved {
        "NOT_IMPROVED"
    } else {
        "REJECTED_REGRESSION"
    };
    Ok(json!({
        "policy":SECONDARY_FORM_GATE_POLICY,
        "policy_sha256":canonical_json_hash(&policy_definition),
        "policy_definition":policy_definition,
        "bundle_sha256":bundle_sha256,
        "baseline_score":baseline_score,
        "proposal_score":proposal_score,
        "aggregate_improvement_delta":aggregate_delta,
        "aggregate_improvement_ppm":aggregate_improvement_ppm,
        "aggregate_improved":aggregate_improved,
        "semantic_non_regressing":semantic_non_regressing,
        "bounded_core_tradeoff":bounded_core_tradeoff,
        "strict_primary_improvement":strict_primary_improvement,
        "core_improvement_winners":core_improvement_winners,
        "reviewable_tradeoff":reviewable_tradeoff,
        "proposal_form_art_evidence_ready":proposal_form_art_evidence_ready,
        "proposal_form_art_evidence_object_sha256":evidence_receipt_object_sha256,
        "proposal_form_art_evidence_canonical_sha256":evidence_receipt_canonical_sha256,
        "blocking_reasons":if proposal_form_art_evidence_ready {
            Vec::<String>::new()
        } else {
            vec!["PROPOSAL_SIDE_SIX_VIEW_FORMART_EVIDENCE_NOT_READY".to_owned()]
        },
        "eligibility_ready":eligibility_ready,
        "status":status,
        "per_view":per_view,
        "secondary_form_eligibility":status,
        "secondary_form_approved":"NOT_CREATED",
        "promotable":false,
        "candidate_confirm_allowed":false,
        "stage_advance_allowed":false,
        "quality_status":"QUALITY_TARGET_NOT_MET",
    }))
}

pub(crate) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let context = load_context(runtime, request)?;
    if !context.source_form_art_current_cohort_compatible {
        return Err(invalid(
            "BASELINE_FORM_ART_COHORT_REFRESH_REQUIRED_BEFORE_PREPARE",
        ));
    }
    let operation = context.typed_edit.operation();
    let edit_policy = context.typed_edit.edit_policy();
    // Keep the revision in memory until every downstream stage has either
    // passed or returned the normal reviewable/rejected result.  In
    // particular, a Worker/candidate/FormArt error must not leave a reachable
    // child that cannot be explained by a prepare response.
    let child_revision = materialize_child_revision(&context)?;
    let derived = materialize_geometry_program(runtime, &context, &child_revision)?;

    let base_version_id = runtime
        .store
        .latest_version_for_project(&context.project_id)?
        .map(|value| value.version_id);
    let candidate_idempotency_key = format!(
        "{}-{}",
        context.typed_edit.candidate_prepare_idempotency_prefix(),
        &canonical_json_hash(&json!({
            "request_input_sha256":context.request_input_sha256,
            "operation_lineage_sha256":context.operation_lineage_sha256,
            "derived_program_sha256":derived.program_sha256,
        }))[..48]
    );
    let mut geometry_request = json!({
        "typed": "geometry",
        "geometry_program": derived.program,
    });
    if let Some(reference_id) = source_reference_id(runtime, &context.candidate_id)? {
        geometry_request["reference_id"] = Value::String(reference_id);
    }
    let prepared = runtime.prepare_geometry_candidate_exact(
        &context.project_id,
        base_version_id.as_deref(),
        &candidate_idempotency_key,
        geometry_request,
    )?;
    let new_candidate = prepared
        .get("candidate")
        .cloned()
        .ok_or_else(|| invalid("derived candidate is missing"))?;
    let new_candidate_id = new_candidate
        .get("candidate_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| invalid("derived candidate ID is invalid"))?
        .to_owned();
    let artifact = prepared
        .get("artifact")
        .ok_or_else(|| invalid("derived candidate ArtifactReadback is missing"))?;
    if artifact.get("program_sha256").and_then(Value::as_str)
        != Some(derived.program_sha256.as_str())
        || artifact.get("validator_status").and_then(Value::as_str) != Some("passed")
    {
        return Err(invalid(
            "derived candidate ArtifactReadback binding differs",
        ));
    }
    let new_evidence = runtime
        .store
        .get_geometry_candidate_evidence(&new_candidate_id)?
        .ok_or_else(|| invalid("derived candidate geometry evidence is unavailable"))?;
    if new_evidence.project_id != context.project_id
        || new_evidence.geometry_program_sha256 != derived.program_sha256
        || new_evidence.artifact_object_sha256
            != artifact
                .get("artifact_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
    {
        return Err(invalid(
            "derived candidate GeometryCandidateEvidence binding differs",
        ));
    }
    let session = runtime
        .store
        .get_agentic_session(&context.art.session_id)?
        .ok_or_else(|| invalid("FormArt DesignSession is unavailable"))?;
    let source_candidate = runtime
        .candidate(&context.candidate_id)?
        .ok_or_else(|| invalid("source candidate is unavailable after prepare"))?;
    let proposal_candidate = runtime
        .candidate(&new_candidate_id)?
        .ok_or_else(|| invalid("derived candidate is unavailable after prepare"))?;
    let view_evaluations = materialize_six_view_evaluations(runtime, &context, &session)?;
    let cross_view = super::agentic_action::evaluate_rear_stock_profile_six_view_gate(
        runtime,
        &session,
        &source_candidate,
        &proposal_candidate,
        &view_evaluations,
    )?;
    let proposal_candidate_state_sha256 = new_candidate
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("derived candidate state hash is missing"))?;
    let form_art_scope = FormArtEvaluationScope::from_proposal_context(&context);
    let proposal_form_art_evidence = materialize_proposal_form_art_evidence(
        runtime,
        &form_art_scope,
        &new_candidate_id,
        proposal_candidate_state_sha256,
        &derived.source_artifact_readback_sha256,
        &new_evidence.artifact_object_sha256,
        artifact
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("derived artifact readback hash is missing"))?,
        derived.worker_build_cohort_sha256.as_deref(),
        &derived.part_ids,
        &cross_view.bundle_sha256,
        &view_evaluations,
    )?;
    let secondary_form_gate = assess_secondary_form_gate(
        runtime,
        &cross_view.bundle_sha256,
        &form_art_scope,
        &new_candidate_id,
        proposal_candidate_state_sha256,
        &new_evidence.artifact_object_sha256,
        &proposal_form_art_evidence,
    )?;
    let secondary_form_gate_eligible = secondary_form_gate
        .get("eligibility_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let secondary_form_reviewable_tradeoff = secondary_form_gate
        .get("reviewable_tradeoff")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let secondary_form_gate_status = secondary_form_gate
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("REJECTED_REGRESSION")
        .to_owned();
    let (durable_child, child_revision) = persist_durable_child(runtime, &context, child_revision)?;
    let mut result = get_result(&context)?;
    result["schema_version"] = Value::String(PREPARE_RESULT_SCHEMA_VERSION.to_owned());
    result["source_candidate_id"] = Value::String(context.candidate_id.clone());
    result["source_candidate_state_sha256"] = Value::String(context.candidate_state_sha256.clone());
    result["child_revision"] = json!({
        "schema_version": child_revision.schema_version,
        "revision": child_revision,
        "revision_id": child_revision.revision_id.0,
        "revision_index": child_revision.revision_index,
        "parent_revision_ids": child_revision.parent_revision_ids,
        "revision_sha256": child_revision.canonical_sha256,
        "revision_object_sha256": durable_child.get("revision_object_sha256"),
        "durable_record_sha256": durable_child.get("durable_record").and_then(|value| value.get("canonical_sha256")),
        "operation": operation,
        "operation_id": context.operation_id,
        "operation_lineage_sha256": context.operation_lineage_sha256,
        "materialization_status": match operation {
            "move_vertices" => "RUNTIME_OWNED_DURABLE_MOVE_VERTICES_REVISION@1",
            "open_frame_notch" => "RUNTIME_OWNED_DURABLE_OPEN_FRAME_NOTCH_REVISION@1",
            "rear_stock_void_rail_bow" => "RUNTIME_OWNED_DURABLE_REAR_STOCK_VOID_RAIL_BOW_REVISION@1",
            "rear_stock_void_boundary_bridge" => {
                "RUNTIME_OWNED_DURABLE_REAR_STOCK_VOID_BOUNDARY_BRIDGE_REVISION@1"
            }
            _ => unreachable!("typed edit operation is closed"),
        },
        "durable": true,
    });
    if let Some(child) = result
        .get_mut("child_revision")
        .and_then(Value::as_object_mut)
    {
        child.insert(
            format!("{operation}_policy"),
            Value::String(edit_policy.to_owned()),
        );
    }
    result["derived_candidate_id"] = Value::String(new_candidate_id.clone());
    result["derived_candidate_state_sha256"] = new_candidate
        .get("canonical_sha256")
        .cloned()
        .ok_or_else(|| invalid("derived candidate state hash is missing"))?;
    result["derived_geometry"] = json!({
        "operator_id":"forgecad.geometry.authoring-mesh@1",
        "source_node_id":context.source_node_id,
        "part_id":context.part_id,
        "source_program_sha256":derived.source_program_sha256,
        "source_program_object_sha256":derived.source_program_object_sha256,
        "derived_program_sha256":derived.program_sha256,
        "derived_program_object_sha256":new_evidence.geometry_program_object_sha256,
        "source_artifact_sha256":derived.source_artifact_sha256,
        "source_artifact_readback_sha256":derived.source_artifact_readback_sha256,
        "derived_artifact_sha256":new_evidence.artifact_object_sha256,
        "derived_artifact_readback_sha256":artifact.get("canonical_sha256"),
        "derived_geometry_candidate_evidence_sha256":new_evidence.canonical_sha256,
        "worker_build_cohort_sha256":derived.worker_build_cohort_sha256,
        "triangle_count":derived.triangle_count,
        "replacement_status":"SINGLE_SOURCE_NODE_REPLACED_PRESERVED_PART_OUTPUTS_AND_OTHER_NODES",
        "runtime_worker_status":"HASHED_COMPILED_AND_STRICT_READBACK_PASSED",
    });
    result["six_view_evaluation"] = json!({
        "bundle_sha256": cross_view.bundle_sha256,
        "aggregate_status": cross_view.aggregate_status,
        "hard_gate_passed": cross_view.hard_gate_passed,
        "non_regressing": cross_view.non_regressing,
        "strict_improvement": cross_view.strict_improvement,
        "baseline_score": cross_view.baseline_score,
        "proposal_score": cross_view.proposal_score,
        "view_count": view_evaluations.len(),
        "view_order": REQUIRED_VIEW_KINDS,
        "legacy_strict_pareto_status": if cross_view.non_regressing {
            "PASSED"
        } else {
            "REJECTED_ANY_METRIC_REGRESSION"
        },
        "secondary_form_gate": secondary_form_gate,
        "promotion_status": if secondary_form_gate_eligible {
            "ELIGIBLE_AWAITING_USER_CONFIRMATION"
        } else if secondary_form_reviewable_tradeoff {
            "BLOCKED_PROPOSAL_FORM_ART_EVIDENCE"
        } else {
            "REJECTED_REGRESSION"
        }
    });
    result["proposal_form_art_evidence"] = proposal_form_art_evidence;
    if let Some(requirement) = result
        .get_mut("six_view_requirement")
        .and_then(Value::as_object_mut)
    {
        requirement.insert(
            "evaluation_status".to_owned(),
            Value::String("EVALUATED".to_owned()),
        );
        requirement.insert(
            "gate_status".to_owned(),
            Value::String(secondary_form_gate_status),
        );
        requirement.insert(
            "bundle_sha256".to_owned(),
            Value::String(cross_view.bundle_sha256.clone()),
        );
    }
    result["proposal_status"] = Value::String(if secondary_form_gate_eligible {
        "PREPARED_CHILD_REVISION_AND_FORMART_SECONDARY_GATE_AWAITING_USER_CONFIRMATION".to_owned()
    } else if secondary_form_reviewable_tradeoff {
        "PREPARED_CHILD_REVISION_REVIEWABLE_TRADEOFF_BLOCKED_PROPOSAL_FORM_ART_EVIDENCE".to_owned()
    } else {
        "PREPARED_CHILD_REVISION_REJECTED_BY_SIX_VIEW_REGRESSION".to_owned()
    });
    result["quality_status"] = Value::String("QUALITY_TARGET_NOT_MET".to_owned());
    result["runtime_write_performed"] = Value::Bool(true);
    result["persistent_user_data_touched"] = Value::Bool(true);
    result["prepare_lifecycle"] = json!({
        "status":"COMMITTED",
        "durable_child_commit":"AFTER_WORKER_CANDIDATE_FORMART_AND_CROSS_VIEW_STAGES",
        "durable_child_committed":true,
        "recovery_policy":"RETRY_SAME_REQUEST_IDEMPOTENCY_REPLAYS_CANDIDATE_AND_FORMART_EVIDENCE",
        "failed_before_child_commit":"NO_REACHABLE_AUTHORING_MESH_CHILD_FROM_THIS_PREPARE"
    });
    result["limitations"] = json!([
        "REAL_D1_DURABLE_AUTHORING_MESH_PARENT_AND_CHILD_BOUND",
        "SINGLE_SOURCE_NODE_AND_PART_BOUND",
        "AUTHORING_MESH_V2_CHILD_MATERIALIZED",
        "AUTHORING_MESH_V1_SOURCE_NODE_REPLACED_IN_DERIVED_PROGRAM",
        "DERIVED_CANDIDATE_PREPARED_BY_RUNTIME_WORKER",
        "SIX_VIEW_FORMART_EVALUATED",
        "LEGACY_STRICT_PARETO_RETAINED_AS_DIAGNOSTIC",
        "FORMART_SECONDARY_PARETO_REVIEW_EVALUATED",
        "PROPOSAL_SIDE_FORMART_OWNER_VOID_PART_ID_EVIDENCE_CREATED_AND_CAS_READBACK_VERIFIED",
        "DURABLE_CHILD_COMMITTED_AFTER_DOWNSTREAM_FAILURE_POINTS",
        "NO_SECONDARY_FORM_APPROVAL",
        "NO_STAGE_ADVANCEMENT",
        "NO_CANDIDATE_CONFIRM",
        "NO_VERSION_CREATED",
        "NO_EXPORT",
        "NO_VISUAL_QUALITY_CLAIM"
    ]);
    result["canonical_sha256"] = Value::String(String::new());
    if canonical_json_bytes(&result)
        .map_err(|error| invalid(error.to_string()))?
        .len()
        > MAX_RESPONSE_BYTES
    {
        return Err(invalid(
            "prepared proposal result exceeds max_response_bytes",
        ));
    }
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    Ok(result)
}

fn source_reference_id(
    runtime: &Runtime,
    candidate_id: &str,
) -> Result<Option<String>, RuntimeError> {
    Ok(runtime
        .store
        .get_geometry_candidate_evidence(candidate_id)?
        .and_then(|value| value.reference_id))
}

fn read_bound_json(
    runtime: &Runtime,
    object_sha256: &str,
    label: &str,
) -> Result<Value, RuntimeError> {
    if !is_sha256(object_sha256) {
        return Err(invalid(format!("{label} object hash is invalid")));
    }
    let bytes = runtime.cas_read_bounded(object_sha256, MAX_JSON_BYTES)?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{label} is invalid JSON: {error}")))?;
    if !value.is_object() {
        return Err(invalid(format!("{label} is not an object")));
    }
    Ok(value)
}

fn validate_canonical_json_object(value: &Value, label: &str) -> Result<String, RuntimeError> {
    let canonical_sha256 = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid(format!("{label} canonical hash is unavailable")))?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != canonical_sha256 {
        return Err(invalid(format!("{label} canonical hash differs")));
    }
    Ok(canonical_sha256.to_owned())
}

fn validate_composite_fresh_baseline(
    runtime: &Runtime,
    plan: &super::production_weapon_form_art_composite_proposal::CompositeProposalPlan,
    baseline_id: &str,
    baseline: &ProductionWeaponFormArtBaselineRecord,
    session: &forgecad_store::AgenticSessionRecord,
    source_geometry: &super::agentic_action::GeometryBindings,
) -> Result<(), RuntimeError> {
    if baseline.baseline_id != baseline_id
        || baseline.project_id != plan.project_id
        || baseline.session_id != session.session_id
        || baseline.candidate_id != plan.original_source_candidate_id
        || baseline.candidate_state_sha256 != plan.original_source_candidate_state_sha256
        || baseline.artifact_sha256 != plan.original_source_artifact_sha256
        || baseline.artifact_sha256 != source_geometry.artifact_sha256
        || baseline.view_kinds
            != REQUIRED_VIEW_KINDS
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
        || baseline.views.len() != REQUIRED_VIEW_KINDS.len()
        || !baseline.worker_started
        || !baseline.worker_cohort_verified
        || baseline.historical_form_art_reused
        || baseline.promotion_eligible
        || baseline.runtime_write_performed && !baseline.persistent_user_data_touched
    {
        return Err(invalid(
            "composite original fresh baseline binding or non-promotion boundary differs",
        ));
    }
    // The proposal plan preserves the baseline that existed when the plan was
    // authored. A later evidence pass must re-render the same registered
    // lineage in the current Runtime/Worker cohort, so its durable baseline
    // canonical hash is expected to differ. Stable session, candidate, state,
    // artifact and registration-lineage bindings above/below are the authority;
    // the historical canonical remains provenance, not a current-cohort lock.
    if !is_sha256(&baseline.runtime_build_cohort_sha256) {
        return Err(invalid("composite fresh baseline worker cohort is invalid"));
    }
    for (index, view) in baseline.views.iter().enumerate() {
        let kind = REQUIRED_VIEW_KINDS[index];
        if view.view_kind != kind
            || view.render_set_view_id != view.view_id
            || view.render_worker_build_cohort_sha256 != baseline.runtime_build_cohort_sha256
            || view.pass_artifact_object_sha256.len() != COMPOSITE_BASELINE_AOV_KINDS.len()
        {
            return Err(invalid(format!(
                "composite fresh baseline {kind} view or AOV inventory differs"
            )));
        }
        for hash in &view.pass_artifact_object_sha256 {
            if !is_sha256(hash) {
                return Err(invalid(format!(
                    "composite fresh baseline {kind} AOV hash is invalid"
                )));
            }
            runtime.cas_read_bounded(hash, 16 * 1024 * 1024)?;
        }
        let render_set = read_bound_json(
            runtime,
            &view.render_set_object_sha256,
            "composite fresh baseline RenderSet",
        )?;
        super::validate_persisted_render_set_v2_output(&render_set)?;
        if validate_canonical_json_object(&render_set, "composite fresh baseline RenderSet")?
            != view.render_set_canonical_sha256
            || render_set.get("view_id").and_then(Value::as_str) != Some(view.view_id.as_str())
            || render_set.get("candidate_id").and_then(Value::as_str)
                != Some(plan.original_source_candidate_id.as_str())
            || render_set.get("artifact_sha256").and_then(Value::as_str)
                != Some(plan.original_source_artifact_sha256.as_str())
            || render_set.get("program_sha256").and_then(Value::as_str)
                != Some(source_geometry.evidence.geometry_program_sha256.as_str())
            || render_set.get("reference_id").and_then(Value::as_str)
                != Some(view.reference_id.as_str())
            || render_set.get("camera_hash").and_then(Value::as_str)
                != Some(view.camera_hash.as_str())
            || render_set
                .get("render_worker_build_cohort_sha256")
                .and_then(Value::as_str)
                != Some(baseline.runtime_build_cohort_sha256.as_str())
        {
            return Err(invalid(format!(
                "composite fresh baseline {kind} RenderSet binding differs"
            )));
        }
        let passes = render_set
            .get("pass_artifacts")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                invalid(format!(
                    "composite fresh baseline {kind} AOV map is missing"
                ))
            })?;
        for (aov_index, aov_kind) in COMPOSITE_BASELINE_AOV_KINDS.iter().enumerate() {
            let pass_sha256 = passes
                .get(*aov_kind)
                .and_then(|value| value.get("sha256"))
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| {
                    invalid(format!(
                        "composite fresh baseline {kind}/{aov_kind} AOV is missing"
                    ))
                })?;
            if view.pass_artifact_object_sha256[aov_index] != pass_sha256 {
                return Err(invalid(format!(
                    "composite fresh baseline {kind}/{aov_kind} AOV hash differs"
                )));
            }
        }
        let comparison = read_bound_json(
            runtime,
            &view.comparison_report_object_sha256,
            "composite fresh baseline comparison report",
        )?;
        validate_canonical_json_object(&comparison, "composite fresh baseline comparison report")?;
        if comparison.get("candidate_id").and_then(Value::as_str)
            != Some(plan.original_source_candidate_id.as_str())
            || comparison.get("artifact_sha256").and_then(Value::as_str)
                != Some(plan.original_source_artifact_sha256.as_str())
            || comparison.get("reference_id").and_then(Value::as_str)
                != Some(view.reference_id.as_str())
            || comparison.get("reference_sha256").and_then(Value::as_str)
                != Some(view.reference_sha256.as_str())
            || comparison.get("render_set_hash").and_then(Value::as_str)
                != Some(view.render_set_object_sha256.as_str())
            || comparison.get("camera_hash").and_then(Value::as_str)
                != Some(view.camera_hash.as_str())
        {
            return Err(invalid(format!(
                "composite fresh baseline {kind} comparison binding differs"
            )));
        }
        let metrics = comparison
            .get("metrics")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                invalid(format!(
                    "composite fresh baseline {kind} metrics are missing"
                ))
            })?;
        for metric in [
            "silhouette_iou",
            "boundary_f1_4px",
            "bbox_edge_error",
            "centroid_error",
            "landmark_coverage",
            "landmark_nme",
            "region_median_iou",
            "critical_region_min_iou",
        ] {
            if !metrics
                .get(metric)
                .and_then(Value::as_f64)
                .is_some_and(f64::is_finite)
            {
                return Err(invalid(format!(
                    "composite fresh baseline {kind} metric {metric} is invalid"
                )));
            }
        }
        let quality = read_bound_json(
            runtime,
            &view.quality_report_object_sha256,
            "composite fresh baseline quality report",
        )?;
        validate_canonical_json_object(&quality, "composite fresh baseline quality report")?;
        if quality.get("candidate_id").and_then(Value::as_str)
            != Some(plan.original_source_candidate_id.as_str())
            || quality.get("artifact_sha256").and_then(Value::as_str)
                != Some(plan.original_source_artifact_sha256.as_str())
            || quality.get("program_sha256").and_then(Value::as_str)
                != Some(source_geometry.evidence.geometry_program_sha256.as_str())
            || quality.get("reference_id").and_then(Value::as_str)
                != Some(view.reference_id.as_str())
            || quality.get("reference_sha256").and_then(Value::as_str)
                != Some(view.reference_sha256.as_str())
            || quality.get("render_set_hash").and_then(Value::as_str)
                != Some(view.render_set_object_sha256.as_str())
            || quality
                .get("comparison_report_hash")
                .and_then(Value::as_str)
                != Some(view.comparison_report_object_sha256.as_str())
        {
            return Err(invalid(format!(
                "composite fresh baseline {kind} quality binding differs"
            )));
        }
    }
    Ok(())
}

fn load_composite_registration_lineage(
    runtime: &Runtime,
    baseline: &ProductionWeaponFormArtBaselineRecord,
) -> Result<ProductionCameraLockRegistrationLineageRecord, RuntimeError> {
    let lineage = runtime
        .store
        .get_production_camera_lock_registration_lineage(&baseline.registration_lineage_id)?
        .ok_or_else(|| invalid("composite fresh baseline registration lineage is unavailable"))?;
    super::agentic_session::validate_production_camera_lock_registration_lineage_runtime(
        runtime, &lineage,
    )?;
    if lineage.registration_lineage_id != baseline.registration_lineage_id
        || lineage.canonical_sha256 != baseline.registration_lineage_canonical_sha256
        || lineage.receipt_object_sha256 != baseline.registration_lineage_receipt_object_sha256
        || lineage.registered_rig_v2_object_sha256 != baseline.registered_rig_v2_object_sha256
        || lineage.registered_rig_v2_canonical_sha256 != baseline.registered_rig_v2_canonical_sha256
        || lineage.session_id != baseline.session_id
        || lineage.project_id != baseline.project_id
        || lineage.candidate_id != baseline.candidate_id
        || lineage.candidate_state_sha256 != baseline.candidate_state_sha256
        || lineage.artifact_id != baseline.artifact_id
        || lineage.artifact_sha256 != baseline.artifact_sha256
        || !lineage.promotable
    {
        return Err(invalid(
            "composite fresh baseline registration lineage or RigV2 binding differs",
        ));
    }
    Ok(lineage)
}

fn composite_visual_metric(metrics: &Value, key: &str) -> Result<f64, RuntimeError> {
    metrics
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid(format!("composite CrossView metric {key} is invalid")))
}

fn compare_composite_visual_metrics(
    baseline: &Value,
    proposal: &Value,
) -> Result<(bool, bool, f64, f64), RuntimeError> {
    let metrics = [
        ("silhouette_iou", true),
        ("boundary_f1_4px", true),
        ("bbox_edge_error", false),
        ("centroid_error", false),
        ("landmark_coverage", true),
        ("landmark_nme", false),
        ("region_median_iou", true),
        ("critical_region_min_iou", true),
    ];
    let mut non_regressing = true;
    let mut strict = false;
    let mut baseline_score = 0.0;
    let mut proposal_score = 0.0;
    for (key, higher_is_better) in metrics {
        let before = composite_visual_metric(baseline, key)?;
        let after = composite_visual_metric(proposal, key)?;
        baseline_score += if higher_is_better {
            before
        } else {
            (1.0 - before).max(0.0)
        };
        proposal_score += if higher_is_better {
            after
        } else {
            (1.0 - after).max(0.0)
        };
        let delta = if higher_is_better {
            after - before
        } else {
            before - after
        };
        if delta < -1.0e-9 {
            non_regressing = false;
        }
        if delta > 1.0e-9 {
            strict = true;
        }
    }
    Ok((
        non_regressing,
        non_regressing && strict,
        baseline_score,
        proposal_score,
    ))
}

fn cross_view_status(value: &Value, label: &str) -> Result<String, RuntimeError> {
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{label} status is unavailable")))?;
    if !matches!(
        status,
        "PARTIAL_VISIBLE_VIEW_PASS" | "QUALITY_TARGET_NOT_MET" | "BLOCKED_REFERENCE_COVERAGE"
    ) {
        return Err(invalid(format!("{label} status is unsupported")));
    }
    Ok(status.to_owned())
}

fn render_set_pass_hashes(render_set: &Value, label: &str) -> Result<Vec<String>, RuntimeError> {
    let passes = render_set
        .get("pass_artifacts")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid(format!("{label} pass_artifacts are unavailable")))?;
    COMPOSITE_BASELINE_AOV_KINDS
        .iter()
        .map(|kind| {
            passes
                .get(*kind)
                .and_then(|value| value.get("sha256"))
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .map(str::to_owned)
                .ok_or_else(|| invalid(format!("{label}/{kind} AOV hash is unavailable")))
        })
        .collect()
}

fn evaluate_composite_cross_view_candidate_only(
    runtime: &Runtime,
    session: &forgecad_store::AgenticSessionRecord,
    scope: &FormArtEvaluationScope<'_>,
    baseline: &ProductionWeaponFormArtBaselineRecord,
    proposal_candidate: &CandidateRecord,
    proposal_geometry: &super::agentic_action::GeometryBindings,
    view_evaluations: &[super::agentic_action::ViewEvaluation],
) -> Result<(super::agentic_action::CrossViewEvaluationResult, Value), RuntimeError> {
    if session.session_id != scope.session_id
        || session.project_id != scope.project_id
        || session.candidate_id != scope.source_candidate_id
        || session.candidate_state_sha256 != scope.source_candidate_state_sha256
        || proposal_candidate.project_id != scope.project_id
    {
        return Err(invalid(
            "composite CrossView session or candidate binding differs",
        ));
    }
    if view_evaluations.len() != REQUIRED_VIEW_KINDS.len()
        || view_evaluations
            .iter()
            .map(|view| view.kind.as_str())
            .collect::<BTreeSet<_>>()
            != REQUIRED_VIEW_KINDS.iter().copied().collect::<BTreeSet<_>>()
    {
        return Err(invalid("composite CrossView six-view coverage differs"));
    }
    let canvas = read_bound_json(
        runtime,
        &scope.art.reference_canvas_object_sha256,
        "composite ReferenceCanvas",
    )?;
    if validate_canonical_json_object(&canvas, "composite ReferenceCanvas")?
        != scope.art.reference_canvas_canonical_sha256
    {
        return Err(invalid(
            "composite ReferenceCanvas canonical binding differs",
        ));
    }
    let canvas_sha256 = scope.art.reference_canvas_object_sha256.clone();
    let coverage = canvas.get("coverage").cloned().unwrap_or_else(|| json!({}));
    let coverage_complete = coverage.get("coverage_status").and_then(Value::as_str)
        == Some("complete")
        && coverage
            .get("missing_views")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty);

    if let Some(existing) = runtime.store.get_cross_view_evidence_by_identity(
        scope.project_id,
        scope.session_id,
        &proposal_candidate.candidate_id,
        &canvas_sha256,
    )? {
        let bundle = read_bound_json(
            runtime,
            &existing.bundle_object_sha256,
            "composite CrossViewEvidenceBundle replay",
        )?;
        super::validate_cross_view_evidence_bundle(&bundle)?;
        if bundle.get("project_id").and_then(Value::as_str) != Some(scope.project_id)
            || bundle.get("session_id").and_then(Value::as_str) != Some(scope.session_id)
            || bundle.get("candidate_id").and_then(Value::as_str)
                != Some(proposal_candidate.candidate_id.as_str())
            || bundle.get("candidate_state_sha256").and_then(Value::as_str)
                != Some(proposal_candidate.canonical_sha256.as_str())
            || bundle.get("artifact_sha256").and_then(Value::as_str)
                != Some(proposal_geometry.artifact_sha256.as_str())
            || bundle.get("program_sha256").and_then(Value::as_str)
                != Some(proposal_geometry.evidence.geometry_program_sha256.as_str())
            || bundle
                .get("reference_canvas_sha256")
                .and_then(Value::as_str)
                != Some(canvas_sha256.as_str())
        {
            return Err(invalid("composite CrossView replay binding differs"));
        }
        let result = super::agentic_action::CrossViewEvaluationResult {
            bundle_sha256: existing.bundle_object_sha256,
            aggregate_status: existing.aggregate_status,
            hard_gate_passed: existing.hard_gate_passed,
            strict_improvement: bundle
                .get("strict_improvement")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            non_regressing: bundle
                .get("non_regressing")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            baseline_score: bundle
                .get("baseline_score")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            proposal_score: bundle
                .get("proposal_score")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
        };
        return Ok((result, bundle));
    }

    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved_objects = Vec::new();
    let result =
        (|| -> Result<(super::agentic_action::CrossViewEvaluationResult, Value), RuntimeError> {
            let mut per_view = Vec::with_capacity(REQUIRED_VIEW_KINDS.len());
            let mut all_pass = true;
            let mut all_non_regressing = true;
            let mut all_strict_improvement = true;
            let mut baseline_total = 0.0;
            let mut proposal_total = 0.0;
            for kind in REQUIRED_VIEW_KINDS {
                let view = view_evaluations
                    .iter()
                    .find(|view| view.kind == kind)
                    .ok_or_else(|| {
                        invalid(format!("composite CrossView {kind} view is missing"))
                    })?;
                let baseline_view = baseline
                    .views
                    .iter()
                    .find(|candidate| candidate.view_kind == kind)
                    .ok_or_else(|| invalid(format!("composite baseline {kind} view is missing")))?;
                if view.reference_id != baseline_view.reference_id
                    || view.reference_sha256 != baseline_view.reference_sha256
                    || view.camera.get("camera_hash").and_then(Value::as_str)
                        != Some(baseline_view.camera_hash.as_str())
                {
                    return Err(invalid(format!(
                        "composite CrossView {kind} source camera/reference differs"
                    )));
                }
                let baseline_render_set = read_bound_json(
                    runtime,
                    &baseline_view.render_set_object_sha256,
                    "composite baseline RenderSet",
                )?;
                let baseline_comparison = read_bound_json(
                    runtime,
                    &baseline_view.comparison_report_object_sha256,
                    "composite baseline comparison",
                )?;
                let mut proposal_request = json!({
                    "project_id":scope.project_id,
                    "candidate_id":proposal_candidate.candidate_id,
                    "reference_id":view.reference_id,
                    "view_id":view.view_id,
                    "view_spec":view.view_spec,
                    "camera":view.camera
                });
                if let Some(target_sha256) = view.target_sha256.as_deref() {
                    proposal_request["target_sha256"] = Value::String(target_sha256.to_owned());
                }
                let proposal = runtime.prepare_reference_comparison_detached(
                    scope.project_id,
                    proposal_request,
                    &reservation,
                    &mut reserved_objects,
                )?;
                let proposal_render_set = proposal.get("render_set").ok_or_else(|| {
                    invalid(format!("composite proposal {kind} RenderSet is missing"))
                })?;
                super::validate_render_set_v2_output(proposal_render_set)?;
                if proposal_render_set
                    .get("candidate_id")
                    .and_then(Value::as_str)
                    != Some(proposal_candidate.candidate_id.as_str())
                    || proposal_render_set
                        .get("artifact_sha256")
                        .and_then(Value::as_str)
                        != Some(proposal_geometry.artifact_sha256.as_str())
                    || proposal_render_set
                        .get("program_sha256")
                        .and_then(Value::as_str)
                        != Some(proposal_geometry.evidence.geometry_program_sha256.as_str())
                    || proposal_render_set.get("view_id").and_then(Value::as_str)
                        != Some(view.view_id.as_str())
                    || proposal_render_set
                        .get("camera_hash")
                        .and_then(Value::as_str)
                        != Some(baseline_view.camera_hash.as_str())
                    || proposal_render_set
                        .get("render_worker_build_cohort_sha256")
                        .and_then(Value::as_str)
                        != Some(baseline.runtime_build_cohort_sha256.as_str())
                {
                    return Err(invalid(format!(
                        "composite proposal {kind} RenderSet binding differs"
                    )));
                }
                let proposal_render_set_sha256 = proposal
                    .get("render_set_object_sha256")
                    .and_then(Value::as_str)
                    .filter(|value| is_sha256(value))
                    .ok_or_else(|| {
                        invalid(format!(
                            "composite proposal {kind} RenderSet hash is missing"
                        ))
                    })?;
                let proposal_comparison = proposal.get("comparison_report").ok_or_else(|| {
                    invalid(format!("composite proposal {kind} comparison is missing"))
                })?;
                let proposal_quality = proposal.get("quality_report").ok_or_else(|| {
                    invalid(format!("composite proposal {kind} quality is missing"))
                })?;
                let proposal_comparison_sha256 = proposal
                    .get("comparison_report_object_sha256")
                    .and_then(Value::as_str)
                    .filter(|value| is_sha256(value))
                    .ok_or_else(|| {
                        invalid(format!(
                            "composite proposal {kind} comparison hash is missing"
                        ))
                    })?;
                let proposal_quality_sha256 = proposal
                    .get("quality_report_object_sha256")
                    .and_then(Value::as_str)
                    .filter(|value| is_sha256(value))
                    .ok_or_else(|| {
                        invalid(format!("composite proposal {kind} quality hash is missing"))
                    })?;
                if proposal_comparison
                    .get("render_set_hash")
                    .and_then(Value::as_str)
                    != Some(proposal_render_set_sha256)
                    || proposal_quality
                        .get("render_set_hash")
                        .and_then(Value::as_str)
                        != Some(proposal_render_set_sha256)
                    || proposal_quality
                        .get("comparison_report_hash")
                        .and_then(Value::as_str)
                        != Some(proposal_comparison_sha256)
                {
                    return Err(invalid(format!(
                        "composite proposal {kind} comparison/quality binding differs"
                    )));
                }
                let (non_regressing, strict_improvement, baseline_score, proposal_score) =
                    compare_composite_visual_metrics(
                        baseline_comparison.get("metrics").ok_or_else(|| {
                            invalid(format!("composite baseline {kind} metrics are missing"))
                        })?,
                        proposal_comparison.get("metrics").ok_or_else(|| {
                            invalid(format!("composite proposal {kind} metrics are missing"))
                        })?,
                    )?;
                let baseline_status =
                    cross_view_status(&baseline_comparison, "composite baseline comparison")?;
                let proposal_status =
                    cross_view_status(proposal_comparison, "composite proposal comparison")?;
                all_pass &= proposal_status == "PARTIAL_VISIBLE_VIEW_PASS";
                all_non_regressing &= non_regressing;
                all_strict_improvement &= strict_improvement;
                baseline_total += baseline_score;
                proposal_total += proposal_score;
                per_view.push(json!({
                "view_id":view.view_id,
                "kind":view.kind,
                "visibility":view.visibility,
                "confidence":view.confidence,
                "reference_id":view.reference_id,
                "reference_sha256":view.reference_sha256,
                "camera_hash":view.camera["camera_hash"],
                "baseline_status":baseline_status,
                "proposal_status":proposal_status,
                "baseline_render_set_sha256":baseline_view.render_set_object_sha256,
                "baseline_comparison_report_sha256":baseline_view.comparison_report_object_sha256,
                "baseline_quality_report_sha256":baseline_view.quality_report_object_sha256,
                "proposal_render_set_sha256":proposal_render_set_sha256,
                "proposal_comparison_report_sha256":proposal_comparison_sha256,
                "proposal_quality_report_sha256":proposal_quality_sha256,
                "baseline_metrics":baseline_comparison["metrics"],
                "proposal_metrics":proposal_comparison["metrics"],
                "non_regressing":non_regressing,
                "strict_improvement":strict_improvement
            }));
            }
            let count = view_evaluations.len() as f64;
            let aggregate_status = if !coverage_complete {
                "BLOCKED_REFERENCE_COVERAGE"
            } else if all_pass {
                "PARTIAL_VISIBLE_VIEW_PASS"
            } else {
                "QUALITY_TARGET_NOT_MET"
            };
            let hard_gate_passed = coverage_complete && all_pass;
            let strict_improvement = all_strict_improvement && all_pass;
            let promotion = if strict_improvement {
                "reviewable"
            } else if all_non_regressing {
                "not-improved"
            } else {
                "rejected-regression"
            };
            let mut bundle = json!({
                "schema_version":"CrossViewEvidenceBundle@1",
                "bundle_id":format!("cross-view-composite-{}", &proposal_candidate.candidate_id[..proposal_candidate.candidate_id.len().min(32)]),
                "project_id":scope.project_id,
                "session_id":scope.session_id,
                "candidate_id":proposal_candidate.candidate_id,
                "candidate_state_sha256":proposal_candidate.canonical_sha256,
                "artifact_sha256":proposal_geometry.artifact_sha256,
                "program_sha256":proposal_geometry.evidence.geometry_program_sha256,
                "reference_canvas_sha256":canvas_sha256,
                "coverage":coverage,
                "view_evaluations":per_view,
                "aggregate_status":aggregate_status,
                "hard_gate_passed":hard_gate_passed,
                "baseline_score":baseline_total / count,
                "proposal_score":proposal_total / count,
                "strict_improvement":strict_improvement,
                "non_regressing":all_non_regressing,
                "promotion":{"status":promotion,"confirm_allowed":false},
                "limitations":[
                    "original_fresh_baseline_render_sets_reused_without_rerender",
                    "human_visual_review_not_run",
                    "export_restart_hash_not_run"
                ],
                "canonical_sha256":""
            });
            bundle = serde_json::from_slice(
                &canonical_json_bytes(&bundle).map_err(|error| invalid(error.to_string()))?,
            )
            .map_err(|error| invalid(error.to_string()))?;
            bundle["canonical_sha256"] = Value::String(canonical_json_hash(&bundle));
            super::validate_cross_view_evidence_bundle(&bundle)?;
            let bundle_bytes =
                canonical_json_bytes(&bundle).map_err(|error| invalid(error.to_string()))?;
            let bundle_object = runtime.store.put_object_reserved(
                &reservation,
                &bundle_bytes,
                None,
                "application/json",
                "cross-view-evidence-bundle",
                &super::now_string(),
            )?;
            reserved_objects.push(bundle_object.clone());
            let timestamp = super::now_string();
            runtime.store.record_cross_view_evidence(
                &forgecad_store::CrossViewEvidenceRecord {
                    bundle_object_sha256: bundle_object.record.sha256.clone(),
                    candidate_id: proposal_candidate.candidate_id.clone(),
                    project_id: scope.project_id.to_owned(),
                    session_id: scope.session_id.to_owned(),
                    reference_canvas_sha256: canvas_sha256.clone(),
                    aggregate_status: aggregate_status.to_owned(),
                    hard_gate_passed,
                    created_at: timestamp.clone(),
                    updated_at: timestamp,
                },
                &bundle_object.record,
            )?;
            Ok((
                super::agentic_action::CrossViewEvaluationResult {
                    bundle_sha256: bundle_object.record.sha256,
                    aggregate_status: aggregate_status.to_owned(),
                    hard_gate_passed,
                    strict_improvement,
                    non_regressing: all_non_regressing,
                    baseline_score: baseline_total / count,
                    proposal_score: proposal_total / count,
                },
                bundle,
            ))
        })();
    let cleanup = result.is_err();
    let mut cleanup_error = None;
    for object in reserved_objects.iter().rev() {
        if let Err(error) = runtime.store.release_cas_reservation_object(
            &reservation,
            object,
            cleanup && object.created_new,
        ) {
            cleanup_error.get_or_insert(error);
        }
    }
    match (result, cleanup_error) {
        (Ok(value), None) => Ok(value),
        (Err(error), None) => Err(error),
        (Ok(_), Some(error)) => Err(RuntimeError::Store(error)),
        (Err(error), Some(cleanup)) => Err(invalid(format!(
            "composite CrossView evaluation failed ({error}); CAS rollback failed ({cleanup})"
        ))),
    }
}

fn materialize_six_view_evaluations(
    runtime: &Runtime,
    context: &ProposalContext,
    session: &forgecad_store::AgenticSessionRecord,
) -> Result<Vec<super::agentic_action::ViewEvaluation>, RuntimeError> {
    if session.session_id != context.art.session_id
        || session.project_id != context.project_id
        || session.candidate_id != context.candidate_id
        || session.candidate_state_sha256 != context.candidate_state_sha256
    {
        return Err(invalid("FormArt DesignSession binding differs"));
    }
    let canvas = read_bound_json(
        runtime,
        &context.art.reference_canvas_object_sha256,
        "FormArt ReferenceCanvas",
    )?;
    if canvas.get("canonical_sha256").and_then(Value::as_str)
        != Some(context.art.reference_canvas_canonical_sha256.as_str())
    {
        return Err(invalid("FormArt ReferenceCanvas canonical hash differs"));
    }
    let authored_views = canvas
        .get("views")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("FormArt ReferenceCanvas views are unavailable"))?;
    if let Some(baseline) = context.fresh_baseline.as_ref() {
        return materialize_six_view_evaluations_from_fresh_baseline(
            runtime,
            &context.project_id,
            &context.candidate_id,
            &context.source_binding.artifact_sha256,
            authored_views,
            baseline,
        );
    }
    let mut entries = Vec::with_capacity(REQUIRED_VIEW_KINDS.len());
    for kind in REQUIRED_VIEW_KINDS {
        let art_view = context
            .art
            .views
            .iter()
            .find(|view| view.view_kind == kind)
            .ok_or_else(|| invalid(format!("FormArt {kind} view is unavailable")))?;
        let authored = authored_views
            .iter()
            .find(|view| {
                view.get("view_id").and_then(Value::as_str) == Some(art_view.view_id.as_str())
                    && view.get("kind").and_then(Value::as_str) == Some(kind)
            })
            .ok_or_else(|| invalid(format!("ReferenceCanvas {kind} view is unavailable")))?;
        let view_spec = authored
            .get("view_spec")
            .cloned()
            .ok_or_else(|| invalid(format!("ReferenceCanvas {kind} view spec is unavailable")))?;
        let receipt = read_bound_json(
            runtime,
            &art_view.form_evidence_view_receipt_object_sha256,
            "FormEvidence view receipt",
        )?;
        if receipt.get("canonical_sha256").and_then(Value::as_str)
            != Some(
                art_view
                    .form_evidence_view_receipt_canonical_sha256
                    .as_str(),
            )
        {
            return Err(invalid(format!("FormEvidence {kind} receipt hash differs")));
        }
        let render_set_object_sha256 = receipt
            .get("render_set_object_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid(format!("FormEvidence {kind} RenderSet is unavailable")))?;
        let render_set =
            read_bound_json(runtime, render_set_object_sha256, "FormEvidence RenderSet")?;
        let camera_object_sha256 = render_set
            .get("camera_object_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid(format!("FormEvidence {kind} camera is unavailable")))?;
        let camera = read_bound_json(runtime, camera_object_sha256, "FormEvidence camera")?;
        if camera.get("camera_hash").and_then(Value::as_str) != Some(art_view.camera_hash.as_str())
            || camera.get("canonical_sha256").and_then(Value::as_str)
                != Some(art_view.camera_canonical_sha256.as_str())
            || render_set.get("camera_hash").and_then(Value::as_str)
                != Some(art_view.camera_hash.as_str())
        {
            return Err(invalid(format!(
                "FormEvidence {kind} camera binding differs"
            )));
        }
        entries.push(json!({
            "view_id": art_view.view_id,
            "reference_id": art_view.reference_id,
            "reference_sha256": art_view.reference_sha256,
            "view_spec": view_spec,
            "camera": camera,
        }));
    }
    let proposal = json!({"view_evaluations": entries});
    let proposal_object = proposal
        .as_object()
        .ok_or_else(|| invalid("six-view proposal envelope is invalid"))?;
    super::agentic_action::validate_view_evaluations(runtime, proposal_object, session)?
        .ok_or_else(|| invalid("six-view evaluations were not materialized"))
}

fn materialize_six_view_evaluations_from_fresh_baseline(
    runtime: &Runtime,
    project_id: &str,
    source_candidate_id: &str,
    source_artifact_sha256: &str,
    authored_views: &[Value],
    baseline: &ProductionWeaponFormArtBaselineRecord,
) -> Result<Vec<super::agentic_action::ViewEvaluation>, RuntimeError> {
    let mut evaluations = Vec::with_capacity(REQUIRED_VIEW_KINDS.len());
    for kind in REQUIRED_VIEW_KINDS {
        let authored = authored_views
            .iter()
            .find(|view| view.get("kind").and_then(Value::as_str) == Some(kind))
            .ok_or_else(|| invalid(format!("ReferenceCanvas {kind} view is unavailable")))?;
        let baseline_view = baseline
            .views
            .iter()
            .find(|view| view.view_kind == kind)
            .ok_or_else(|| invalid(format!("fresh baseline {kind} view is unavailable")))?;
        let view_id = authored
            .get("view_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid(format!("ReferenceCanvas {kind} view_id is invalid")))?;
        let reference_id = authored
            .get("reference_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid(format!("ReferenceCanvas {kind} reference_id is invalid")))?;
        let reference_sha256 = authored
            .get("reference_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| {
                invalid(format!(
                    "ReferenceCanvas {kind} reference_sha256 is invalid"
                ))
            })?;
        if baseline_view.reference_id != reference_id
            || baseline_view.reference_sha256 != reference_sha256
            || baseline_view.render_worker_build_cohort_sha256
                != baseline.runtime_build_cohort_sha256
        {
            return Err(invalid(format!(
                "fresh baseline {kind} reference or cohort binding differs"
            )));
        }
        let reference = runtime
            .reference(reference_id)?
            .ok_or_else(|| invalid(format!("ReferenceCanvas {kind} reference is missing")))?;
        if reference.project_id != project_id || reference.object_sha256 != reference_sha256 {
            return Err(invalid(format!(
                "ReferenceCanvas {kind} reference scope differs"
            )));
        }
        let view_spec = authored
            .get("view_spec")
            .cloned()
            .ok_or_else(|| invalid(format!("ReferenceCanvas {kind} view spec is unavailable")))?;
        if view_spec.get("view_id").and_then(Value::as_str) != Some(view_id) {
            return Err(invalid(format!(
                "ReferenceCanvas {kind} view spec identity differs"
            )));
        }
        super::validate_reference_view_spec(&view_spec, &reference)?;
        let camera = read_bound_json(
            runtime,
            &baseline_view.camera_object_sha256,
            "fresh baseline registered camera",
        )?;
        super::validate_camera_calibration(&camera)?;
        if camera.get("camera_hash").and_then(Value::as_str)
            != Some(baseline_view.camera_hash.as_str())
            || camera.get("canonical_sha256").and_then(Value::as_str)
                != Some(baseline_view.camera_canonical_sha256.as_str())
        {
            return Err(invalid(format!(
                "fresh baseline {kind} camera binding differs"
            )));
        }
        let render_set = read_bound_json(
            runtime,
            &baseline_view.render_set_object_sha256,
            "fresh baseline RenderSet",
        )?;
        if render_set.get("canonical_sha256").and_then(Value::as_str)
            != Some(baseline_view.render_set_canonical_sha256.as_str())
            || render_set.get("camera_hash").and_then(Value::as_str)
                != Some(baseline_view.camera_hash.as_str())
            || render_set
                .get("render_worker_build_cohort_sha256")
                .and_then(Value::as_str)
                != Some(baseline.runtime_build_cohort_sha256.as_str())
            || render_set.get("candidate_id").and_then(Value::as_str) != Some(source_candidate_id)
            || render_set.get("artifact_sha256").and_then(Value::as_str)
                != Some(source_artifact_sha256)
        {
            return Err(invalid(format!(
                "fresh baseline {kind} RenderSet binding differs"
            )));
        }
        let target_sha256 = authored
            .get("target_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let mask_sha256 = authored.get("mask_sha256").and_then(Value::as_str);
        if target_sha256.is_some() != mask_sha256.is_some() {
            return Err(invalid(format!(
                "ReferenceCanvas {kind} target/mask pairing differs"
            )));
        }
        if let Some(target_sha256) = target_sha256.as_deref() {
            let target = runtime.read_silhouette_target(target_sha256)?;
            if target.get("reference_id").and_then(Value::as_str) != Some(reference_id)
                || target.get("reference_sha256").and_then(Value::as_str) != Some(reference_sha256)
                || target.get("mask_sha256").and_then(Value::as_str) != mask_sha256
            {
                return Err(invalid(format!(
                    "ReferenceCanvas {kind} target binding differs"
                )));
            }
        }
        let visibility = authored
            .pointer("/camera_claim/visibility")
            .and_then(Value::as_str)
            .filter(|value| matches!(*value, "observed" | "inferred"))
            .ok_or_else(|| invalid(format!("ReferenceCanvas {kind} visibility is invalid")))?;
        evaluations.push(super::agentic_action::ViewEvaluation {
            view_id: view_id.to_owned(),
            kind: kind.to_owned(),
            visibility: visibility.to_owned(),
            confidence: if visibility == "observed" { 1.0 } else { 0.5 },
            reference_id: reference_id.to_owned(),
            reference_sha256: reference_sha256.to_owned(),
            target_sha256,
            view_spec,
            camera,
        });
    }
    Ok(evaluations)
}

/// Evaluate an already-materialized cumulative composite candidate without
/// creating another candidate or AuthoringMesh child.  The immutable source
/// candidate remains the DesignSession anchor; only the exact proposal
/// candidate is rendered in the six registered baseline cameras.
pub(crate) fn evaluate_existing_composite_candidate(
    runtime: &Runtime,
    parent: &forgecad_store::ProductionWeaponFormArtCompositeProposalStoreRecord,
    original_fresh_baseline_id: &str,
    source_form_art_evidence_id: &str,
    source_form_art_evidence_object_sha256: &str,
    source_form_art_evidence_canonical_sha256: &str,
) -> Result<Value, RuntimeError> {
    let plan_value = read_bound_json(runtime, &parent.plan_object_sha256, "composite plan")?;
    let plan: super::production_weapon_form_art_composite_proposal::CompositeProposalPlan =
        serde_json::from_value(plan_value.clone())
            .map_err(|error| invalid(format!("composite plan is malformed: {error}")))?;
    super::production_weapon_form_art_composite_proposal::validate_composite_proposal_plan(&plan)?;
    if plan.canonical_sha256 != parent.plan_canonical_sha256
        || plan.project_id != parent.project_id
        || plan.current_base_candidate_id != parent.current_base_candidate_id
        || plan.current_base_candidate_state_sha256 != parent.current_base_candidate_state_sha256
        || plan.current_base_artifact_sha256 != parent.current_base_artifact_sha256
    {
        return Err(invalid("composite plan parent binding differs"));
    }

    let session = runtime
        .store
        .get_agentic_session(&parent.session_id)?
        .ok_or_else(|| invalid("composite DesignSession is unavailable"))?;
    if session.project_id != parent.project_id
        || session.candidate_id != plan.original_source_candidate_id
        || session.candidate_state_sha256 != plan.original_source_candidate_state_sha256
    {
        return Err(invalid("composite DesignSession source binding differs"));
    }
    let source_candidate = runtime
        .candidate(&plan.original_source_candidate_id)?
        .ok_or_else(|| invalid("composite source candidate is unavailable"))?;
    let proposal_candidate = runtime
        .candidate(&parent.proposal_candidate_id)?
        .ok_or_else(|| invalid("composite proposal candidate is unavailable"))?;
    if source_candidate.canonical_sha256 != plan.original_source_candidate_state_sha256
        || proposal_candidate.project_id != parent.project_id
        || proposal_candidate.canonical_sha256 != parent.proposal_candidate_state_sha256
    {
        return Err(invalid("composite source/proposal candidate state differs"));
    }
    let source_geometry = super::agentic_action::load_geometry_bindings(
        runtime,
        &source_candidate,
        &parent.project_id,
        &session,
    )?;
    let proposal_geometry = super::agentic_action::load_geometry_bindings(
        runtime,
        &proposal_candidate,
        &parent.project_id,
        &session,
    )?;
    if source_geometry.artifact_sha256 != plan.original_source_artifact_sha256
        || proposal_geometry.artifact_sha256 != parent.proposal_artifact_sha256
        || proposal_geometry.evidence.geometry_program_sha256
            != parent.composed_geometry_program_sha256
        || proposal_geometry.evidence.artifact_readback_object_sha256
            != parent.proposal_artifact_readback_object_sha256
    {
        return Err(invalid("composite geometry evidence binding differs"));
    }
    let proposal_inspection =
        super::agentic_action::recompile_candidate(runtime, &proposal_geometry)?;
    let proposal_readback_object_sha256 = super::agentic_action::verify_artifact_readback(
        runtime,
        &proposal_candidate,
        &proposal_geometry,
        &proposal_inspection,
    )?;
    if proposal_readback_object_sha256 != parent.proposal_artifact_readback_object_sha256 {
        return Err(invalid(
            "composite proposal ArtifactReadback object differs",
        ));
    }
    let source_readback = read_bound_json(
        runtime,
        &source_geometry.evidence.artifact_readback_object_sha256,
        "composite source ArtifactReadback",
    )?;
    let source_artifact_readback_sha256 =
        validate_canonical_json_object(&source_readback, "composite source ArtifactReadback")?;
    let proposal_readback = read_bound_json(
        runtime,
        &proposal_geometry.evidence.artifact_readback_object_sha256,
        "composite proposal ArtifactReadback",
    )?;
    let proposal_artifact_readback_sha256 =
        validate_canonical_json_object(&proposal_readback, "composite proposal ArtifactReadback")?;
    if proposal_artifact_readback_sha256 != parent.proposal_artifact_readback_sha256 {
        return Err(invalid(
            "composite proposal ArtifactReadback canonical hash differs",
        ));
    }
    let proposal_part_ids = proposal_readback
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("composite proposal Part vocabulary is unavailable"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| is_opaque_id(value))
                .map(str::to_owned)
                .ok_or_else(|| invalid("composite proposal Part ID is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let baseline = runtime
        .store
        .get_production_weapon_form_art_baseline_by_id(original_fresh_baseline_id)?
        .ok_or_else(|| invalid("composite fresh baseline is unavailable"))?;
    validate_composite_fresh_baseline(
        runtime,
        &plan,
        original_fresh_baseline_id,
        &baseline,
        &session,
        &source_geometry,
    )?;
    let runtime_cohort = build_cohort_sha256()
        .ok_or_else(|| invalid("composite Runtime build cohort is unavailable"))?;
    if baseline.runtime_build_cohort_sha256 != runtime_cohort {
        return Err(invalid("COMPOSITE_FRESH_BASELINE_CURRENT_COHORT_MISMATCH"));
    }
    let registration_lineage = load_composite_registration_lineage(runtime, &baseline)?;
    let (art, _) = load_form_art(
        runtime,
        source_form_art_evidence_id,
        &parent.project_id,
        &plan.original_source_candidate_id,
        source_form_art_evidence_object_sha256,
        source_form_art_evidence_canonical_sha256,
    )?;
    if art.session_id != parent.session_id
        || art.candidate_state_sha256 != plan.original_source_candidate_state_sha256
        || art.artifact_sha256 != plan.original_source_artifact_sha256
    {
        return Err(invalid("composite source FormArt binding differs"));
    }
    require_six_views(&art, PROPOSAL_FORM_ART_EVIDENCE_POLICY)?;
    let scope = FormArtEvaluationScope {
        project_id: &parent.project_id,
        session_id: &parent.session_id,
        source_candidate_id: &plan.original_source_candidate_id,
        source_candidate_state_sha256: &plan.original_source_candidate_state_sha256,
        source_artifact_sha256: &plan.original_source_artifact_sha256,
        source_artifact_readback_sha256: &source_artifact_readback_sha256,
        source_form_art_evidence_id,
        source_form_art_evidence_object_sha256,
        source_form_art_evidence_canonical_sha256,
        art: &art,
        fresh_baseline: Some(&baseline),
        fresh_registration_lineage: Some(&registration_lineage),
    };
    let canvas = read_bound_json(
        runtime,
        &art.reference_canvas_object_sha256,
        "composite ReferenceCanvas",
    )?;
    let authored_views = canvas
        .get("views")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("composite ReferenceCanvas views are unavailable"))?;
    let view_evaluations = materialize_six_view_evaluations_from_fresh_baseline(
        runtime,
        &parent.project_id,
        &plan.original_source_candidate_id,
        &plan.original_source_artifact_sha256,
        authored_views,
        &baseline,
    )?;
    let (cross_view, cross_view_bundle) = evaluate_composite_cross_view_candidate_only(
        runtime,
        &session,
        &scope,
        &baseline,
        &proposal_candidate,
        &proposal_geometry,
        &view_evaluations,
    )?;
    let proposal_form_art_evidence = materialize_proposal_form_art_evidence(
        runtime,
        &scope,
        &parent.proposal_candidate_id,
        &parent.proposal_candidate_state_sha256,
        &source_artifact_readback_sha256,
        &parent.proposal_artifact_sha256,
        &proposal_artifact_readback_sha256,
        Some(&runtime_cohort),
        &proposal_part_ids,
        &cross_view.bundle_sha256,
        &view_evaluations,
    )?;
    let secondary_form_gate = assess_secondary_form_gate(
        runtime,
        &cross_view.bundle_sha256,
        &scope,
        &parent.proposal_candidate_id,
        &parent.proposal_candidate_state_sha256,
        &parent.proposal_artifact_sha256,
        &proposal_form_art_evidence,
    )?;
    Ok(json!({
        "schema_version":"ProductionWeaponFormArtCompositeCandidateEvaluation@1",
        "project_id":parent.project_id,
        "proposal_id":parent.proposal_id,
        "session_id":parent.session_id,
        "source_candidate_id":plan.original_source_candidate_id,
        "source_candidate_state_sha256":plan.original_source_candidate_state_sha256,
        "source_artifact_sha256":plan.original_source_artifact_sha256,
        "source_artifact_readback_sha256":source_artifact_readback_sha256,
        "original_fresh_baseline_id":baseline.baseline_id,
        "original_fresh_baseline_canonical_sha256":baseline.canonical_sha256,
        "runtime_build_cohort_sha256":runtime_cohort,
        "proposal_candidate_id":parent.proposal_candidate_id,
        "proposal_candidate_state_sha256":parent.proposal_candidate_state_sha256,
        "proposal_artifact_sha256":parent.proposal_artifact_sha256,
        "proposal_artifact_readback_object_sha256":parent.proposal_artifact_readback_object_sha256,
        "proposal_artifact_readback_sha256":proposal_artifact_readback_sha256,
        "cross_view_evidence_bundle_sha256":cross_view.bundle_sha256,
        "cross_view_evidence_bundle":cross_view_bundle,
        "aggregate_status":cross_view.aggregate_status,
        "hard_gate_passed":cross_view.hard_gate_passed,
        "non_regressing":cross_view.non_regressing,
        "strict_improvement":cross_view.strict_improvement,
        "baseline_score":cross_view.baseline_score,
        "proposal_score":cross_view.proposal_score,
        "proposal_form_art_evidence":proposal_form_art_evidence,
        "secondary_form_gate":secondary_form_gate,
        "view_count":view_evaluations.len(),
        "view_order":REQUIRED_VIEW_KINDS,
        "aov_count":view_evaluations.len() * COMPOSITE_BASELINE_AOV_KINDS.len(),
        "candidate_confirm_allowed":false,
        "secondary_form_approved":"NOT_CREATED",
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "quality_status":"QUALITY_TARGET_NOT_MET"
    }))
}
