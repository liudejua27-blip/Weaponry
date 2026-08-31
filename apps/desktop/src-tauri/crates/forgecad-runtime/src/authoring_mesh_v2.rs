//! ForgeCAD-owned `AuthoringMesh@2` kernel primitives.
//!
//! This module is intentionally below the Runtime/MCP transaction wiring.  It
//! owns no SQLite/CAS state and performs no promotion.  A caller gives it a
//! typed original topology, the kernel validates it, and a local topology
//! edit returns a new immutable revision record.  Evaluated geometry is an
//! artifact sidecar and is never used to manufacture authored element IDs.

use super::{canonical_json_hash, is_opaque_id, is_sha256, RuntimeError};
use forgecad_contracts::{
    AuthoringMeshCorner, AuthoringMeshCornerId, AuthoringMeshEdge, AuthoringMeshEdgeId,
    AuthoringMeshElementKind, AuthoringMeshElementRef, AuthoringMeshEvaluated, AuthoringMeshFace,
    AuthoringMeshFaceExtrudeRequest, AuthoringMeshFaceExtrudeResult, AuthoringMeshFaceId,
    AuthoringMeshHalfEdge, AuthoringMeshHalfEdgeId, AuthoringMeshId, AuthoringMeshLineageId,
    AuthoringMeshLoop, AuthoringMeshLoopId, AuthoringMeshMoveVerticesRequest,
    AuthoringMeshMoveVerticesResult, AuthoringMeshOpenFrameNotchRequest,
    AuthoringMeshOpenFrameNotchResult, AuthoringMeshOriginal,
    AuthoringMeshRearStockVoidBoundaryBridgeRequest,
    AuthoringMeshRearStockVoidBoundaryBridgeResult, AuthoringMeshRearStockVoidRailBowRequest,
    AuthoringMeshRearStockVoidRailBowResult, AuthoringMeshRevision, AuthoringMeshRevisionId,
    AuthoringMeshRing, AuthoringMeshRingId, AuthoringMeshSplitEdgeRequest,
    AuthoringMeshSplitEdgeResult, AuthoringMeshTopologyOperation,
    AuthoringMeshTopologyOperationKind, AuthoringMeshV2FoundationSourceBinding,
    AuthoringMeshV2SourceBinding, AuthoringMeshV2Tombstone, AuthoringMeshVertex,
    AuthoringMeshVertexId, AUTHORING_MESH_V2_EVALUATED_NAMESPACE,
    AUTHORING_MESH_V2_FOUNDATION_SOURCE_BINDING_SCHEMA_VERSION, AUTHORING_MESH_V2_ID_POLICY,
    AUTHORING_MESH_V2_OPERATION_SCHEMA_VERSION, AUTHORING_MESH_V2_ORIGINAL_NAMESPACE,
    AUTHORING_MESH_V2_REVISION_SCHEMA_VERSION, AUTHORING_MESH_V2_SCHEMA_VERSION,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_CANONICALIZATION_POLICY,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PROFILE,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_QUALITY_STATUS,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_REVIEW_STATUS,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const MAX_VERTICES: usize = 32_768;
const MAX_EDGES: usize = 65_536;
const MAX_HALF_EDGES: usize = 131_072;
const MAX_CORNERS: usize = 131_072;
const MAX_FACES: usize = 32_768;
const MAX_FACE_DEGREE: usize = 32;
const MAX_TRANSACTION_COMMANDS: usize = 32;
const MAX_ID_LENGTH: usize = 128;
const MAX_COORDINATE_M: f64 = 10.0;
const MIN_EDGE_LENGTH_M: f64 = 1.0e-7;
const MIN_FACE_AREA_M2: f64 = 1.0e-12;
const REAR_STOCK_VOID_RAIL_BOW_STATIONS: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
const REAR_STOCK_VOID_RAIL_BOW_OFFSETS_M: [f64; 5] = [0.0, 0.030, 0.045, 0.030, 0.0];
const REAR_STOCK_VOID_BOUNDARY_BRIDGE_STATIONS: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
const REAR_STOCK_VOID_BOUNDARY_BRIDGE_Y_OFFSETS_M: [f64; 5] = [0.0, -0.012, -0.018, -0.012, 0.0];
// The wedge is fixed in source-local Z and deliberately smaller than the
// source depth half-span.  The sign is applied from the depth mid-plane so
// the two rails remain exactly depth-symmetric.
const REAR_STOCK_VOID_BOUNDARY_BRIDGE_Z_WEDGE_M: [f64; 5] = [0.0, 0.006, 0.009, 0.006, 0.0];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("AUTHORING_MESH_V2_INVALID: {}", message.into()))
}

fn checked_id(value: &str, field: &str) -> Result<(), RuntimeError> {
    if value.len() > MAX_ID_LENGTH || !is_opaque_id(value) {
        return Err(invalid(format!("{field} is not a bounded opaque ID")));
    }
    Ok(())
}

fn checked_sha(value: &str, field: &str) -> Result<(), RuntimeError> {
    if !is_sha256(value) {
        return Err(invalid(format!("{field} is not a SHA-256")));
    }
    Ok(())
}

fn stable_id(prefix: &str, lineage_id: &str, role: &str, key: &Value) -> String {
    let digest = canonical_json_hash(&json!({
        "schema_version": AUTHORING_MESH_V2_SCHEMA_VERSION,
        "lineage_id": lineage_id,
        "role": role,
        "key": key,
    }));
    format!("{prefix}-{}", &digest[..56])
}

fn canonical_hash_without_field<T: serde::Serialize>(value: &T, field: &str) -> String {
    let mut object = serde_json::to_value(value).expect("AuthoringMesh@2 contracts serialize");
    if let Some(map) = object.as_object_mut() {
        map.insert(field.to_owned(), Value::String(String::new()));
    }
    canonical_json_hash(&object)
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn subtract(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn triangle_area(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    let n = cross(subtract(b, a), subtract(c, a));
    0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
}

fn normalized_face_normal(positions: &[[f64; 3]]) -> Result<[f64; 3], RuntimeError> {
    if positions.len() < 3 {
        return Err(invalid("face needs at least three positions"));
    }
    let origin = positions[0];
    let mut normal = [0.0; 3];
    for index in 1..positions.len() - 1 {
        let candidate = cross(
            subtract(positions[index], origin),
            subtract(positions[index + 1], origin),
        );
        let length = (candidate[0] * candidate[0]
            + candidate[1] * candidate[1]
            + candidate[2] * candidate[2])
            .sqrt();
        if length > MIN_FACE_AREA_M2.sqrt() {
            normal = [
                candidate[0] / length,
                candidate[1] / length,
                candidate[2] / length,
            ];
            break;
        }
    }
    let normal_length =
        (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if normal_length <= 0.0 {
        return Err(invalid("face is degenerate"));
    }
    // Inset/extrude is deliberately closed over planar faces.  A generous
    // absolute tolerance keeps ordinary millimetre-scale authoring stable
    // while rejecting a warped polygon that would need a triangulation rule.
    for position in positions.iter().skip(1) {
        let distance_from_plane = subtract(*position, origin);
        let signed_distance = distance_from_plane[0] * normal[0]
            + distance_from_plane[1] * normal[1]
            + distance_from_plane[2] * normal[2];
        if signed_distance.abs() > 1.0e-7 {
            return Err(invalid("face extrude requires a planar source face"));
        }
    }
    let mut sign = 0.0;
    for index in 0..positions.len() {
        let next = (index + 1) % positions.len();
        let after_next = (index + 2) % positions.len();
        let turn = cross(
            subtract(positions[next], positions[index]),
            subtract(positions[after_next], positions[next]),
        );
        let signed_turn = turn[0] * normal[0] + turn[1] * normal[1] + turn[2] * normal[2];
        if signed_turn.abs() <= MIN_FACE_AREA_M2.sqrt() {
            return Err(invalid(
                "face extrude requires a strictly convex source face",
            ));
        }
        if sign == 0.0 {
            sign = signed_turn.signum();
        } else if signed_turn.signum() != sign {
            return Err(invalid("face extrude requires a convex source face"));
        }
    }
    Ok(normal)
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

#[derive(Clone, Debug)]
pub(crate) struct AuthoringMeshV2EvaluatedBinding {
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub readback_sha256: String,
    pub correspondence_status: String,
}

#[derive(Clone, Debug)]
pub(crate) struct AuthoringMeshV2GenesisInput {
    pub mesh_id: AuthoringMeshId,
    pub lineage_id: AuthoringMeshLineageId,
    pub positions_m: Vec<[f64; 3]>,
    pub faces: Vec<Vec<usize>>,
    pub evaluated: Option<AuthoringMeshV2EvaluatedBinding>,
    pub source_binding: Option<AuthoringMeshV2SourceBinding>,
    pub foundation_source_binding: Option<AuthoringMeshV2FoundationSourceBinding>,
}

#[derive(Clone, Debug)]
struct Topology {
    vertices: BTreeMap<AuthoringMeshVertexId, AuthoringMeshVertex>,
    edges: BTreeMap<AuthoringMeshEdgeId, AuthoringMeshEdge>,
    half_edges: BTreeMap<AuthoringMeshHalfEdgeId, AuthoringMeshHalfEdge>,
    corners: BTreeMap<AuthoringMeshCornerId, AuthoringMeshCorner>,
    faces: BTreeMap<AuthoringMeshFaceId, AuthoringMeshFace>,
    loops: BTreeMap<AuthoringMeshLoopId, AuthoringMeshLoop>,
    rings: BTreeMap<AuthoringMeshRingId, AuthoringMeshRing>,
    tombstones: Vec<AuthoringMeshV2Tombstone>,
}

#[derive(Clone, Debug)]
pub(crate) struct AuthoringMeshV2Revision {
    record: AuthoringMeshRevision,
}

/// A stable authored element or one generated by an earlier command in the
/// same in-memory transaction. `output_index` is scoped to `kind`, not to the
/// mixed generated-element array.
#[derive(Clone, Debug)]
pub(crate) enum AuthoringMeshV2TransactionRef {
    Stable(AuthoringMeshElementRef),
    Generated {
        command_index: usize,
        kind: AuthoringMeshElementKind,
        output_index: usize,
    },
}

#[derive(Clone, Debug)]
pub(crate) enum AuthoringMeshV2TransactionCommand {
    SplitEdge {
        operation_id: String,
        edge: AuthoringMeshV2TransactionRef,
        split_ratio_milli: u32,
        operation_lineage_sha256: String,
    },
    MoveVertices {
        operation_id: String,
        vertices: Vec<AuthoringMeshV2TransactionRef>,
        delta_m: Vec<[f64; 3]>,
        operation_lineage_sha256: String,
    },
    FaceExtrude {
        operation_id: String,
        face: AuthoringMeshV2TransactionRef,
        distance_m: f64,
        operation_lineage_sha256: String,
    },
}

impl AuthoringMeshV2TransactionCommand {
    fn operation_id(&self) -> &str {
        match self {
            Self::SplitEdge { operation_id, .. }
            | Self::MoveVertices { operation_id, .. }
            | Self::FaceExtrude { operation_id, .. } => operation_id,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AuthoringMeshV2Transaction {
    pub commands: Vec<AuthoringMeshV2TransactionCommand>,
}

#[derive(Clone, Debug)]
pub(crate) struct AuthoringMeshV2TransactionStep {
    pub command_index: usize,
    pub parent_revision_id: AuthoringMeshRevisionId,
    pub child_revision_id: AuthoringMeshRevisionId,
    pub changed_elements: Vec<AuthoringMeshElementRef>,
    pub generated_elements: Vec<AuthoringMeshElementRef>,
    pub retired_elements: Vec<AuthoringMeshElementRef>,
}

#[derive(Clone, Debug)]
pub(crate) struct AuthoringMeshV2TransactionResult {
    pub parent_revision_id: AuthoringMeshRevisionId,
    pub final_revision: AuthoringMeshRevision,
    /// Every intermediate immutable revision in command order. Persistence
    /// can commit this vector atomically after the pure kernel succeeds.
    pub revision_chain: Vec<AuthoringMeshRevision>,
    pub steps: Vec<AuthoringMeshV2TransactionStep>,
}

impl AuthoringMeshV2Revision {
    /// Build a genesis revision from positions and face vertex indices.  No
    /// caller-supplied element IDs are accepted; all IDs are derived from the
    /// mesh lineage and deterministic source ordinals.
    pub(crate) fn genesis(input: AuthoringMeshV2GenesisInput) -> Result<Self, RuntimeError> {
        checked_id(input.mesh_id.as_ref(), "mesh_id")?;
        checked_id(input.lineage_id.as_ref(), "lineage_id")?;
        if !(3..=MAX_VERTICES).contains(&input.positions_m.len()) {
            return Err(invalid("genesis vertex budget is outside bounds"));
        }
        if !(1..=MAX_FACES).contains(&input.faces.len()) {
            return Err(invalid("genesis face budget is outside bounds"));
        }
        for (index, position) in input.positions_m.iter().copied().enumerate() {
            finite_position(position, &format!("positions_m[{index}]"))?;
        }

        let vertex_ids = input
            .positions_m
            .iter()
            .enumerate()
            .map(|(index, _)| {
                AuthoringMeshVertexId(stable_id(
                    "v",
                    input.lineage_id.as_ref(),
                    "genesis-vertex",
                    &json!({"ordinal":index}),
                ))
            })
            .collect::<Vec<_>>();
        let mut topology = Topology {
            vertices: BTreeMap::new(),
            edges: BTreeMap::new(),
            half_edges: BTreeMap::new(),
            corners: BTreeMap::new(),
            faces: BTreeMap::new(),
            loops: BTreeMap::new(),
            rings: BTreeMap::new(),
            tombstones: Vec::new(),
        };
        for (index, position) in input.positions_m.iter().copied().enumerate() {
            topology.vertices.insert(
                vertex_ids[index].clone(),
                AuthoringMeshVertex {
                    vertex_id: vertex_ids[index].clone(),
                    position_m: position,
                },
            );
        }

        let mut edge_by_endpoints =
            BTreeMap::<(AuthoringMeshVertexId, AuthoringMeshVertexId), AuthoringMeshEdgeId>::new();
        for (face_index, face_indices) in input.faces.iter().enumerate() {
            if !(3..=MAX_FACE_DEGREE).contains(&face_indices.len()) {
                return Err(invalid(format!(
                    "face {face_index} degree is outside bounds"
                )));
            }
            let mut seen = BTreeSet::new();
            for vertex_index in face_indices {
                if *vertex_index >= vertex_ids.len() || !seen.insert(*vertex_index) {
                    return Err(invalid(format!(
                        "face {face_index} has an invalid/repeated vertex"
                    )));
                }
            }
            let face_id = AuthoringMeshFaceId(stable_id(
                "f",
                input.lineage_id.as_ref(),
                "genesis-face",
                &json!({"ordinal":face_index}),
            ));
            let loop_id = AuthoringMeshLoopId(stable_id(
                "loop",
                input.lineage_id.as_ref(),
                "genesis-loop",
                &json!({"face_id":face_id}),
            ));
            let mut half_edge_ids = Vec::with_capacity(face_indices.len());
            for ordinal in 0..face_indices.len() {
                let origin = vertex_ids[face_indices[ordinal]].clone();
                let target = vertex_ids[face_indices[(ordinal + 1) % face_indices.len()]].clone();
                let key = if origin <= target {
                    (origin.clone(), target.clone())
                } else {
                    (target.clone(), origin.clone())
                };
                let edge_id = if let Some(edge_id) = edge_by_endpoints.get(&key) {
                    edge_id.clone()
                } else {
                    let edge_id = AuthoringMeshEdgeId(stable_id(
                        "e",
                        input.lineage_id.as_ref(),
                        "genesis-edge",
                        &json!({"vertex_ids":[key.0,key.1]}),
                    ));
                    edge_by_endpoints.insert(key, edge_id.clone());
                    topology.edges.insert(
                        edge_id.clone(),
                        AuthoringMeshEdge {
                            edge_id: edge_id.clone(),
                            vertex_ids: [origin.clone(), target.clone()],
                            half_edge_ids: Vec::new(),
                            boundary: true,
                        },
                    );
                    edge_id
                };
                let half_edge_id = AuthoringMeshHalfEdgeId(stable_id(
                    "he",
                    input.lineage_id.as_ref(),
                    "genesis-half-edge",
                    &json!({"face_id":face_id,"ordinal":ordinal}),
                ));
                let corner_id = AuthoringMeshCornerId(stable_id(
                    "c",
                    input.lineage_id.as_ref(),
                    "genesis-corner",
                    &json!({"face_id":face_id,"ordinal":ordinal}),
                ));
                half_edge_ids.push(half_edge_id.clone());
                topology.half_edges.insert(
                    half_edge_id.clone(),
                    AuthoringMeshHalfEdge {
                        half_edge_id: half_edge_id.clone(),
                        origin_vertex_id: origin.clone(),
                        edge_id: edge_id.clone(),
                        face_id: face_id.clone(),
                        corner_id: corner_id.clone(),
                        next_id: half_edge_id.clone(),
                        prev_id: half_edge_id.clone(),
                        twin_id: None,
                        boundary: true,
                    },
                );
                topology.corners.insert(
                    corner_id.clone(),
                    AuthoringMeshCorner {
                        corner_id,
                        half_edge_id: half_edge_id.clone(),
                        vertex_id: origin,
                        face_id: face_id.clone(),
                        ordinal: ordinal as u32,
                        uv0: None,
                        normal: None,
                        tangent: None,
                        seam: false,
                    },
                );
                topology
                    .edges
                    .get_mut(&edge_id)
                    .expect("edge inserted")
                    .half_edge_ids
                    .push(half_edge_id);
            }
            for ordinal in 0..half_edge_ids.len() {
                let current = &half_edge_ids[ordinal];
                let next = half_edge_ids[(ordinal + 1) % half_edge_ids.len()].clone();
                let prev = half_edge_ids[(ordinal + half_edge_ids.len() - 1) % half_edge_ids.len()]
                    .clone();
                let half_edge = topology
                    .half_edges
                    .get_mut(current)
                    .expect("half-edge inserted");
                half_edge.next_id = next;
                half_edge.prev_id = prev;
            }
            topology.faces.insert(
                face_id.clone(),
                AuthoringMeshFace {
                    face_id: face_id.clone(),
                    half_edge_ids: half_edge_ids.clone(),
                    loop_id: loop_id.clone(),
                    boundary: true,
                },
            );
            topology.loops.insert(
                loop_id.clone(),
                AuthoringMeshLoop {
                    loop_id,
                    face_id,
                    half_edge_ids,
                    boundary: true,
                },
            );
        }
        rebuild_edge_incidence(&mut topology)?;
        rebuild_twins(&mut topology)?;
        rebuild_boundary_rings(&mut topology, input.lineage_id.as_ref())?;
        validate_topology(&topology)?;
        let original = original_record(&input.lineage_id, topology)?;
        let original_hash = original.canonical_sha256.clone();
        let revision_id = AuthoringMeshRevisionId(stable_id(
            "amrev",
            input.lineage_id.as_ref(),
            "genesis-revision",
            &json!({"mesh_id":input.mesh_id,"original_sha256":original_hash}),
        ));
        let evaluated = evaluated_record(input.evaluated, &revision_id)?;
        if input.source_binding.is_some() && input.foundation_source_binding.is_some() {
            return Err(invalid(
                "candidate source binding and foundation source binding are mutually exclusive",
            ));
        }
        if let Some(binding) = &input.source_binding {
            validate_source_binding(binding)?;
        }
        if let Some(binding) = &input.foundation_source_binding {
            validate_foundation_source_binding(binding)?;
        }
        let record = revision_record(
            input.mesh_id,
            input.lineage_id,
            revision_id,
            Vec::new(),
            0,
            None,
            original,
            evaluated,
            input.source_binding,
            input.foundation_source_binding,
        );
        Ok(Self { record })
    }

    /// Materialize one importer-owned [`FoundationMesh`] as an authored
    /// revision.  The importer only supplies positions/faces; Runtime still
    /// derives all AuthoringMesh element identities and keeps the foundation
    /// provenance in its independent binding namespace.
    pub(crate) fn genesis_from_foundation_mesh(
        mesh: &super::weapon_foundation_import::FoundationMesh,
        lineage_id: AuthoringMeshLineageId,
        foundation_source_binding: AuthoringMeshV2FoundationSourceBinding,
    ) -> Result<Self, RuntimeError> {
        if foundation_source_binding.authoring_mesh_id != mesh.mesh_id
            || foundation_source_binding.part_id != mesh.part_id
            || foundation_source_binding.source_part_topology_sha256
                != mesh.topology.topology_sha256
        {
            return Err(invalid(
                "foundation binding does not match the imported Part geometry",
            ));
        }
        let faces = mesh
            .faces
            .iter()
            .map(|face| face.iter().map(|index| *index as usize).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        Self::genesis(AuthoringMeshV2GenesisInput {
            mesh_id: AuthoringMeshId(mesh.mesh_id.clone()),
            lineage_id,
            positions_m: mesh.positions_m.clone(),
            faces,
            evaluated: None,
            source_binding: None,
            foundation_source_binding: Some(foundation_source_binding),
        })
    }

    pub(crate) fn record(&self) -> &AuthoringMeshRevision {
        &self.record
    }

    /// Apply a bounded command journal against a cloned revision. The method
    /// performs no Store/CAS write. If any command or rehydration check fails,
    /// it returns no revision chain and the receiver remains unchanged.
    pub(crate) fn apply_transaction(
        &self,
        transaction: AuthoringMeshV2Transaction,
    ) -> Result<AuthoringMeshV2TransactionResult, RuntimeError> {
        if transaction.commands.is_empty() {
            return Err(invalid("authoring transaction must not be empty"));
        }
        if transaction.commands.len() > MAX_TRANSACTION_COMMANDS {
            return Err(invalid(format!(
                "authoring transaction command budget exceeded: {} > {MAX_TRANSACTION_COMMANDS}",
                transaction.commands.len()
            )));
        }
        let mut operation_ids = BTreeSet::new();
        for command in &transaction.commands {
            checked_id(command.operation_id(), "transaction.operation_id")?;
            if !operation_ids.insert(command.operation_id().to_owned()) {
                return Err(invalid("authoring transaction repeats operation_id"));
            }
        }

        let parent_revision_id = self.record.revision_id.clone();
        let mut working = Self::from_record(self.record.clone())?;
        let mut revision_chain = Vec::with_capacity(transaction.commands.len());
        let mut steps = Vec::with_capacity(transaction.commands.len());

        for (command_index, command) in transaction.commands.into_iter().enumerate() {
            let current_parent = working.record.revision_id.clone();
            let (child_revision, changed_elements, generated_elements, retired_elements) =
                match command {
                    AuthoringMeshV2TransactionCommand::SplitEdge {
                        operation_id,
                        edge,
                        split_ratio_milli,
                        operation_lineage_sha256,
                    } => {
                        let edge = resolve_transaction_ref(
                            command_index,
                            edge,
                            AuthoringMeshElementKind::Edge,
                            &steps,
                        )?;
                        let result = working.split_edge(AuthoringMeshSplitEdgeRequest {
                            operation_id,
                            parent_revision_id: current_parent.clone(),
                            edge_id: AuthoringMeshEdgeId(edge.id),
                            split_ratio_milli,
                            operation_lineage_sha256,
                        })?;
                        (
                            result.child_revision,
                            result.changed_elements,
                            result.generated_elements,
                            result.retired_elements,
                        )
                    }
                    AuthoringMeshV2TransactionCommand::MoveVertices {
                        operation_id,
                        vertices,
                        delta_m,
                        operation_lineage_sha256,
                    } => {
                        let vertex_ids = vertices
                            .into_iter()
                            .map(|reference| {
                                resolve_transaction_ref(
                                    command_index,
                                    reference,
                                    AuthoringMeshElementKind::Vertex,
                                    &steps,
                                )
                                .map(|element| AuthoringMeshVertexId(element.id))
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let result = working.move_vertices(AuthoringMeshMoveVerticesRequest {
                            operation_id,
                            parent_revision_id: current_parent.clone(),
                            vertex_ids,
                            delta_m,
                            operation_lineage_sha256,
                        })?;
                        (
                            result.child_revision,
                            result.changed_elements,
                            result.generated_elements,
                            result.retired_elements,
                        )
                    }
                    AuthoringMeshV2TransactionCommand::FaceExtrude {
                        operation_id,
                        face,
                        distance_m,
                        operation_lineage_sha256,
                    } => {
                        let face = resolve_transaction_ref(
                            command_index,
                            face,
                            AuthoringMeshElementKind::Face,
                            &steps,
                        )?;
                        let result = working.face_extrude(AuthoringMeshFaceExtrudeRequest {
                            operation_id,
                            parent_revision_id: current_parent.clone(),
                            face_id: AuthoringMeshFaceId(face.id),
                            distance_m,
                            operation_lineage_sha256,
                        })?;
                        (
                            result.child_revision,
                            result.changed_elements,
                            result.generated_elements,
                            result.retired_elements,
                        )
                    }
                };

            let child_revision_id = child_revision.revision_id.clone();
            working = Self::from_record(child_revision.clone())?;
            steps.push(AuthoringMeshV2TransactionStep {
                command_index,
                parent_revision_id: current_parent,
                child_revision_id,
                changed_elements,
                generated_elements,
                retired_elements,
            });
            revision_chain.push(child_revision);
        }

        Ok(AuthoringMeshV2TransactionResult {
            parent_revision_id,
            final_revision: working.record.clone(),
            revision_chain,
            steps,
        })
    }

    /// Rehydrate one immutable revision read from CAS.  Durable readback must
    /// pass through the same topology/DAG checks as genesis and local edits;
    /// deserializing a revision alone is not sufficient because JSON is not
    /// an authority for active/tombstoned identity or half-edge invariants.
    pub(crate) fn from_record(record: AuthoringMeshRevision) -> Result<Self, RuntimeError> {
        if record.schema_version != AUTHORING_MESH_V2_REVISION_SCHEMA_VERSION {
            return Err(invalid("revision schema_version is invalid"));
        }
        checked_id(record.mesh_id.as_ref(), "revision.mesh_id")?;
        checked_id(record.lineage_id.as_ref(), "revision.lineage_id")?;
        checked_id(record.revision_id.as_ref(), "revision.revision_id")?;
        if record.id_policy != AUTHORING_MESH_V2_ID_POLICY {
            return Err(invalid("revision id_policy is invalid"));
        }
        if record.parent_revision_ids.len() > 8 {
            return Err(invalid("revision parent DAG exceeds the bounded fan-in"));
        }
        let mut parents = BTreeSet::new();
        for parent in &record.parent_revision_ids {
            checked_id(parent.as_ref(), "revision.parent_revision_id")?;
            if parent == &record.revision_id || !parents.insert(parent.clone()) {
                return Err(invalid(
                    "revision parent DAG contains a self-edge or duplicate",
                ));
            }
        }
        if record.revision_index == 0 && !record.parent_revision_ids.is_empty() {
            return Err(invalid("genesis revision must not have a parent"));
        }
        if record.revision_index > 0 && record.parent_revision_ids.is_empty() {
            return Err(invalid("non-genesis revision must have a parent"));
        }
        if record.original.lineage_id != record.lineage_id {
            return Err(invalid("original lineage differs from revision lineage"));
        }
        if record.original.canonical_sha256
            != canonical_hash_without_field(&record.original, "canonical_sha256")
        {
            return Err(invalid("original canonical_sha256 does not match payload"));
        }
        let topology = topology_from_original(&record.original)?;
        validate_topology(&topology)?;
        if record
            .original
            .tombstones
            .iter()
            .any(|tombstone| tombstone.retired_revision_index > record.revision_index)
        {
            return Err(invalid("tombstone points into a future revision"));
        }
        if let Some(evaluated) = &record.evaluated {
            if evaluated.namespace != AUTHORING_MESH_V2_EVALUATED_NAMESPACE
                || evaluated.source_revision_id != record.revision_id
                || evaluated.canonical_sha256
                    != canonical_hash_without_field(evaluated, "canonical_sha256")
            {
                return Err(invalid("evaluated sidecar binding is invalid"));
            }
            checked_id(&evaluated.artifact_id, "evaluated.artifact_id")?;
            checked_sha(&evaluated.artifact_sha256, "evaluated.artifact_sha256")?;
            checked_sha(&evaluated.readback_sha256, "evaluated.readback_sha256")?;
            if evaluated.correspondence_status.is_empty()
                || evaluated.correspondence_status.len() > MAX_ID_LENGTH
            {
                return Err(invalid("evaluated correspondence_status is invalid"));
            }
        }
        if let Some(binding) = &record.source_binding {
            validate_source_binding(binding)?;
        }
        if record.source_binding.is_some() && record.foundation_source_binding.is_some() {
            return Err(invalid(
                "candidate source binding and foundation source binding are mutually exclusive",
            ));
        }
        if let Some(binding) = &record.foundation_source_binding {
            validate_foundation_source_binding(binding)?;
        }
        if let Some(operation) = &record.operation {
            if operation.schema_version != AUTHORING_MESH_V2_OPERATION_SCHEMA_VERSION
                || operation.parent_revision_id
                    != record
                        .parent_revision_ids
                        .first()
                        .cloned()
                        .unwrap_or_else(|| AuthoringMeshRevisionId(String::new()))
                || operation.canonical_sha256
                    != canonical_hash_without_field(operation, "canonical_sha256")
            {
                return Err(invalid("topology operation binding is invalid"));
            }
            checked_id(&operation.operation_id, "operation.operation_id")?;
            checked_sha(
                &operation.operation_lineage_sha256,
                "operation.operation_lineage_sha256",
            )?;
            match &operation.kind {
                AuthoringMeshTopologyOperationKind::SplitEdge => {
                    if operation.source_elements.len() != 1
                        || operation.source_elements[0].kind != AuthoringMeshElementKind::Edge
                        || operation.tombstones.iter().any(|tombstone| {
                            tombstone.retired_revision_index != record.revision_index
                                || tombstone.operation_lineage_sha256
                                    != operation.operation_lineage_sha256
                        })
                    {
                        return Err(invalid(
                            "split-edge operation source/tombstone binding is invalid",
                        ));
                    }
                }
                AuthoringMeshTopologyOperationKind::FaceExtrude => {
                    if operation.source_elements.len() != 1
                        || operation.source_elements[0].kind != AuthoringMeshElementKind::Face
                        || !operation.tombstones.is_empty()
                        || !operation.retired_elements.is_empty()
                    {
                        return Err(invalid(
                            "face-extrude operation source/tombstone binding is invalid",
                        ));
                    }
                }
                AuthoringMeshTopologyOperationKind::MoveVertices => {
                    if !(1..=32).contains(&operation.source_elements.len())
                        || operation
                            .source_elements
                            .iter()
                            .any(|element| element.kind != AuthoringMeshElementKind::Vertex)
                        || !operation.generated_elements.is_empty()
                        || !operation.retired_elements.is_empty()
                        || !operation.tombstones.is_empty()
                        || operation
                            .source_elements
                            .windows(2)
                            .any(|pair| pair[0].id.as_str() >= pair[1].id.as_str())
                        || operation.source_elements.iter().any(|element| {
                            !record
                                .original
                                .vertices
                                .iter()
                                .any(|vertex| vertex.vertex_id.0 == element.id)
                        })
                    {
                        return Err(invalid(
                            "move-vertices operation source/journal binding is invalid",
                        ));
                    }
                }
                AuthoringMeshTopologyOperationKind::OpenFrameNotch => {
                    if operation.source_elements.len() != 4
                        || operation
                            .source_elements
                            .iter()
                            .any(|element| element.kind != AuthoringMeshElementKind::Face)
                        || operation.generated_elements.is_empty()
                        || operation.retired_elements.is_empty()
                        || operation.tombstones.is_empty()
                        || operation
                            .source_elements
                            .windows(2)
                            .any(|pair| pair[0].id.as_str() >= pair[1].id.as_str())
                        || operation.source_elements.iter().any(|element| {
                            !record
                                .original
                                .edges
                                .iter()
                                .any(|edge| edge.edge_id.0 == element.id)
                                && !record
                                    .original
                                    .tombstones
                                    .iter()
                                    .any(|tombstone| tombstone.element == *element)
                        })
                        || operation.tombstones.iter().any(|tombstone| {
                            tombstone.retired_revision_index != record.revision_index
                                || tombstone.operation_lineage_sha256
                                    != operation.operation_lineage_sha256
                        })
                    {
                        return Err(invalid(
                            "open-frame-notch operation source/journal binding is invalid",
                        ));
                    }
                }
                AuthoringMeshTopologyOperationKind::RearStockVoidRailBow => {
                    if operation.source_elements.len() != 2
                        || operation
                            .source_elements
                            .iter()
                            .any(|element| element.kind != AuthoringMeshElementKind::Edge)
                        || operation.generated_elements.is_empty()
                        || operation.retired_elements.is_empty()
                        || operation.tombstones.is_empty()
                        || operation
                            .source_elements
                            .windows(2)
                            .any(|pair| pair[0].id.as_str() >= pair[1].id.as_str())
                        || operation.source_elements.iter().any(|element| {
                            !record
                                .original
                                .edges
                                .iter()
                                .any(|edge| edge.edge_id.0 == element.id)
                                && !record
                                    .original
                                    .tombstones
                                    .iter()
                                    .any(|tombstone| tombstone.element == *element)
                        })
                        || operation.tombstones.iter().any(|tombstone| {
                            tombstone.retired_revision_index != record.revision_index
                                || tombstone.operation_lineage_sha256
                                    != operation.operation_lineage_sha256
                        })
                    {
                        return Err(invalid(
                            "rear-stock-void-rail-bow operation source/journal binding is invalid",
                        ));
                    }
                }
                AuthoringMeshTopologyOperationKind::RearStockVoidBoundaryBridge => {
                    if operation.source_elements.len() != 2
                        || operation
                            .source_elements
                            .iter()
                            .any(|element| element.kind != AuthoringMeshElementKind::Edge)
                        || operation.generated_elements.is_empty()
                        || operation.retired_elements.is_empty()
                        || operation.tombstones.is_empty()
                        || operation
                            .source_elements
                            .windows(2)
                            .any(|pair| pair[0].id.as_str() >= pair[1].id.as_str())
                        || operation.source_elements.iter().any(|element| {
                            !record
                                .original
                                .edges
                                .iter()
                                .any(|edge| edge.edge_id.0 == element.id)
                                && !record
                                    .original
                                    .tombstones
                                    .iter()
                                    .any(|tombstone| tombstone.element == *element)
                        })
                        || operation.tombstones.iter().any(|tombstone| {
                            tombstone.retired_revision_index != record.revision_index
                                || tombstone.operation_lineage_sha256
                                    != operation.operation_lineage_sha256
                        })
                        || operation.locality_policy
                            != "rear-stock-void-upper-inner-boundary-bridge-fixed-five-station-depth-wedge@1"
                    {
                        return Err(invalid(
                            "rear-stock-void-boundary-bridge operation source/journal binding is invalid",
                        ));
                    }
                }
            }
            let operation_tombstones = operation
                .tombstones
                .iter()
                .map(|tombstone| {
                    (
                        tombstone.element.kind.clone(),
                        tombstone.element.id.clone(),
                        tombstone.retired_revision_index,
                        tombstone.operation_lineage_sha256.clone(),
                    )
                })
                .collect::<BTreeSet<_>>();
            let original_tombstones = record
                .original
                .tombstones
                .iter()
                .map(|tombstone| {
                    (
                        tombstone.element.kind.clone(),
                        tombstone.element.id.clone(),
                        tombstone.retired_revision_index,
                        tombstone.operation_lineage_sha256.clone(),
                    )
                })
                .collect::<BTreeSet<_>>();
            if operation_tombstones
                .iter()
                .any(|tombstone| !original_tombstones.contains(tombstone))
            {
                return Err(invalid(
                    "operation tombstone is absent from the original namespace",
                ));
            }
            for retired in &operation.retired_elements {
                if !operation
                    .tombstones
                    .iter()
                    .any(|tombstone| tombstone.element == *retired)
                {
                    return Err(invalid(
                        "operation retired element has no matching tombstone",
                    ));
                }
            }
        } else if !record.original.tombstones.is_empty() {
            return Err(invalid("genesis revision cannot carry tombstones"));
        }
        if record.canonical_sha256 != canonical_hash_without_field(&record, "canonical_sha256") {
            return Err(invalid("revision canonical_sha256 does not match payload"));
        }
        Ok(Self { record })
    }

    /// Apply one bounded split-edge mutation.  Only the edge, its incident
    /// face cycles, and the immediate cycle neighbours are rewritten; all
    /// unrelated authored records retain their IDs and values byte-for-byte.
    pub(crate) fn split_edge(
        &self,
        request: AuthoringMeshSplitEdgeRequest,
    ) -> Result<AuthoringMeshSplitEdgeResult, RuntimeError> {
        if request.parent_revision_id != self.record.revision_id {
            return Err(invalid("split parent revision does not match the receiver"));
        }
        checked_id(&request.operation_id, "operation_id")?;
        checked_id(request.edge_id.as_ref(), "edge_id")?;
        checked_sha(
            &request.operation_lineage_sha256,
            "operation_lineage_sha256",
        )?;
        if !(1..=999).contains(&request.split_ratio_milli) {
            return Err(invalid("split_ratio_milli must be between 1 and 999"));
        }
        let parent_topology = topology_from_original(&self.record.original)?;
        validate_topology(&parent_topology)?;
        let parent_edge = parent_topology
            .edges
            .get(&request.edge_id)
            .cloned()
            .ok_or_else(|| invalid("split edge does not exist"))?;
        if parent_edge.half_edge_ids.is_empty() || parent_edge.half_edge_ids.len() > 2 {
            return Err(invalid(
                "split edge incidence is outside the manifold bound",
            ));
        }

        let midpoint_id = AuthoringMeshVertexId(stable_id(
            "v",
            self.record.lineage_id.as_ref(),
            "split-edge-midpoint",
            &json!({
                "parent_revision_id":self.record.revision_id,
                "operation_lineage_sha256":request.operation_lineage_sha256,
                "edge_id":request.edge_id,
            }),
        ));
        let edge_a_id = AuthoringMeshEdgeId(stable_id(
            "e",
            self.record.lineage_id.as_ref(),
            "split-edge-child",
            &json!({"parent_revision_id":self.record.revision_id,"operation_lineage_sha256":request.operation_lineage_sha256,"edge_id":request.edge_id,"endpoint":parent_edge.vertex_ids[0]}),
        ));
        let edge_b_id = AuthoringMeshEdgeId(stable_id(
            "e",
            self.record.lineage_id.as_ref(),
            "split-edge-child",
            &json!({"parent_revision_id":self.record.revision_id,"operation_lineage_sha256":request.operation_lineage_sha256,"edge_id":request.edge_id,"endpoint":parent_edge.vertex_ids[1]}),
        ));
        if parent_topology.vertices.contains_key(&midpoint_id)
            || parent_topology.edges.contains_key(&edge_a_id)
            || parent_topology.edges.contains_key(&edge_b_id)
        {
            return Err(invalid("split operation would reuse an active stable ID"));
        }
        let left = parent_topology
            .vertices
            .get(&parent_edge.vertex_ids[0])
            .expect("edge endpoint validated")
            .position_m;
        let right = parent_topology
            .vertices
            .get(&parent_edge.vertex_ids[1])
            .expect("edge endpoint validated")
            .position_m;
        let ratio = request.split_ratio_milli as f64 / 1000.0;
        let midpoint = [
            left[0] + (right[0] - left[0]) * ratio,
            left[1] + (right[1] - left[1]) * ratio,
            left[2] + (right[2] - left[2]) * ratio,
        ];
        finite_position(midpoint, "split midpoint")?;

        let mut child_topology = parent_topology.clone();
        child_topology.vertices.insert(
            midpoint_id.clone(),
            AuthoringMeshVertex {
                vertex_id: midpoint_id.clone(),
                position_m: midpoint,
            },
        );
        child_topology.edges.remove(&request.edge_id);
        child_topology.edges.insert(
            edge_a_id.clone(),
            AuthoringMeshEdge {
                edge_id: edge_a_id.clone(),
                vertex_ids: [parent_edge.vertex_ids[0].clone(), midpoint_id.clone()],
                half_edge_ids: Vec::new(),
                boundary: true,
            },
        );
        child_topology.edges.insert(
            edge_b_id.clone(),
            AuthoringMeshEdge {
                edge_id: edge_b_id.clone(),
                vertex_ids: [midpoint_id.clone(), parent_edge.vertex_ids[1].clone()],
                half_edge_ids: Vec::new(),
                boundary: true,
            },
        );

        let mut changed = BTreeSet::<(AuthoringMeshElementKind, String)>::new();
        let mut generated = BTreeSet::<(AuthoringMeshElementKind, String)>::new();
        let mut retired = BTreeSet::<(AuthoringMeshElementKind, String)>::new();
        let mut tombstones = Vec::new();
        mark_ref(
            &mut changed,
            AuthoringMeshElementKind::Edge,
            request.edge_id.0.clone(),
        );
        mark_ref(
            &mut changed,
            AuthoringMeshElementKind::Vertex,
            parent_edge.vertex_ids[0].0.clone(),
        );
        mark_ref(
            &mut changed,
            AuthoringMeshElementKind::Vertex,
            parent_edge.vertex_ids[1].0.clone(),
        );
        mark_ref(
            &mut generated,
            AuthoringMeshElementKind::Vertex,
            midpoint_id.0.clone(),
        );
        mark_ref(
            &mut generated,
            AuthoringMeshElementKind::Edge,
            edge_a_id.0.clone(),
        );
        mark_ref(
            &mut generated,
            AuthoringMeshElementKind::Edge,
            edge_b_id.0.clone(),
        );
        retire_ref(
            &mut retired,
            &mut tombstones,
            AuthoringMeshElementKind::Edge,
            request.edge_id.0.clone(),
            self.record.revision_index + 1,
            &request.operation_lineage_sha256,
            "split_edge replaced the source edge",
        );

        let incident_half_edges = parent_edge.half_edge_ids.clone();
        for old_half_edge_id in &incident_half_edges {
            let old_half_edge = child_topology
                .half_edges
                .remove(old_half_edge_id)
                .ok_or_else(|| invalid("split edge references a missing half-edge"))?;
            let old_corner = child_topology
                .corners
                .remove(&old_half_edge.corner_id)
                .ok_or_else(|| invalid("split edge references a missing corner"))?;
            let old_corner_id = old_corner.corner_id.0.clone();
            let old_next_id = old_half_edge.next_id.clone();
            let old_prev_id = old_half_edge.prev_id.clone();
            let old_end_id = child_topology
                .half_edges
                .get(&old_next_id)
                .ok_or_else(|| invalid("split edge next half-edge is missing"))?
                .origin_vertex_id
                .clone();
            let first_half_edge_id = AuthoringMeshHalfEdgeId(stable_id(
                "he",
                self.record.lineage_id.as_ref(),
                "split-edge-half-edge",
                &json!({"parent_revision_id":self.record.revision_id,"operation_lineage_sha256":request.operation_lineage_sha256,"source_half_edge_id":old_half_edge_id,"segment":0}),
            ));
            let second_half_edge_id = AuthoringMeshHalfEdgeId(stable_id(
                "he",
                self.record.lineage_id.as_ref(),
                "split-edge-half-edge",
                &json!({"parent_revision_id":self.record.revision_id,"operation_lineage_sha256":request.operation_lineage_sha256,"source_half_edge_id":old_half_edge_id,"segment":1}),
            ));
            let first_corner_id = AuthoringMeshCornerId(stable_id(
                "c",
                self.record.lineage_id.as_ref(),
                "split-edge-corner",
                &json!({"parent_revision_id":self.record.revision_id,"operation_lineage_sha256":request.operation_lineage_sha256,"source_corner_id":old_corner.corner_id,"segment":0}),
            ));
            let second_corner_id = AuthoringMeshCornerId(stable_id(
                "c",
                self.record.lineage_id.as_ref(),
                "split-edge-corner",
                &json!({"parent_revision_id":self.record.revision_id,"operation_lineage_sha256":request.operation_lineage_sha256,"source_corner_id":old_corner.corner_id,"segment":1}),
            ));
            if child_topology.half_edges.contains_key(&first_half_edge_id)
                || child_topology.half_edges.contains_key(&second_half_edge_id)
                || child_topology.corners.contains_key(&first_corner_id)
                || child_topology.corners.contains_key(&second_corner_id)
            {
                return Err(invalid("split operation would reuse a half-edge/corner ID"));
            }
            let first_edge_id = if old_half_edge.origin_vertex_id == parent_edge.vertex_ids[0]
                || old_half_edge.origin_vertex_id == parent_edge.vertex_ids[1]
            {
                if old_half_edge.origin_vertex_id == parent_edge.vertex_ids[0] {
                    edge_a_id.clone()
                } else {
                    edge_b_id.clone()
                }
            } else {
                return Err(invalid(
                    "split edge origin is not one of the edge endpoints",
                ));
            };
            let second_edge_id = if old_end_id == parent_edge.vertex_ids[0] {
                edge_a_id.clone()
            } else if old_end_id == parent_edge.vertex_ids[1] {
                edge_b_id.clone()
            } else {
                return Err(invalid("split edge end is not one of the edge endpoints"));
            };
            let first_half_edge = AuthoringMeshHalfEdge {
                half_edge_id: first_half_edge_id.clone(),
                origin_vertex_id: old_half_edge.origin_vertex_id.clone(),
                edge_id: first_edge_id,
                face_id: old_half_edge.face_id.clone(),
                corner_id: first_corner_id.clone(),
                next_id: second_half_edge_id.clone(),
                prev_id: old_prev_id.clone(),
                twin_id: None,
                boundary: true,
            };
            let second_half_edge = AuthoringMeshHalfEdge {
                half_edge_id: second_half_edge_id.clone(),
                origin_vertex_id: midpoint_id.clone(),
                edge_id: second_edge_id,
                face_id: old_half_edge.face_id.clone(),
                corner_id: second_corner_id.clone(),
                next_id: old_next_id.clone(),
                prev_id: first_half_edge_id.clone(),
                twin_id: None,
                boundary: true,
            };
            let mut first_corner = old_corner.clone();
            first_corner.corner_id = first_corner_id.clone();
            first_corner.half_edge_id = first_half_edge_id.clone();
            let mut second_corner = old_corner.clone();
            second_corner.corner_id = second_corner_id.clone();
            second_corner.half_edge_id = second_half_edge_id.clone();
            second_corner.vertex_id = midpoint_id.clone();
            second_corner.ordinal = second_corner.ordinal.saturating_add(1);
            child_topology
                .half_edges
                .insert(first_half_edge_id.clone(), first_half_edge);
            child_topology
                .half_edges
                .insert(second_half_edge_id.clone(), second_half_edge);
            child_topology
                .corners
                .insert(first_corner_id.clone(), first_corner);
            child_topology
                .corners
                .insert(second_corner_id.clone(), second_corner);
            if let Some(previous) = child_topology.half_edges.get_mut(&old_prev_id) {
                previous.next_id = first_half_edge_id.clone();
            }
            if let Some(next) = child_topology.half_edges.get_mut(&old_next_id) {
                next.prev_id = second_half_edge_id.clone();
            }
            let face = child_topology
                .faces
                .get_mut(&old_half_edge.face_id)
                .ok_or_else(|| invalid("split edge face is missing"))?;
            let position = face
                .half_edge_ids
                .iter()
                .position(|id| id == old_half_edge_id)
                .ok_or_else(|| invalid("split edge half-edge is absent from its face cycle"))?;
            face.half_edge_ids.splice(
                position..=position,
                [first_half_edge_id.clone(), second_half_edge_id.clone()],
            );
            if let Some(loop_record) = child_topology.loops.get_mut(&face.loop_id) {
                loop_record.half_edge_ids = face.half_edge_ids.clone();
            }
            mark_ref(
                &mut changed,
                AuthoringMeshElementKind::Face,
                old_half_edge.face_id.0.clone(),
            );
            mark_ref(
                &mut changed,
                AuthoringMeshElementKind::Loop,
                face.loop_id.0.clone(),
            );
            mark_ref(
                &mut changed,
                AuthoringMeshElementKind::HalfEdge,
                old_half_edge_id.0.clone(),
            );
            mark_ref(
                &mut changed,
                AuthoringMeshElementKind::HalfEdge,
                old_prev_id.0.clone(),
            );
            mark_ref(
                &mut changed,
                AuthoringMeshElementKind::HalfEdge,
                old_next_id.0.clone(),
            );
            mark_ref(
                &mut changed,
                AuthoringMeshElementKind::Corner,
                old_corner_id.clone(),
            );
            mark_ref(
                &mut generated,
                AuthoringMeshElementKind::HalfEdge,
                first_half_edge_id.0.clone(),
            );
            mark_ref(
                &mut generated,
                AuthoringMeshElementKind::HalfEdge,
                second_half_edge_id.0.clone(),
            );
            mark_ref(
                &mut generated,
                AuthoringMeshElementKind::Corner,
                first_corner_id.0.clone(),
            );
            mark_ref(
                &mut generated,
                AuthoringMeshElementKind::Corner,
                second_corner_id.0.clone(),
            );
            retire_ref(
                &mut retired,
                &mut tombstones,
                AuthoringMeshElementKind::HalfEdge,
                old_half_edge_id.0.clone(),
                self.record.revision_index + 1,
                &request.operation_lineage_sha256,
                "split_edge replaced the source half-edge",
            );
            retire_ref(
                &mut retired,
                &mut tombstones,
                AuthoringMeshElementKind::Corner,
                old_corner_id,
                self.record.revision_index + 1,
                &request.operation_lineage_sha256,
                "split_edge replaced the source corner",
            );
        }

        // Recompute ordinals and local flags after inserting the midpoint.
        for face in child_topology.faces.values_mut() {
            face.boundary = face
                .half_edge_ids
                .iter()
                .any(|id| child_topology.half_edges[id].boundary);
            for (ordinal, half_edge_id) in face.half_edge_ids.iter().enumerate() {
                let corner_id = child_topology.half_edges[half_edge_id].corner_id.clone();
                mark_ref(
                    &mut changed,
                    AuthoringMeshElementKind::Corner,
                    corner_id.0.clone(),
                );
                child_topology
                    .corners
                    .get_mut(&corner_id)
                    .expect("corner exists")
                    .ordinal = ordinal as u32;
            }
        }
        rebuild_edge_incidence(&mut child_topology)?;
        rebuild_twins(&mut child_topology)?;
        for loop_record in child_topology.loops.values_mut() {
            loop_record.boundary = loop_record
                .half_edge_ids
                .iter()
                .any(|id| child_topology.half_edges[id].boundary);
        }
        for ring in child_topology.rings.values_mut() {
            if ring.edge_ids.iter().any(|id| id == &request.edge_id) {
                mark_ref(
                    &mut changed,
                    AuthoringMeshElementKind::Ring,
                    ring.ring_id.0.clone(),
                );
                let mut replaced = Vec::new();
                for edge_id in &ring.edge_ids {
                    if edge_id == &request.edge_id {
                        replaced.push(edge_a_id.clone());
                        replaced.push(edge_b_id.clone());
                    } else {
                        replaced.push(edge_id.clone());
                    }
                }
                replaced.sort();
                ring.edge_ids = replaced;
            }
        }
        child_topology.tombstones.extend(tombstones.clone());
        validate_topology(&child_topology)?;
        let touched = changed.clone();
        verify_locality(&parent_topology, &child_topology, &touched)?;

        let generated_refs = refs_from_set(&generated);
        let retired_refs = refs_from_set(&retired);
        let changed_refs = refs_from_set(&changed);
        let operation = operation_record(
            request.operation_id,
            request.edge_id.clone(),
            request.operation_lineage_sha256.clone(),
            self.record.revision_id.clone(),
            generated_refs.clone(),
            retired_refs.clone(),
            tombstones,
        );
        let original = original_record(&self.record.lineage_id, child_topology)?;
        let child_revision_id = AuthoringMeshRevisionId(stable_id(
            "amrev",
            self.record.lineage_id.as_ref(),
            "split-edge-revision",
            &json!({"parent_revision_id":self.record.revision_id,"operation_lineage_sha256":request.operation_lineage_sha256,"original_sha256":original.canonical_sha256}),
        ));
        let child_record = revision_record(
            self.record.mesh_id.clone(),
            self.record.lineage_id.clone(),
            child_revision_id,
            vec![self.record.revision_id.clone()],
            self.record.revision_index + 1,
            Some(operation),
            original,
            None,
            self.record.source_binding.clone(),
            self.record.foundation_source_binding.clone(),
        );
        Ok(AuthoringMeshSplitEdgeResult {
            schema_version: AUTHORING_MESH_V2_REVISION_SCHEMA_VERSION.to_owned(),
            parent_revision_id: self.record.revision_id.clone(),
            child_revision: child_record,
            changed_elements: changed_refs,
            generated_elements: generated_refs,
            retired_elements: retired_refs,
            locality_status: "local-topology-edit-preserves-unaffected-records@2".to_owned(),
            evaluated_status: if self.record.evaluated.is_some() {
                "evaluated-sidecar-invalidated-requires-new-evaluation@2".to_owned()
            } else {
                "evaluated-sidecar-not-present@2".to_owned()
            },
        })
    }

    /// Move a bounded set of authored vertices by per-vertex deltas.  This is
    /// deliberately a position-only edit: topology, stable IDs, source
    /// binding and all unrelated records remain unchanged.  The evaluated
    /// sidecar is invalidated because its geometry no longer matches the
    /// authored revision.
    pub(crate) fn move_vertices(
        &self,
        request: AuthoringMeshMoveVerticesRequest,
    ) -> Result<AuthoringMeshMoveVerticesResult, RuntimeError> {
        if request.parent_revision_id != self.record.revision_id {
            return Err(invalid(
                "move vertices parent revision does not match the receiver",
            ));
        }
        checked_id(&request.operation_id, "operation_id")?;
        checked_sha(
            &request.operation_lineage_sha256,
            "operation_lineage_sha256",
        )?;
        if !(1..=32).contains(&request.vertex_ids.len()) {
            return Err(invalid(
                "move vertices must select between 1 and 32 vertices",
            ));
        }
        if request.delta_m.len() != request.vertex_ids.len() {
            return Err(invalid(
                "move vertices vertex_ids and delta_m must have equal lengths",
            ));
        }
        if request.delta_m.iter().any(|delta| {
            delta
                .iter()
                .any(|value| !value.is_finite() || value.abs() > 1.0)
        }) || request
            .delta_m
            .iter()
            .all(|delta| delta.iter().all(|value| value.abs() <= f64::EPSILON))
        {
            return Err(invalid(
                "move vertices delta_m entries must be finite, collectively non-zero and inside [-1,1]m",
            ));
        }

        let parent_topology = topology_from_original(&self.record.original)?;
        validate_topology(&parent_topology)?;
        let mut moves = request
            .vertex_ids
            .iter()
            .cloned()
            .zip(request.delta_m.iter().copied())
            .collect::<Vec<_>>();
        moves.sort_by(|left, right| left.0.cmp(&right.0));
        if moves.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(invalid("move vertices vertex_ids must be unique"));
        }
        for (vertex_id, _) in &moves {
            checked_id(vertex_id.as_ref(), "vertex_id")?;
            if !parent_topology.vertices.contains_key(vertex_id) {
                return Err(invalid("move vertices references an unknown vertex"));
            }
        }

        let mut child_topology = parent_topology.clone();
        let mut changed = BTreeSet::<(AuthoringMeshElementKind, String)>::new();
        for (vertex_id, delta) in &moves {
            let vertex = child_topology
                .vertices
                .get_mut(vertex_id)
                .expect("move vertices source vertex was validated");
            vertex.position_m = [
                vertex.position_m[0] + delta[0],
                vertex.position_m[1] + delta[1],
                vertex.position_m[2] + delta[2],
            ];
            finite_position(vertex.position_m, "move vertices position")?;
            mark_ref(
                &mut changed,
                AuthoringMeshElementKind::Vertex,
                vertex_id.0.clone(),
            );
        }

        // No topology maps need rebuilding: this operation changes only
        // vertex positions.  Full validation still rejects collapsed edges,
        // zero-area faces, non-finite coordinates and any stale identity.
        validate_topology(&child_topology)?;
        verify_locality(&parent_topology, &child_topology, &changed)?;

        let changed_refs = refs_from_set(&changed);
        let moved_vertex_ids = moves
            .iter()
            .map(|(vertex_id, _)| vertex_id.clone())
            .collect::<Vec<_>>();
        let moved_deltas = moves.iter().map(|(_, delta)| *delta).collect::<Vec<_>>();
        let operation = move_vertices_operation_record(
            request.operation_id,
            moved_vertex_ids.clone(),
            request.operation_lineage_sha256.clone(),
            self.record.revision_id.clone(),
        );
        let original = original_record(&self.record.lineage_id, child_topology)?;
        let child_revision_id = AuthoringMeshRevisionId(stable_id(
            "amrev",
            self.record.lineage_id.as_ref(),
            "move-vertices-revision",
            &json!({
                "parent_revision_id": self.record.revision_id,
                "operation_lineage_sha256": request.operation_lineage_sha256,
                "vertex_ids": moved_vertex_ids,
                "delta_m": moved_deltas,
                "original_sha256": original.canonical_sha256,
            }),
        ));
        let child_record = revision_record(
            self.record.mesh_id.clone(),
            self.record.lineage_id.clone(),
            child_revision_id,
            vec![self.record.revision_id.clone()],
            self.record.revision_index + 1,
            Some(operation),
            original,
            None,
            self.record.source_binding.clone(),
            self.record.foundation_source_binding.clone(),
        );
        Ok(AuthoringMeshMoveVerticesResult {
            schema_version: AUTHORING_MESH_V2_REVISION_SCHEMA_VERSION.to_owned(),
            parent_revision_id: self.record.revision_id.clone(),
            child_revision: child_record,
            moved_vertex_ids,
            changed_elements: changed_refs,
            generated_elements: Vec::new(),
            retired_elements: Vec::new(),
            locality_status:
                "vertex-position-only-local-edit-preserves-topology-and-unaffected-records@2"
                    .to_owned(),
            evaluated_status: if self.record.evaluated.is_some() {
                "evaluated-sidecar-invalidated-requires-new-evaluation@2".to_owned()
            } else {
                "evaluated-sidecar-not-present@2".to_owned()
            },
        })
    }

    /// Extrude one planar convex boundary face into a typed shell.  The
    /// source face and its outer-ring IDs remain stable; Runtime derives the
    /// top ring, vertical edges, side faces and their half-edge/corner IDs
    /// from the parent revision and operation lineage.  A face with adjacent
    /// authored faces is rejected so the kernel never guesses how to merge a
    /// pre-existing neighborhood.
    pub(crate) fn face_extrude(
        &self,
        request: AuthoringMeshFaceExtrudeRequest,
    ) -> Result<AuthoringMeshFaceExtrudeResult, RuntimeError> {
        if request.parent_revision_id != self.record.revision_id {
            return Err(invalid(
                "face extrude parent revision does not match the receiver",
            ));
        }
        checked_id(&request.operation_id, "operation_id")?;
        checked_id(request.face_id.as_ref(), "face_id")?;
        checked_sha(
            &request.operation_lineage_sha256,
            "operation_lineage_sha256",
        )?;
        if !request.distance_m.is_finite()
            || request.distance_m.abs() > MAX_COORDINATE_M
            || request.distance_m.abs() <= MIN_EDGE_LENGTH_M
        {
            return Err(invalid("face extrude distance is outside bounds"));
        }

        let parent_topology = topology_from_original(&self.record.original)?;
        validate_topology(&parent_topology)?;
        let source_face = parent_topology
            .faces
            .get(&request.face_id)
            .cloned()
            .ok_or_else(|| invalid("face extrude source face does not exist"))?;
        if source_face.half_edge_ids.len() > MAX_FACE_DEGREE {
            return Err(invalid("face extrude source face exceeds the degree bound"));
        }
        let source_half_edges = source_face
            .half_edge_ids
            .iter()
            .map(|id| {
                parent_topology
                    .half_edges
                    .get(id)
                    .cloned()
                    .ok_or_else(|| invalid("face extrude source half-edge is missing"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if source_half_edges
            .iter()
            .any(|half_edge| !parent_topology.edges[&half_edge.edge_id].boundary)
        {
            return Err(invalid(
                "face extrude only accepts a face whose complete edge ring is boundary",
            ));
        }
        let source_vertex_ids = source_half_edges
            .iter()
            .map(|half_edge| half_edge.origin_vertex_id.clone())
            .collect::<Vec<_>>();
        let source_positions = source_vertex_ids
            .iter()
            .map(|id| parent_topology.vertices[id].position_m)
            .collect::<Vec<_>>();
        let normal = normalized_face_normal(&source_positions)?;
        let count = source_vertex_ids.len();
        if count < 3 {
            return Err(invalid("face extrude source face has too few corners"));
        }

        let mut child_topology = parent_topology.clone();
        let mut changed = BTreeSet::<(AuthoringMeshElementKind, String)>::new();
        let mut generated = BTreeSet::<(AuthoringMeshElementKind, String)>::new();

        let top_vertex_ids = source_vertex_ids
            .iter()
            .enumerate()
            .map(|(index, source_vertex_id)| {
                AuthoringMeshVertexId(stable_id(
                    "v",
                    self.record.lineage_id.as_ref(),
                    "face-extrude-top-vertex",
                    &json!({
                        "parent_revision_id": self.record.revision_id,
                        "operation_lineage_sha256": request.operation_lineage_sha256,
                        "face_id": request.face_id,
                        "source_vertex_id": source_vertex_id,
                        "ordinal": index,
                    }),
                ))
            })
            .collect::<Vec<_>>();
        for (index, top_vertex_id) in top_vertex_ids.iter().enumerate() {
            if child_topology.vertices.contains_key(top_vertex_id) {
                return Err(invalid("face extrude would reuse a vertex stable ID"));
            }
            let source = source_positions[index];
            let position = [
                source[0] + normal[0] * request.distance_m,
                source[1] + normal[1] * request.distance_m,
                source[2] + normal[2] * request.distance_m,
            ];
            finite_position(position, "face extrude top vertex")?;
            child_topology.vertices.insert(
                top_vertex_id.clone(),
                AuthoringMeshVertex {
                    vertex_id: top_vertex_id.clone(),
                    position_m: position,
                },
            );
            mark_ref(
                &mut generated,
                AuthoringMeshElementKind::Vertex,
                top_vertex_id.0.clone(),
            );
        }

        let vertical_edge_ids = source_vertex_ids
            .iter()
            .enumerate()
            .map(|(index, source_vertex_id)| {
                AuthoringMeshEdgeId(stable_id(
                    "e",
                    self.record.lineage_id.as_ref(),
                    "face-extrude-vertical-edge",
                    &json!({
                        "parent_revision_id": self.record.revision_id,
                        "operation_lineage_sha256": request.operation_lineage_sha256,
                        "face_id": request.face_id,
                        "source_vertex_id": source_vertex_id,
                        "ordinal": index,
                    }),
                ))
            })
            .collect::<Vec<_>>();
        for (index, edge_id) in vertical_edge_ids.iter().enumerate() {
            if child_topology.edges.contains_key(edge_id) {
                return Err(invalid(
                    "face extrude would reuse a vertical edge stable ID",
                ));
            }
            child_topology.edges.insert(
                edge_id.clone(),
                AuthoringMeshEdge {
                    edge_id: edge_id.clone(),
                    vertex_ids: [
                        source_vertex_ids[index].clone(),
                        top_vertex_ids[index].clone(),
                    ],
                    half_edge_ids: Vec::new(),
                    boundary: true,
                },
            );
            mark_ref(
                &mut generated,
                AuthoringMeshElementKind::Edge,
                edge_id.0.clone(),
            );
        }

        let top_edge_ids = source_half_edges
            .iter()
            .enumerate()
            .map(|(index, source_half_edge)| {
                AuthoringMeshEdgeId(stable_id(
                    "e",
                    self.record.lineage_id.as_ref(),
                    "face-extrude-top-edge",
                    &json!({
                        "parent_revision_id": self.record.revision_id,
                        "operation_lineage_sha256": request.operation_lineage_sha256,
                        "face_id": request.face_id,
                        "source_edge_id": source_half_edge.edge_id,
                        "ordinal": index,
                    }),
                ))
            })
            .collect::<Vec<_>>();
        for (index, edge_id) in top_edge_ids.iter().enumerate() {
            if child_topology.edges.contains_key(edge_id) {
                return Err(invalid("face extrude would reuse a top edge stable ID"));
            }
            let next = (index + 1) % count;
            child_topology.edges.insert(
                edge_id.clone(),
                AuthoringMeshEdge {
                    edge_id: edge_id.clone(),
                    vertex_ids: [top_vertex_ids[index].clone(), top_vertex_ids[next].clone()],
                    half_edge_ids: Vec::new(),
                    boundary: true,
                },
            );
            mark_ref(
                &mut generated,
                AuthoringMeshElementKind::Edge,
                edge_id.0.clone(),
            );
        }

        let top_face_id = AuthoringMeshFaceId(stable_id(
            "f",
            self.record.lineage_id.as_ref(),
            "face-extrude-top-face",
            &json!({
                "parent_revision_id": self.record.revision_id,
                "operation_lineage_sha256": request.operation_lineage_sha256,
                "source_face_id": request.face_id,
            }),
        ));
        let top_loop_id = AuthoringMeshLoopId(stable_id(
            "loop",
            self.record.lineage_id.as_ref(),
            "face-extrude-top-loop",
            &json!({"face_id": top_face_id}),
        ));
        if child_topology.faces.contains_key(&top_face_id)
            || child_topology.loops.contains_key(&top_loop_id)
        {
            return Err(invalid("face extrude would reuse the top face stable ID"));
        }
        let side_face_ids = source_half_edges
            .iter()
            .enumerate()
            .map(|(index, source_half_edge)| {
                AuthoringMeshFaceId(stable_id(
                    "f",
                    self.record.lineage_id.as_ref(),
                    "face-extrude-side-face",
                    &json!({
                        "parent_revision_id": self.record.revision_id,
                        "operation_lineage_sha256": request.operation_lineage_sha256,
                        "source_face_id": request.face_id,
                        "source_edge_id": source_half_edge.edge_id,
                        "ordinal": index,
                    }),
                ))
            })
            .collect::<Vec<_>>();
        let side_loop_ids = side_face_ids
            .iter()
            .map(|face_id| {
                AuthoringMeshLoopId(stable_id(
                    "loop",
                    self.record.lineage_id.as_ref(),
                    "face-extrude-side-loop",
                    &json!({"face_id": face_id}),
                ))
            })
            .collect::<Vec<_>>();
        for (face_id, loop_id) in side_face_ids.iter().zip(&side_loop_ids) {
            if child_topology.faces.contains_key(face_id)
                || child_topology.loops.contains_key(loop_id)
            {
                return Err(invalid("face extrude would reuse a side face stable ID"));
            }
            child_topology.faces.insert(
                face_id.clone(),
                AuthoringMeshFace {
                    face_id: face_id.clone(),
                    half_edge_ids: Vec::new(),
                    loop_id: loop_id.clone(),
                    boundary: false,
                },
            );
            child_topology.loops.insert(
                loop_id.clone(),
                AuthoringMeshLoop {
                    loop_id: loop_id.clone(),
                    face_id: face_id.clone(),
                    half_edge_ids: Vec::new(),
                    boundary: false,
                },
            );
            mark_ref(
                &mut generated,
                AuthoringMeshElementKind::Face,
                face_id.0.clone(),
            );
            mark_ref(
                &mut generated,
                AuthoringMeshElementKind::Loop,
                loop_id.0.clone(),
            );
        }
        child_topology.faces.insert(
            top_face_id.clone(),
            AuthoringMeshFace {
                face_id: top_face_id.clone(),
                half_edge_ids: Vec::new(),
                loop_id: top_loop_id.clone(),
                boundary: false,
            },
        );
        child_topology.loops.insert(
            top_loop_id.clone(),
            AuthoringMeshLoop {
                loop_id: top_loop_id.clone(),
                face_id: top_face_id.clone(),
                half_edge_ids: Vec::new(),
                boundary: false,
            },
        );
        mark_ref(
            &mut generated,
            AuthoringMeshElementKind::Face,
            top_face_id.0.clone(),
        );
        mark_ref(
            &mut generated,
            AuthoringMeshElementKind::Loop,
            top_loop_id.0.clone(),
        );

        let mut side_half_edges = Vec::with_capacity(count);
        let mut side_corners = Vec::with_capacity(count);
        let mut top_half_edges = Vec::with_capacity(count);
        let mut top_corners = Vec::with_capacity(count);
        for (index, source_half_edge) in source_half_edges.iter().enumerate() {
            let next = (index + 1) % count;
            let side_face_id = side_face_ids[index].clone();
            let outer_half_edge_id = AuthoringMeshHalfEdgeId(stable_id(
                "he",
                self.record.lineage_id.as_ref(),
                "face-extrude-side-outer-half-edge",
                &json!({"parent_revision_id":self.record.revision_id,"operation_lineage_sha256":request.operation_lineage_sha256,"source_edge_id":source_half_edge.edge_id}),
            ));
            let vertical_forward_id = AuthoringMeshHalfEdgeId(stable_id(
                "he",
                self.record.lineage_id.as_ref(),
                "face-extrude-side-vertical-forward-half-edge",
                &json!({"parent_revision_id":self.record.revision_id,"operation_lineage_sha256":request.operation_lineage_sha256,"source_vertex_id":source_vertex_ids[index]}),
            ));
            let top_side_id = AuthoringMeshHalfEdgeId(stable_id(
                "he",
                self.record.lineage_id.as_ref(),
                "face-extrude-side-top-half-edge",
                &json!({"parent_revision_id":self.record.revision_id,"operation_lineage_sha256":request.operation_lineage_sha256,"source_edge_id":source_half_edge.edge_id}),
            ));
            let vertical_back_id = AuthoringMeshHalfEdgeId(stable_id(
                "he",
                self.record.lineage_id.as_ref(),
                "face-extrude-side-vertical-back-half-edge",
                &json!({"parent_revision_id":self.record.revision_id,"operation_lineage_sha256":request.operation_lineage_sha256,"source_vertex_id":source_vertex_ids[next]}),
            ));
            let outer_corner_id = AuthoringMeshCornerId(stable_id(
                "c",
                self.record.lineage_id.as_ref(),
                "face-extrude-side-outer-corner",
                &json!({"face_id":side_face_id,"ordinal":0}),
            ));
            let forward_corner_id = AuthoringMeshCornerId(stable_id(
                "c",
                self.record.lineage_id.as_ref(),
                "face-extrude-side-vertical-forward-corner",
                &json!({"face_id":side_face_id,"ordinal":1}),
            ));
            let top_side_corner_id = AuthoringMeshCornerId(stable_id(
                "c",
                self.record.lineage_id.as_ref(),
                "face-extrude-side-top-corner",
                &json!({"face_id":side_face_id,"ordinal":2}),
            ));
            let back_corner_id = AuthoringMeshCornerId(stable_id(
                "c",
                self.record.lineage_id.as_ref(),
                "face-extrude-side-vertical-back-corner",
                &json!({"face_id":side_face_id,"ordinal":3}),
            ));
            for id in [
                &outer_half_edge_id,
                &vertical_forward_id,
                &top_side_id,
                &vertical_back_id,
            ] {
                if child_topology.half_edges.contains_key(id) {
                    return Err(invalid("face extrude would reuse a half-edge stable ID"));
                }
            }
            for id in [
                &outer_corner_id,
                &forward_corner_id,
                &top_side_corner_id,
                &back_corner_id,
            ] {
                if child_topology.corners.contains_key(id) {
                    return Err(invalid("face extrude would reuse a corner stable ID"));
                }
            }
            child_topology.half_edges.insert(
                outer_half_edge_id.clone(),
                AuthoringMeshHalfEdge {
                    half_edge_id: outer_half_edge_id.clone(),
                    origin_vertex_id: source_vertex_ids[next].clone(),
                    edge_id: source_half_edge.edge_id.clone(),
                    face_id: side_face_id.clone(),
                    corner_id: outer_corner_id.clone(),
                    next_id: vertical_forward_id.clone(),
                    prev_id: vertical_back_id.clone(),
                    twin_id: None,
                    boundary: false,
                },
            );
            child_topology.half_edges.insert(
                vertical_forward_id.clone(),
                AuthoringMeshHalfEdge {
                    half_edge_id: vertical_forward_id.clone(),
                    origin_vertex_id: source_vertex_ids[index].clone(),
                    edge_id: vertical_edge_ids[index].clone(),
                    face_id: side_face_id.clone(),
                    corner_id: forward_corner_id.clone(),
                    next_id: top_side_id.clone(),
                    prev_id: outer_half_edge_id.clone(),
                    twin_id: None,
                    boundary: false,
                },
            );
            child_topology.half_edges.insert(
                top_side_id.clone(),
                AuthoringMeshHalfEdge {
                    half_edge_id: top_side_id.clone(),
                    origin_vertex_id: top_vertex_ids[index].clone(),
                    edge_id: top_edge_ids[index].clone(),
                    face_id: side_face_id.clone(),
                    corner_id: top_side_corner_id.clone(),
                    next_id: vertical_back_id.clone(),
                    prev_id: vertical_forward_id.clone(),
                    twin_id: None,
                    boundary: false,
                },
            );
            child_topology.half_edges.insert(
                vertical_back_id.clone(),
                AuthoringMeshHalfEdge {
                    half_edge_id: vertical_back_id.clone(),
                    origin_vertex_id: top_vertex_ids[next].clone(),
                    edge_id: vertical_edge_ids[next].clone(),
                    face_id: side_face_id.clone(),
                    corner_id: back_corner_id.clone(),
                    next_id: outer_half_edge_id.clone(),
                    prev_id: top_side_id.clone(),
                    twin_id: None,
                    boundary: false,
                },
            );
            child_topology.corners.insert(
                outer_corner_id.clone(),
                AuthoringMeshCorner {
                    corner_id: outer_corner_id.clone(),
                    half_edge_id: outer_half_edge_id.clone(),
                    vertex_id: source_vertex_ids[next].clone(),
                    face_id: side_face_id.clone(),
                    ordinal: 0,
                    uv0: None,
                    normal: None,
                    tangent: None,
                    seam: false,
                },
            );
            child_topology.corners.insert(
                forward_corner_id.clone(),
                AuthoringMeshCorner {
                    corner_id: forward_corner_id.clone(),
                    half_edge_id: vertical_forward_id.clone(),
                    vertex_id: source_vertex_ids[index].clone(),
                    face_id: side_face_id.clone(),
                    ordinal: 1,
                    uv0: None,
                    normal: None,
                    tangent: None,
                    seam: false,
                },
            );
            child_topology.corners.insert(
                top_side_corner_id.clone(),
                AuthoringMeshCorner {
                    corner_id: top_side_corner_id.clone(),
                    half_edge_id: top_side_id.clone(),
                    vertex_id: top_vertex_ids[index].clone(),
                    face_id: side_face_id.clone(),
                    ordinal: 2,
                    uv0: None,
                    normal: None,
                    tangent: None,
                    seam: false,
                },
            );
            child_topology.corners.insert(
                back_corner_id.clone(),
                AuthoringMeshCorner {
                    corner_id: back_corner_id.clone(),
                    half_edge_id: vertical_back_id.clone(),
                    vertex_id: top_vertex_ids[next].clone(),
                    face_id: side_face_id,
                    ordinal: 3,
                    uv0: None,
                    normal: None,
                    tangent: None,
                    seam: false,
                },
            );
            side_half_edges.push([
                outer_half_edge_id,
                vertical_forward_id,
                top_side_id,
                vertical_back_id,
            ]);
            side_corners.push([
                outer_corner_id,
                forward_corner_id,
                top_side_corner_id,
                back_corner_id,
            ]);

            let top_half_edge_id = AuthoringMeshHalfEdgeId(stable_id(
                "he",
                self.record.lineage_id.as_ref(),
                "face-extrude-top-face-half-edge",
                &json!({"parent_revision_id":self.record.revision_id,"operation_lineage_sha256":request.operation_lineage_sha256,"source_edge_id":source_half_edge.edge_id}),
            ));
            let top_corner_id = AuthoringMeshCornerId(stable_id(
                "c",
                self.record.lineage_id.as_ref(),
                "face-extrude-top-face-corner",
                &json!({"face_id":top_face_id,"source_edge_id":source_half_edge.edge_id}),
            ));
            if child_topology.half_edges.contains_key(&top_half_edge_id)
                || child_topology.corners.contains_key(&top_corner_id)
            {
                return Err(invalid(
                    "face extrude would reuse a top half-edge/corner ID",
                ));
            }
            child_topology.half_edges.insert(
                top_half_edge_id.clone(),
                AuthoringMeshHalfEdge {
                    half_edge_id: top_half_edge_id.clone(),
                    origin_vertex_id: top_vertex_ids[next].clone(),
                    edge_id: top_edge_ids[index].clone(),
                    face_id: top_face_id.clone(),
                    corner_id: top_corner_id.clone(),
                    next_id: top_half_edge_id.clone(),
                    prev_id: top_half_edge_id.clone(),
                    twin_id: None,
                    boundary: false,
                },
            );
            child_topology.corners.insert(
                top_corner_id.clone(),
                AuthoringMeshCorner {
                    corner_id: top_corner_id.clone(),
                    half_edge_id: top_half_edge_id.clone(),
                    vertex_id: top_vertex_ids[next].clone(),
                    face_id: top_face_id.clone(),
                    ordinal: index as u32,
                    uv0: None,
                    normal: None,
                    tangent: None,
                    seam: false,
                },
            );
            top_half_edges.push(top_half_edge_id);
            top_corners.push(top_corner_id);
            mark_ref(
                &mut generated,
                AuthoringMeshElementKind::HalfEdge,
                side_half_edges[index][0].0.clone(),
            );
            for half_edge_id in &side_half_edges[index][1..] {
                mark_ref(
                    &mut generated,
                    AuthoringMeshElementKind::HalfEdge,
                    half_edge_id.0.clone(),
                );
            }
            for corner_id in &side_corners[index] {
                mark_ref(
                    &mut generated,
                    AuthoringMeshElementKind::Corner,
                    corner_id.0.clone(),
                );
            }
            mark_ref(
                &mut generated,
                AuthoringMeshElementKind::HalfEdge,
                top_half_edges[index].0.clone(),
            );
            mark_ref(
                &mut generated,
                AuthoringMeshElementKind::Corner,
                top_corners[index].0.clone(),
            );
        }

        // The source face remains the base.  Add the side cycles and reverse
        // the top cycle so every shared edge has opposing half-edge winding.
        let top_face_cycle = top_half_edges.iter().rev().cloned().collect::<Vec<_>>();
        for index in 0..top_face_cycle.len() {
            let half_edge_id = &top_face_cycle[index];
            let next_id = top_face_cycle[(index + 1) % top_face_cycle.len()].clone();
            let prev_id =
                top_face_cycle[(index + top_face_cycle.len() - 1) % top_face_cycle.len()].clone();
            let half_edge = child_topology
                .half_edges
                .get_mut(half_edge_id)
                .expect("top face half-edge exists");
            half_edge.next_id = next_id;
            half_edge.prev_id = prev_id;
        }
        child_topology
            .faces
            .get_mut(&request.face_id)
            .expect("source face exists")
            .boundary = false;
        child_topology
            .loops
            .get_mut(&source_face.loop_id)
            .expect("source loop exists")
            .boundary = false;
        child_topology
            .faces
            .get_mut(&top_face_id)
            .expect("top face exists")
            .half_edge_ids = top_face_cycle.clone();
        child_topology
            .loops
            .get_mut(&top_loop_id)
            .expect("top loop exists")
            .half_edge_ids = top_face_cycle;
        for index in 0..count {
            child_topology
                .faces
                .get_mut(&side_face_ids[index])
                .expect("side face exists")
                .half_edge_ids = side_half_edges[index].to_vec();
            child_topology
                .loops
                .get_mut(&side_loop_ids[index])
                .expect("side loop exists")
                .half_edge_ids = side_half_edges[index].to_vec();
        }
        for half_edge_id in &source_face.half_edge_ids {
            mark_ref(
                &mut changed,
                AuthoringMeshElementKind::HalfEdge,
                half_edge_id.0.clone(),
            );
        }
        mark_ref(
            &mut changed,
            AuthoringMeshElementKind::Face,
            request.face_id.0.clone(),
        );
        mark_ref(
            &mut changed,
            AuthoringMeshElementKind::Loop,
            source_face.loop_id.0.clone(),
        );
        for source_half_edge in &source_half_edges {
            mark_ref(
                &mut changed,
                AuthoringMeshElementKind::Edge,
                source_half_edge.edge_id.0.clone(),
            );
        }
        for ring in parent_topology.rings.values() {
            if ring.edge_ids.iter().any(|edge_id| {
                source_half_edges
                    .iter()
                    .any(|half_edge| &half_edge.edge_id == edge_id)
            }) {
                mark_ref(
                    &mut changed,
                    AuthoringMeshElementKind::Ring,
                    ring.ring_id.0.clone(),
                );
            }
        }

        rebuild_edge_incidence(&mut child_topology)?;
        rebuild_twins(&mut child_topology)?;
        rebuild_boundary_rings(&mut child_topology, self.record.lineage_id.as_ref())?;
        validate_topology(&child_topology)?;
        verify_locality(&parent_topology, &child_topology, &changed)?;

        let generated_refs = refs_from_set(&generated);
        let changed_refs = refs_from_set(&changed);
        let operation = face_extrude_operation_record(
            request.operation_id,
            request.face_id.clone(),
            request.operation_lineage_sha256.clone(),
            self.record.revision_id.clone(),
            generated_refs.clone(),
        );
        let original = original_record(&self.record.lineage_id, child_topology)?;
        let child_revision_id = AuthoringMeshRevisionId(stable_id(
            "amrev",
            self.record.lineage_id.as_ref(),
            "face-extrude-revision",
            &json!({
                "parent_revision_id": self.record.revision_id,
                "operation_lineage_sha256": request.operation_lineage_sha256,
                "original_sha256": original.canonical_sha256,
            }),
        ));
        let child_record = revision_record(
            self.record.mesh_id.clone(),
            self.record.lineage_id.clone(),
            child_revision_id,
            vec![self.record.revision_id.clone()],
            self.record.revision_index + 1,
            Some(operation),
            original,
            None,
            self.record.source_binding.clone(),
            self.record.foundation_source_binding.clone(),
        );
        Ok(AuthoringMeshFaceExtrudeResult {
            schema_version: AUTHORING_MESH_V2_REVISION_SCHEMA_VERSION.to_owned(),
            parent_revision_id: self.record.revision_id.clone(),
            child_revision: child_record,
            changed_elements: changed_refs,
            generated_elements: generated_refs,
            retired_elements: Vec::new(),
            locality_status: "face-extrude-local-shell-preserves-unaffected-records@2".to_owned(),
            evaluated_status: if self.record.evaluated.is_some() {
                "evaluated-sidecar-invalidated-requires-new-evaluation@2".to_owned()
            } else {
                "evaluated-sidecar-not-present@2".to_owned()
            },
        })
    }

    /// Apply the single bounded topology edit needed to express the current
    /// rear-stock open frame: a centered, local -Y connected notch extruded
    /// through the local-Z span of an axis-aligned closed box.  The operation
    /// is deliberately specialized rather than exposing a general Boolean or
    /// caller-supplied mesh.  It replaces the four box faces intersected by
    /// the notch with triangle/quad-compatible U-extrusion faces, preserving
    /// the two end-cap face identities and all mesh/source lineage.
    pub(crate) fn open_frame_notch(
        &self,
        request: AuthoringMeshOpenFrameNotchRequest,
    ) -> Result<AuthoringMeshOpenFrameNotchResult, RuntimeError> {
        if request.parent_revision_id != self.record.revision_id {
            return Err(invalid(
                "open frame notch parent revision does not match the receiver",
            ));
        }
        checked_id(&request.operation_id, "operation_id")?;
        checked_sha(
            &request.operation_lineage_sha256,
            "operation_lineage_sha256",
        )?;
        if !(1..=999).contains(&request.opening_width_milli)
            || !(1..=999).contains(&request.opening_height_milli)
        {
            return Err(invalid(
                "open frame notch normalized width/height must be between 1 and 999 milli",
            ));
        }

        let parent_topology = topology_from_original(&self.record.original)?;
        validate_topology(&parent_topology)?;
        if parent_topology.vertices.len() != 8
            || parent_topology.edges.len() != 12
            || parent_topology.half_edges.len() != 24
            || parent_topology.corners.len() != 24
            || parent_topology.faces.len() != 6
            || parent_topology.loops.len() != 6
            || !parent_topology.tombstones.is_empty()
            || !parent_topology.rings.is_empty()
            || parent_topology.edges.values().any(|edge| edge.boundary)
        {
            return Err(invalid(
                "open frame notch requires an untombstoned closed six-face box",
            ));
        }
        if parent_topology
            .faces
            .values()
            .any(|face| face.half_edge_ids.len() != 4)
        {
            return Err(invalid("open frame notch requires six quad source faces"));
        }

        let mut minimum = [f64::INFINITY; 3];
        let mut maximum = [f64::NEG_INFINITY; 3];
        for vertex in parent_topology.vertices.values() {
            for axis in 0..3 {
                minimum[axis] = minimum[axis].min(vertex.position_m[axis]);
                maximum[axis] = maximum[axis].max(vertex.position_m[axis]);
            }
        }
        for axis in 0..3 {
            if !minimum[axis].is_finite()
                || !maximum[axis].is_finite()
                || maximum[axis] - minimum[axis] <= MIN_EDGE_LENGTH_M
            {
                return Err(invalid(
                    "open frame notch source box axis span is outside bounds",
                ));
            }
        }
        let axis_side = |value: f64, axis: usize| -> Result<i8, RuntimeError> {
            let tolerance = 1.0e-7_f64.max((maximum[axis] - minimum[axis]) * 1.0e-7);
            if (value - minimum[axis]).abs() <= tolerance {
                Ok(-1)
            } else if (value - maximum[axis]).abs() <= tolerance {
                Ok(1)
            } else {
                Err(invalid(
                    "open frame notch source is not an axis-aligned box",
                ))
            }
        };

        let mut box_vertices = BTreeMap::<(i8, i8, i8), AuthoringMeshVertexId>::new();
        for vertex in parent_topology.vertices.values() {
            let key = (
                axis_side(vertex.position_m[0], 0)?,
                axis_side(vertex.position_m[1], 1)?,
                axis_side(vertex.position_m[2], 2)?,
            );
            if box_vertices.insert(key, vertex.vertex_id.clone()).is_some() {
                return Err(invalid("open frame notch source box has duplicate corners"));
            }
        }
        if box_vertices.len() != 8 {
            return Err(invalid(
                "open frame notch source box does not contain all eight corners",
            ));
        }
        let box_vertex = |x: i8, y: i8, z: i8| -> Result<AuthoringMeshVertexId, RuntimeError> {
            box_vertices
                .get(&(x, y, z))
                .cloned()
                .ok_or_else(|| invalid("open frame notch source box corner is missing"))
        };

        // Resolve the six source planes from geometry rather than trusting
        // array order.  Genesis currently emits ordinals 0..5, but resolving
        // the planes keeps this operation source-bound after a position-only
        // authoring child.
        let mut plane_faces = BTreeMap::<(usize, i8), AuthoringMeshFaceId>::new();
        for face in parent_topology.faces.values() {
            let positions = face
                .half_edge_ids
                .iter()
                .map(|half_edge_id| {
                    parent_topology
                        .half_edges
                        .get(half_edge_id)
                        .and_then(|half_edge| {
                            parent_topology.vertices.get(&half_edge.origin_vertex_id)
                        })
                        .map(|vertex| vertex.position_m)
                        .ok_or_else(|| invalid("open frame notch source face vertex is missing"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut constant_axes = Vec::new();
            for axis in 0..3 {
                if positions
                    .iter()
                    .all(|position| (position[axis] - positions[0][axis]).abs() <= 1.0e-7)
                {
                    constant_axes.push(axis);
                }
            }
            if constant_axes.len() != 1 {
                return Err(invalid(
                    "open frame notch source face is not one box side plane",
                ));
            }
            let axis = constant_axes[0];
            let side = axis_side(positions[0][axis], axis)?;
            if plane_faces
                .insert((axis, side), face.face_id.clone())
                .is_some()
            {
                return Err(invalid(
                    "open frame notch source box has duplicate side faces",
                ));
            }
        }
        if plane_faces.len() != 6 {
            return Err(invalid(
                "open frame notch source box does not expose six side planes",
            ));
        }
        let source_face_ids = [
            plane_faces
                .get(&(2, -1))
                .cloned()
                .ok_or_else(|| invalid("open frame notch z-minus face is missing"))?,
            plane_faces
                .get(&(2, 1))
                .cloned()
                .ok_or_else(|| invalid("open frame notch z-plus face is missing"))?,
            plane_faces
                .get(&(1, -1))
                .cloned()
                .ok_or_else(|| invalid("open frame notch y-minus face is missing"))?,
            plane_faces
                .get(&(1, 1))
                .cloned()
                .ok_or_else(|| invalid("open frame notch y-plus face is missing"))?,
        ];
        let _preserved_face_ids = [
            plane_faces
                .get(&(0, -1))
                .cloned()
                .ok_or_else(|| invalid("open frame notch x-minus face is missing"))?,
            plane_faces
                .get(&(0, 1))
                .cloned()
                .ok_or_else(|| invalid("open frame notch x-plus face is missing"))?,
        ];

        let source_width = maximum[0] - minimum[0];
        let source_height = maximum[1] - minimum[1];
        let opening_width = source_width * request.opening_width_milli as f64 / 1000.0;
        let opening_height = source_height * request.opening_height_milli as f64 / 1000.0;
        let opening_x_min = (minimum[0] + maximum[0] - opening_width) * 0.5;
        let opening_x_max = opening_x_min + opening_width;
        let opening_y_max = minimum[1] + opening_height;
        if opening_x_max - opening_x_min <= MIN_EDGE_LENGTH_M
            || opening_y_max - minimum[1] <= MIN_EDGE_LENGTH_M
            || maximum[0] - opening_x_max <= MIN_EDGE_LENGTH_M
            || opening_x_min - minimum[0] <= MIN_EDGE_LENGTH_M
            || maximum[1] - opening_y_max <= MIN_EDGE_LENGTH_M
        {
            return Err(invalid(
                "open frame notch leaves an edge below the minimum geometric margin",
            ));
        }

        let mut child_topology = parent_topology.clone();
        let mut changed = BTreeSet::<(AuthoringMeshElementKind, String)>::new();
        let mut generated = BTreeSet::<(AuthoringMeshElementKind, String)>::new();
        let mut retired = BTreeSet::<(AuthoringMeshElementKind, String)>::new();
        let mut tombstones = Vec::new();

        // Retire the four box faces intersected by the through-notch.  The
        // x-minus/x-plus end caps stay in place and therefore retain their
        // face, loop, half-edge, corner and semantic source identity.
        for face_id in &source_face_ids {
            let face = child_topology
                .faces
                .remove(face_id)
                .ok_or_else(|| invalid("open frame notch source face disappeared"))?;
            mark_ref(
                &mut changed,
                AuthoringMeshElementKind::Face,
                face.face_id.0.clone(),
            );
            retire_ref(
                &mut retired,
                &mut tombstones,
                AuthoringMeshElementKind::Face,
                face.face_id.0.clone(),
                self.record.revision_index + 1,
                &request.operation_lineage_sha256,
                "open_frame_notch replaced the source side face",
            );
            let loop_record = child_topology
                .loops
                .remove(&face.loop_id)
                .ok_or_else(|| invalid("open frame notch source loop is missing"))?;
            mark_ref(
                &mut changed,
                AuthoringMeshElementKind::Loop,
                loop_record.loop_id.0.clone(),
            );
            retire_ref(
                &mut retired,
                &mut tombstones,
                AuthoringMeshElementKind::Loop,
                loop_record.loop_id.0.clone(),
                self.record.revision_index + 1,
                &request.operation_lineage_sha256,
                "open_frame_notch replaced the source face loop",
            );
            for half_edge_id in face.half_edge_ids {
                let half_edge = child_topology
                    .half_edges
                    .remove(&half_edge_id)
                    .ok_or_else(|| invalid("open frame notch source half-edge is missing"))?;
                mark_ref(
                    &mut changed,
                    AuthoringMeshElementKind::HalfEdge,
                    half_edge.half_edge_id.0.clone(),
                );
                retire_ref(
                    &mut retired,
                    &mut tombstones,
                    AuthoringMeshElementKind::HalfEdge,
                    half_edge.half_edge_id.0.clone(),
                    self.record.revision_index + 1,
                    &request.operation_lineage_sha256,
                    "open_frame_notch replaced the source face half-edge",
                );
                let corner = child_topology
                    .corners
                    .remove(&half_edge.corner_id)
                    .ok_or_else(|| invalid("open frame notch source corner is missing"))?;
                mark_ref(
                    &mut changed,
                    AuthoringMeshElementKind::Corner,
                    corner.corner_id.0.clone(),
                );
                retire_ref(
                    &mut retired,
                    &mut tombstones,
                    AuthoringMeshElementKind::Corner,
                    corner.corner_id.0.clone(),
                    self.record.revision_index + 1,
                    &request.operation_lineage_sha256,
                    "open_frame_notch replaced the source face corner",
                );
            }
        }

        // Remove only edges that became orphaned.  The eight end-cap edges
        // remain stable and will receive new opposing half-edges below.
        let referenced_edges = child_topology
            .half_edges
            .values()
            .map(|half_edge| half_edge.edge_id.clone())
            .collect::<BTreeSet<_>>();
        let orphan_edges = child_topology
            .edges
            .keys()
            .filter(|edge_id| !referenced_edges.contains(*edge_id))
            .cloned()
            .collect::<Vec<_>>();
        for edge_id in orphan_edges {
            child_topology
                .edges
                .remove(&edge_id)
                .ok_or_else(|| invalid("open frame notch orphan edge is missing"))?;
            mark_ref(
                &mut changed,
                AuthoringMeshElementKind::Edge,
                edge_id.0.clone(),
            );
            retire_ref(
                &mut retired,
                &mut tombstones,
                AuthoringMeshElementKind::Edge,
                edge_id.0.clone(),
                self.record.revision_index + 1,
                &request.operation_lineage_sha256,
                "open_frame_notch replaced the source edge",
            );
        }

        // Existing end-cap half-edges and edges change their incidence/twin
        // binding when the four source faces are replaced.  Mark them as
        // touched while preserving their IDs and records otherwise.
        for half_edge_id in parent_topology.half_edges.keys() {
            if child_topology.half_edges.contains_key(half_edge_id) {
                mark_ref(
                    &mut changed,
                    AuthoringMeshElementKind::HalfEdge,
                    half_edge_id.0.clone(),
                );
            }
        }
        for edge_id in parent_topology.edges.keys() {
            if child_topology.edges.contains_key(edge_id) {
                mark_ref(
                    &mut changed,
                    AuthoringMeshElementKind::Edge,
                    edge_id.0.clone(),
                );
            }
        }

        let mut notch_vertices = BTreeMap::<(i8, u8, u8), AuthoringMeshVertexId>::new();
        let x_positions = [opening_x_min, opening_x_max];
        let y_positions = [minimum[1], opening_y_max, maximum[1]];
        for z_side in [-1_i8, 1_i8] {
            for x_slot in 0_u8..2 {
                for y_slot in 0_u8..3 {
                    let vertex_id = AuthoringMeshVertexId(stable_id(
                        "v",
                        self.record.lineage_id.as_ref(),
                        "open-frame-notch-vertex",
                        &json!({
                            "parent_revision_id":self.record.revision_id,
                            "operation_lineage_sha256":request.operation_lineage_sha256,
                            "z_side":z_side,
                            "x_slot":x_slot,
                            "y_slot":y_slot,
                        }),
                    ));
                    if child_topology.vertices.contains_key(&vertex_id)
                        || child_topology
                            .edges
                            .contains_key(&AuthoringMeshEdgeId(vertex_id.0.clone()))
                    {
                        return Err(invalid(
                            "open frame notch would reuse an active stable vertex ID",
                        ));
                    }
                    let position = [
                        x_positions[x_slot as usize],
                        y_positions[y_slot as usize],
                        if z_side < 0 { minimum[2] } else { maximum[2] },
                    ];
                    finite_position(position, "open frame notch generated vertex")?;
                    child_topology.vertices.insert(
                        vertex_id.clone(),
                        AuthoringMeshVertex {
                            vertex_id: vertex_id.clone(),
                            position_m: position,
                        },
                    );
                    notch_vertices.insert((z_side, x_slot, y_slot), vertex_id.clone());
                    mark_ref(
                        &mut generated,
                        AuthoringMeshElementKind::Vertex,
                        vertex_id.0.clone(),
                    );
                }
            }
        }
        let notch_vertex =
            |z_side: i8, x_slot: u8, y_slot: u8| -> Result<AuthoringMeshVertexId, RuntimeError> {
                notch_vertices
                    .get(&(z_side, x_slot, y_slot))
                    .cloned()
                    .ok_or_else(|| invalid("open frame notch generated corner is missing"))
            };

        let mut face_ordinal = 0_usize;
        let mut add_face = |vertices: Vec<AuthoringMeshVertexId>| -> Result<(), RuntimeError> {
            insert_open_frame_notch_face(
                &mut child_topology,
                self.record.lineage_id.as_ref(),
                &self.record.revision_id,
                &request.operation_lineage_sha256,
                face_ordinal,
                vertices,
                &mut generated,
            )?;
            face_ordinal += 1;
            Ok(())
        };

        // z-minus side: split each leg at the notch roof vertex so every
        // shared edge has identical endpoints (no T-junction boundary ring).
        add_face(vec![
            box_vertex(-1, -1, -1)?,
            box_vertex(-1, 1, -1)?,
            notch_vertex(-1, 0, 1)?,
        ])?;
        add_face(vec![
            box_vertex(-1, -1, -1)?,
            notch_vertex(-1, 0, 1)?,
            notch_vertex(-1, 0, 0)?,
        ])?;
        add_face(vec![
            box_vertex(-1, 1, -1)?,
            notch_vertex(-1, 0, 2)?,
            notch_vertex(-1, 0, 1)?,
        ])?;
        add_face(vec![
            notch_vertex(-1, 0, 1)?,
            notch_vertex(-1, 0, 2)?,
            notch_vertex(-1, 1, 2)?,
            notch_vertex(-1, 1, 1)?,
        ])?;
        add_face(vec![
            notch_vertex(-1, 1, 0)?,
            notch_vertex(-1, 1, 1)?,
            box_vertex(1, -1, -1)?,
        ])?;
        add_face(vec![
            notch_vertex(-1, 1, 1)?,
            box_vertex(1, 1, -1)?,
            box_vertex(1, -1, -1)?,
        ])?;
        add_face(vec![
            notch_vertex(-1, 1, 1)?,
            notch_vertex(-1, 1, 2)?,
            box_vertex(1, 1, -1)?,
        ])?;

        // z-plus side uses the same split with reversed winding.
        add_face(vec![
            box_vertex(-1, -1, 1)?,
            notch_vertex(1, 0, 0)?,
            notch_vertex(1, 0, 1)?,
        ])?;
        add_face(vec![
            box_vertex(-1, -1, 1)?,
            notch_vertex(1, 0, 1)?,
            box_vertex(-1, 1, 1)?,
        ])?;
        add_face(vec![
            box_vertex(-1, 1, 1)?,
            notch_vertex(1, 0, 1)?,
            notch_vertex(1, 0, 2)?,
        ])?;
        add_face(vec![
            notch_vertex(1, 0, 1)?,
            notch_vertex(1, 1, 1)?,
            notch_vertex(1, 1, 2)?,
            notch_vertex(1, 0, 2)?,
        ])?;
        add_face(vec![
            notch_vertex(1, 1, 0)?,
            box_vertex(1, -1, 1)?,
            notch_vertex(1, 1, 1)?,
        ])?;
        add_face(vec![
            box_vertex(1, -1, 1)?,
            box_vertex(1, 1, 1)?,
            notch_vertex(1, 1, 1)?,
        ])?;
        add_face(vec![
            notch_vertex(1, 1, 1)?,
            box_vertex(1, 1, 1)?,
            notch_vertex(1, 1, 2)?,
        ])?;

        // Bottom strips (outward -Y), split around the opening.
        add_face(vec![
            box_vertex(-1, -1, -1)?,
            notch_vertex(-1, 0, 0)?,
            notch_vertex(1, 0, 0)?,
            box_vertex(-1, -1, 1)?,
        ])?;
        add_face(vec![
            notch_vertex(-1, 1, 0)?,
            box_vertex(1, -1, -1)?,
            box_vertex(1, -1, 1)?,
            notch_vertex(1, 1, 0)?,
        ])?;

        // Top surface (outward +Y), split into three quads to retain the
        // fixed Worker triangle/quad lowering policy.
        add_face(vec![
            box_vertex(-1, 1, -1)?,
            box_vertex(-1, 1, 1)?,
            notch_vertex(1, 0, 2)?,
            notch_vertex(-1, 0, 2)?,
        ])?;
        add_face(vec![
            notch_vertex(-1, 0, 2)?,
            notch_vertex(1, 0, 2)?,
            notch_vertex(1, 1, 2)?,
            notch_vertex(-1, 1, 2)?,
        ])?;
        add_face(vec![
            notch_vertex(-1, 1, 2)?,
            notch_vertex(1, 1, 2)?,
            box_vertex(1, 1, 1)?,
            box_vertex(1, 1, -1)?,
        ])?;

        // Concave notch boundary: two vertical walls and the roof.
        add_face(vec![
            notch_vertex(-1, 0, 0)?,
            notch_vertex(-1, 0, 1)?,
            notch_vertex(1, 0, 1)?,
            notch_vertex(1, 0, 0)?,
        ])?;
        add_face(vec![
            notch_vertex(-1, 1, 0)?,
            notch_vertex(1, 1, 0)?,
            notch_vertex(1, 1, 1)?,
            notch_vertex(-1, 1, 1)?,
        ])?;
        add_face(vec![
            notch_vertex(-1, 0, 1)?,
            notch_vertex(-1, 1, 1)?,
            notch_vertex(1, 1, 1)?,
            notch_vertex(1, 0, 1)?,
        ])?;

        rebuild_edge_incidence(&mut child_topology)?;
        rebuild_twins(&mut child_topology)?;
        rebuild_boundary_rings(&mut child_topology, self.record.lineage_id.as_ref())?;
        child_topology.tombstones.extend(tombstones.clone());
        validate_topology(&child_topology)?;
        verify_locality(&parent_topology, &child_topology, &changed)?;

        let generated_refs = refs_from_set(&generated);
        let retired_refs = refs_from_set(&retired);
        let changed_refs = refs_from_set(&changed);
        let operation = open_frame_notch_operation_record(
            request.operation_id,
            source_face_ids.to_vec(),
            request.opening_width_milli,
            request.opening_height_milli,
            request.operation_lineage_sha256.clone(),
            self.record.revision_id.clone(),
            generated_refs.clone(),
            retired_refs.clone(),
            tombstones,
        );
        let original = original_record(&self.record.lineage_id, child_topology)?;
        let child_revision_id = AuthoringMeshRevisionId(stable_id(
            "amrev",
            self.record.lineage_id.as_ref(),
            "open-frame-notch-revision",
            &json!({
                "parent_revision_id":self.record.revision_id,
                "operation_lineage_sha256":request.operation_lineage_sha256,
                "opening_width_milli":request.opening_width_milli,
                "opening_height_milli":request.opening_height_milli,
                "original_sha256":original.canonical_sha256,
            }),
        ));
        let child_record = revision_record(
            self.record.mesh_id.clone(),
            self.record.lineage_id.clone(),
            child_revision_id,
            vec![self.record.revision_id.clone()],
            self.record.revision_index + 1,
            Some(operation),
            original,
            None,
            self.record.source_binding.clone(),
            self.record.foundation_source_binding.clone(),
        );
        Ok(AuthoringMeshOpenFrameNotchResult {
            schema_version: AUTHORING_MESH_V2_REVISION_SCHEMA_VERSION.to_owned(),
            parent_revision_id: self.record.revision_id.clone(),
            child_revision: child_record,
            opening_width_milli: request.opening_width_milli,
            opening_height_milli: request.opening_height_milli,
            changed_elements: changed_refs,
            generated_elements: generated_refs,
            retired_elements: retired_refs,
            locality_status:
                "open-frame-notch-local-closed-u-extrusion-preserves-endcaps-and-lineage@1"
                    .to_owned(),
            evaluated_status: if self.record.evaluated.is_some() {
                "evaluated-sidecar-invalidated-requires-new-evaluation@2".to_owned()
            } else {
                "evaluated-sidecar-not-present@2".to_owned()
            },
        })
    }

    /// Sculpt only the rear-stock upper rail's void-facing longitudinal edge
    /// pair. The five stations and offsets are product owned; callers provide
    /// semantic orientation evidence, never vertex IDs or replacement mesh.
    pub(crate) fn rear_stock_void_rail_bow(
        &self,
        request: AuthoringMeshRearStockVoidRailBowRequest,
    ) -> Result<AuthoringMeshRearStockVoidRailBowResult, RuntimeError> {
        if request.parent_revision_id != self.record.revision_id {
            return Err(invalid("rear-stock rail-bow parent revision differs"));
        }
        checked_id(&request.operation_id, "operation_id")?;
        checked_sha(
            &request.operation_lineage_sha256,
            "operation_lineage_sha256",
        )?;
        finite_position(request.expected_void_centroid_m, "expected_void_centroid_m")?;
        finite_position(
            request.expected_void_face_normal_m,
            "expected_void_face_normal_m",
        )?;

        let normal_length = request
            .expected_void_face_normal_m
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt();
        if normal_length < 0.999 || normal_length > 1.001 {
            return Err(invalid(
                "rear-stock rail-bow face normal must be normalized",
            ));
        }
        let void_axis = (0..3)
            .max_by(|left, right| {
                request.expected_void_face_normal_m[*left]
                    .abs()
                    .total_cmp(&request.expected_void_face_normal_m[*right].abs())
            })
            .expect("three axes");
        if request.expected_void_face_normal_m[void_axis].abs() < 0.999
            || (0..3).any(|axis| {
                axis != void_axis && request.expected_void_face_normal_m[axis].abs() > 0.001
            })
        {
            return Err(invalid(
                "rear-stock rail-bow face normal must select one source-local axis",
            ));
        }

        let parent_topology = topology_from_original(&self.record.original)?;
        let mut minimum = [f64::INFINITY; 3];
        let mut maximum = [f64::NEG_INFINITY; 3];
        for vertex in parent_topology.vertices.values() {
            for axis in 0..3 {
                minimum[axis] = minimum[axis].min(vertex.position_m[axis]);
                maximum[axis] = maximum[axis].max(vertex.position_m[axis]);
            }
        }
        let spans = [
            maximum[0] - minimum[0],
            maximum[1] - minimum[1],
            maximum[2] - minimum[2],
        ];
        let longitudinal_axis = (0..3)
            .filter(|axis| *axis != void_axis)
            .max_by(|left, right| spans[*left].total_cmp(&spans[*right]))
            .ok_or_else(|| invalid("rear-stock rail-bow longitudinal axis is unavailable"))?;
        let depth_axis = (0..3)
            .find(|axis| *axis != void_axis && *axis != longitudinal_axis)
            .expect("three distinct axes");
        if spans[longitudinal_axis] <= MIN_EDGE_LENGTH_M
            || spans[void_axis] <= REAR_STOCK_VOID_RAIL_BOW_OFFSETS_M[2]
            || spans[depth_axis] <= MIN_EDGE_LENGTH_M
        {
            return Err(invalid(
                "rear-stock rail-bow source envelope is outside bounds",
            ));
        }
        let normal_sign = request.expected_void_face_normal_m[void_axis].signum();
        let void_face_coordinate = if normal_sign < 0.0 {
            minimum[void_axis]
        } else {
            maximum[void_axis]
        };
        if (request.expected_void_centroid_m[void_axis] - void_face_coordinate) * normal_sign
            <= MIN_EDGE_LENGTH_M
        {
            return Err(invalid(
                "rear-stock rail-bow centroid is not beyond the selected void-facing boundary",
            ));
        }
        let tolerance = spans.iter().copied().fold(1.0_f64, f64::max) * 1.0e-7;
        let mut source_edges = parent_topology
            .edges
            .iter()
            .filter_map(|(edge_id, edge)| {
                let left = &parent_topology.vertices[&edge.vertex_ids[0]];
                let right = &parent_topology.vertices[&edge.vertex_ids[1]];
                let on_void_face = (left.position_m[void_axis] - void_face_coordinate).abs()
                    <= tolerance
                    && (right.position_m[void_axis] - void_face_coordinate).abs() <= tolerance;
                let spans_longitudinal = (left.position_m[longitudinal_axis]
                    - minimum[longitudinal_axis])
                    .abs()
                    <= tolerance
                    && (right.position_m[longitudinal_axis] - maximum[longitudinal_axis]).abs()
                        <= tolerance
                    || (right.position_m[longitudinal_axis] - minimum[longitudinal_axis]).abs()
                        <= tolerance
                        && (left.position_m[longitudinal_axis] - maximum[longitudinal_axis]).abs()
                            <= tolerance;
                let on_depth_boundary =
                    (left.position_m[depth_axis] - right.position_m[depth_axis]).abs() <= tolerance
                        && ((left.position_m[depth_axis] - minimum[depth_axis]).abs() <= tolerance
                            || (left.position_m[depth_axis] - maximum[depth_axis]).abs()
                                <= tolerance);
                (on_void_face && spans_longitudinal && on_depth_boundary).then(|| edge_id.clone())
            })
            .collect::<Vec<_>>();
        source_edges.sort();
        if source_edges.len() != 2 {
            return Err(invalid(
                "rear-stock rail-bow requires exactly two depth-symmetric longitudinal edges",
            ));
        }

        // Splitting only the two void-facing rails leaves the three adjacent
        // box faces as 7/10-gons.  Derive the matching pair on the opposite
        // void-plane as support edges and split them at the same stations.
        // Those support points let the face-split primitive emit an all-quad
        // strip on every longitudinal box face while the bow itself remains
        // confined to the two source rails.
        let support_void_face_coordinate = if normal_sign < 0.0 {
            maximum[void_axis]
        } else {
            minimum[void_axis]
        };
        let mut support_edges = parent_topology
            .edges
            .iter()
            .filter_map(|(edge_id, edge)| {
                let left = &parent_topology.vertices[&edge.vertex_ids[0]];
                let right = &parent_topology.vertices[&edge.vertex_ids[1]];
                let on_support_face =
                    (left.position_m[void_axis] - support_void_face_coordinate).abs() <= tolerance
                        && (right.position_m[void_axis] - support_void_face_coordinate).abs()
                            <= tolerance;
                let spans_longitudinal = ((left.position_m[longitudinal_axis]
                    - minimum[longitudinal_axis])
                    .abs()
                    <= tolerance
                    && (right.position_m[longitudinal_axis] - maximum[longitudinal_axis]).abs()
                        <= tolerance)
                    || ((right.position_m[longitudinal_axis] - minimum[longitudinal_axis]).abs()
                        <= tolerance
                        && (left.position_m[longitudinal_axis] - maximum[longitudinal_axis]).abs()
                            <= tolerance);
                let on_depth_boundary =
                    (left.position_m[depth_axis] - right.position_m[depth_axis]).abs() <= tolerance
                        && ((left.position_m[depth_axis] - minimum[depth_axis]).abs() <= tolerance
                            || (left.position_m[depth_axis] - maximum[depth_axis]).abs()
                                <= tolerance);
                (on_support_face && spans_longitudinal && on_depth_boundary)
                    .then(|| edge_id.clone())
            })
            .collect::<Vec<_>>();
        support_edges.sort();
        if support_edges.len() != 2
            || support_edges
                .iter()
                .any(|edge_id| source_edges.binary_search(edge_id).is_ok())
        {
            return Err(invalid(
                "rear-stock rail-bow requires exactly two opposite support edges",
            ));
        }

        let mut worker_face_ids = BTreeSet::<AuthoringMeshFaceId>::new();
        for edge_id in source_edges.iter().chain(support_edges.iter()) {
            let edge = parent_topology
                .edges
                .get(edge_id)
                .ok_or_else(|| invalid("rear-stock rail-bow support edge is unavailable"))?;
            for half_edge_id in &edge.half_edge_ids {
                let half_edge = parent_topology
                    .half_edges
                    .get(half_edge_id)
                    .ok_or_else(|| {
                        invalid("rear-stock rail-bow support half-edge is unavailable")
                    })?;
                worker_face_ids.insert(half_edge.face_id.clone());
            }
        }

        let mut working = AuthoringMeshV2Revision::from_record(self.record.clone())?;
        let mut station_vertices = Vec::<(AuthoringMeshVertexId, f64, f64)>::new();
        for (edge_role, edge_ids) in [
            ("bow", source_edges.clone()),
            ("support", support_edges.clone()),
        ] {
            for (side_index, source_edge_id) in edge_ids.iter().enumerate() {
                let source_edge = &parent_topology.edges[source_edge_id];
                let far_vertex_id = source_edge
                    .vertex_ids
                    .iter()
                    .find(|vertex_id| {
                        (parent_topology.vertices[*vertex_id].position_m[longitudinal_axis]
                            - maximum[longitudinal_axis])
                            .abs()
                            <= tolerance
                    })
                    .cloned()
                    .ok_or_else(|| invalid("rear-stock rail-bow far endpoint is unavailable"))?;
                let mut active_edge_id = source_edge_id.clone();
                for station_index in 1..=3 {
                    let station = REAR_STOCK_VOID_RAIL_BOW_STATIONS[station_index];
                    let target = minimum[longitudinal_axis] + spans[longitudinal_axis] * station;
                    let topology = topology_from_original(&working.record.original)?;
                    let edge = topology
                        .edges
                        .get(&active_edge_id)
                        .ok_or_else(|| invalid("rear-stock rail-bow active edge is unavailable"))?;
                    let start =
                        topology.vertices[&edge.vertex_ids[0]].position_m[longitudinal_axis];
                    let end = topology.vertices[&edge.vertex_ids[1]].position_m[longitudinal_axis];
                    if (end - start).abs() <= MIN_EDGE_LENGTH_M {
                        return Err(invalid("rear-stock rail-bow active edge is degenerate"));
                    }
                    let ratio = (target - start) / (end - start);
                    let split_ratio_milli = (ratio * 1000.0).round() as u32;
                    if !(1..=999).contains(&split_ratio_milli) {
                        return Err(invalid("rear-stock rail-bow split ratio is outside bounds"));
                    }
                    let split_lineage_sha256 = canonical_json_hash(&json!({
                        "parent_operation_lineage_sha256":request.operation_lineage_sha256,
                        "edge_role":edge_role,
                        "side_index":side_index,
                        "station_index":station_index,
                    }));
                    let split_operation_id = format!("amop-{}", &split_lineage_sha256[..56]);
                    let split = working.split_edge(AuthoringMeshSplitEdgeRequest {
                        operation_id: split_operation_id,
                        parent_revision_id: working.record.revision_id.clone(),
                        edge_id: active_edge_id.clone(),
                        split_ratio_milli,
                        operation_lineage_sha256: split_lineage_sha256,
                    })?;
                    let midpoint_id = split
                        .generated_elements
                        .iter()
                        .find(|element| element.kind == AuthoringMeshElementKind::Vertex)
                        .map(|element| AuthoringMeshVertexId(element.id.clone()))
                        .ok_or_else(|| invalid("rear-stock rail-bow midpoint is unavailable"))?;
                    let next_topology = topology_from_original(&split.child_revision.original)?;
                    active_edge_id = split
                        .generated_elements
                        .iter()
                        .filter(|element| element.kind == AuthoringMeshElementKind::Edge)
                        .map(|element| AuthoringMeshEdgeId(element.id.clone()))
                        .find(|edge_id| {
                            next_topology.edges[edge_id]
                                .vertex_ids
                                .contains(&far_vertex_id)
                        })
                        .ok_or_else(|| {
                            invalid("rear-stock rail-bow continuation edge is unavailable")
                        })?;
                    station_vertices.push((
                        midpoint_id,
                        station,
                        if edge_role == "bow" {
                            REAR_STOCK_VOID_RAIL_BOW_OFFSETS_M[station_index]
                        } else {
                            0.0
                        },
                    ));
                    working = AuthoringMeshV2Revision::from_record(split.child_revision)?;
                }
            }
        }

        let mut child_topology = topology_from_original(&working.record.original)?;
        for (vertex_id, station, offset_m) in &station_vertices {
            let vertex = child_topology
                .vertices
                .get_mut(vertex_id)
                .ok_or_else(|| invalid("rear-stock rail-bow generated vertex is missing"))?;
            vertex.position_m[longitudinal_axis] =
                minimum[longitudinal_axis] + spans[longitudinal_axis] * station;
            vertex.position_m[void_axis] -= normal_sign * offset_m;
            finite_position(vertex.position_m, "rear-stock rail-bow vertex")?;
        }

        // Every station split adds one boundary point to each incident face.
        // Split each affected decagon into a deterministic sequence of quads
        // (and a triangle only for an odd source degree) using existing
        // vertices and Runtime-derived diagonals.  The box path above is all
        // even, so the normal result is a complete quad strip.
        let mut face_split_ordinal = 0_usize;
        for face_id in worker_face_ids {
            if child_topology
                .faces
                .get(&face_id)
                .is_some_and(|face| face.half_edge_ids.len() > 4)
            {
                split_face_into_worker_faces(
                    &mut child_topology,
                    self.record.lineage_id.as_ref(),
                    &self.record.revision_id,
                    &request.operation_lineage_sha256,
                    &face_id,
                    longitudinal_axis,
                    tolerance,
                    self.record.revision_index + 1,
                    &mut face_split_ordinal,
                    &mut BTreeSet::new(),
                    &mut BTreeSet::new(),
                )?;
            }
        }
        if child_topology
            .faces
            .values()
            .any(|face| face.half_edge_ids.len() > 4)
        {
            return Err(invalid(
                "rear-stock rail-bow left an active face outside the triangle/quad Worker policy",
            ));
        }
        validate_topology(&child_topology)?;

        let mut changed = BTreeSet::<(AuthoringMeshElementKind, String)>::new();
        for id in parent_topology.vertices.keys() {
            mark_ref(&mut changed, AuthoringMeshElementKind::Vertex, id.0.clone());
        }
        for id in parent_topology.edges.keys() {
            mark_ref(&mut changed, AuthoringMeshElementKind::Edge, id.0.clone());
        }
        for id in parent_topology.half_edges.keys() {
            mark_ref(
                &mut changed,
                AuthoringMeshElementKind::HalfEdge,
                id.0.clone(),
            );
        }
        for id in parent_topology.corners.keys() {
            mark_ref(&mut changed, AuthoringMeshElementKind::Corner, id.0.clone());
        }
        for id in parent_topology.faces.keys() {
            mark_ref(&mut changed, AuthoringMeshElementKind::Face, id.0.clone());
        }
        for id in parent_topology.loops.keys() {
            mark_ref(&mut changed, AuthoringMeshElementKind::Loop, id.0.clone());
        }
        for id in parent_topology.rings.keys() {
            mark_ref(&mut changed, AuthoringMeshElementKind::Ring, id.0.clone());
        }
        verify_locality(&parent_topology, &child_topology, &changed)?;

        // The internal edge/face split operations are an implementation detail
        // of one public atomic edit. Collapse their temporary revision indices
        // and lineages into the final child journal before serialization.
        // Otherwise durable rehydration would observe tombstones that point
        // beyond this single child revision.
        let parent_tombstone_keys = parent_topology
            .tombstones
            .iter()
            .map(|tombstone| (tombstone.element.kind.clone(), tombstone.element.id.clone()))
            .collect::<BTreeSet<_>>();
        let mut operation_tombstones = child_topology
            .tombstones
            .iter()
            .filter(|tombstone| {
                !parent_tombstone_keys
                    .contains(&(tombstone.element.kind.clone(), tombstone.element.id.clone()))
            })
            .map(|tombstone| AuthoringMeshV2Tombstone {
                element: tombstone.element.clone(),
                retired_revision_index: self.record.revision_index + 1,
                operation_lineage_sha256: request.operation_lineage_sha256.clone(),
                reason: "rear_stock_void_rail_bow replaced an internal split element".to_owned(),
            })
            .collect::<Vec<_>>();
        operation_tombstones.sort_by(|left, right| {
            left.element
                .kind
                .cmp(&right.element.kind)
                .then(left.element.id.cmp(&right.element.id))
        });
        operation_tombstones.dedup_by(|left, right| left.element == right.element);
        child_topology.tombstones = parent_topology.tombstones.clone();
        child_topology
            .tombstones
            .extend(operation_tombstones.iter().cloned());

        let mut parent_active = BTreeSet::<(AuthoringMeshElementKind, String)>::new();
        let mut child_active = BTreeSet::<(AuthoringMeshElementKind, String)>::new();
        for id in parent_topology.vertices.keys() {
            mark_ref(
                &mut parent_active,
                AuthoringMeshElementKind::Vertex,
                id.0.clone(),
            );
        }
        for id in child_topology.vertices.keys() {
            mark_ref(
                &mut child_active,
                AuthoringMeshElementKind::Vertex,
                id.0.clone(),
            );
        }
        for id in parent_topology.edges.keys() {
            mark_ref(
                &mut parent_active,
                AuthoringMeshElementKind::Edge,
                id.0.clone(),
            );
        }
        for id in child_topology.edges.keys() {
            mark_ref(
                &mut child_active,
                AuthoringMeshElementKind::Edge,
                id.0.clone(),
            );
        }
        for id in parent_topology.half_edges.keys() {
            mark_ref(
                &mut parent_active,
                AuthoringMeshElementKind::HalfEdge,
                id.0.clone(),
            );
        }
        for id in child_topology.half_edges.keys() {
            mark_ref(
                &mut child_active,
                AuthoringMeshElementKind::HalfEdge,
                id.0.clone(),
            );
        }
        for id in parent_topology.corners.keys() {
            mark_ref(
                &mut parent_active,
                AuthoringMeshElementKind::Corner,
                id.0.clone(),
            );
        }
        for id in child_topology.corners.keys() {
            mark_ref(
                &mut child_active,
                AuthoringMeshElementKind::Corner,
                id.0.clone(),
            );
        }
        for id in parent_topology.faces.keys() {
            mark_ref(
                &mut parent_active,
                AuthoringMeshElementKind::Face,
                id.0.clone(),
            );
        }
        for id in child_topology.faces.keys() {
            mark_ref(
                &mut child_active,
                AuthoringMeshElementKind::Face,
                id.0.clone(),
            );
        }
        for id in parent_topology.loops.keys() {
            mark_ref(
                &mut parent_active,
                AuthoringMeshElementKind::Loop,
                id.0.clone(),
            );
        }
        for id in child_topology.loops.keys() {
            mark_ref(
                &mut child_active,
                AuthoringMeshElementKind::Loop,
                id.0.clone(),
            );
        }
        for id in parent_topology.rings.keys() {
            mark_ref(
                &mut parent_active,
                AuthoringMeshElementKind::Ring,
                id.0.clone(),
            );
        }
        for id in child_topology.rings.keys() {
            mark_ref(
                &mut child_active,
                AuthoringMeshElementKind::Ring,
                id.0.clone(),
            );
        }
        let generated = child_active
            .difference(&parent_active)
            .cloned()
            .collect::<BTreeSet<_>>();
        let generated_refs = refs_from_set(&generated);
        let retired_refs = operation_tombstones
            .iter()
            .map(|tombstone| tombstone.element.clone())
            .collect::<Vec<_>>();
        let source_elements = source_edges
            .iter()
            .map(|edge_id| AuthoringMeshElementRef {
                kind: AuthoringMeshElementKind::Edge,
                id: edge_id.0.clone(),
            })
            .collect::<Vec<_>>();
        let operation = rear_stock_void_rail_bow_operation_record(
            request.operation_id,
            source_elements,
            request.operation_lineage_sha256.clone(),
            self.record.revision_id.clone(),
            generated_refs.clone(),
            retired_refs.clone(),
            operation_tombstones,
        );
        let original = original_record(&self.record.lineage_id, child_topology)?;
        let child_revision_id = AuthoringMeshRevisionId(stable_id(
            "amrev",
            self.record.lineage_id.as_ref(),
            "rear-stock-void-rail-bow-revision",
            &json!({
                "parent_revision_id":self.record.revision_id,
                "operation_lineage_sha256":request.operation_lineage_sha256,
                "expected_void_centroid_m":request.expected_void_centroid_m,
                "expected_void_face_normal_m":request.expected_void_face_normal_m,
                "original_sha256":original.canonical_sha256,
            }),
        ));
        let child_record = revision_record(
            self.record.mesh_id.clone(),
            self.record.lineage_id.clone(),
            child_revision_id,
            vec![self.record.revision_id.clone()],
            self.record.revision_index + 1,
            Some(operation),
            original,
            None,
            self.record.source_binding.clone(),
            self.record.foundation_source_binding.clone(),
        );
        AuthoringMeshV2Revision::from_record(child_record.clone())?;
        Ok(AuthoringMeshRearStockVoidRailBowResult {
            schema_version: AUTHORING_MESH_V2_REVISION_SCHEMA_VERSION.to_owned(),
            parent_revision_id: self.record.revision_id.clone(),
            child_revision: child_record,
            station_parameters_m: REAR_STOCK_VOID_RAIL_BOW_STATIONS
                .iter()
                .copied()
                .zip(REAR_STOCK_VOID_RAIL_BOW_OFFSETS_M.iter().copied())
                .map(|(station, offset)| [station, offset])
                .collect(),
            expected_void_centroid_m: request.expected_void_centroid_m,
            expected_void_face_normal_m: request.expected_void_face_normal_m,
            changed_elements: refs_from_set(&changed),
            generated_elements: generated_refs,
            retired_elements: retired_refs,
            locality_status:
                "rear-stock-void-rail-bow-upper-inner-chain-only-preserves-outer-envelope@1"
                    .to_owned(),
            evaluated_status: if self.record.evaluated.is_some() {
                "evaluated-sidecar-invalidated-requires-new-evaluation@2".to_owned()
            } else {
                "evaluated-sidecar-not-present@2".to_owned()
            },
        })
    }

    /// Bridge the rear-stock upper-inner boundary with a Runtime-owned,
    /// source-local five-station chain.  The public request deliberately has
    /// no selection, mesh, camera, mask, or transform fields.  This kernel
    /// only accepts the closed source box projection, discovers the two
    /// Y-min longitudinal rails and their opposite support rails, and then
    /// rebuilds the four adjacent faces into continuous triangle/quad strips.
    pub(crate) fn rear_stock_void_boundary_bridge(
        &self,
        request: AuthoringMeshRearStockVoidBoundaryBridgeRequest,
    ) -> Result<AuthoringMeshRearStockVoidBoundaryBridgeResult, RuntimeError> {
        if request.parent_revision_id != self.record.revision_id {
            return Err(invalid(
                "rear-stock boundary bridge parent revision differs",
            ));
        }
        checked_id(&request.operation_id, "operation_id")?;
        checked_sha(
            &request.operation_lineage_sha256,
            "operation_lineage_sha256",
        )?;
        if let Some(binding) = &self.record.source_binding {
            if binding.source_node_id != "rear-stock" || binding.part_id != "rear-stock" {
                return Err(invalid(
                    "rear-stock boundary bridge is bound to the rear-stock source Part",
                ));
            }
        }

        let parent_topology = topology_from_original(&self.record.original)?;
        validate_topology(&parent_topology)?;
        if parent_topology.vertices.len() != 8
            || parent_topology.edges.len() != 12
            || parent_topology.half_edges.len() != 24
            || parent_topology.corners.len() != 24
            || parent_topology.faces.len() != 6
            || parent_topology.loops.len() != 6
            || !parent_topology.tombstones.is_empty()
            || !parent_topology.rings.is_empty()
            || parent_topology.edges.values().any(|edge| edge.boundary)
        {
            return Err(invalid(
                "rear-stock boundary bridge requires an untombstoned closed source box",
            ));
        }
        if parent_topology
            .faces
            .values()
            .any(|face| face.half_edge_ids.len() != 4)
        {
            return Err(invalid(
                "rear-stock boundary bridge requires six quad source faces",
            ));
        }

        // The bridge contract is source-local: X is the rear-stock
        // longitudinal axis, Y points toward the opening, and Z is the depth
        // axis.  No client-selected transform can change these axes.
        let longitudinal_axis = 0_usize;
        let void_axis = 1_usize;
        let depth_axis = 2_usize;
        let mut minimum = [f64::INFINITY; 3];
        let mut maximum = [f64::NEG_INFINITY; 3];
        for vertex in parent_topology.vertices.values() {
            for axis in 0..3 {
                minimum[axis] = minimum[axis].min(vertex.position_m[axis]);
                maximum[axis] = maximum[axis].max(vertex.position_m[axis]);
            }
        }
        let spans = [
            maximum[0] - minimum[0],
            maximum[1] - minimum[1],
            maximum[2] - minimum[2],
        ];
        if spans[longitudinal_axis] <= MIN_EDGE_LENGTH_M
            || spans[void_axis] <= REAR_STOCK_VOID_BOUNDARY_BRIDGE_Y_OFFSETS_M[2].abs()
            || spans[depth_axis] <= 2.0 * REAR_STOCK_VOID_BOUNDARY_BRIDGE_Z_WEDGE_M[2]
        {
            return Err(invalid(
                "rear-stock boundary bridge source envelope is outside bounds",
            ));
        }
        let tolerance = spans.iter().copied().fold(1.0_f64, f64::max) * 1.0e-7;
        let axis_side = |value: f64, axis: usize| -> Result<i8, RuntimeError> {
            if (value - minimum[axis]).abs() <= tolerance {
                Ok(-1)
            } else if (value - maximum[axis]).abs() <= tolerance {
                Ok(1)
            } else {
                Err(invalid(
                    "rear-stock boundary bridge source is not an axis-aligned box",
                ))
            }
        };
        let mut box_vertices = BTreeMap::<(i8, i8, i8), AuthoringMeshVertexId>::new();
        for vertex in parent_topology.vertices.values() {
            let key = (
                axis_side(vertex.position_m[longitudinal_axis], longitudinal_axis)?,
                axis_side(vertex.position_m[void_axis], void_axis)?,
                axis_side(vertex.position_m[depth_axis], depth_axis)?,
            );
            if box_vertices.insert(key, vertex.vertex_id.clone()).is_some() {
                return Err(invalid(
                    "rear-stock boundary bridge source box has duplicate corners",
                ));
            }
        }
        if box_vertices.len() != 8 {
            return Err(invalid(
                "rear-stock boundary bridge source box does not contain all corners",
            ));
        }

        let edge_at = |void_side: i8,
                       depth_side: i8|
         -> Result<AuthoringMeshEdgeId, RuntimeError> {
            let mut matches = parent_topology
                .edges
                .iter()
                .filter_map(|(edge_id, edge)| {
                    let left = parent_topology.vertices.get(&edge.vertex_ids[0])?;
                    let right = parent_topology.vertices.get(&edge.vertex_ids[1])?;
                    let spans_longitudinal = (left.position_m[longitudinal_axis]
                        - minimum[longitudinal_axis])
                        .abs()
                        <= tolerance
                        && (right.position_m[longitudinal_axis] - maximum[longitudinal_axis]).abs()
                            <= tolerance
                        || (right.position_m[longitudinal_axis] - minimum[longitudinal_axis]).abs()
                            <= tolerance
                            && (left.position_m[longitudinal_axis] - maximum[longitudinal_axis])
                                .abs()
                                <= tolerance;
                    let on_void_plane = left.position_m[void_axis]
                        .total_cmp(&right.position_m[void_axis])
                        == std::cmp::Ordering::Equal
                        && (left.position_m[void_axis]
                            - if void_side < 0 {
                                minimum[void_axis]
                            } else {
                                maximum[void_axis]
                            })
                        .abs()
                            <= tolerance
                        && (right.position_m[void_axis]
                            - if void_side < 0 {
                                minimum[void_axis]
                            } else {
                                maximum[void_axis]
                            })
                        .abs()
                            <= tolerance;
                    let on_depth_plane = (left.position_m[depth_axis]
                        - if depth_side < 0 {
                            minimum[depth_axis]
                        } else {
                            maximum[depth_axis]
                        })
                    .abs()
                        <= tolerance
                        && (right.position_m[depth_axis]
                            - if depth_side < 0 {
                                minimum[depth_axis]
                            } else {
                                maximum[depth_axis]
                            })
                        .abs()
                            <= tolerance;
                    (spans_longitudinal && on_void_plane && on_depth_plane).then(|| edge_id.clone())
                })
                .collect::<Vec<_>>();
            matches.sort();
            match matches.as_slice() {
                [edge_id] => Ok(edge_id.clone()),
                _ => Err(invalid(
                    "rear-stock boundary bridge source longitudinal edge is ambiguous",
                )),
            }
        };

        // Keep the depth ordering explicit; stable source-side ordering is
        // part of the operation lineage and cannot depend on array order.
        let source_edges = vec![(-1_i8, edge_at(-1, -1)?), (1_i8, edge_at(-1, 1)?)];
        let support_edges = vec![(-1_i8, edge_at(1, -1)?), (1_i8, edge_at(1, 1)?)];
        if source_edges
            .iter()
            .chain(support_edges.iter())
            .any(|(_, edge_id)| {
                parent_topology
                    .edges
                    .get(edge_id)
                    .is_none_or(|edge| edge.boundary)
            })
        {
            return Err(invalid(
                "rear-stock boundary bridge source/support chain is not closed",
            ));
        }

        let mut worker_face_ids = BTreeSet::<AuthoringMeshFaceId>::new();
        for (_, edge_id) in source_edges.iter().chain(support_edges.iter()) {
            let edge = parent_topology
                .edges
                .get(edge_id)
                .ok_or_else(|| invalid("rear-stock boundary bridge edge is unavailable"))?;
            for half_edge_id in &edge.half_edge_ids {
                let half_edge = parent_topology
                    .half_edges
                    .get(half_edge_id)
                    .ok_or_else(|| {
                        invalid("rear-stock boundary bridge half-edge is unavailable")
                    })?;
                worker_face_ids.insert(half_edge.face_id.clone());
            }
        }
        if worker_face_ids.len() != 4 {
            return Err(invalid(
                "rear-stock boundary bridge requires four adjacent support faces",
            ));
        }

        let mut working = AuthoringMeshV2Revision::from_record(self.record.clone())?;
        let mut station_vertices = Vec::<(&str, usize, usize, AuthoringMeshVertexId, i8)>::new();
        for (edge_role, edge_ids) in [
            ("upper-inner", source_edges.as_slice()),
            ("support", support_edges.as_slice()),
        ] {
            for (side_index, (depth_sign, source_edge_id)) in edge_ids.iter().enumerate() {
                let source_edge = parent_topology
                    .edges
                    .get(source_edge_id)
                    .ok_or_else(|| invalid("rear-stock boundary bridge source edge is missing"))?;
                let far_vertex_id = source_edge
                    .vertex_ids
                    .iter()
                    .find(|vertex_id| {
                        (parent_topology.vertices[*vertex_id].position_m[longitudinal_axis]
                            - maximum[longitudinal_axis])
                            .abs()
                            <= tolerance
                    })
                    .cloned()
                    .ok_or_else(|| invalid("rear-stock boundary bridge far endpoint is missing"))?;
                let mut active_edge_id = source_edge_id.clone();
                for station_index in 1..=3 {
                    let station = REAR_STOCK_VOID_BOUNDARY_BRIDGE_STATIONS[station_index];
                    let target = minimum[longitudinal_axis] + spans[longitudinal_axis] * station;
                    let topology = topology_from_original(&working.record.original)?;
                    let edge = topology.edges.get(&active_edge_id).ok_or_else(|| {
                        invalid("rear-stock boundary bridge active edge is missing")
                    })?;
                    let start =
                        topology.vertices[&edge.vertex_ids[0]].position_m[longitudinal_axis];
                    let end = topology.vertices[&edge.vertex_ids[1]].position_m[longitudinal_axis];
                    if (end - start).abs() <= MIN_EDGE_LENGTH_M {
                        return Err(invalid(
                            "rear-stock boundary bridge active edge is degenerate",
                        ));
                    }
                    let ratio = (target - start) / (end - start);
                    let split_ratio_milli = (ratio * 1000.0).round() as u32;
                    if !(1..=999).contains(&split_ratio_milli) {
                        return Err(invalid(
                            "rear-stock boundary bridge station ratio is outside bounds",
                        ));
                    }
                    let split_lineage_sha256 = canonical_json_hash(&json!({
                        "parent_operation_lineage_sha256":request.operation_lineage_sha256,
                        "operation":"rear_stock_void_boundary_bridge",
                        "edge_role":edge_role,
                        "side_index":side_index,
                        "station_index":station_index,
                    }));
                    let split_operation_id = format!("amop-{}", &split_lineage_sha256[..56]);
                    let split = working.split_edge(AuthoringMeshSplitEdgeRequest {
                        operation_id: split_operation_id,
                        parent_revision_id: working.record.revision_id.clone(),
                        edge_id: active_edge_id.clone(),
                        split_ratio_milli,
                        operation_lineage_sha256: split_lineage_sha256,
                    })?;
                    let midpoint_id = split
                        .generated_elements
                        .iter()
                        .find(|element| element.kind == AuthoringMeshElementKind::Vertex)
                        .map(|element| AuthoringMeshVertexId(element.id.clone()))
                        .ok_or_else(|| {
                            invalid("rear-stock boundary bridge station vertex is unavailable")
                        })?;
                    let next_topology = topology_from_original(&split.child_revision.original)?;
                    active_edge_id = split
                        .generated_elements
                        .iter()
                        .filter(|element| element.kind == AuthoringMeshElementKind::Edge)
                        .map(|element| AuthoringMeshEdgeId(element.id.clone()))
                        .find(|edge_id| {
                            next_topology
                                .edges
                                .get(edge_id)
                                .is_some_and(|edge| edge.vertex_ids.contains(&far_vertex_id))
                        })
                        .ok_or_else(|| {
                            invalid("rear-stock boundary bridge continuation edge is unavailable")
                        })?;
                    station_vertices.push((
                        edge_role,
                        side_index,
                        station_index,
                        midpoint_id,
                        *depth_sign,
                    ));
                    working = AuthoringMeshV2Revision::from_record(split.child_revision)?;
                }
            }
        }

        let mut child_topology = topology_from_original(&working.record.original)?;
        for (edge_role, _side_index, station_index, vertex_id, depth_sign) in &station_vertices {
            let vertex = child_topology
                .vertices
                .get_mut(vertex_id)
                .ok_or_else(|| invalid("rear-stock boundary bridge station vertex is missing"))?;
            vertex.position_m[longitudinal_axis] = minimum[longitudinal_axis]
                + spans[longitudinal_axis]
                    * REAR_STOCK_VOID_BOUNDARY_BRIDGE_STATIONS[*station_index];
            if *edge_role == "upper-inner" {
                vertex.position_m[void_axis] = minimum[void_axis]
                    + REAR_STOCK_VOID_BOUNDARY_BRIDGE_Y_OFFSETS_M[*station_index];
                vertex.position_m[depth_axis] = if *depth_sign < 0 {
                    minimum[depth_axis] - REAR_STOCK_VOID_BOUNDARY_BRIDGE_Z_WEDGE_M[*station_index]
                } else {
                    maximum[depth_axis] + REAR_STOCK_VOID_BOUNDARY_BRIDGE_Z_WEDGE_M[*station_index]
                };
            }
            finite_position(
                vertex.position_m,
                "rear-stock boundary bridge station vertex",
            )?;
        }

        // Splitting the four faces adjacent to the source/support chains
        // turns the station boundaries into continuous quad strips.  The
        // helper chooses equal-X opposite-chain diagonals and never touches
        // the two X-end cap faces.
        let mut face_split_ordinal = 0_usize;
        for face_id in worker_face_ids {
            if child_topology
                .faces
                .get(&face_id)
                .is_some_and(|face| face.half_edge_ids.len() > 4)
            {
                split_face_into_worker_faces(
                    &mut child_topology,
                    self.record.lineage_id.as_ref(),
                    &self.record.revision_id,
                    &request.operation_lineage_sha256,
                    &face_id,
                    longitudinal_axis,
                    tolerance,
                    self.record.revision_index + 1,
                    &mut face_split_ordinal,
                    &mut BTreeSet::new(),
                    &mut BTreeSet::new(),
                )?;
            }
        }
        if child_topology
            .faces
            .values()
            .any(|face| face.half_edge_ids.len() > 4)
        {
            return Err(invalid(
                "rear-stock boundary bridge left a face outside the triangle/quad policy",
            ));
        }
        validate_topology(&child_topology)?;

        // Existing source vertices are never moved.  In particular both X
        // endpoints, the lower/support beam and the rear cap remain byte-for-
        // byte identical; only Runtime-generated station vertices carry the
        // boundary profile.
        if parent_topology.vertices.iter().any(|(vertex_id, parent)| {
            child_topology
                .vertices
                .get(vertex_id)
                .is_some_and(|child| child.position_m != parent.position_m)
        }) {
            return Err(invalid(
                "rear-stock boundary bridge moved an existing endpoint or support vertex",
            ));
        }

        let mut changed = BTreeSet::<(AuthoringMeshElementKind, String)>::new();
        macro_rules! mark_changed_map {
            ($field:ident, $kind:expr) => {
                for (id, parent_value) in &parent_topology.$field {
                    if child_topology.$field.get(id) != Some(parent_value) {
                        mark_ref(&mut changed, $kind, id.0.clone());
                    }
                }
            };
        }
        mark_changed_map!(vertices, AuthoringMeshElementKind::Vertex);
        mark_changed_map!(edges, AuthoringMeshElementKind::Edge);
        mark_changed_map!(half_edges, AuthoringMeshElementKind::HalfEdge);
        mark_changed_map!(corners, AuthoringMeshElementKind::Corner);
        mark_changed_map!(faces, AuthoringMeshElementKind::Face);
        mark_changed_map!(loops, AuthoringMeshElementKind::Loop);
        mark_changed_map!(rings, AuthoringMeshElementKind::Ring);
        verify_locality(&parent_topology, &child_topology, &changed)?;

        // Internal split operations are collapsed into one atomic journal so
        // durable rehydrate sees exactly one child revision and one lineage.
        let parent_tombstone_keys = parent_topology
            .tombstones
            .iter()
            .map(|tombstone| (tombstone.element.kind.clone(), tombstone.element.id.clone()))
            .collect::<BTreeSet<_>>();
        let mut operation_tombstones = child_topology
            .tombstones
            .iter()
            .filter(|tombstone| {
                !parent_tombstone_keys
                    .contains(&(tombstone.element.kind.clone(), tombstone.element.id.clone()))
            })
            .map(|tombstone| AuthoringMeshV2Tombstone {
                element: tombstone.element.clone(),
                retired_revision_index: self.record.revision_index + 1,
                operation_lineage_sha256: request.operation_lineage_sha256.clone(),
                reason: "rear_stock_void_boundary_bridge replaced an internal chain element"
                    .to_owned(),
            })
            .collect::<Vec<_>>();
        operation_tombstones.sort_by(|left, right| {
            left.element
                .kind
                .cmp(&right.element.kind)
                .then(left.element.id.cmp(&right.element.id))
        });
        operation_tombstones.dedup_by(|left, right| left.element == right.element);
        child_topology.tombstones = parent_topology.tombstones.clone();
        child_topology
            .tombstones
            .extend(operation_tombstones.iter().cloned());

        let parent_active = active_element_set(&parent_topology);
        let child_active = active_element_set(&child_topology);
        let generated = child_active
            .difference(&parent_active)
            .cloned()
            .collect::<BTreeSet<_>>();
        let generated_refs = refs_from_set(&generated);
        let retired_refs = operation_tombstones
            .iter()
            .map(|tombstone| tombstone.element.clone())
            .collect::<Vec<_>>();
        let source_elements = source_edges
            .iter()
            .map(|(_, edge_id)| AuthoringMeshElementRef {
                kind: AuthoringMeshElementKind::Edge,
                id: edge_id.0.clone(),
            })
            .collect::<Vec<_>>();
        let operation = rear_stock_void_boundary_bridge_operation_record(
            request.operation_id,
            source_elements,
            request.operation_lineage_sha256.clone(),
            self.record.revision_id.clone(),
            generated_refs.clone(),
            retired_refs.clone(),
            operation_tombstones,
        );
        let original = original_record(&self.record.lineage_id, child_topology)?;
        let child_revision_id = AuthoringMeshRevisionId(stable_id(
            "amrev",
            self.record.lineage_id.as_ref(),
            "rear-stock-void-boundary-bridge-revision",
            &json!({
                "parent_revision_id":self.record.revision_id,
                "operation_lineage_sha256":request.operation_lineage_sha256,
                "original_sha256":original.canonical_sha256,
            }),
        ));
        let child_record = revision_record(
            self.record.mesh_id.clone(),
            self.record.lineage_id.clone(),
            child_revision_id,
            vec![self.record.revision_id.clone()],
            self.record.revision_index + 1,
            Some(operation),
            original,
            None,
            self.record.source_binding.clone(),
            self.record.foundation_source_binding.clone(),
        );
        AuthoringMeshV2Revision::from_record(child_record.clone())?;
        Ok(AuthoringMeshRearStockVoidBoundaryBridgeResult {
            schema_version: AUTHORING_MESH_V2_REVISION_SCHEMA_VERSION.to_owned(),
            parent_revision_id: self.record.revision_id.clone(),
            child_revision: child_record,
            station_parameters_m: REAR_STOCK_VOID_BOUNDARY_BRIDGE_STATIONS
                .iter()
                .copied()
                .zip(REAR_STOCK_VOID_BOUNDARY_BRIDGE_Y_OFFSETS_M.iter().copied())
                .zip(REAR_STOCK_VOID_BOUNDARY_BRIDGE_Z_WEDGE_M.iter().copied())
                .map(|((station, y_offset), z_wedge)| [station, y_offset, z_wedge])
                .collect(),
            changed_elements: refs_from_set(&changed),
            generated_elements: generated_refs,
            retired_elements: retired_refs,
            locality_status:
                "rear-stock-void-upper-inner-boundary-bridge-support-quad-strips-preserve-endpoints-lower-beam-rear-cap-outer-envelope@1"
                    .to_owned(),
            evaluated_status: if self.record.evaluated.is_some() {
                "evaluated-sidecar-invalidated-requires-new-evaluation@2".to_owned()
            } else {
                "evaluated-sidecar-not-present@2".to_owned()
            },
        })
    }
}

fn insert_open_frame_notch_face(
    topology: &mut Topology,
    lineage_id: &str,
    parent_revision_id: &AuthoringMeshRevisionId,
    operation_lineage_sha256: &str,
    face_ordinal: usize,
    vertex_ids: Vec<AuthoringMeshVertexId>,
    generated: &mut BTreeSet<(AuthoringMeshElementKind, String)>,
) -> Result<(), RuntimeError> {
    if !(3..=4).contains(&vertex_ids.len())
        || vertex_ids.iter().collect::<BTreeSet<_>>().len() != vertex_ids.len()
    {
        return Err(invalid(
            "open frame notch generated face must be a unique triangle or quad",
        ));
    }
    if vertex_ids
        .iter()
        .any(|vertex_id| !topology.vertices.contains_key(vertex_id))
    {
        return Err(invalid(
            "open frame notch generated face references an unknown vertex",
        ));
    }
    let face_id = AuthoringMeshFaceId(stable_id(
        "f",
        lineage_id,
        "open-frame-notch-face",
        &json!({
            "parent_revision_id":parent_revision_id,
            "operation_lineage_sha256":operation_lineage_sha256,
            "face_ordinal":face_ordinal,
        }),
    ));
    let loop_id = AuthoringMeshLoopId(stable_id(
        "loop",
        lineage_id,
        "open-frame-notch-loop",
        &json!({"face_id":face_id}),
    ));
    if topology.faces.contains_key(&face_id) || topology.loops.contains_key(&loop_id) {
        return Err(invalid(
            "open frame notch would reuse a face or loop stable ID",
        ));
    }
    let mut half_edge_ids = Vec::with_capacity(vertex_ids.len());
    for ordinal in 0..vertex_ids.len() {
        let origin_vertex_id = vertex_ids[ordinal].clone();
        let target_vertex_id = vertex_ids[(ordinal + 1) % vertex_ids.len()].clone();
        let edge_id = topology
            .edges
            .iter()
            .find(|(_, edge)| {
                (edge.vertex_ids[0] == origin_vertex_id && edge.vertex_ids[1] == target_vertex_id)
                    || (edge.vertex_ids[0] == target_vertex_id
                        && edge.vertex_ids[1] == origin_vertex_id)
            })
            .map(|(edge_id, _)| edge_id.clone())
            .unwrap_or_else(|| {
                AuthoringMeshEdgeId(stable_id(
                    "e",
                    lineage_id,
                    "open-frame-notch-edge",
                    &json!({
                        "parent_revision_id":parent_revision_id,
                        "operation_lineage_sha256":operation_lineage_sha256,
                        "vertex_ids":if origin_vertex_id <= target_vertex_id {
                            json!([origin_vertex_id,target_vertex_id])
                        } else {
                            json!([target_vertex_id,origin_vertex_id])
                        },
                    }),
                ))
            });
        if !topology.edges.contains_key(&edge_id) {
            topology.edges.insert(
                edge_id.clone(),
                AuthoringMeshEdge {
                    edge_id: edge_id.clone(),
                    vertex_ids: [origin_vertex_id.clone(), target_vertex_id.clone()],
                    half_edge_ids: Vec::new(),
                    boundary: true,
                },
            );
            mark_ref(generated, AuthoringMeshElementKind::Edge, edge_id.0.clone());
        }
        let half_edge_id = AuthoringMeshHalfEdgeId(stable_id(
            "he",
            lineage_id,
            "open-frame-notch-half-edge",
            &json!({
                "parent_revision_id":parent_revision_id,
                "operation_lineage_sha256":operation_lineage_sha256,
                "face_id":face_id,
                "ordinal":ordinal,
            }),
        ));
        let corner_id = AuthoringMeshCornerId(stable_id(
            "c",
            lineage_id,
            "open-frame-notch-corner",
            &json!({"face_id":face_id,"ordinal":ordinal}),
        ));
        if topology.half_edges.contains_key(&half_edge_id)
            || topology.corners.contains_key(&corner_id)
        {
            return Err(invalid(
                "open frame notch would reuse a half-edge or corner stable ID",
            ));
        }
        half_edge_ids.push(half_edge_id.clone());
        topology.half_edges.insert(
            half_edge_id.clone(),
            AuthoringMeshHalfEdge {
                half_edge_id: half_edge_id.clone(),
                origin_vertex_id: origin_vertex_id.clone(),
                edge_id: edge_id.clone(),
                face_id: face_id.clone(),
                corner_id: corner_id.clone(),
                next_id: half_edge_id.clone(),
                prev_id: half_edge_id.clone(),
                twin_id: None,
                boundary: true,
            },
        );
        topology.corners.insert(
            corner_id.clone(),
            AuthoringMeshCorner {
                corner_id: corner_id.clone(),
                half_edge_id,
                vertex_id: origin_vertex_id,
                face_id: face_id.clone(),
                ordinal: ordinal as u32,
                uv0: None,
                normal: None,
                tangent: None,
                seam: false,
            },
        );
        mark_ref(
            generated,
            AuthoringMeshElementKind::HalfEdge,
            topology
                .half_edges
                .get(half_edge_ids.last().expect("half-edge was pushed"))
                .expect("half-edge was inserted")
                .half_edge_id
                .0
                .clone(),
        );
        mark_ref(
            generated,
            AuthoringMeshElementKind::Corner,
            topology
                .half_edges
                .get(half_edge_ids.last().expect("half-edge was pushed"))
                .expect("half-edge was inserted")
                .corner_id
                .0
                .clone(),
        );
    }
    for ordinal in 0..half_edge_ids.len() {
        let half_edge_id = &half_edge_ids[ordinal];
        let next_id = half_edge_ids[(ordinal + 1) % half_edge_ids.len()].clone();
        let prev_id =
            half_edge_ids[(ordinal + half_edge_ids.len() - 1) % half_edge_ids.len()].clone();
        let half_edge = topology
            .half_edges
            .get_mut(half_edge_id)
            .expect("open frame notch half-edge exists");
        half_edge.next_id = next_id;
        half_edge.prev_id = prev_id;
    }
    topology.faces.insert(
        face_id.clone(),
        AuthoringMeshFace {
            face_id: face_id.clone(),
            half_edge_ids: half_edge_ids.clone(),
            loop_id: loop_id.clone(),
            boundary: true,
        },
    );
    topology.loops.insert(
        loop_id.clone(),
        AuthoringMeshLoop {
            loop_id: loop_id.clone(),
            face_id: face_id.clone(),
            half_edge_ids,
            boundary: true,
        },
    );
    mark_ref(
        generated,
        AuthoringMeshElementKind::Face,
        topology.faces[&face_id].face_id.0.clone(),
    );
    mark_ref(generated, AuthoringMeshElementKind::Loop, loop_id.0.clone());
    Ok(())
}

/// Split one authored face by a diagonal whose endpoints already exist in
/// the face cycle.  `split_edge` intentionally only inserts a point on an
/// edge; on a box this turns a quad into a pentagon (and repeated station
/// splits into an n-gon).  The Worker lowering is deliberately narrower than
/// the authoring kernel, so rail-bow uses this local face primitive to turn
/// each supported longitudinal face back into deterministic triangle/quad
/// strips before the atomic child revision is serialized.
fn split_face(
    topology: &mut Topology,
    lineage_id: &str,
    parent_revision_id: &AuthoringMeshRevisionId,
    operation_lineage_sha256: &str,
    source_face_id: &AuthoringMeshFaceId,
    first_vertex_id: &AuthoringMeshVertexId,
    second_vertex_id: &AuthoringMeshVertexId,
    split_ordinal: usize,
    retired_revision_index: u64,
    generated: &mut BTreeSet<(AuthoringMeshElementKind, String)>,
    retired: &mut BTreeSet<(AuthoringMeshElementKind, String)>,
) -> Result<(AuthoringMeshFaceId, AuthoringMeshFaceId), RuntimeError> {
    let source_face = topology
        .faces
        .get(source_face_id)
        .cloned()
        .ok_or_else(|| invalid("face split source face is unavailable"))?;
    let source_loop = topology
        .loops
        .get(&source_face.loop_id)
        .cloned()
        .ok_or_else(|| invalid("face split source loop is unavailable"))?;
    if source_face.half_edge_ids.len() != source_loop.half_edge_ids.len()
        || source_face.half_edge_ids != source_loop.half_edge_ids
    {
        return Err(invalid("face split source loop differs from face cycle"));
    }
    let mut cycle_vertices = Vec::with_capacity(source_face.half_edge_ids.len());
    let mut cycle_edges = Vec::with_capacity(source_face.half_edge_ids.len());
    for half_edge_id in &source_face.half_edge_ids {
        let half_edge = topology
            .half_edges
            .get(half_edge_id)
            .ok_or_else(|| invalid("face split source half-edge is unavailable"))?;
        cycle_vertices.push(half_edge.origin_vertex_id.clone());
        cycle_edges.push(half_edge.edge_id.clone());
    }
    let face_degree = cycle_vertices.len();
    if !(4..=MAX_FACE_DEGREE).contains(&face_degree) {
        return Err(invalid(
            "face split source face must contain at least four vertices",
        ));
    }
    let first_index = cycle_vertices
        .iter()
        .position(|vertex_id| vertex_id == first_vertex_id)
        .ok_or_else(|| invalid("face split first endpoint is not on the source face"))?;
    let second_index = cycle_vertices
        .iter()
        .position(|vertex_id| vertex_id == second_vertex_id)
        .ok_or_else(|| invalid("face split second endpoint is not on the source face"))?;
    if first_index == second_index {
        return Err(invalid("face split diagonal endpoints must differ"));
    }
    let forward_distance = (second_index + face_degree - first_index) % face_degree;
    if !(2..=(face_degree - 2)).contains(&forward_distance) {
        return Err(invalid(
            "face split diagonal endpoints must be non-adjacent",
        ));
    }
    let mut diagonal_endpoints = [first_vertex_id.clone(), second_vertex_id.clone()];
    diagonal_endpoints.sort();
    let diagonal_id = AuthoringMeshEdgeId(stable_id(
        "e",
        lineage_id,
        "rear-stock-rail-bow-face-split-edge",
        &json!({
            "parent_revision_id": parent_revision_id,
            "operation_lineage_sha256": operation_lineage_sha256,
            "source_face_id": source_face_id,
            "split_ordinal": split_ordinal,
            "vertex_ids": diagonal_endpoints,
        }),
    ));
    if topology.edges.contains_key(&diagonal_id) {
        return Err(invalid("face split diagonal would reuse an active edge ID"));
    }

    let face_a_id = AuthoringMeshFaceId(stable_id(
        "f",
        lineage_id,
        "rear-stock-rail-bow-face-split-face",
        &json!({
            "parent_revision_id": parent_revision_id,
            "operation_lineage_sha256": operation_lineage_sha256,
            "source_face_id": source_face_id,
            "split_ordinal": split_ordinal,
            "side": 0,
        }),
    ));
    let face_b_id = AuthoringMeshFaceId(stable_id(
        "f",
        lineage_id,
        "rear-stock-rail-bow-face-split-face",
        &json!({
            "parent_revision_id": parent_revision_id,
            "operation_lineage_sha256": operation_lineage_sha256,
            "source_face_id": source_face_id,
            "split_ordinal": split_ordinal,
            "side": 1,
        }),
    ));
    let loop_a_id = AuthoringMeshLoopId(stable_id(
        "loop",
        lineage_id,
        "rear-stock-rail-bow-face-split-loop",
        &json!({"face_id": face_a_id}),
    ));
    let loop_b_id = AuthoringMeshLoopId(stable_id(
        "loop",
        lineage_id,
        "rear-stock-rail-bow-face-split-loop",
        &json!({"face_id": face_b_id}),
    ));
    if topology.faces.contains_key(&face_a_id)
        || topology.faces.contains_key(&face_b_id)
        || topology.loops.contains_key(&loop_a_id)
        || topology.loops.contains_key(&loop_b_id)
    {
        return Err(invalid("face split would reuse a face or loop stable ID"));
    }

    let cycle = |start: usize, end: usize| {
        let mut vertices = Vec::new();
        let mut edges = Vec::new();
        let mut index = start;
        loop {
            vertices.push(cycle_vertices[index].clone());
            if index == end {
                break;
            }
            edges.push(cycle_edges[index].clone());
            index = (index + 1) % face_degree;
        }
        // The first cycle closes from second -> first; the second closes
        // from first -> second.  Both half-edges therefore share one edge
        // with opposite winding and rebuild into a proper twin pair.
        edges.push(diagonal_id.clone());
        (vertices, edges)
    };
    let (vertices_a, edges_a) = cycle(first_index, second_index);
    let (vertices_b, edges_b) = cycle(second_index, first_index);
    if !(3..=4).contains(&vertices_a.len()) || !(3..=MAX_FACE_DEGREE).contains(&vertices_b.len()) {
        return Err(invalid("face split produced an invalid child cycle"));
    }
    if vertices_a.len() < 3 || vertices_b.len() < 3 {
        return Err(invalid("face split produced a degenerate child cycle"));
    }
    if vertices_a
        .iter()
        .chain(vertices_b.iter())
        .any(|vertex_id| !topology.vertices.contains_key(vertex_id))
    {
        return Err(invalid("face split child references an unknown vertex"));
    }

    // Remove the source cycle first.  Its boundary edges remain in place and
    // are rebound by rebuild_edge_incidence after both child cycles exist.
    topology.faces.remove(source_face_id);
    topology.loops.remove(&source_face.loop_id);
    for half_edge_id in &source_face.half_edge_ids {
        let half_edge = topology
            .half_edges
            .remove(half_edge_id)
            .ok_or_else(|| invalid("face split source half-edge disappeared"))?;
        topology
            .corners
            .remove(&half_edge.corner_id)
            .ok_or_else(|| invalid("face split source corner disappeared"))?;
        retire_ref(
            retired,
            &mut topology.tombstones,
            AuthoringMeshElementKind::HalfEdge,
            half_edge_id.0.clone(),
            retired_revision_index,
            operation_lineage_sha256,
            "rear_stock_void_rail_bow face_split replaced the source half-edge",
        );
        retire_ref(
            retired,
            &mut topology.tombstones,
            AuthoringMeshElementKind::Corner,
            half_edge.corner_id.0,
            retired_revision_index,
            operation_lineage_sha256,
            "rear_stock_void_rail_bow face_split replaced the source corner",
        );
    }
    retire_ref(
        retired,
        &mut topology.tombstones,
        AuthoringMeshElementKind::Face,
        source_face_id.0.clone(),
        retired_revision_index,
        operation_lineage_sha256,
        "rear_stock_void_rail_bow face_split replaced the source face",
    );
    retire_ref(
        retired,
        &mut topology.tombstones,
        AuthoringMeshElementKind::Loop,
        source_face.loop_id.0,
        retired_revision_index,
        operation_lineage_sha256,
        "rear_stock_void_rail_bow face_split replaced the source loop",
    );
    topology.edges.insert(
        diagonal_id.clone(),
        AuthoringMeshEdge {
            edge_id: diagonal_id.clone(),
            vertex_ids: [first_vertex_id.clone(), second_vertex_id.clone()],
            half_edge_ids: Vec::new(),
            boundary: false,
        },
    );
    mark_ref(
        generated,
        AuthoringMeshElementKind::Edge,
        diagonal_id.0.clone(),
    );

    let mut insert_cycle = |topology: &mut Topology,
                            face_id: AuthoringMeshFaceId,
                            loop_id: AuthoringMeshLoopId,
                            vertices: Vec<AuthoringMeshVertexId>,
                            edges: Vec<AuthoringMeshEdgeId>,
                            side: usize|
     -> Result<(), RuntimeError> {
        if vertices.len() != edges.len() || !(3..=MAX_FACE_DEGREE).contains(&vertices.len()) {
            return Err(invalid(
                "face split child cycle is outside the authored face-degree bound",
            ));
        }
        let mut half_edge_ids = Vec::with_capacity(vertices.len());
        for ordinal in 0..vertices.len() {
            let half_edge_id = AuthoringMeshHalfEdgeId(stable_id(
                "he",
                lineage_id,
                "rear-stock-rail-bow-face-split-half-edge",
                &json!({
                    "parent_revision_id": parent_revision_id,
                    "operation_lineage_sha256": operation_lineage_sha256,
                    "source_face_id": source_face_id,
                    "split_ordinal": split_ordinal,
                    "side": side,
                    "ordinal": ordinal,
                }),
            ));
            let corner_id = AuthoringMeshCornerId(stable_id(
                "c",
                lineage_id,
                "rear-stock-rail-bow-face-split-corner",
                &json!({
                    "parent_revision_id": parent_revision_id,
                    "operation_lineage_sha256": operation_lineage_sha256,
                    "source_face_id": source_face_id,
                    "split_ordinal": split_ordinal,
                    "side": side,
                    "ordinal": ordinal,
                }),
            ));
            if topology.half_edges.contains_key(&half_edge_id)
                || topology.corners.contains_key(&corner_id)
            {
                return Err(invalid(
                    "face split would reuse a half-edge or corner stable ID",
                ));
            }
            let edge_id = &edges[ordinal];
            if !topology.edges.contains_key(edge_id) {
                return Err(invalid("face split child references an unknown edge"));
            }
            half_edge_ids.push(half_edge_id.clone());
            topology.half_edges.insert(
                half_edge_id.clone(),
                AuthoringMeshHalfEdge {
                    half_edge_id: half_edge_id.clone(),
                    origin_vertex_id: vertices[ordinal].clone(),
                    edge_id: edge_id.clone(),
                    face_id: face_id.clone(),
                    corner_id: corner_id.clone(),
                    next_id: half_edge_id.clone(),
                    prev_id: half_edge_id.clone(),
                    twin_id: None,
                    boundary: true,
                },
            );
            topology.corners.insert(
                corner_id.clone(),
                AuthoringMeshCorner {
                    corner_id: corner_id.clone(),
                    half_edge_id,
                    vertex_id: vertices[ordinal].clone(),
                    face_id: face_id.clone(),
                    ordinal: ordinal as u32,
                    uv0: None,
                    normal: None,
                    tangent: None,
                    seam: false,
                },
            );
            mark_ref(
                generated,
                AuthoringMeshElementKind::HalfEdge,
                half_edge_ids[ordinal].0.clone(),
            );
            mark_ref(generated, AuthoringMeshElementKind::Corner, corner_id.0);
        }
        for ordinal in 0..half_edge_ids.len() {
            let half_edge = topology
                .half_edges
                .get_mut(&half_edge_ids[ordinal])
                .expect("face split half-edge exists");
            half_edge.next_id = half_edge_ids[(ordinal + 1) % half_edge_ids.len()].clone();
            half_edge.prev_id =
                half_edge_ids[(ordinal + half_edge_ids.len() - 1) % half_edge_ids.len()].clone();
        }
        topology.faces.insert(
            face_id.clone(),
            AuthoringMeshFace {
                face_id: face_id.clone(),
                half_edge_ids: half_edge_ids.clone(),
                loop_id: loop_id.clone(),
                boundary: true,
            },
        );
        topology.loops.insert(
            loop_id.clone(),
            AuthoringMeshLoop {
                loop_id: loop_id.clone(),
                face_id: face_id.clone(),
                half_edge_ids,
                boundary: true,
            },
        );
        mark_ref(generated, AuthoringMeshElementKind::Face, face_id.0.clone());
        mark_ref(generated, AuthoringMeshElementKind::Loop, loop_id.0);
        Ok(())
    };
    insert_cycle(
        topology,
        face_a_id.clone(),
        loop_a_id,
        vertices_a.clone(),
        edges_a,
        0,
    )?;
    insert_cycle(
        topology,
        face_b_id.clone(),
        loop_b_id,
        vertices_b,
        edges_b,
        1,
    )?;
    rebuild_edge_incidence(topology)?;
    rebuild_twins(topology)?;
    rebuild_boundary_rings(topology, lineage_id)?;
    validate_topology(topology)?;
    Ok((face_a_id, face_b_id))
}

fn split_face_into_worker_faces(
    topology: &mut Topology,
    lineage_id: &str,
    parent_revision_id: &AuthoringMeshRevisionId,
    operation_lineage_sha256: &str,
    source_face_id: &AuthoringMeshFaceId,
    longitudinal_axis: usize,
    position_tolerance_m: f64,
    retired_revision_index: u64,
    split_ordinal: &mut usize,
    generated: &mut BTreeSet<(AuthoringMeshElementKind, String)>,
    retired: &mut BTreeSet<(AuthoringMeshElementKind, String)>,
) -> Result<(), RuntimeError> {
    let mut current_face_id = source_face_id.clone();
    loop {
        let face = topology
            .faces
            .get(&current_face_id)
            .ok_or_else(|| invalid("worker face split continuation is unavailable"))?;
        if face.half_edge_ids.len() <= 4 {
            return Ok(());
        }
        let face_vertices = face
            .half_edge_ids
            .iter()
            .map(|half_edge_id| {
                topology
                    .half_edges
                    .get(half_edge_id)
                    .ok_or_else(|| invalid("worker face split cycle half-edge is unavailable"))
                    .map(|half_edge| half_edge.origin_vertex_id.clone())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let polygon_area = |vertices: &[AuthoringMeshVertexId]| {
            (1..vertices.len().saturating_sub(1))
                .map(|index| {
                    triangle_area(
                        topology.vertices[&vertices[0]].position_m,
                        topology.vertices[&vertices[index]].position_m,
                        topology.vertices[&vertices[index + 1]].position_m,
                    )
                })
                .sum::<f64>()
        };
        let worker_face_area_compatible = |vertices: &[AuthoringMeshVertexId]| {
            (0..vertices.len()).all(|index| {
                triangle_area(
                    topology.vertices[&vertices[index]].position_m,
                    topology.vertices[&vertices[(index + 1) % vertices.len()]].position_m,
                    topology.vertices[&vertices[(index + 2) % vertices.len()]].position_m,
                ) > MIN_FACE_AREA_M2
            })
        };
        // A fixed index-0 diagonal can follow one bowed boundary chain and
        // duplicate part of the adjacent box face.  Pair only opposite-chain
        // vertices at the same product-owned longitudinal station, producing
        // a true cross-face quad strip. If the source degree is odd, a valid
        // triangle cut is the bounded fallback and the remainder is split on
        // the next iteration.
        let choose_diagonal = |span: usize| {
            (0..face_vertices.len()).find_map(|start| {
                let end = (start + span) % face_vertices.len();
                let start_position = topology.vertices[&face_vertices[start]].position_m;
                let end_position = topology.vertices[&face_vertices[end]].position_m;
                if (start_position[longitudinal_axis] - end_position[longitudinal_axis]).abs()
                    > position_tolerance_m
                    || (0..3)
                        .filter(|axis| *axis != longitudinal_axis)
                        .all(|axis| {
                            (start_position[axis] - end_position[axis]).abs()
                                <= position_tolerance_m
                        })
                {
                    return None;
                }
                let mut first = Vec::with_capacity(span + 1);
                let mut index = start;
                loop {
                    first.push(face_vertices[index].clone());
                    if index == end {
                        break;
                    }
                    index = (index + 1) % face_vertices.len();
                }
                let mut second = Vec::with_capacity(face_vertices.len() - span + 1);
                index = end;
                loop {
                    second.push(face_vertices[index].clone());
                    if index == start {
                        break;
                    }
                    index = (index + 1) % face_vertices.len();
                }
                (polygon_area(&first) > MIN_FACE_AREA_M2
                    && worker_face_area_compatible(&first)
                    && polygon_area(&second) > MIN_FACE_AREA_M2
                    && (second.len() > 4 || worker_face_area_compatible(&second)))
                .then(|| (first[0].clone(), first[span].clone()))
            })
        };
        let (first_vertex_id, third_vertex_id) = choose_diagonal(3)
            .or_else(|| choose_diagonal(2))
            .ok_or_else(|| invalid("worker face split has no non-degenerate diagonal"))?;
        let (_, remainder_face_id) = split_face(
            topology,
            lineage_id,
            parent_revision_id,
            operation_lineage_sha256,
            &current_face_id,
            &first_vertex_id,
            &third_vertex_id,
            *split_ordinal,
            retired_revision_index,
            generated,
            retired,
        )?;
        *split_ordinal += 1;
        current_face_id = remainder_face_id;
    }
}

fn mark_ref(
    set: &mut BTreeSet<(AuthoringMeshElementKind, String)>,
    kind: AuthoringMeshElementKind,
    id: String,
) {
    set.insert((kind, id));
}

fn retire_ref(
    retired: &mut BTreeSet<(AuthoringMeshElementKind, String)>,
    tombstones: &mut Vec<AuthoringMeshV2Tombstone>,
    kind: AuthoringMeshElementKind,
    id: String,
    revision_index: u64,
    operation_lineage_sha256: &str,
    reason: &str,
) {
    retired.insert((kind.clone(), id.clone()));
    tombstones.push(AuthoringMeshV2Tombstone {
        element: AuthoringMeshElementRef { kind, id },
        retired_revision_index: revision_index,
        operation_lineage_sha256: operation_lineage_sha256.to_owned(),
        reason: reason.to_owned(),
    });
}

fn refs_from_set(
    set: &BTreeSet<(AuthoringMeshElementKind, String)>,
) -> Vec<AuthoringMeshElementRef> {
    set.iter()
        .map(|(kind, id)| AuthoringMeshElementRef {
            kind: kind.clone(),
            id: id.clone(),
        })
        .collect()
}

fn active_element_set(topology: &Topology) -> BTreeSet<(AuthoringMeshElementKind, String)> {
    let mut result = BTreeSet::new();
    for id in topology.vertices.keys() {
        mark_ref(&mut result, AuthoringMeshElementKind::Vertex, id.0.clone());
    }
    for id in topology.edges.keys() {
        mark_ref(&mut result, AuthoringMeshElementKind::Edge, id.0.clone());
    }
    for id in topology.half_edges.keys() {
        mark_ref(
            &mut result,
            AuthoringMeshElementKind::HalfEdge,
            id.0.clone(),
        );
    }
    for id in topology.corners.keys() {
        mark_ref(&mut result, AuthoringMeshElementKind::Corner, id.0.clone());
    }
    for id in topology.faces.keys() {
        mark_ref(&mut result, AuthoringMeshElementKind::Face, id.0.clone());
    }
    for id in topology.loops.keys() {
        mark_ref(&mut result, AuthoringMeshElementKind::Loop, id.0.clone());
    }
    for id in topology.rings.keys() {
        mark_ref(&mut result, AuthoringMeshElementKind::Ring, id.0.clone());
    }
    result
}

fn operation_record(
    operation_id: String,
    source_edge_id: AuthoringMeshEdgeId,
    operation_lineage_sha256: String,
    parent_revision_id: AuthoringMeshRevisionId,
    generated_elements: Vec<AuthoringMeshElementRef>,
    retired_elements: Vec<AuthoringMeshElementRef>,
    tombstones: Vec<AuthoringMeshV2Tombstone>,
) -> AuthoringMeshTopologyOperation {
    let mut operation = AuthoringMeshTopologyOperation {
        schema_version: AUTHORING_MESH_V2_OPERATION_SCHEMA_VERSION.to_owned(),
        operation_id,
        kind: AuthoringMeshTopologyOperationKind::SplitEdge,
        parent_revision_id,
        operation_lineage_sha256,
        source_elements: vec![AuthoringMeshElementRef {
            kind: AuthoringMeshElementKind::Edge,
            id: source_edge_id.0,
        }],
        generated_elements,
        retired_elements,
        tombstones,
        locality_policy: "edge-and-incident-face-cycles-only@2".to_owned(),
        canonical_sha256: String::new(),
    };
    operation.canonical_sha256 = canonical_hash_without_field(&operation, "canonical_sha256");
    operation
}

fn face_extrude_operation_record(
    operation_id: String,
    source_face_id: AuthoringMeshFaceId,
    operation_lineage_sha256: String,
    parent_revision_id: AuthoringMeshRevisionId,
    generated_elements: Vec<AuthoringMeshElementRef>,
) -> AuthoringMeshTopologyOperation {
    let mut operation = AuthoringMeshTopologyOperation {
        schema_version: AUTHORING_MESH_V2_OPERATION_SCHEMA_VERSION.to_owned(),
        operation_id,
        kind: AuthoringMeshTopologyOperationKind::FaceExtrude,
        parent_revision_id,
        operation_lineage_sha256,
        source_elements: vec![AuthoringMeshElementRef {
            kind: AuthoringMeshElementKind::Face,
            id: source_face_id.0,
        }],
        generated_elements,
        retired_elements: Vec::new(),
        tombstones: Vec::new(),
        locality_policy: "face-and-extrusion-shell-only@2".to_owned(),
        canonical_sha256: String::new(),
    };
    operation.canonical_sha256 = canonical_hash_without_field(&operation, "canonical_sha256");
    operation
}

fn move_vertices_operation_record(
    operation_id: String,
    source_vertex_ids: Vec<AuthoringMeshVertexId>,
    operation_lineage_sha256: String,
    parent_revision_id: AuthoringMeshRevisionId,
) -> AuthoringMeshTopologyOperation {
    let mut operation = AuthoringMeshTopologyOperation {
        schema_version: AUTHORING_MESH_V2_OPERATION_SCHEMA_VERSION.to_owned(),
        operation_id,
        kind: AuthoringMeshTopologyOperationKind::MoveVertices,
        parent_revision_id,
        operation_lineage_sha256,
        source_elements: source_vertex_ids
            .into_iter()
            .map(|vertex_id| AuthoringMeshElementRef {
                kind: AuthoringMeshElementKind::Vertex,
                id: vertex_id.0,
            })
            .collect(),
        generated_elements: Vec::new(),
        retired_elements: Vec::new(),
        tombstones: Vec::new(),
        locality_policy: "selected-vertex-position-only-no-topology-rewrite@2".to_owned(),
        canonical_sha256: String::new(),
    };
    operation.canonical_sha256 = canonical_hash_without_field(&operation, "canonical_sha256");
    operation
}

fn open_frame_notch_operation_record(
    operation_id: String,
    mut source_face_ids: Vec<AuthoringMeshFaceId>,
    opening_width_milli: u32,
    opening_height_milli: u32,
    operation_lineage_sha256: String,
    parent_revision_id: AuthoringMeshRevisionId,
    generated_elements: Vec<AuthoringMeshElementRef>,
    retired_elements: Vec<AuthoringMeshElementRef>,
    tombstones: Vec<AuthoringMeshV2Tombstone>,
) -> AuthoringMeshTopologyOperation {
    source_face_ids.sort();
    let mut operation = AuthoringMeshTopologyOperation {
        schema_version: AUTHORING_MESH_V2_OPERATION_SCHEMA_VERSION.to_owned(),
        operation_id,
        kind: AuthoringMeshTopologyOperationKind::OpenFrameNotch,
        parent_revision_id,
        operation_lineage_sha256,
        source_elements: source_face_ids
            .into_iter()
            .map(|face_id| AuthoringMeshElementRef {
                kind: AuthoringMeshElementKind::Face,
                id: face_id.0,
            })
            .collect(),
        generated_elements,
        retired_elements,
        tombstones,
        locality_policy: format!(
            "closed-box-u-notch-through-local-z-width-{}-height-{}-preserve-endcaps@1",
            opening_width_milli, opening_height_milli
        ),
        canonical_sha256: String::new(),
    };
    operation.canonical_sha256 = canonical_hash_without_field(&operation, "canonical_sha256");
    operation
}

fn rear_stock_void_rail_bow_operation_record(
    operation_id: String,
    mut source_elements: Vec<AuthoringMeshElementRef>,
    operation_lineage_sha256: String,
    parent_revision_id: AuthoringMeshRevisionId,
    generated_elements: Vec<AuthoringMeshElementRef>,
    retired_elements: Vec<AuthoringMeshElementRef>,
    tombstones: Vec<AuthoringMeshV2Tombstone>,
) -> AuthoringMeshTopologyOperation {
    source_elements.sort_by(|left, right| left.id.cmp(&right.id));
    let mut operation = AuthoringMeshTopologyOperation {
        schema_version: AUTHORING_MESH_V2_OPERATION_SCHEMA_VERSION.to_owned(),
        operation_id,
        kind: AuthoringMeshTopologyOperationKind::RearStockVoidRailBow,
        parent_revision_id,
        operation_lineage_sha256,
        source_elements,
        generated_elements,
        retired_elements,
        tombstones,
        locality_policy:
            "rear-stock-void-facing-longitudinal-paired-splits-fixed-five-station-bow@1".to_owned(),
        canonical_sha256: String::new(),
    };
    operation.canonical_sha256 = canonical_hash_without_field(&operation, "canonical_sha256");
    operation
}

fn rear_stock_void_boundary_bridge_operation_record(
    operation_id: String,
    mut source_elements: Vec<AuthoringMeshElementRef>,
    operation_lineage_sha256: String,
    parent_revision_id: AuthoringMeshRevisionId,
    generated_elements: Vec<AuthoringMeshElementRef>,
    retired_elements: Vec<AuthoringMeshElementRef>,
    tombstones: Vec<AuthoringMeshV2Tombstone>,
) -> AuthoringMeshTopologyOperation {
    source_elements.sort_by(|left, right| left.id.cmp(&right.id));
    let mut operation = AuthoringMeshTopologyOperation {
        schema_version: AUTHORING_MESH_V2_OPERATION_SCHEMA_VERSION.to_owned(),
        operation_id,
        kind: AuthoringMeshTopologyOperationKind::RearStockVoidBoundaryBridge,
        parent_revision_id,
        operation_lineage_sha256,
        source_elements,
        generated_elements,
        retired_elements,
        tombstones,
        locality_policy:
            "rear-stock-void-upper-inner-boundary-bridge-fixed-five-station-depth-wedge@1"
                .to_owned(),
        canonical_sha256: String::new(),
    };
    operation.canonical_sha256 = canonical_hash_without_field(&operation, "canonical_sha256");
    operation
}

fn evaluated_record(
    binding: Option<AuthoringMeshV2EvaluatedBinding>,
    source_revision_id: &AuthoringMeshRevisionId,
) -> Result<Option<AuthoringMeshEvaluated>, RuntimeError> {
    let Some(binding) = binding else {
        return Ok(None);
    };
    checked_id(&binding.artifact_id, "evaluated.artifact_id")?;
    checked_sha(&binding.artifact_sha256, "evaluated.artifact_sha256")?;
    checked_sha(&binding.readback_sha256, "evaluated.readback_sha256")?;
    if binding.correspondence_status.is_empty()
        || binding.correspondence_status.len() > MAX_ID_LENGTH
    {
        return Err(invalid("evaluated correspondence_status is invalid"));
    }
    let mut evaluated = AuthoringMeshEvaluated {
        namespace: AUTHORING_MESH_V2_EVALUATED_NAMESPACE.to_owned(),
        source_revision_id: source_revision_id.clone(),
        artifact_id: binding.artifact_id,
        artifact_sha256: binding.artifact_sha256,
        readback_sha256: binding.readback_sha256,
        correspondence_status: binding.correspondence_status,
        canonical_sha256: String::new(),
    };
    evaluated.canonical_sha256 = canonical_hash_without_field(&evaluated, "canonical_sha256");
    Ok(Some(evaluated))
}

pub(crate) fn validate_source_binding(
    binding: &AuthoringMeshV2SourceBinding,
) -> Result<(), RuntimeError> {
    if binding.schema_version != "AuthoringMeshV2SourceBinding@1" {
        return Err(invalid("source binding schema_version is invalid"));
    }
    for (field, value) in [
        ("project_id", binding.project_id.as_str()),
        ("candidate_id", binding.candidate_id.as_str()),
        ("artifact_id", binding.artifact_id.as_str()),
        ("source_node_id", binding.source_node_id.as_str()),
        ("part_id", binding.part_id.as_str()),
        ("material_zone_id", binding.material_zone_id.as_str()),
    ] {
        checked_id(value, field)?;
    }
    if !matches!(
        binding.source_operator_id.as_str(),
        "forgecad.geometry.primitive@2"
            | "forgecad.geometry.profile-extrude@1"
            | "forgecad.geometry.authoring-mesh@1"
    ) {
        return Err(invalid(
            "source_operator_id is outside the closed source bridge",
        ));
    }
    for (field, value) in [
        (
            "candidate_state_sha256",
            binding.candidate_state_sha256.as_str(),
        ),
        ("artifact_sha256", binding.artifact_sha256.as_str()),
        (
            "artifact_readback_sha256",
            binding.artifact_readback_sha256.as_str(),
        ),
        (
            "geometry_program_sha256",
            binding.geometry_program_sha256.as_str(),
        ),
        (
            "source_parameters_sha256",
            binding.source_parameters_sha256.as_str(),
        ),
        ("part_output_sha256", binding.part_output_sha256.as_str()),
    ] {
        checked_sha(value, field)?;
    }
    finite_position(binding.position_m, "source_binding.position_m")?;
    finite_position(binding.rotation_rad, "source_binding.rotation_rad")?;
    if binding.canonical_sha256 != canonical_hash_without_field(binding, "canonical_sha256") {
        return Err(invalid(
            "source binding canonical_sha256 does not match payload",
        ));
    }
    Ok(())
}

/// Validate the additive, foundation-owned provenance binding.  This is kept
/// separate from [`validate_source_binding`] on purpose: a foundation import
/// is not a candidate-owned GeometryProgram and must never acquire candidate
/// source semantics by sharing that contract.
pub(crate) fn validate_foundation_source_binding(
    binding: &AuthoringMeshV2FoundationSourceBinding,
) -> Result<(), RuntimeError> {
    if binding.schema_version != AUTHORING_MESH_V2_FOUNDATION_SOURCE_BINDING_SCHEMA_VERSION {
        return Err(invalid(
            "foundation source binding schema_version is invalid",
        ));
    }
    for (field, value) in [
        ("foundation.project_id", binding.project_id.as_str()),
        (
            "foundation.materialization_id",
            binding.materialization_id.as_str(),
        ),
        ("foundation.record_id", binding.record_id.as_str()),
        (
            "foundation.foundation_request_id",
            binding.foundation_request_id.as_str(),
        ),
        (
            "foundation.source_asset_id",
            binding.source_asset_id.as_str(),
        ),
        ("foundation.part_id", binding.part_id.as_str()),
        (
            "foundation.material_zone_id",
            binding.material_zone_id.as_str(),
        ),
        (
            "foundation.authoring_mesh_id",
            binding.authoring_mesh_id.as_str(),
        ),
        (
            "foundation.authoring_mesh_lineage_id",
            binding.authoring_mesh_lineage_id.as_str(),
        ),
        (
            "foundation.authoring_mesh_revision_id",
            binding.authoring_mesh_revision_id.as_str(),
        ),
    ] {
        checked_id(value, field)?;
    }
    for (field, value) in [
        (
            "foundation.foundation_request_sha256",
            binding.foundation_request_sha256.as_str(),
        ),
        (
            "foundation.foundation_result_object_sha256",
            binding.foundation_result_object_sha256.as_str(),
        ),
        (
            "foundation.topology_object_sha256",
            binding.topology_object_sha256.as_str(),
        ),
        (
            "foundation.socket_map_object_sha256",
            binding.socket_map_object_sha256.as_str(),
        ),
        (
            "foundation.rig_map_object_sha256",
            binding.rig_map_object_sha256.as_str(),
        ),
        (
            "foundation.fps_presentation_package_object_sha256",
            binding.fps_presentation_package_object_sha256.as_str(),
        ),
        (
            "foundation.source_asset_sha256",
            binding.source_asset_sha256.as_str(),
        ),
        (
            "foundation.source_part_topology_sha256",
            binding.source_part_topology_sha256.as_str(),
        ),
    ] {
        checked_sha(value, field)?;
    }
    if binding.source_asset_role.is_empty() || binding.source_asset_role.len() > MAX_ID_LENGTH {
        return Err(invalid("foundation.source_asset_role is invalid"));
    }
    if binding.binding_policy != "foundation-import-part-to-authoring-mesh-v2-source@1" {
        return Err(invalid("foundation.binding_policy is invalid"));
    }
    if binding.materialization_profile != WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PROFILE {
        return Err(invalid("foundation.materialization_profile is invalid"));
    }
    if !binding.source_only {
        return Err(invalid("foundation.source_only must be true"));
    }
    if binding.quality_status != WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_QUALITY_STATUS
        || binding.review_status != WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_REVIEW_STATUS
    {
        return Err(invalid("foundation quality/review status is invalid"));
    }
    if binding.canonicalization_policy
        != WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_CANONICALIZATION_POLICY
    {
        return Err(invalid("foundation.canonicalization_policy is invalid"));
    }
    if binding.canonical_sha256 != canonical_hash_without_field(binding, "canonical_sha256") {
        return Err(invalid(
            "foundation source binding canonical_sha256 does not match payload",
        ));
    }
    Ok(())
}

fn revision_record(
    mesh_id: AuthoringMeshId,
    lineage_id: AuthoringMeshLineageId,
    revision_id: AuthoringMeshRevisionId,
    parent_revision_ids: Vec<AuthoringMeshRevisionId>,
    revision_index: u64,
    operation: Option<AuthoringMeshTopologyOperation>,
    original: AuthoringMeshOriginal,
    evaluated: Option<AuthoringMeshEvaluated>,
    source_binding: Option<AuthoringMeshV2SourceBinding>,
    foundation_source_binding: Option<AuthoringMeshV2FoundationSourceBinding>,
) -> AuthoringMeshRevision {
    let mut record = AuthoringMeshRevision {
        schema_version: AUTHORING_MESH_V2_REVISION_SCHEMA_VERSION.to_owned(),
        mesh_id,
        lineage_id,
        revision_id,
        parent_revision_ids,
        revision_index,
        operation,
        original,
        evaluated,
        source_binding,
        foundation_source_binding,
        id_policy: AUTHORING_MESH_V2_ID_POLICY.to_owned(),
        canonical_sha256: String::new(),
    };
    record.canonical_sha256 = canonical_hash_without_field(&record, "canonical_sha256");
    record
}

fn original_record(
    lineage_id: &AuthoringMeshLineageId,
    topology: Topology,
) -> Result<AuthoringMeshOriginal, RuntimeError> {
    let mut original = AuthoringMeshOriginal {
        namespace: AUTHORING_MESH_V2_ORIGINAL_NAMESPACE.to_owned(),
        lineage_id: lineage_id.clone(),
        vertices: topology.vertices.into_values().collect(),
        edges: topology.edges.into_values().collect(),
        half_edges: topology.half_edges.into_values().collect(),
        corners: topology.corners.into_values().collect(),
        faces: topology.faces.into_values().collect(),
        loops: topology.loops.into_values().collect(),
        rings: topology.rings.into_values().collect(),
        tombstones: topology.tombstones,
        canonical_sha256: String::new(),
    };
    original.canonical_sha256 = canonical_hash_without_field(&original, "canonical_sha256");
    Ok(original)
}

fn topology_from_original(original: &AuthoringMeshOriginal) -> Result<Topology, RuntimeError> {
    if original.namespace != AUTHORING_MESH_V2_ORIGINAL_NAMESPACE {
        return Err(invalid("original namespace is required"));
    }
    checked_id(original.lineage_id.as_ref(), "original.lineage_id")?;
    let mut topology = Topology {
        vertices: BTreeMap::new(),
        edges: BTreeMap::new(),
        half_edges: BTreeMap::new(),
        corners: BTreeMap::new(),
        faces: BTreeMap::new(),
        loops: BTreeMap::new(),
        rings: BTreeMap::new(),
        tombstones: original.tombstones.clone(),
    };
    for vertex in &original.vertices {
        if topology
            .vertices
            .insert(vertex.vertex_id.clone(), vertex.clone())
            .is_some()
        {
            return Err(invalid("duplicate vertex stable ID"));
        }
    }
    for edge in &original.edges {
        if topology
            .edges
            .insert(edge.edge_id.clone(), edge.clone())
            .is_some()
        {
            return Err(invalid("duplicate edge stable ID"));
        }
    }
    for half_edge in &original.half_edges {
        if topology
            .half_edges
            .insert(half_edge.half_edge_id.clone(), half_edge.clone())
            .is_some()
        {
            return Err(invalid("duplicate half-edge stable ID"));
        }
    }
    for corner in &original.corners {
        if topology
            .corners
            .insert(corner.corner_id.clone(), corner.clone())
            .is_some()
        {
            return Err(invalid("duplicate corner stable ID"));
        }
    }
    for face in &original.faces {
        if topology
            .faces
            .insert(face.face_id.clone(), face.clone())
            .is_some()
        {
            return Err(invalid("duplicate face stable ID"));
        }
    }
    for loop_record in &original.loops {
        if topology
            .loops
            .insert(loop_record.loop_id.clone(), loop_record.clone())
            .is_some()
        {
            return Err(invalid("duplicate loop stable ID"));
        }
    }
    for ring in &original.rings {
        if topology
            .rings
            .insert(ring.ring_id.clone(), ring.clone())
            .is_some()
        {
            return Err(invalid("duplicate ring stable ID"));
        }
    }
    validate_topology(&topology)?;
    Ok(topology)
}

fn rebuild_edge_incidence(topology: &mut Topology) -> Result<(), RuntimeError> {
    for edge in topology.edges.values_mut() {
        edge.half_edge_ids.clear();
    }
    for half_edge in topology.half_edges.values() {
        let edge = topology
            .edges
            .get_mut(&half_edge.edge_id)
            .ok_or_else(|| invalid("half-edge references a missing edge"))?;
        edge.half_edge_ids.push(half_edge.half_edge_id.clone());
        if edge.half_edge_ids.len() > 2 {
            return Err(invalid("edge has more than two incident half-edges"));
        }
    }
    for edge in topology.edges.values_mut() {
        if edge.half_edge_ids.is_empty() {
            return Err(invalid("edge has no incident half-edge"));
        }
        edge.half_edge_ids.sort();
        edge.boundary = edge.half_edge_ids.len() == 1;
    }
    for half_edge in topology.half_edges.values_mut() {
        let edge = topology
            .edges
            .get(&half_edge.edge_id)
            .ok_or_else(|| invalid("half-edge edge lookup failed"))?;
        half_edge.boundary = edge.boundary;
    }
    for face in topology.faces.values_mut() {
        face.boundary = face.half_edge_ids.iter().any(|half_edge_id| {
            topology
                .half_edges
                .get(half_edge_id)
                .is_some_and(|half_edge| half_edge.boundary)
        });
    }
    for loop_record in topology.loops.values_mut() {
        loop_record.boundary = loop_record.half_edge_ids.iter().any(|half_edge_id| {
            topology
                .half_edges
                .get(half_edge_id)
                .is_some_and(|half_edge| half_edge.boundary)
        });
    }
    Ok(())
}

fn rebuild_twins(topology: &mut Topology) -> Result<(), RuntimeError> {
    for half_edge in topology.half_edges.values_mut() {
        half_edge.twin_id = None;
    }
    for edge in topology.edges.values() {
        match edge.half_edge_ids.as_slice() {
            [only] => {
                if !topology.half_edges.contains_key(only) {
                    return Err(invalid("edge references a missing boundary half-edge"));
                }
            }
            [left, right] => {
                if !topology.half_edges.contains_key(left)
                    || !topology.half_edges.contains_key(right)
                {
                    return Err(invalid("edge references a missing twin half-edge"));
                }
                topology
                    .half_edges
                    .get_mut(left)
                    .expect("left half-edge exists")
                    .twin_id = Some(right.clone());
                topology
                    .half_edges
                    .get_mut(right)
                    .expect("right half-edge exists")
                    .twin_id = Some(left.clone());
            }
            _ => return Err(invalid("edge incidence is outside the manifold bound")),
        }
    }
    Ok(())
}

fn rebuild_boundary_rings(topology: &mut Topology, lineage_id: &str) -> Result<(), RuntimeError> {
    topology.rings.clear();
    let mut pending = topology
        .edges
        .values()
        .filter(|edge| edge.boundary)
        .map(|edge| edge.edge_id.clone())
        .collect::<BTreeSet<_>>();
    while let Some(seed) = pending.pop_first() {
        let mut component = BTreeSet::new();
        let mut frontier = vec![seed.clone()];
        while let Some(edge_id) = frontier.pop() {
            if !component.insert(edge_id.clone()) {
                continue;
            }
            let edge = topology
                .edges
                .get(&edge_id)
                .ok_or_else(|| invalid("boundary ring references a missing edge"))?;
            let endpoints = edge.vertex_ids.clone();
            let neighbours = pending
                .iter()
                .filter_map(|candidate_id| {
                    let candidate = topology.edges.get(candidate_id)?;
                    (candidate.vertex_ids.iter().any(|id| endpoints.contains(id)))
                        .then(|| candidate_id.clone())
                })
                .collect::<Vec<_>>();
            for neighbour in neighbours {
                pending.remove(&neighbour);
                frontier.push(neighbour);
            }
        }
        let edge_ids = component.into_iter().collect::<Vec<_>>();
        if edge_ids.is_empty() {
            continue;
        }
        let mut degree = BTreeMap::<AuthoringMeshVertexId, usize>::new();
        for edge_id in &edge_ids {
            let edge = &topology.edges[edge_id];
            for vertex_id in &edge.vertex_ids {
                *degree.entry(vertex_id.clone()).or_default() += 1;
            }
        }
        let closed = edge_ids.len() >= 3 && degree.values().all(|degree| *degree == 2);
        let ring_id = AuthoringMeshRingId(stable_id(
            "ring",
            lineage_id,
            "boundary-ring",
            &json!({"edge_ids":edge_ids}),
        ));
        topology.rings.insert(
            ring_id.clone(),
            AuthoringMeshRing {
                ring_id,
                edge_ids,
                closed,
                boundary: true,
            },
        );
    }
    Ok(())
}

fn validate_topology(topology: &Topology) -> Result<(), RuntimeError> {
    if topology.vertices.is_empty()
        || topology.vertices.len() > MAX_VERTICES
        || topology.edges.is_empty()
        || topology.edges.len() > MAX_EDGES
        || topology.half_edges.is_empty()
        || topology.half_edges.len() > MAX_HALF_EDGES
        || topology.corners.is_empty()
        || topology.corners.len() > MAX_CORNERS
        || topology.faces.is_empty()
        || topology.faces.len() > MAX_FACES
    {
        return Err(invalid("topology element budget is outside bounds"));
    }
    let mut all_active_ids = BTreeSet::new();
    for vertex in topology.vertices.values() {
        checked_id(vertex.vertex_id.as_ref(), "vertex_id")?;
        if !all_active_ids.insert(vertex.vertex_id.0.clone()) {
            return Err(invalid("stable ID is reused across active elements"));
        }
        finite_position(vertex.position_m, "vertex.position_m")?;
    }
    for edge in topology.edges.values() {
        checked_id(edge.edge_id.as_ref(), "edge_id")?;
        if !all_active_ids.insert(edge.edge_id.0.clone()) {
            return Err(invalid("stable ID is reused across active elements"));
        }
        if edge.vertex_ids[0] == edge.vertex_ids[1]
            || !topology.vertices.contains_key(&edge.vertex_ids[0])
            || !topology.vertices.contains_key(&edge.vertex_ids[1])
            || !(1..=2).contains(&edge.half_edge_ids.len())
            || edge.boundary != (edge.half_edge_ids.len() == 1)
        {
            return Err(invalid("edge endpoints/incidence are invalid"));
        }
        let left = topology.vertices[&edge.vertex_ids[0]].position_m;
        let right = topology.vertices[&edge.vertex_ids[1]].position_m;
        if distance(left, right) <= MIN_EDGE_LENGTH_M {
            return Err(invalid("edge length is below tolerance"));
        }
        let mut unique = BTreeSet::new();
        for half_edge_id in &edge.half_edge_ids {
            if !unique.insert(half_edge_id) || !topology.half_edges.contains_key(half_edge_id) {
                return Err(invalid("edge half-edge IDs are invalid or repeated"));
            }
        }
    }
    for half_edge in topology.half_edges.values() {
        checked_id(half_edge.half_edge_id.as_ref(), "half_edge_id")?;
        if !all_active_ids.insert(half_edge.half_edge_id.0.clone()) {
            return Err(invalid("stable ID is reused across active elements"));
        }
        let edge = topology
            .edges
            .get(&half_edge.edge_id)
            .ok_or_else(|| invalid("half-edge edge reference is missing"))?;
        let next = topology
            .half_edges
            .get(&half_edge.next_id)
            .ok_or_else(|| invalid("half-edge next reference is missing"))?;
        let previous = topology
            .half_edges
            .get(&half_edge.prev_id)
            .ok_or_else(|| invalid("half-edge prev reference is missing"))?;
        let corner = topology
            .corners
            .get(&half_edge.corner_id)
            .ok_or_else(|| invalid("half-edge corner reference is missing"))?;
        if next.prev_id != half_edge.half_edge_id
            || previous.next_id != half_edge.half_edge_id
            || next.face_id != half_edge.face_id
            || previous.face_id != half_edge.face_id
            || corner.half_edge_id != half_edge.half_edge_id
            || corner.face_id != half_edge.face_id
            || corner.vertex_id != half_edge.origin_vertex_id
            || half_edge.boundary != edge.boundary
        {
            return Err(invalid("half-edge next/prev/face/corner invariants failed"));
        }
        let end = &next.origin_vertex_id;
        let endpoint_match = (half_edge.origin_vertex_id == edge.vertex_ids[0]
            && *end == edge.vertex_ids[1])
            || (half_edge.origin_vertex_id == edge.vertex_ids[1] && *end == edge.vertex_ids[0]);
        if !endpoint_match {
            return Err(invalid("half-edge direction differs from edge endpoints"));
        }
        match (&half_edge.twin_id, edge.half_edge_ids.len()) {
            (None, 1) => {}
            (Some(twin_id), 2) => {
                let twin = topology
                    .half_edges
                    .get(twin_id)
                    .ok_or_else(|| invalid("half-edge twin reference is missing"))?;
                if twin.twin_id.as_ref() != Some(&half_edge.half_edge_id)
                    || twin.edge_id != half_edge.edge_id
                    || twin.face_id == half_edge.face_id
                    || twin.origin_vertex_id != *end
                    || topology.half_edges[&twin.next_id].origin_vertex_id
                        != half_edge.origin_vertex_id
                {
                    return Err(invalid("half-edge twin symmetry/orientation failed"));
                }
            }
            _ => return Err(invalid("half-edge twin/boundary policy failed")),
        }
    }
    for corner in topology.corners.values() {
        checked_id(corner.corner_id.as_ref(), "corner_id")?;
        if !all_active_ids.insert(corner.corner_id.0.clone()) {
            return Err(invalid("stable ID is reused across active elements"));
        }
        if corner.ordinal > MAX_FACE_DEGREE as u32 {
            return Err(invalid("corner ordinal exceeds face degree bound"));
        }
        for (name, values) in [
            ("corner.uv0", corner.uv0.map(|uv| vec![uv[0], uv[1]])),
            ("corner.normal", corner.normal.map(|normal| normal.to_vec())),
            (
                "corner.tangent",
                corner.tangent.map(|tangent| tangent.to_vec()),
            ),
        ] {
            if let Some(values) = values {
                if values.iter().any(|value| !value.is_finite()) {
                    return Err(invalid(format!("{name} contains a non-finite value")));
                }
            }
        }
    }
    let mut face_half_edges = BTreeSet::new();
    for face in topology.faces.values() {
        checked_id(face.face_id.as_ref(), "face_id")?;
        if !all_active_ids.insert(face.face_id.0.clone()) {
            return Err(invalid("stable ID is reused across active elements"));
        }
        if !(3..=MAX_FACE_DEGREE).contains(&face.half_edge_ids.len())
            || !topology.loops.contains_key(&face.loop_id)
        {
            return Err(invalid("face cycle/loop is invalid"));
        }
        let mut cycle_ids = BTreeSet::new();
        let mut positions = Vec::with_capacity(face.half_edge_ids.len());
        for (ordinal, half_edge_id) in face.half_edge_ids.iter().enumerate() {
            if !cycle_ids.insert(half_edge_id) || !face_half_edges.insert(half_edge_id) {
                return Err(invalid("face cycle repeats or shares a half-edge"));
            }
            let half_edge = topology
                .half_edges
                .get(half_edge_id)
                .ok_or_else(|| invalid("face cycle references a missing half-edge"))?;
            if half_edge.face_id != face.face_id
                || half_edge.next_id != face.half_edge_ids[(ordinal + 1) % face.half_edge_ids.len()]
                || half_edge.prev_id
                    != face.half_edge_ids
                        [(ordinal + face.half_edge_ids.len() - 1) % face.half_edge_ids.len()]
            {
                return Err(invalid("face cycle is not next/prev canonical"));
            }
            positions.push(topology.vertices[&half_edge.origin_vertex_id].position_m);
        }
        let polygon_area = (1..positions.len() - 1)
            .map(|index| triangle_area(positions[0], positions[index], positions[index + 1]))
            .sum::<f64>();
        if polygon_area <= MIN_FACE_AREA_M2 {
            return Err(invalid("face contains zero-area geometry"));
        }
        let loop_record = &topology.loops[&face.loop_id];
        if loop_record.face_id != face.face_id || loop_record.half_edge_ids != face.half_edge_ids {
            return Err(invalid("face loop does not mirror the face cycle"));
        }
        let computed_boundary = face
            .half_edge_ids
            .iter()
            .any(|half_edge_id| topology.half_edges[half_edge_id].boundary);
        if face.boundary != computed_boundary || loop_record.boundary != computed_boundary {
            return Err(invalid("face/loop boundary flag differs from topology"));
        }
    }
    if face_half_edges.len() != topology.half_edges.len() {
        return Err(invalid(
            "a half-edge is not owned by exactly one face cycle",
        ));
    }
    for loop_record in topology.loops.values() {
        checked_id(loop_record.loop_id.as_ref(), "loop_id")?;
        if !all_active_ids.insert(loop_record.loop_id.0.clone()) {
            return Err(invalid("stable ID is reused across active elements"));
        }
    }
    for ring in topology.rings.values() {
        checked_id(ring.ring_id.as_ref(), "ring_id")?;
        if !all_active_ids.insert(ring.ring_id.0.clone()) {
            return Err(invalid("stable ID is reused across active elements"));
        }
        if !ring.boundary || ring.edge_ids.is_empty() {
            return Err(invalid("boundary ring metadata is invalid"));
        }
        if ring.edge_ids.iter().any(|edge_id| {
            topology
                .edges
                .get(edge_id)
                .is_none_or(|edge| !edge.boundary)
        }) {
            return Err(invalid("boundary ring references an interior/missing edge"));
        }
    }
    validate_tombstones(topology, &all_active_ids)?;
    Ok(())
}

fn validate_tombstones(
    topology: &Topology,
    active_ids: &BTreeSet<String>,
) -> Result<(), RuntimeError> {
    let mut retired = BTreeSet::new();
    for tombstone in &topology.tombstones {
        checked_id(&tombstone.element.id, "tombstone.element.id")?;
        checked_sha(
            &tombstone.operation_lineage_sha256,
            "tombstone.operation_lineage_sha256",
        )?;
        if tombstone.reason.is_empty()
            || !retired.insert((tombstone.element.kind.clone(), tombstone.element.id.clone()))
        {
            return Err(invalid("tombstone is empty or repeated"));
        }
        if active_ids.contains(&tombstone.element.id) {
            return Err(invalid("tombstone ID is active again"));
        }
    }
    Ok(())
}

fn verify_locality(
    parent: &Topology,
    child: &Topology,
    touched: &BTreeSet<(AuthoringMeshElementKind, String)>,
) -> Result<(), RuntimeError> {
    fn is_touched(
        touched: &BTreeSet<(AuthoringMeshElementKind, String)>,
        kind: &AuthoringMeshElementKind,
        id: &str,
    ) -> bool {
        touched.contains(&(kind.clone(), id.to_owned()))
    }
    macro_rules! compare_maps {
        ($kind:expr, $field:ident) => {
            for (id, parent_value) in &parent.$field {
                if is_touched(touched, &$kind, id.as_ref()) {
                    continue;
                }
                let child_value = child.$field.get(id).ok_or_else(|| {
                    invalid(format!(
                        "local operation removed unrelated {}",
                        stringify!($field)
                    ))
                })?;
                if child_value != parent_value {
                    return Err(invalid(format!(
                        "local operation changed unrelated {}",
                        stringify!($field)
                    )));
                }
            }
        };
    }
    compare_maps!(AuthoringMeshElementKind::Vertex, vertices);
    compare_maps!(AuthoringMeshElementKind::Edge, edges);
    compare_maps!(AuthoringMeshElementKind::HalfEdge, half_edges);
    compare_maps!(AuthoringMeshElementKind::Corner, corners);
    compare_maps!(AuthoringMeshElementKind::Face, faces);
    compare_maps!(AuthoringMeshElementKind::Loop, loops);
    compare_maps!(AuthoringMeshElementKind::Ring, rings);
    Ok(())
}

fn resolve_transaction_ref(
    command_index: usize,
    reference: AuthoringMeshV2TransactionRef,
    expected_kind: AuthoringMeshElementKind,
    steps: &[AuthoringMeshV2TransactionStep],
) -> Result<AuthoringMeshElementRef, RuntimeError> {
    let element = match reference {
        AuthoringMeshV2TransactionRef::Stable(element) => element,
        AuthoringMeshV2TransactionRef::Generated {
            command_index: referenced_command,
            kind,
            output_index,
        } => {
            if referenced_command >= command_index || kind != expected_kind {
                return Err(invalid(
                    "transaction generated reference is forward, self-referential, or has the wrong kind",
                ));
            }
            steps
                .get(referenced_command)
                .and_then(|step| {
                    step.generated_elements
                        .iter()
                        .filter(|element| element.kind == kind)
                        .nth(output_index)
                })
                .cloned()
                .ok_or_else(|| {
                    invalid(format!(
                        "transaction command {command_index} cannot resolve {kind:?} output {output_index} from command {referenced_command}"
                    ))
                })?
        }
    };
    if element.kind != expected_kind {
        return Err(invalid(format!(
            "transaction reference kind {:?} does not match expected {expected_kind:?}",
            element.kind
        )));
    }
    checked_id(&element.id, "transaction.element_id")?;
    Ok(element)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad() -> AuthoringMeshV2GenesisInput {
        AuthoringMeshV2GenesisInput {
            mesh_id: AuthoringMeshId::from("mesh-demo"),
            lineage_id: AuthoringMeshLineageId::from("lineage-demo"),
            positions_m: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            faces: vec![vec![0, 1, 2, 3]],
            evaluated: Some(AuthoringMeshV2EvaluatedBinding {
                artifact_id: "artifact-demo".to_owned(),
                artifact_sha256: "a".repeat(64),
                readback_sha256: "b".repeat(64),
                correspondence_status: "not-materialized@2".to_owned(),
            }),
            source_binding: None,
            foundation_source_binding: None,
        }
    }

    fn closed_box() -> AuthoringMeshV2GenesisInput {
        AuthoringMeshV2GenesisInput {
            mesh_id: AuthoringMeshId::from("mesh-open-frame-demo"),
            lineage_id: AuthoringMeshLineageId::from("lineage-open-frame-demo"),
            positions_m: vec![
                [-0.5, -0.5, -0.5],
                [0.5, -0.5, -0.5],
                [0.5, 0.5, -0.5],
                [-0.5, 0.5, -0.5],
                [-0.5, -0.5, 0.5],
                [0.5, -0.5, 0.5],
                [0.5, 0.5, 0.5],
                [-0.5, 0.5, 0.5],
            ],
            faces: vec![
                vec![0, 3, 2, 1],
                vec![4, 5, 6, 7],
                vec![0, 1, 5, 4],
                vec![3, 7, 6, 2],
                vec![0, 4, 7, 3],
                vec![1, 2, 6, 5],
            ],
            evaluated: None,
            source_binding: None,
            foundation_source_binding: None,
        }
    }

    #[test]
    fn genesis_has_stable_typed_identity_and_separate_evaluated_sidecar() {
        let revision = AuthoringMeshV2Revision::genesis(quad()).expect("genesis");
        assert_eq!(
            revision.record.schema_version,
            AUTHORING_MESH_V2_REVISION_SCHEMA_VERSION
        );
        assert_eq!(
            revision.record.original.namespace,
            AUTHORING_MESH_V2_ORIGINAL_NAMESPACE
        );
        assert!(revision.record.evaluated.is_some());
        assert_eq!(revision.record.original.vertices.len(), 4);
        assert_eq!(revision.record.original.edges.len(), 4);
        assert_eq!(revision.record.original.half_edges.len(), 4);
        assert_eq!(revision.record.original.corners.len(), 4);
        assert!(revision.record.parent_revision_ids.is_empty());
    }

    #[test]
    fn split_edge_is_local_and_retires_source_ids() {
        let revision = AuthoringMeshV2Revision::genesis(quad()).expect("genesis");
        let edge_id = revision.record.original.edges[0].edge_id.clone();
        let result = revision
            .split_edge(AuthoringMeshSplitEdgeRequest {
                operation_id: "split-demo".to_owned(),
                parent_revision_id: revision.record.revision_id.clone(),
                edge_id: edge_id.clone(),
                split_ratio_milli: 500,
                operation_lineage_sha256: "c".repeat(64),
            })
            .expect("split");
        assert_eq!(
            result.child_revision.parent_revision_ids,
            vec![revision.record.revision_id.clone()]
        );
        assert_eq!(result.child_revision.revision_index, 1);
        assert_eq!(result.child_revision.original.vertices.len(), 5);
        assert_eq!(result.child_revision.original.edges.len(), 5);
        assert_eq!(result.child_revision.original.half_edges.len(), 5);
        assert!(result
            .retired_elements
            .iter()
            .any(|element| element.id == edge_id.0));
        assert_eq!(
            result.evaluated_status,
            "evaluated-sidecar-invalidated-requires-new-evaluation@2"
        );
    }

    #[test]
    fn split_requires_the_exact_parent_revision() {
        let revision = AuthoringMeshV2Revision::genesis(quad()).expect("genesis");
        let error = revision.split_edge(AuthoringMeshSplitEdgeRequest {
            operation_id: "split-demo".to_owned(),
            parent_revision_id: AuthoringMeshRevisionId::from("other-revision"),
            edge_id: revision.record.original.edges[0].edge_id.clone(),
            split_ratio_milli: 500,
            operation_lineage_sha256: "c".repeat(64),
        });
        assert!(error.is_err());
    }

    fn split_then_move_transaction(
        revision: &AuthoringMeshV2Revision,
    ) -> AuthoringMeshV2Transaction {
        AuthoringMeshV2Transaction {
            commands: vec![
                AuthoringMeshV2TransactionCommand::SplitEdge {
                    operation_id: "tx-split".to_owned(),
                    edge: AuthoringMeshV2TransactionRef::Stable(AuthoringMeshElementRef {
                        kind: AuthoringMeshElementKind::Edge,
                        id: revision.record.original.edges[0].edge_id.0.clone(),
                    }),
                    split_ratio_milli: 500,
                    operation_lineage_sha256: "1".repeat(64),
                },
                AuthoringMeshV2TransactionCommand::MoveVertices {
                    operation_id: "tx-move-generated".to_owned(),
                    vertices: vec![AuthoringMeshV2TransactionRef::Generated {
                        command_index: 0,
                        kind: AuthoringMeshElementKind::Vertex,
                        output_index: 0,
                    }],
                    delta_m: vec![[0.0, 0.0, 0.125]],
                    operation_lineage_sha256: "2".repeat(64),
                },
            ],
        }
    }

    #[test]
    fn transaction_resolves_generated_ids_and_returns_full_revision_chain() {
        let revision = AuthoringMeshV2Revision::genesis(quad()).expect("genesis");
        let result = revision
            .apply_transaction(split_then_move_transaction(&revision))
            .expect("transaction");
        assert_eq!(result.parent_revision_id, revision.record.revision_id);
        assert_eq!(result.revision_chain.len(), 2);
        assert_eq!(result.steps.len(), 2);
        assert_eq!(result.steps[0].command_index, 0);
        assert!(!result.steps[0].changed_elements.is_empty());
        assert!(!result.steps[0].retired_elements.is_empty());
        assert_eq!(result.final_revision.revision_index, 2);
        assert_eq!(
            result.steps[0].child_revision_id,
            result.steps[1].parent_revision_id
        );
        let generated_vertex = result.steps[0]
            .generated_elements
            .iter()
            .find(|element| element.kind == AuthoringMeshElementKind::Vertex)
            .expect("split midpoint");
        let moved = result
            .final_revision
            .original
            .vertices
            .iter()
            .find(|vertex| vertex.vertex_id.0 == generated_vertex.id)
            .expect("generated vertex remains active");
        assert_eq!(moved.position_m[2], 0.125);
        assert!(result.final_revision.evaluated.is_none());
    }

    #[test]
    fn transaction_can_edit_a_vertex_generated_by_face_extrude() {
        let revision = AuthoringMeshV2Revision::genesis(quad()).expect("genesis");
        let transaction = AuthoringMeshV2Transaction {
            commands: vec![
                AuthoringMeshV2TransactionCommand::FaceExtrude {
                    operation_id: "tx-extrude".to_owned(),
                    face: AuthoringMeshV2TransactionRef::Stable(AuthoringMeshElementRef {
                        kind: AuthoringMeshElementKind::Face,
                        id: revision.record.original.faces[0].face_id.0.clone(),
                    }),
                    distance_m: 0.25,
                    operation_lineage_sha256: "5".repeat(64),
                },
                AuthoringMeshV2TransactionCommand::MoveVertices {
                    operation_id: "tx-move-extruded-vertex".to_owned(),
                    vertices: vec![AuthoringMeshV2TransactionRef::Generated {
                        command_index: 0,
                        kind: AuthoringMeshElementKind::Vertex,
                        output_index: 0,
                    }],
                    delta_m: vec![[0.0, 0.0, 0.05]],
                    operation_lineage_sha256: "6".repeat(64),
                },
            ],
        };
        let result = revision
            .apply_transaction(transaction)
            .expect("extrude transaction");
        assert_eq!(result.final_revision.revision_index, 2);
        assert_eq!(result.final_revision.original.faces.len(), 6);
        let generated_vertex = result.steps[0]
            .generated_elements
            .iter()
            .find(|element| element.kind == AuthoringMeshElementKind::Vertex)
            .expect("extruded vertex");
        let moved = result
            .final_revision
            .original
            .vertices
            .iter()
            .find(|vertex| vertex.vertex_id.0 == generated_vertex.id)
            .expect("generated vertex remains active");
        assert!((moved.position_m[2] - 0.30).abs() < 1.0e-12);
    }

    #[test]
    fn failed_late_transaction_command_returns_no_partial_revision() {
        let revision = AuthoringMeshV2Revision::genesis(quad()).expect("genesis");
        let original_hash = revision.record.canonical_sha256.clone();
        let transaction = AuthoringMeshV2Transaction {
            commands: vec![
                AuthoringMeshV2TransactionCommand::MoveVertices {
                    operation_id: "tx-valid-move".to_owned(),
                    vertices: vec![AuthoringMeshV2TransactionRef::Stable(
                        AuthoringMeshElementRef {
                            kind: AuthoringMeshElementKind::Vertex,
                            id: revision.record.original.vertices[0].vertex_id.0.clone(),
                        },
                    )],
                    delta_m: vec![[0.0, 0.0, 0.1]],
                    operation_lineage_sha256: "3".repeat(64),
                },
                AuthoringMeshV2TransactionCommand::SplitEdge {
                    operation_id: "tx-invalid-split".to_owned(),
                    edge: AuthoringMeshV2TransactionRef::Stable(AuthoringMeshElementRef {
                        kind: AuthoringMeshElementKind::Edge,
                        id: "missing-edge".to_owned(),
                    }),
                    split_ratio_milli: 500,
                    operation_lineage_sha256: "4".repeat(64),
                },
            ],
        };
        assert!(revision.apply_transaction(transaction).is_err());
        assert_eq!(revision.record.canonical_sha256, original_hash);
        assert_eq!(revision.record.revision_index, 0);
    }

    #[test]
    fn transaction_replay_is_deterministic() {
        let revision = AuthoringMeshV2Revision::genesis(quad()).expect("genesis");
        let first = revision
            .apply_transaction(split_then_move_transaction(&revision))
            .expect("first replay");
        let second = revision
            .apply_transaction(split_then_move_transaction(&revision))
            .expect("second replay");
        assert_eq!(
            first.final_revision.canonical_sha256,
            second.final_revision.canonical_sha256
        );
        assert_eq!(first.final_revision, second.final_revision);
    }

    #[test]
    fn face_extrude_builds_a_closed_local_shell_and_invalidates_evaluated_sidecar() {
        let revision = AuthoringMeshV2Revision::genesis(quad()).expect("genesis");
        let face_id = revision.record.original.faces[0].face_id.clone();
        let result = revision
            .face_extrude(AuthoringMeshFaceExtrudeRequest {
                operation_id: "extrude-demo".to_owned(),
                parent_revision_id: revision.record.revision_id.clone(),
                face_id,
                distance_m: 0.25,
                operation_lineage_sha256: "d".repeat(64),
            })
            .expect("face extrude");
        assert_eq!(result.child_revision.revision_index, 1);
        assert_eq!(result.child_revision.original.vertices.len(), 8);
        assert_eq!(result.child_revision.original.faces.len(), 6);
        assert_eq!(result.child_revision.original.edges.len(), 12);
        assert!(
            result.child_revision.original.rings.is_empty(),
            "unexpected boundary rings: {:?}",
            result.child_revision.original.rings
        );
        assert!(result.retired_elements.is_empty());
        assert!(result
            .generated_elements
            .iter()
            .any(|element| element.kind == AuthoringMeshElementKind::Face));
        assert_eq!(
            result.evaluated_status,
            "evaluated-sidecar-invalidated-requires-new-evaluation@2"
        );
        AuthoringMeshV2Revision::from_record(result.child_revision).expect("rehydrate extrude");
    }

    #[test]
    fn face_extrude_rejects_non_planar_source_faces() {
        let mut input = quad();
        input.positions_m[2][2] = 0.01;
        let revision = AuthoringMeshV2Revision::genesis(input).expect("genesis");
        let error = revision.face_extrude(AuthoringMeshFaceExtrudeRequest {
            operation_id: "extrude-non-planar".to_owned(),
            parent_revision_id: revision.record.revision_id.clone(),
            face_id: revision.record.original.faces[0].face_id.clone(),
            distance_m: 0.25,
            operation_lineage_sha256: "e".repeat(64),
        });
        assert!(error.is_err());
    }

    #[test]
    fn open_frame_notch_builds_a_closed_u_frame_and_rehydrates() {
        let revision = AuthoringMeshV2Revision::genesis(closed_box()).expect("box genesis");
        let result = revision
            .open_frame_notch(AuthoringMeshOpenFrameNotchRequest {
                operation_id: "open-frame-demo".to_owned(),
                parent_revision_id: revision.record.revision_id.clone(),
                opening_width_milli: 560,
                opening_height_milli: 620,
                operation_lineage_sha256: "f".repeat(64),
            })
            .expect("open frame notch");
        assert_eq!(result.child_revision.revision_index, 1);
        assert_eq!(result.child_revision.parent_revision_ids.len(), 1);
        assert!(result.child_revision.original.faces.len() > 6);
        assert!(
            result.child_revision.original.rings.is_empty(),
            "unexpected boundary rings: {:?}",
            result.child_revision.original.rings
        );
        assert!(!result.generated_elements.is_empty());
        assert!(!result.retired_elements.is_empty());
        AuthoringMeshV2Revision::from_record(result.child_revision)
            .expect("rehydrate open frame child");
    }

    #[test]
    fn rear_stock_void_rail_bow_is_one_rehydratable_atomic_revision() {
        let revision = AuthoringMeshV2Revision::genesis(closed_box()).expect("box genesis");
        let result = revision
            .rear_stock_void_rail_bow(AuthoringMeshRearStockVoidRailBowRequest {
                operation_id: "rear-stock-rail-bow-demo".to_owned(),
                parent_revision_id: revision.record.revision_id.clone(),
                expected_void_centroid_m: [0.0, -1.0, 0.0],
                expected_void_face_normal_m: [0.0, -1.0, 0.0],
                operation_lineage_sha256: "9".repeat(64),
            })
            .expect("rear-stock void rail bow");
        assert_eq!(result.child_revision.revision_index, 1);
        assert_eq!(result.child_revision.original.vertices.len(), 20);
        assert!(result
            .child_revision
            .original
            .faces
            .iter()
            .all(|face| (3..=4).contains(&face.half_edge_ids.len())));
        let worker_topology = topology_from_original(&result.child_revision.original)
            .expect("worker-compatible child topology");
        for face in worker_topology.faces.values() {
            let positions = face
                .half_edge_ids
                .iter()
                .map(|half_edge_id| {
                    worker_topology.vertices
                        [&worker_topology.half_edges[half_edge_id].origin_vertex_id]
                        .position_m
                })
                .collect::<Vec<_>>();
            assert!(
                triangle_area(positions[0], positions[1], positions[2]) > 1.0e-8,
                "Worker first-triangle area is degenerate for {}",
                face.face_id.0
            );
        }
        let parameters = crate::authoring_mesh_v2_geometry::authoring_mesh_v2_geometry_parameters(
            &result.child_revision,
            [0.0; 3],
            [0.0; 3],
        )
        .expect("rehydrated revision lowers to triangle/quad Worker topology");
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"rear-stock-rail-bow-worker-regression",
            "representation_plan_sha256":"8".repeat(64),
            "operator_catalog_sha256":forgecad_geometry_worker::operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":1,
                "max_triangles":4096,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[{
                "node_id":"rear-stock",
                "operator_id":"forgecad.geometry.authoring-mesh@1",
                "inputs":[],
                "parameters":parameters
            }],
            "part_outputs":[{
                "part_id":"rear-stock",
                "input_node_ids":["rear-stock"],
                "material_zone_id":"zone-rear-stock",
                "solid":true
            }]
        });
        let program_sha256 = forgecad_geometry_worker::geometry_program_v2_draft_hash(&program)
            .expect("RailBow GeometryProgram draft hashes");
        program["canonical_sha256"] = Value::String(program_sha256);
        crate::geometry_worker::compile_geometry_test_fallback(&program, None)
            .expect("RailBow GeometryProgram compiles in the fixed Worker");
        assert_eq!(result.station_parameters_m[2], [0.5, 0.045]);
        assert!(result
            .generated_elements
            .iter()
            .any(|element| { element.kind == AuthoringMeshElementKind::HalfEdge }));
        assert!(result.retired_elements.len() > 2);
        assert!(result
            .child_revision
            .original
            .tombstones
            .iter()
            .all(|tombstone| {
                tombstone.retired_revision_index == 1
                    && tombstone.operation_lineage_sha256 == "9".repeat(64)
            }));
        AuthoringMeshV2Revision::from_record(result.child_revision)
            .expect("rehydrate rear-stock rail-bow child");
    }
}
