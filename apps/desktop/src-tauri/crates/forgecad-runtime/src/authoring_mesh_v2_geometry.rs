//! Closed lowering from Runtime-owned `AuthoringMeshRevision@2` truth to the
//! existing fixed `forgecad.geometry.authoring-mesh@1` Worker operator.
//!
//! This adapter never accepts caller-owned element IDs or a GeometryProgram.
//! It first revalidates the immutable AuthoringMesh revision, then projects
//! the exact stable topology into the older triangle/quad operator surface.
//! Part/source/material ownership remains a separate Runtime concern.

use super::{authoring_mesh_v2::AuthoringMeshV2Revision, canonical_json_hash, RuntimeError};
use forgecad_contracts::{AuthoringMeshEdge, AuthoringMeshHalfEdge, AuthoringMeshRevision};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const MAX_TRANSFORM_ABS: f64 = 10.0;

#[derive(Clone, Debug)]
pub(crate) struct AuthoringMeshV2SourceGenesis {
    pub positions_m: Vec<[f64; 3]>,
    pub faces: Vec<Vec<usize>>,
    pub position_m: [f64; 3],
    pub rotation_rad: [f64; 3],
    pub source_node_id: String,
    pub source_operator_id: String,
    pub source_parameters_sha256: String,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "AUTHORING_MESH_V2_GEOMETRY_INVALID: {}",
        message.into()
    ))
}

fn validate_transform(value: [f64; 3], field: &str) -> Result<(), RuntimeError> {
    if value
        .iter()
        .any(|component| !component.is_finite() || component.abs() > MAX_TRANSFORM_ABS)
    {
        return Err(invalid(format!("{field} is not finite or bounded")));
    }
    Ok(())
}

/// Convert the current product-owned primitive box source into an editable
/// local-space AuthoringMesh genesis.  This is deliberately narrow: it is the
/// exact real-D1 rear-stock source shape and cannot act as a generic mesh
/// importer.  Runtime must still bind the returned projection to the durable
/// candidate/program/Part before it may be persisted or used by an ActionRun.
pub(crate) fn primitive_box_source_genesis(
    node: &Value,
    expected_node_id: &str,
) -> Result<AuthoringMeshV2SourceGenesis, RuntimeError> {
    let object = node
        .as_object()
        .ok_or_else(|| invalid("source node must be an object"))?;
    let expected = ["node_id", "operator_id", "inputs", "parameters"];
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid(
            "source node differs from the closed GeometryProgram node",
        ));
    }
    let node_id = object
        .get("node_id")
        .and_then(Value::as_str)
        .filter(|value| *value == expected_node_id)
        .ok_or_else(|| invalid("source node identity differs"))?;
    let operator_id = object
        .get("operator_id")
        .and_then(Value::as_str)
        .filter(|value| *value == "forgecad.geometry.primitive@2")
        .ok_or_else(|| invalid("source operator is not the fixed primitive@2 box path"))?;
    if object
        .get("inputs")
        .and_then(Value::as_array)
        .is_none_or(|inputs| !inputs.is_empty())
    {
        return Err(invalid("primitive box source must not have inputs"));
    }
    let parameters = object
        .get("parameters")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("primitive box parameters are missing"))?;
    let parameter_keys = ["shape", "size_m", "position_m", "rotation_rad"];
    if parameters.len() != parameter_keys.len()
        || parameter_keys
            .iter()
            .any(|key| !parameters.contains_key(*key))
        || parameters.get("shape").and_then(Value::as_str) != Some("box")
    {
        return Err(invalid(
            "primitive box parameters differ from the closed source",
        ));
    }
    let vector = |field: &str| -> Result<[f64; 3], RuntimeError> {
        let values = parameters
            .get(field)
            .and_then(Value::as_array)
            .filter(|values| values.len() == 3)
            .ok_or_else(|| invalid(format!("{field} must contain three values")))?;
        let result = [
            values[0]
                .as_f64()
                .ok_or_else(|| invalid(format!("{field}[0] is invalid")))?,
            values[1]
                .as_f64()
                .ok_or_else(|| invalid(format!("{field}[1] is invalid")))?,
            values[2]
                .as_f64()
                .ok_or_else(|| invalid(format!("{field}[2] is invalid")))?,
        ];
        validate_transform(result, field)?;
        Ok(result)
    };
    let size_m = vector("size_m")?;
    if size_m.iter().any(|value| *value <= 1.0e-6) {
        return Err(invalid("primitive box size is degenerate"));
    }
    let position_m = vector("position_m")?;
    let rotation_rad = vector("rotation_rad")?;
    let [x, y, z] = [size_m[0] * 0.5, size_m[1] * 0.5, size_m[2] * 0.5];
    let positions_m = vec![
        [-x, -y, -z],
        [x, -y, -z],
        [x, y, -z],
        [-x, y, -z],
        [-x, -y, z],
        [x, -y, z],
        [x, y, z],
        [-x, y, z],
    ];
    let faces = vec![
        vec![0, 3, 2, 1],
        vec![4, 5, 6, 7],
        vec![0, 1, 5, 4],
        vec![3, 7, 6, 2],
        vec![0, 4, 7, 3],
        vec![1, 2, 6, 5],
    ];
    Ok(AuthoringMeshV2SourceGenesis {
        positions_m,
        faces,
        position_m,
        rotation_rad,
        source_node_id: node_id.to_owned(),
        source_operator_id: operator_id.to_owned(),
        source_parameters_sha256: canonical_json_hash(&Value::Object(parameters.clone())),
    })
}

/// Materialize one fixed Worker node parameter object from an immutable V2
/// revision.  The current Geometry Worker authoring operator is intentionally
/// triangle/quad-only; n-gons fail closed until the Worker surface is raised.
pub(crate) fn authoring_mesh_v2_geometry_parameters(
    revision: &AuthoringMeshRevision,
    position_m: [f64; 3],
    rotation_rad: [f64; 3],
) -> Result<Value, RuntimeError> {
    AuthoringMeshV2Revision::from_record(revision.clone())?;
    validate_transform(position_m, "position_m")?;
    validate_transform(rotation_rad, "rotation_rad")?;

    let edges_by_id = revision
        .original
        .edges
        .iter()
        .map(|edge| (edge.edge_id.0.as_str(), edge))
        .collect::<BTreeMap<_, _>>();
    let half_edges_by_id = revision
        .original
        .half_edges
        .iter()
        .map(|half_edge| (half_edge.half_edge_id.0.as_str(), half_edge))
        .collect::<BTreeMap<_, _>>();

    let mut loops = Vec::new();
    let mut faces = Vec::new();
    let mut referenced_vertex_ids = BTreeSet::new();
    let mut referenced_edge_ids = BTreeSet::new();
    for face in &revision.original.faces {
        if !(3..=4).contains(&face.half_edge_ids.len()) {
            return Err(invalid(format!(
                "face {} is not triangle/quad Worker-compatible",
                face.face_id.0
            )));
        }
        let mut ordered = face
            .half_edge_ids
            .iter()
            .map(|id| {
                half_edges_by_id
                    .get(id.0.as_str())
                    .copied()
                    .ok_or_else(|| invalid("face references an unknown half-edge"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let first = ordered
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| left.corner_id.0.cmp(&right.corner_id.0))
            .map(|(index, _)| index)
            .ok_or_else(|| invalid("face has no half-edges"))?;
        ordered.rotate_left(first);

        let mut face_loop_ids = Vec::with_capacity(ordered.len());
        for (ordinal, half_edge) in ordered.iter().enumerate() {
            let next = ordered[(ordinal + 1) % ordered.len()];
            let edge = edges_by_id
                .get(half_edge.edge_id.0.as_str())
                .copied()
                .ok_or_else(|| invalid("half-edge references an unknown edge"))?;
            let edge_forward = edge_direction(edge, half_edge, next)?;
            referenced_vertex_ids.insert(half_edge.origin_vertex_id.0.as_str());
            referenced_edge_ids.insert(half_edge.edge_id.0.as_str());
            face_loop_ids.push(half_edge.corner_id.0.clone());
            loops.push(json!({
                "element_id":half_edge.corner_id.0,
                "face_id":face.face_id.0,
                "ordinal":ordinal,
                "vertex_id":half_edge.origin_vertex_id.0,
                "edge_id":half_edge.edge_id.0,
                "edge_forward":edge_forward,
            }));
        }
        faces.push(json!({
            "element_id":face.face_id.0,
            "loop_ids":face_loop_ids,
        }));
    }
    loops.sort_by(|left, right| {
        left["element_id"]
            .as_str()
            .cmp(&right["element_id"].as_str())
    });

    // AuthoringMesh truth may preserve source vertices/edges that became
    // unreachable after deterministic degenerate-face removal.  The fixed
    // Worker operator deliberately rejects such elements.  Project only the
    // face-referenced stable IDs here; the durable source revision and its
    // canonical hash remain untouched and continue to bind this projection.
    let vertices = revision
        .original
        .vertices
        .iter()
        .filter(|vertex| referenced_vertex_ids.contains(vertex.vertex_id.0.as_str()))
        .map(|vertex| {
            json!({
                "element_id":vertex.vertex_id.0,
                "position_m":vertex.position_m,
            })
        })
        .collect::<Vec<_>>();
    let edges = revision
        .original
        .edges
        .iter()
        .filter(|edge| referenced_edge_ids.contains(edge.edge_id.0.as_str()))
        .map(|edge| {
            let mut endpoints = [edge.vertex_ids[0].0.clone(), edge.vertex_ids[1].0.clone()];
            endpoints.sort();
            json!({
                "element_id":edge.edge_id.0,
                "vertex_ids":endpoints,
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "shape":"authoring-mesh",
        "topology_policy":"triangle-quad-manifold-with-boundary@1",
        "vertices":vertices,
        "edges":edges,
        "loops":loops,
        "faces":faces,
        "position_m":position_m,
        "rotation_rad":rotation_rad,
    }))
}

/// Build the fixed Worker projection used by imported foundation candidates.
/// glTF commonly duplicates vertices at material, normal and UV seams even
/// when their positions describe one welded surface. Runtime preserves those
/// source IDs in the durable revision, while this representation welds the
/// same 1e-6 metre position buckets used by strict GLB readback, removes only
/// faces that collapse or duplicate after welding, and deterministically
/// rebuilds edge/loop IDs. The resulting program remains bound to the exact
/// source revision hash and is structural preview topology, not a retopo or
/// Hero-UV result.
pub(crate) fn authoring_mesh_v2_welded_geometry_parameters(
    revision: &AuthoringMeshRevision,
    position_m: [f64; 3],
    rotation_rad: [f64; 3],
) -> Result<Value, RuntimeError> {
    AuthoringMeshV2Revision::from_record(revision.clone())?;
    validate_transform(position_m, "position_m")?;
    validate_transform(rotation_rad, "rotation_rad")?;

    let vertex_by_id = revision
        .original
        .vertices
        .iter()
        .map(|vertex| (vertex.vertex_id.0.as_str(), vertex))
        .collect::<BTreeMap<_, _>>();
    let half_edges_by_id = revision
        .original
        .half_edges
        .iter()
        .map(|half_edge| (half_edge.half_edge_id.0.as_str(), half_edge))
        .collect::<BTreeMap<_, _>>();

    let mut representative_by_bucket = BTreeMap::<[i64; 3], String>::new();
    let mut representative_by_vertex = BTreeMap::<String, String>::new();
    let mut representative_positions = BTreeMap::<String, [f64; 3]>::new();
    for vertex in &revision.original.vertices {
        let bucket = vertex
            .position_m
            .map(|component| (component * 1_000_000.0).round() as i64);
        let representative = representative_by_bucket
            .entry(bucket)
            .or_insert_with(|| vertex.vertex_id.0.clone())
            .clone();
        representative_by_vertex.insert(vertex.vertex_id.0.clone(), representative.clone());
        representative_positions
            .entry(representative)
            .or_insert(vertex.position_m);
    }

    let mut face_windings = Vec::<(String, Vec<String>)>::new();
    let mut canonical_face_sets = BTreeSet::<Vec<String>>::new();
    for face in &revision.original.faces {
        if !(3..=4).contains(&face.half_edge_ids.len()) {
            return Err(invalid(format!(
                "face {} is not triangle/quad Worker-compatible",
                face.face_id.0
            )));
        }
        let mut ordered = face
            .half_edge_ids
            .iter()
            .map(|id| {
                half_edges_by_id
                    .get(id.0.as_str())
                    .copied()
                    .ok_or_else(|| invalid("face references an unknown half-edge"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let first = ordered
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| left.corner_id.0.cmp(&right.corner_id.0))
            .map(|(index, _)| index)
            .ok_or_else(|| invalid("face has no half-edges"))?;
        ordered.rotate_left(first);
        let welded = ordered
            .iter()
            .map(|half_edge| {
                representative_by_vertex
                    .get(&half_edge.origin_vertex_id.0)
                    .cloned()
                    .ok_or_else(|| invalid("half-edge references an unknown vertex"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if welded.iter().collect::<BTreeSet<_>>().len() != welded.len() {
            continue;
        }
        let mut face_set = welded.clone();
        face_set.sort();
        if !canonical_face_sets.insert(face_set) {
            continue;
        }
        face_windings.push((face.face_id.0.clone(), welded));
    }
    if face_windings.is_empty() {
        return Err(invalid("welded projection contains no faces"));
    }

    // A foundation GLB may package several intersecting hard-surface shells
    // in one primitive. Keep every deterministically closed, consistently
    // wound manifold component, while excluding components that touch an
    // over-shared or one-sided welded edge. This is a review representation;
    // the complete source revision remains durable and unchanged.
    let mut edge_incidence = BTreeMap::<(String, String), Vec<(usize, bool)>>::new();
    for (face_index, (_, winding)) in face_windings.iter().enumerate() {
        for ordinal in 0..winding.len() {
            let origin = winding[ordinal].clone();
            let target = winding[(ordinal + 1) % winding.len()].clone();
            let endpoints = if origin < target {
                (origin.clone(), target.clone())
            } else {
                (target.clone(), origin.clone())
            };
            edge_incidence
                .entry(endpoints.clone())
                .or_default()
                .push((face_index, origin == endpoints.0));
        }
    }
    let mut invalid_faces = BTreeSet::<usize>::new();
    let mut adjacency = vec![Vec::<usize>::new(); face_windings.len()];
    for incidence in edge_incidence.values() {
        if incidence.len() == 2 && incidence[0].1 != incidence[1].1 {
            adjacency[incidence[0].0].push(incidence[1].0);
            adjacency[incidence[1].0].push(incidence[0].0);
        } else {
            invalid_faces.extend(incidence.iter().map(|(face_index, _)| *face_index));
        }
    }
    let mut visited = BTreeSet::<usize>::new();
    let mut retained = BTreeSet::<usize>::new();
    for seed in 0..face_windings.len() {
        if visited.contains(&seed) {
            continue;
        }
        let mut pending = vec![seed];
        let mut component = Vec::new();
        let mut valid = true;
        while let Some(face_index) = pending.pop() {
            if !visited.insert(face_index) {
                continue;
            }
            valid &= !invalid_faces.contains(&face_index);
            component.push(face_index);
            for neighbor in &adjacency[face_index] {
                if !visited.contains(neighbor) {
                    pending.push(*neighbor);
                }
            }
        }
        if valid {
            retained.extend(component);
        }
    }
    face_windings = face_windings
        .into_iter()
        .enumerate()
        .filter_map(|(index, face)| retained.contains(&index).then_some(face))
        .collect();
    if face_windings.is_empty() {
        return Err(invalid(
            "foundation weapon contains no closed consistently wound manifold shell",
        ));
    }

    let mut used_vertices = BTreeSet::<String>::new();
    let mut edge_id_by_endpoints = BTreeMap::<(String, String), String>::new();
    for (_, winding) in &face_windings {
        for ordinal in 0..winding.len() {
            let left = winding[ordinal].clone();
            let right = winding[(ordinal + 1) % winding.len()].clone();
            used_vertices.insert(left.clone());
            used_vertices.insert(right.clone());
            let endpoints = if left < right {
                (left, right)
            } else {
                (right, left)
            };
            edge_id_by_endpoints
                .entry(endpoints.clone())
                .or_insert_with(|| {
                    format!(
                        "weld-edge-{}",
                        &canonical_json_hash(&json!([endpoints.0, endpoints.1]))[..32]
                    )
                });
        }
    }

    let mut vertices = used_vertices
        .iter()
        .map(|vertex_id| {
            let position = representative_positions
                .get(vertex_id)
                .copied()
                .or_else(|| {
                    vertex_by_id
                        .get(vertex_id.as_str())
                        .map(|value| value.position_m)
                })
                .ok_or_else(|| invalid("welded representative position disappeared"))?;
            Ok(json!({"element_id":vertex_id,"position_m":position}))
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    vertices.sort_by(|left, right| {
        left["element_id"]
            .as_str()
            .cmp(&right["element_id"].as_str())
    });

    let mut edges = edge_id_by_endpoints
        .iter()
        .map(|(endpoints, edge_id)| {
            json!({"element_id":edge_id,"vertex_ids":[endpoints.0,endpoints.1]})
        })
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| {
        left["element_id"]
            .as_str()
            .cmp(&right["element_id"].as_str())
    });

    let mut loops = Vec::new();
    let mut faces = Vec::new();
    for (source_face_id, winding) in face_windings {
        let face_hash = canonical_json_hash(&json!([&source_face_id, &winding]));
        let face_id = format!("weld-face-{}", &face_hash[..32]);
        let mut loop_ids = Vec::with_capacity(winding.len());
        for ordinal in 0..winding.len() {
            let origin = winding[ordinal].clone();
            let target = winding[(ordinal + 1) % winding.len()].clone();
            let endpoints = if origin < target {
                (origin.clone(), target.clone())
            } else {
                (target.clone(), origin.clone())
            };
            let edge_id = edge_id_by_endpoints
                .get(&endpoints)
                .ok_or_else(|| invalid("welded edge disappeared"))?;
            let loop_id = format!("weld-loop-{}-{ordinal:02}", &face_hash[..24]);
            let edge_forward = origin == endpoints.0;
            loop_ids.push(loop_id.clone());
            loops.push(json!({
                "element_id":loop_id,
                "face_id":face_id,
                "ordinal":ordinal,
                "vertex_id":origin,
                "edge_id":edge_id,
                "edge_forward":edge_forward,
            }));
        }
        faces.push(json!({"element_id":face_id,"loop_ids":loop_ids}));
    }
    loops.sort_by(|left, right| {
        left["element_id"]
            .as_str()
            .cmp(&right["element_id"].as_str())
    });
    faces.sort_by(|left, right| {
        left["element_id"]
            .as_str()
            .cmp(&right["element_id"].as_str())
    });

    Ok(json!({
        "shape":"authoring-mesh",
        "topology_policy":"triangle-quad-manifold-with-boundary@1",
        "vertices":vertices,
        "edges":edges,
        "loops":loops,
        "faces":faces,
        "position_m":position_m,
        "rotation_rad":rotation_rad,
    }))
}

fn edge_direction(
    edge: &AuthoringMeshEdge,
    current: &AuthoringMeshHalfEdge,
    next: &AuthoringMeshHalfEdge,
) -> Result<bool, RuntimeError> {
    let mut endpoints = [edge.vertex_ids[0].0.as_str(), edge.vertex_ids[1].0.as_str()];
    endpoints.sort();
    let origin = current.origin_vertex_id.0.as_str();
    let target = next.origin_vertex_id.0.as_str();
    if origin == endpoints[0] && target == endpoints[1] {
        Ok(true)
    } else if origin == endpoints[1] && target == endpoints[0] {
        Ok(false)
    } else {
        Err(invalid(
            "half-edge direction differs from its edge endpoints",
        ))
    }
}

/// Hash used by later Runtime proposal materializers to bind the exact V2
/// revision and its Worker-compatible projection without adding a second mesh
/// identity namespace.
pub(crate) fn authoring_mesh_v2_geometry_projection_sha256(
    revision: &AuthoringMeshRevision,
    parameters: &Value,
) -> String {
    canonical_json_hash(&json!({
        "schema_version":"AuthoringMeshV2GeometryProjection@1",
        "revision_id":revision.revision_id.0,
        "revision_sha256":revision.canonical_sha256,
        "operator_id":"forgecad.geometry.authoring-mesh@1",
        "parameters":parameters,
    }))
}
