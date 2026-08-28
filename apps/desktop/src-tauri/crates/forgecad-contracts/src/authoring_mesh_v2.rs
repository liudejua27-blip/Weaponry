//! Closed Rust contracts for the ForgeCAD-owned `AuthoringMesh@2` kernel.
//!
//! These types intentionally describe the authored/original topology and the
//! evaluated sidecar as different namespaces.  An evaluated artifact never
//! supplies element IDs for the authored mesh.  Runtime generates the IDs and
//! validates the topology before a value is exposed at a product boundary.

use serde::{Deserialize, Serialize};

pub const AUTHORING_MESH_V2_SCHEMA_VERSION: &str = "AuthoringMesh@2";
pub const AUTHORING_MESH_V2_REVISION_SCHEMA_VERSION: &str = "AuthoringMeshRevision@2";
pub const AUTHORING_MESH_V2_OPERATION_SCHEMA_VERSION: &str = "AuthoringMeshTopologyOperation@2";
/// Runtime-owned persistence envelopes for the immutable revision payload.
/// The revision contract remains the single topology source of truth; these
/// versions only close the prepare/get transport boundary around it.
pub const AUTHORING_MESH_V2_DURABLE_PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "AuthoringMeshV2DurablePrepareRequest@1";
pub const AUTHORING_MESH_V2_DURABLE_GET_REQUEST_SCHEMA_VERSION: &str =
    "AuthoringMeshV2DurableGetRequest@1";
pub const AUTHORING_MESH_V2_DURABLE_RESULT_SCHEMA_VERSION: &str = "AuthoringMeshV2DurableResult@1";
pub const AUTHORING_MESH_V2_DURABLE_RECORD_SCHEMA_VERSION: &str = "AuthoringMeshV2DurableRecord@1";
pub const AUTHORING_MESH_V2_ORIGINAL_NAMESPACE: &str = "original";
pub const AUTHORING_MESH_V2_EVALUATED_NAMESPACE: &str = "evaluated";
pub const AUTHORING_MESH_V2_ID_POLICY: &str =
    "runtime-derived-lineage-operation-parent-stable-no-reuse@2";

macro_rules! stable_id {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

stable_id!(AuthoringMeshId);
stable_id!(AuthoringMeshLineageId);
stable_id!(AuthoringMeshRevisionId);
stable_id!(AuthoringMeshVertexId);
stable_id!(AuthoringMeshEdgeId);
stable_id!(AuthoringMeshHalfEdgeId);
stable_id!(AuthoringMeshCornerId);
stable_id!(AuthoringMeshFaceId);
stable_id!(AuthoringMeshLoopId);
stable_id!(AuthoringMeshRingId);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoringMeshElementKind {
    Vertex,
    Edge,
    HalfEdge,
    Corner,
    Face,
    Loop,
    Ring,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoringMeshTopologyOperationKind {
    SplitEdge,
    FaceExtrude,
    MoveVertices,
    OpenFrameNotch,
    RearStockVoidRailBow,
    RearStockVoidBoundaryBridge,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshElementRef {
    pub kind: AuthoringMeshElementKind,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshV2Tombstone {
    pub element: AuthoringMeshElementRef,
    pub retired_revision_index: u64,
    pub operation_lineage_sha256: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshTopologyOperation {
    pub schema_version: String,
    pub operation_id: String,
    pub kind: AuthoringMeshTopologyOperationKind,
    pub parent_revision_id: AuthoringMeshRevisionId,
    pub operation_lineage_sha256: String,
    pub source_elements: Vec<AuthoringMeshElementRef>,
    pub generated_elements: Vec<AuthoringMeshElementRef>,
    pub retired_elements: Vec<AuthoringMeshElementRef>,
    pub tombstones: Vec<AuthoringMeshV2Tombstone>,
    pub locality_policy: String,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshVertex {
    pub vertex_id: AuthoringMeshVertexId,
    pub position_m: [f64; 3],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshEdge {
    pub edge_id: AuthoringMeshEdgeId,
    pub vertex_ids: [AuthoringMeshVertexId; 2],
    pub half_edge_ids: Vec<AuthoringMeshHalfEdgeId>,
    pub boundary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshHalfEdge {
    pub half_edge_id: AuthoringMeshHalfEdgeId,
    pub origin_vertex_id: AuthoringMeshVertexId,
    pub edge_id: AuthoringMeshEdgeId,
    pub face_id: AuthoringMeshFaceId,
    pub corner_id: AuthoringMeshCornerId,
    pub next_id: AuthoringMeshHalfEdgeId,
    pub prev_id: AuthoringMeshHalfEdgeId,
    pub twin_id: Option<AuthoringMeshHalfEdgeId>,
    pub boundary: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshCorner {
    pub corner_id: AuthoringMeshCornerId,
    pub half_edge_id: AuthoringMeshHalfEdgeId,
    pub vertex_id: AuthoringMeshVertexId,
    pub face_id: AuthoringMeshFaceId,
    pub ordinal: u32,
    pub uv0: Option<[f64; 2]>,
    pub normal: Option<[f64; 3]>,
    pub tangent: Option<[f64; 4]>,
    pub seam: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshFace {
    pub face_id: AuthoringMeshFaceId,
    pub half_edge_ids: Vec<AuthoringMeshHalfEdgeId>,
    pub loop_id: AuthoringMeshLoopId,
    pub boundary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshLoop {
    pub loop_id: AuthoringMeshLoopId,
    pub face_id: AuthoringMeshFaceId,
    pub half_edge_ids: Vec<AuthoringMeshHalfEdgeId>,
    pub boundary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshRing {
    pub ring_id: AuthoringMeshRingId,
    pub edge_ids: Vec<AuthoringMeshEdgeId>,
    pub closed: bool,
    pub boundary: bool,
}

/// The authoritative original topology.  The arrays are canonicalized by
/// Runtime; clients must not treat array position as identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshOriginal {
    pub namespace: String,
    pub lineage_id: AuthoringMeshLineageId,
    pub vertices: Vec<AuthoringMeshVertex>,
    pub edges: Vec<AuthoringMeshEdge>,
    pub half_edges: Vec<AuthoringMeshHalfEdge>,
    pub corners: Vec<AuthoringMeshCorner>,
    pub faces: Vec<AuthoringMeshFace>,
    pub loops: Vec<AuthoringMeshLoop>,
    pub rings: Vec<AuthoringMeshRing>,
    pub tombstones: Vec<AuthoringMeshV2Tombstone>,
    pub canonical_sha256: String,
}

/// Evaluated geometry is an artifact/readback sidecar only.  It intentionally
/// has no vertex/edge/face IDs that could be mistaken for authored identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshEvaluated {
    pub namespace: String,
    pub source_revision_id: AuthoringMeshRevisionId,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub readback_sha256: String,
    pub correspondence_status: String,
    pub canonical_sha256: String,
}

/// Runtime-derived provenance that binds an authored mesh lineage to one
/// exact candidate-owned GeometryProgram source. Clients cannot supply this
/// through the generic AuthoringMesh durable transport.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshV2SourceBinding {
    pub schema_version: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub artifact_readback_sha256: String,
    pub geometry_program_sha256: String,
    pub source_node_id: String,
    pub part_id: String,
    pub material_zone_id: String,
    pub solid: bool,
    pub source_operator_id: String,
    pub source_parameters_sha256: String,
    pub part_output_sha256: String,
    pub position_m: [f64; 3],
    pub rotation_rad: [f64; 3],
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshRevision {
    pub schema_version: String,
    pub mesh_id: AuthoringMeshId,
    pub lineage_id: AuthoringMeshLineageId,
    pub revision_id: AuthoringMeshRevisionId,
    pub parent_revision_ids: Vec<AuthoringMeshRevisionId>,
    pub revision_index: u64,
    pub operation: Option<AuthoringMeshTopologyOperation>,
    pub original: AuthoringMeshOriginal,
    pub evaluated: Option<AuthoringMeshEvaluated>,
    #[serde(default)]
    pub source_binding: Option<AuthoringMeshV2SourceBinding>,
    /// Optional provenance for a Runtime-derived foundation import.  This is
    /// deliberately separate from `source_binding`: foundation meshes are not
    /// candidate-owned GeometryProgram outputs and must never be represented
    /// as one.  Omitting the field keeps pre-foundation revision JSON and
    /// canonical hashes byte-for-byte compatible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foundation_source_binding: Option<crate::AuthoringMeshV2FoundationSourceBinding>,
    pub id_policy: String,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoringMeshSplitEdgeRequest {
    pub operation_id: String,
    pub parent_revision_id: AuthoringMeshRevisionId,
    pub edge_id: AuthoringMeshEdgeId,
    pub split_ratio_milli: u32,
    pub operation_lineage_sha256: String,
}

/// Move an ordered, bounded set of authored vertices by per-vertex deltas.
/// `vertex_ids` and `delta_m` are parallel arrays.  The Runtime canonicalizes
/// their pair order by vertex ID before applying and journaling the edit.
/// Topology and stable element IDs remain unchanged; Runtime invalidates the
/// evaluated sidecar and records the position-only journal operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshMoveVerticesRequest {
    pub operation_id: String,
    pub parent_revision_id: AuthoringMeshRevisionId,
    pub vertex_ids: Vec<AuthoringMeshVertexId>,
    pub delta_m: Vec<[f64; 3]>,
    pub operation_lineage_sha256: String,
}

/// A bounded hard-surface authoring operation.  The source face must be a
/// planar convex boundary face; Runtime derives the top ring and side faces.
/// No caller-provided element identity is accepted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshFaceExtrudeRequest {
    pub operation_id: String,
    pub parent_revision_id: AuthoringMeshRevisionId,
    pub face_id: AuthoringMeshFaceId,
    pub distance_m: f64,
    pub operation_lineage_sha256: String,
}

/// Cut a bounded, local-Y-facing opening through the lower edge of the
/// current box-like authored mesh.  The Runtime owns face selection and all
/// generated identity; the normalized dimensions are the only shape inputs.
/// The operation is intentionally closed over a local-Z through-cut so it can
/// express an open-stock/U-frame silhouette without accepting caller mesh or
/// arbitrary topology/script data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshOpenFrameNotchRequest {
    pub operation_id: String,
    pub parent_revision_id: AuthoringMeshRevisionId,
    /// Width of the opening as a fraction of the source box local-X span,
    /// expressed in thousandths (1..=999).
    pub opening_width_milli: u32,
    /// Height of the opening from the source box local -Y edge as a fraction
    /// of the local-Y span, expressed in thousandths (1..=999).
    pub opening_height_milli: u32,
    pub operation_lineage_sha256: String,
}

/// Sculpt the void-facing inner boundary of the rear-stock upper rail with a
/// Runtime-owned five-station bow. Runtime derives all topology and element
/// identity; callers cannot provide points, vertex IDs, scripts, or replacement
/// meshes. The fixed station parameters are s=0,.25,.5,.75,1 and d=0,.030,
/// .045,.030,0 metres. The expected void centroid and face normal are typed
/// semantic evidence used only to choose the inward sign.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshRearStockVoidRailBowRequest {
    pub operation_id: String,
    pub parent_revision_id: AuthoringMeshRevisionId,
    pub expected_void_centroid_m: [f64; 3],
    pub expected_void_face_normal_m: [f64; 3],
    pub operation_lineage_sha256: String,
}

/// Bridge the fixed rear-stock upper-inner boundary chain in source-local
/// space.  Runtime owns the chain discovery, five station samples, Y opening
/// offsets, and symmetric Z depth wedge; the public request carries no mesh
/// data, element IDs, camera state, masks, or transforms.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshRearStockVoidBoundaryBridgeRequest {
    pub operation_id: String,
    pub parent_revision_id: AuthoringMeshRevisionId,
    pub operation_lineage_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshFaceExtrudeResult {
    pub schema_version: String,
    pub parent_revision_id: AuthoringMeshRevisionId,
    pub child_revision: AuthoringMeshRevision,
    pub changed_elements: Vec<AuthoringMeshElementRef>,
    pub generated_elements: Vec<AuthoringMeshElementRef>,
    pub retired_elements: Vec<AuthoringMeshElementRef>,
    pub locality_status: String,
    pub evaluated_status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshOpenFrameNotchResult {
    pub schema_version: String,
    pub parent_revision_id: AuthoringMeshRevisionId,
    pub child_revision: AuthoringMeshRevision,
    pub opening_width_milli: u32,
    pub opening_height_milli: u32,
    pub changed_elements: Vec<AuthoringMeshElementRef>,
    pub generated_elements: Vec<AuthoringMeshElementRef>,
    pub retired_elements: Vec<AuthoringMeshElementRef>,
    pub locality_status: String,
    pub evaluated_status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshRearStockVoidRailBowResult {
    pub schema_version: String,
    pub parent_revision_id: AuthoringMeshRevisionId,
    pub child_revision: AuthoringMeshRevision,
    pub station_parameters_m: Vec<[f64; 2]>,
    pub expected_void_centroid_m: [f64; 3],
    pub expected_void_face_normal_m: [f64; 3],
    pub changed_elements: Vec<AuthoringMeshElementRef>,
    pub generated_elements: Vec<AuthoringMeshElementRef>,
    pub retired_elements: Vec<AuthoringMeshElementRef>,
    pub locality_status: String,
    pub evaluated_status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshRearStockVoidBoundaryBridgeResult {
    pub schema_version: String,
    pub parent_revision_id: AuthoringMeshRevisionId,
    pub child_revision: AuthoringMeshRevision,
    /// Each tuple is `[station_fraction, y_opening_offset_m,
    /// z_depth_wedge_m]`.  The sign of the Z wedge is applied symmetrically
    /// to the two depth rails by Runtime.
    pub station_parameters_m: Vec<[f64; 3]>,
    pub changed_elements: Vec<AuthoringMeshElementRef>,
    pub generated_elements: Vec<AuthoringMeshElementRef>,
    pub retired_elements: Vec<AuthoringMeshElementRef>,
    pub locality_status: String,
    pub evaluated_status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshSplitEdgeResult {
    pub schema_version: String,
    pub parent_revision_id: AuthoringMeshRevisionId,
    pub child_revision: AuthoringMeshRevision,
    pub changed_elements: Vec<AuthoringMeshElementRef>,
    pub generated_elements: Vec<AuthoringMeshElementRef>,
    pub retired_elements: Vec<AuthoringMeshElementRef>,
    pub locality_status: String,
    pub evaluated_status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshMoveVerticesResult {
    pub schema_version: String,
    pub parent_revision_id: AuthoringMeshRevisionId,
    pub child_revision: AuthoringMeshRevision,
    pub moved_vertex_ids: Vec<AuthoringMeshVertexId>,
    pub changed_elements: Vec<AuthoringMeshElementRef>,
    pub generated_elements: Vec<AuthoringMeshElementRef>,
    pub retired_elements: Vec<AuthoringMeshElementRef>,
    pub locality_status: String,
    pub evaluated_status: String,
}
