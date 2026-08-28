use forgecad_high_worker::{DetailGraph, HighMeshWorkerRequest};
use forgecad_worker_protocol::canonical_json_sha256;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const AUTHORING_MESH_SCHEMA_VERSION: &str = "AuthoringMeshCanonical@1";
const HIGH_REQUEST_SCHEMA_VERSION: &str = "HighMeshWorkerRequest@1";
const HIGH_OPERATION: &str = "forgecad.production.high-mesh-prepare@1";

fn fixture_sha() -> String {
    "1".repeat(64)
}

fn fixture_lineage(element_id: &str) -> Value {
    let mut lineage = json!({
        "original_element_ids": [element_id],
        "evaluated_element_ids": [],
        "correspondence_kind": "not_materialized",
        "correspondence_sha256": "",
    });
    lineage["correspondence_sha256"] = Value::String(canonical_json_sha256(&json!({
        "original_element_ids": [element_id],
        "evaluated_element_ids": [],
        "correspondence_kind": "not_materialized",
    })));
    lineage
}

/// Build the complete Runtime-owned AuthoringMeshCanonical@1 source shape.
/// Every vertex/edge/half-edge/corner/face/loop carries an opaque stable ID
/// and source lineage; the transport test must exercise the same closed
/// source contract as the worker rather than a marker-only payload.
fn canonical_mesh() -> Value {
    let positions = [
        [0.0_f32, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.2],
        [1.0, 0.0, 0.2],
        [1.0, 1.0, 0.2],
        [0.0, 1.0, 0.2],
    ];
    let vertex_id = |index: usize| format!("v{index:03}");
    let edge_id = |a: usize, b: usize| {
        let (low, high) = if a < b { (a, b) } else { (b, a) };
        format!("e-v{low:03}-v{high:03}")
    };
    let face_specs: [(&str, [usize; 4]); 6] = [
        ("f-bottom", [0, 3, 2, 1]),
        ("f-front", [0, 1, 5, 4]),
        ("f-left", [0, 4, 7, 3]),
        ("f-rear", [3, 7, 6, 2]),
        ("f-right", [1, 2, 6, 5]),
        ("f-top", [4, 5, 6, 7]),
    ];

    #[derive(Clone)]
    struct RawHalfEdge {
        id: String,
        origin: usize,
        destination: usize,
        edge_id: String,
        face_id: String,
        corner_id: String,
        next_id: String,
        prev_id: String,
    }

    let mut raw_half_edges = Vec::with_capacity(24);
    for (face_id, cycle) in face_specs {
        for ordinal in 0..cycle.len() {
            let origin = cycle[ordinal];
            let destination = cycle[(ordinal + 1) % cycle.len()];
            raw_half_edges.push(RawHalfEdge {
                id: format!("he-{face_id}-{ordinal}"),
                origin,
                destination,
                edge_id: edge_id(origin, destination),
                face_id: face_id.to_owned(),
                corner_id: format!("corner-{face_id}-{ordinal}"),
                next_id: format!("he-{face_id}-{}", (ordinal + 1) % cycle.len()),
                prev_id: format!("he-{face_id}-{}", (ordinal + cycle.len() - 1) % cycle.len()),
            });
        }
    }

    let directed_half_edges = raw_half_edges
        .iter()
        .map(|half_edge| {
            (
                (half_edge.origin, half_edge.destination),
                half_edge.id.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut edge_half_edges = BTreeMap::<String, Vec<String>>::new();
    let mut edge_endpoints = BTreeMap::<String, [String; 2]>::new();
    let mut outgoing_half_edges = BTreeMap::<usize, String>::new();
    for half_edge in &raw_half_edges {
        edge_half_edges
            .entry(half_edge.edge_id.clone())
            .or_default()
            .push(half_edge.id.clone());
        let low = vertex_id(half_edge.origin.min(half_edge.destination));
        let high = vertex_id(half_edge.origin.max(half_edge.destination));
        edge_endpoints
            .entry(half_edge.edge_id.clone())
            .or_insert([low, high]);
        outgoing_half_edges
            .entry(half_edge.origin)
            .or_insert_with(|| half_edge.id.clone());
    }

    let vertices = positions
        .iter()
        .enumerate()
        .map(|(index, position)| {
            let id = vertex_id(index);
            json!({
                "vertex_id": id.clone(),
                "position_m": position,
                "outgoing_half_edge_id": outgoing_half_edges[&index],
                "boundary": false,
                "lineage": fixture_lineage(&id),
            })
        })
        .collect::<Vec<_>>();

    let edges = edge_endpoints
        .iter()
        .map(|(id, endpoints)| {
            json!({
                "edge_id": id,
                "vertex_ids": endpoints,
                "half_edge_ids": edge_half_edges[id],
                "boundary": false,
                "hard_edge": true,
                "crease": 0,
                "uv_seam": false,
                "lineage": fixture_lineage(id),
            })
        })
        .collect::<Vec<_>>();

    let half_edges = raw_half_edges
        .iter()
        .map(|half_edge| {
            let twin_id = directed_half_edges
                .get(&(half_edge.destination, half_edge.origin))
                .expect("closed cube half-edge twin");
            let orientation =
                if vertex_id(half_edge.origin) == edge_endpoints[&half_edge.edge_id][0] {
                    "forward"
                } else {
                    "reverse"
                };
            json!({
                "half_edge_id": half_edge.id,
                "origin_vertex_id": vertex_id(half_edge.origin),
                "edge_id": half_edge.edge_id,
                "face_id": half_edge.face_id,
                "corner_id": half_edge.corner_id,
                "twin_id": twin_id,
                "next_id": half_edge.next_id,
                "prev_id": half_edge.prev_id,
                "boundary": false,
                "orientation": orientation,
                "lineage": fixture_lineage(&half_edge.id),
            })
        })
        .collect::<Vec<_>>();

    let corners = raw_half_edges
        .iter()
        .enumerate()
        .map(|(ordinal, half_edge)| {
            json!({
                "corner_id": half_edge.corner_id,
                "face_id": half_edge.face_id,
                "half_edge_id": half_edge.id,
                "vertex_id": vertex_id(half_edge.origin),
                "ordinal": ordinal % 4,
                "lineage": fixture_lineage(&half_edge.corner_id),
            })
        })
        .collect::<Vec<_>>();

    let faces = face_specs
        .iter()
        .map(|(face_id, cycle)| {
            let corner_ids = (0..cycle.len())
                .map(|ordinal| format!("corner-{face_id}-{ordinal}"))
                .collect::<Vec<_>>();
            let id = (*face_id).to_owned();
            json!({
                "face_id": id.clone(),
                "first_half_edge_id": format!("he-{face_id}-0"),
                "corner_ids": corner_ids,
                "degree": cycle.len(),
                "boundary": false,
                "lineage": fixture_lineage(&id),
            })
        })
        .collect::<Vec<_>>();

    let loops = face_specs
        .iter()
        .map(|(face_id, _cycle)| {
            let half_edge_ids = (0..4)
                .map(|ordinal| format!("he-{face_id}-{ordinal}"))
                .collect::<Vec<_>>();
            let id = format!("loop-{face_id}");
            json!({
                "loop_id": id.clone(),
                "face_id": face_id,
                "first_half_edge_id": half_edge_ids[0],
                "half_edge_ids": half_edge_ids,
                "boundary": false,
                "lineage": fixture_lineage(&id),
            })
        })
        .collect::<Vec<_>>();

    let lineage = fixture_sha();
    let mut canonical = json!({
        "schema_version": AUTHORING_MESH_SCHEMA_VERSION,
        "canonical_mesh_id": "canonical-mesh-transport-fixture",
        "project_id": "project-transport-fixture",
        "candidate_id": "candidate-transport-fixture",
        "candidate_state_sha256": fixture_sha(),
        "base_version_id": Value::Null,
        "authoring_node_id": "receiver-source",
        "part_id": "receiver",
        "source_program_object_sha256": fixture_sha(),
        "source_program_sha256": fixture_sha(),
        "source_artifact_object_sha256": fixture_sha(),
        "source_artifact_sha256": fixture_sha(),
        "source_artifact_readback_object_sha256": fixture_sha(),
        "source_artifact_readback_sha256": fixture_sha(),
        "source_lineage_sha256": lineage,
        "representation": "runtime-owned-original-half-edge@1",
        "storage_policy": "runtime-owned-sqlite-cas-canonical-authoring-mesh@1",
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "original_identity": {
            "identity_id": "identity-original-transport-fixture",
            "namespace": "original",
            "identity_kind": "runtime-owned-original-authoring@1",
            "element_id_policy": "lineage-scoped-opaque-not-cross-version-stable@1",
            "topology_sha256": fixture_sha(),
            "source_lineage_sha256": fixture_sha(),
            "stability_scope": "same-canonical-mesh-lineage-only@1",
        },
        "evaluated_identity": {
            "identity_id": "identity-evaluated-transport-fixture",
            "namespace": "evaluated",
            "identity_kind": "runtime-derived-evaluated-artifact-readback@1",
            "element_id_policy": "artifact-local-no-authoring-bijection@1",
            "correspondence_policy": "non-bijective-derived-only@1",
            "artifact_object_sha256": fixture_sha(),
            "artifact_readback_sha256": fixture_sha(),
            "source_lineage_sha256": fixture_sha(),
            "cross_version_stable": false,
        },
        "cross_version_stable": false,
        "cross_version_stability": {
            "status": "not-proven@1",
            "scope": "same-canonical-mesh-lineage-only@1",
            "stable_id_claim": "none-across-revisions@1",
            "deleted_id_reuse_policy": "not-proven-and-not-a-contract@1",
            "new_id_policy": "lineage-operation-parent-derived-draft-only@1",
            "evaluated_id_policy": "artifact-local-unstable-derived-only@1",
        },
        "counts": {
            "vertex_count": 8,
            "edge_count": 12,
            "half_edge_count": 24,
            "corner_count": 24,
            "face_count": 6,
            "loop_count": 6,
            "ring_count": 0,
            "boundary_edge_count": 0,
            "boundary_half_edge_count": 0,
            "hard_edge_count": 12,
            "crease_edge_count": 0,
            "uv_seam_count": 0,
        },
        "vertices": vertices,
        "edges": edges,
        "half_edges": half_edges,
        "corners": corners,
        "faces": faces,
        "loops": loops,
        "rings": [],
        "topology": {
            "boundary_edge_count": 0,
            "boundary_half_edge_count": 0,
            "non_manifold_edge_count": 0,
            "orientation_conflict_count": 0,
            "status": "closed_manifold",
            "validation_status": "passed",
            "rejection_policy": "fail-closed-on-non-manifold@1",
            "face_cycle_policy": "next-prev-complete-mutual@1",
            "twin_policy": "boundary-only-null-symmetric@1",
            "boundary_policy": "single-half-edge-per-boundary-edge@1",
        },
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
        "runtime_write_performed": true,
        "persistent_user_data_touched": true,
        "stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "quality_status": "structural_only",
        "canonical_sha256": "",
    });
    canonical["canonical_sha256"] = Value::String(canonical_json_sha256(&canonical));
    canonical
}

fn source_adapter() -> Value {
    let canonical = canonical_mesh();
    let canonical_sha256 = canonical["canonical_sha256"]
        .as_str()
        .expect("canonical fixture hash")
        .to_owned();
    let candidate_state_sha256 = canonical["candidate_state_sha256"]
        .as_str()
        .expect("candidate fixture state hash")
        .to_owned();
    json!({
        "schema_version": "HighWorkerAuthoringMeshAdapter@1",
        "canonical_mesh": canonical,
        "candidate_id": "candidate-transport-fixture",
        "candidate_state_sha256": candidate_state_sha256,
        "head_candidate_id": "candidate-transport-fixture",
        "head_candidate_state_sha256": fixture_sha(),
        "source_mesh_sha256": canonical_sha256,
    })
}

fn detail_graph() -> Value {
    json!({
        "schema_version": "DetailGraph@1",
        "nodes": [
            {
                "node_id": "crease-top",
                "kind": "crease",
                "parent_part_id": "receiver",
                "parent_node_id": Value::Null,
                "source_edge": "e-v001-v005",
                "width_m": Value::Null,
                "count": Value::Null,
                "sharpness": 3.0,
                "center_m": Value::Null,
                "size_m": Value::Null,
            },
            {
                "node_id": "floater-side",
                "kind": "floating_detail",
                "parent_part_id": "receiver",
                "parent_node_id": Value::Null,
                "source_edge": Value::Null,
                "width_m": Value::Null,
                "count": Value::Null,
                "sharpness": Value::Null,
                "center_m": [0.5, 0.5, 0.45],
                "size_m": [0.2, 0.2, 0.05],
            },
            {
                "node_id": "support-top",
                "kind": "support_loop",
                "parent_part_id": "receiver",
                "parent_node_id": Value::Null,
                "source_edge": "e-v000-v001",
                "width_m": 0.02,
                "count": 2,
                "sharpness": Value::Null,
                "center_m": Value::Null,
                "size_m": Value::Null,
            },
        ],
    })
}

pub fn high_mesh_payload() -> Value {
    let source_authoring_mesh = source_adapter();
    let detail_graph = detail_graph();
    let mut payload = json!({
        "schema_version": HIGH_REQUEST_SCHEMA_VERSION,
        "operation": HIGH_OPERATION,
        "source_authoring_mesh": source_authoring_mesh,
        "source_authoring_mesh_sha256": "",
        "detail_graph": detail_graph,
        "detail_graph_canonical_sha256": "",
        "budgets": {
            "max_detail_nodes": 16,
            "max_output_vertices": 1024,
            "max_output_triangles": 2048,
        },
        "canonical_sha256": "",
    });
    payload["source_authoring_mesh_sha256"] =
        Value::String(canonical_json_sha256(&payload["source_authoring_mesh"]));
    // The worker hashes its typed DetailGraph/HighMeshWorkerRequest after
    // deserialization.  Re-serialize those public production contracts here
    // so serde's f32/Option representation cannot make the transport fixture
    // accidentally differ from the worker's canonical preimage.
    let typed_detail_graph: DetailGraph =
        serde_json::from_value(payload["detail_graph"].clone()).expect("detail graph fixture");
    payload["detail_graph_canonical_sha256"] = Value::String(canonical_json_sha256(
        &serde_json::to_value(typed_detail_graph).expect("detail graph JSON"),
    ));
    let typed_request: HighMeshWorkerRequest =
        serde_json::from_value(payload.clone()).expect("high worker request fixture");
    let mut request_preimage = serde_json::to_value(typed_request).expect("request JSON");
    request_preimage["canonical_sha256"] = Value::String(String::new());
    payload["canonical_sha256"] = Value::String(canonical_json_sha256(&request_preimage));
    payload
}

pub fn worker_request() -> Value {
    json!({
        "protocol": "forgecad-worker-protocol@1",
        "request_id": "native-high-transport-positive-1",
        "operation": HIGH_OPERATION,
        "payload": high_mesh_payload(),
    })
}
