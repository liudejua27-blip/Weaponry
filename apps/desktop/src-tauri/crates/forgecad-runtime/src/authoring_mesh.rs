//! Runtime-owned, read-only half-edge projection for one authored Part.
//!
//! The source of truth remains the candidate-bound `GeometryProgram@2` and
//! its durable `GeometryCandidateEvidence@2`.  This module only projects the
//! already validated direct `authoring-mesh@1` source node into an explicit
//! V/E/half-edge/corner/face view.  It deliberately does not write CAS,
//! SQLite, candidates, versions, jobs, or stage state.

use super::{
    authoring_topology::{self, AuthoringContext},
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, Runtime, RuntimeError,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_VERTICES: usize = 8192;
const MAX_EDGES: usize = 16384;
const MAX_HALF_EDGES: usize = 32768;
const MAX_CORNERS: usize = 32768;
const MAX_FACES: usize = 8192;
const MAX_FACE_DEGREE: usize = 32;
const MAX_COORDINATE_M: f64 = 10.0;
const MIN_EDGE_LENGTH_M: f64 = 1.0e-6;
const MIN_TRIANGLE_AREA: f64 = 1.0e-12;

// Keep this value aligned with the closed AuthoringMesh request/result
// contracts.  The source topology policy is checked by AuthoringTopology's
// existing loader; this policy additionally binds the half-edge projection.
const AUTHORING_MESH_POLICY_SHA256: &str =
    "aa72cadabba90ddb43dd0014cfa434ab9b13f4e072b09258072f37334c72e709";

#[derive(Clone, Debug)]
struct SourceVertex {
    id: String,
    position: [f64; 3],
}

#[derive(Clone, Debug)]
struct SourceEdge {
    id: String,
    vertex_ids: [String; 2],
}

#[derive(Clone, Debug)]
struct SourceLoop {
    id: String,
    face_id: String,
    ordinal: usize,
    vertex_id: String,
    edge_id: String,
    edge_forward: bool,
}

#[derive(Clone, Debug)]
struct SourceFace {
    id: String,
    loop_ids: Vec<String>,
}

#[derive(Clone, Debug)]
struct SourceMesh {
    vertices: Vec<SourceVertex>,
    edges: Vec<SourceEdge>,
    loops: Vec<SourceLoop>,
    faces: Vec<SourceFace>,
    edge_incidence: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug)]
struct HalfEdgeWork {
    source_loop_id: String,
    source_face_id: String,
    source_edge_id: String,
    source_vertex_id: String,
    edge_forward: bool,
    half_edge_id: String,
    next_id: String,
    prev_id: String,
    twin_id: Option<String>,
    boundary: bool,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("AUTHORING_MESH_INVALID: {}", message.into()))
}

fn exact_object<'a>(
    value: &'a Value,
    keys: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{context} must be an object")))?;
    let expected = keys.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(invalid(format!("{context} fields differ")));
    }
    Ok(object)
}

fn required_text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{key} is required")))
}

fn required_identifier<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, RuntimeError> {
    let value = required_text(object, key)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!("{key} is not an identifier")));
    }
    Ok(value)
}

fn required_sha<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = required_text(object, key)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{key} is not a SHA-256")));
    }
    Ok(value)
}

fn required_array<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a Vec<Value>, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("{key} must be an array")))
}

fn required_array_value<'a>(
    value: &'a Value,
    context: &str,
) -> Result<&'a Vec<Value>, RuntimeError> {
    value
        .as_array()
        .ok_or_else(|| invalid(format!("{context} must be an array")))
}

fn finite_vec3(value: &Value, context: &str) -> Result<[f64; 3], RuntimeError> {
    let values = required_array_value(value, context)?;
    if values.len() != 3 {
        return Err(invalid(format!("{context} must have three coordinates")));
    }
    let result = [
        values[0].as_f64().unwrap_or(f64::NAN),
        values[1].as_f64().unwrap_or(f64::NAN),
        values[2].as_f64().unwrap_or(f64::NAN),
    ];
    if result
        .iter()
        .any(|value| !value.is_finite() || value.abs() > MAX_COORDINATE_M)
    {
        return Err(invalid(format!("{context} must be finite and bounded")));
    }
    Ok(result)
}

fn canonical_ids<T>(
    items: &[T],
    mut id: impl FnMut(&T) -> &str,
    label: &str,
) -> Result<(), RuntimeError> {
    let mut previous: Option<&str> = None;
    for item in items {
        let current = id(item);
        if !is_opaque_id(current) {
            return Err(invalid(format!("{label} ID is invalid")));
        }
        if previous.is_some_and(|previous| previous >= current) {
            return Err(invalid(format!(
                "{label} IDs must be unique and lexically sorted"
            )));
        }
        previous = Some(current);
    }
    Ok(())
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let delta = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt()
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
    let cross = cross(subtract(b, a), subtract(c, a));
    0.5 * (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt()
}

fn parse_source(parameters: &Map<String, Value>) -> Result<SourceMesh, RuntimeError> {
    exact_object(
        &Value::Object(parameters.clone()),
        &[
            "shape",
            "topology_policy",
            "vertices",
            "edges",
            "loops",
            "faces",
            "position_m",
            "rotation_rad",
        ],
        "AuthoringMeshParameters@1",
    )?;
    if parameters.get("shape").and_then(Value::as_str) != Some("authoring-mesh")
        || parameters.get("topology_policy").and_then(Value::as_str)
            != Some("triangle-quad-manifold-with-boundary@1")
    {
        return Err(invalid("authoring-mesh source policy differs"));
    }
    // The transform is deliberately not applied to authored coordinates.  It
    // belongs to the source node and is already covered by the candidate GLB
    // replay; the half-edge projection remains authoring-local.
    finite_vec3(
        parameters
            .get("position_m")
            .ok_or_else(|| invalid("position_m is missing"))?,
        "position_m",
    )?;
    finite_vec3(
        parameters
            .get("rotation_rad")
            .ok_or_else(|| invalid("rotation_rad is missing"))?,
        "rotation_rad",
    )?;

    let vertex_values = required_array(parameters, "vertices")?;
    if !(3..=MAX_VERTICES).contains(&vertex_values.len()) {
        return Err(invalid("vertices count is outside the bounded range"));
    }
    let mut vertices = Vec::with_capacity(vertex_values.len());
    for value in vertex_values {
        let object = exact_object(value, &["element_id", "position_m"], "authoring vertex")?;
        vertices.push(SourceVertex {
            id: required_identifier(object, "element_id")?.to_owned(),
            position: finite_vec3(
                object
                    .get("position_m")
                    .ok_or_else(|| invalid("vertex position is missing"))?,
                "vertex position",
            )?,
        });
    }
    canonical_ids(&vertices, |item| item.id.as_str(), "vertex")?;
    let vertex_by_id = vertices
        .iter()
        .enumerate()
        .map(|(index, vertex)| (vertex.id.clone(), index))
        .collect::<BTreeMap<_, _>>();

    let edge_values = required_array(parameters, "edges")?;
    if !(3..=MAX_EDGES).contains(&edge_values.len()) {
        return Err(invalid("edges count is outside the bounded range"));
    }
    let mut edges = Vec::with_capacity(edge_values.len());
    for value in edge_values {
        let object = exact_object(value, &["element_id", "vertex_ids"], "authoring edge")?;
        let endpoints = required_array(object, "vertex_ids")?;
        if endpoints.len() != 2 {
            return Err(invalid("edge must have two vertex IDs"));
        }
        let left = endpoints[0]
            .as_str()
            .filter(|id| is_opaque_id(id))
            .ok_or_else(|| invalid("edge vertex ID is invalid"))?;
        let right = endpoints[1]
            .as_str()
            .filter(|id| is_opaque_id(id))
            .ok_or_else(|| invalid("edge vertex ID is invalid"))?;
        if left >= right || !vertex_by_id.contains_key(left) || !vertex_by_id.contains_key(right) {
            return Err(invalid(
                "edge endpoints must be known, distinct and lexical",
            ));
        }
        edges.push(SourceEdge {
            id: required_identifier(object, "element_id")?.to_owned(),
            vertex_ids: [left.to_owned(), right.to_owned()],
        });
    }
    canonical_ids(&edges, |item| item.id.as_str(), "edge")?;
    let edge_by_id = edges
        .iter()
        .enumerate()
        .map(|(index, edge)| (edge.id.clone(), index))
        .collect::<BTreeMap<_, _>>();

    let loop_values = required_array(parameters, "loops")?;
    if !(3..=MAX_HALF_EDGES).contains(&loop_values.len()) {
        return Err(invalid("loops count is outside the bounded range"));
    }
    let mut loops = Vec::with_capacity(loop_values.len());
    for value in loop_values {
        let object = exact_object(
            value,
            &[
                "element_id",
                "face_id",
                "ordinal",
                "vertex_id",
                "edge_id",
                "edge_forward",
            ],
            "authoring loop",
        )?;
        let ordinal = object
            .get("ordinal")
            .and_then(Value::as_u64)
            .filter(|ordinal| *ordinal < MAX_FACE_DEGREE as u64)
            .ok_or_else(|| invalid("loop ordinal is invalid"))? as usize;
        let vertex_id = required_text(object, "vertex_id")?.to_owned();
        let edge_id = required_text(object, "edge_id")?.to_owned();
        if !is_opaque_id(&vertex_id)
            || !is_opaque_id(&edge_id)
            || !vertex_by_id.contains_key(&vertex_id)
            || !edge_by_id.contains_key(&edge_id)
        {
            return Err(invalid("loop references an unknown vertex or edge"));
        }
        loops.push(SourceLoop {
            id: required_identifier(object, "element_id")?.to_owned(),
            face_id: required_identifier(object, "face_id")?.to_owned(),
            ordinal,
            vertex_id,
            edge_id,
            edge_forward: object
                .get("edge_forward")
                .and_then(Value::as_bool)
                .ok_or_else(|| invalid("loop edge_forward is invalid"))?,
        });
    }
    canonical_ids(&loops, |item| item.id.as_str(), "loop")?;
    let loop_by_id = loops
        .iter()
        .enumerate()
        .map(|(index, item)| (item.id.clone(), index))
        .collect::<BTreeMap<_, _>>();

    let face_values = required_array(parameters, "faces")?;
    if !(1..=MAX_FACES).contains(&face_values.len()) {
        return Err(invalid("faces count is outside the bounded range"));
    }
    let mut faces = Vec::with_capacity(face_values.len());
    let mut edge_incidence = BTreeMap::<String, Vec<String>>::new();
    let mut used_vertices = BTreeSet::new();
    let mut used_loops = BTreeSet::new();
    let mut seen_face_vertex_sets = BTreeSet::<Vec<String>>::new();
    for value in face_values {
        let object = exact_object(value, &["element_id", "loop_ids"], "authoring face")?;
        let face_id = required_identifier(object, "element_id")?.to_owned();
        let loop_values = required_array(object, "loop_ids")?;
        if !(3..=MAX_FACE_DEGREE).contains(&loop_values.len()) {
            return Err(invalid("face degree is outside the bounded range"));
        }
        let loop_ids = loop_values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|id| is_opaque_id(id))
                    .map(str::to_owned)
                    .ok_or_else(|| invalid("face loop ID is invalid"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if loop_ids.iter().collect::<BTreeSet<_>>().len() != loop_ids.len()
            || loop_ids.first() != loop_ids.iter().min()
        {
            return Err(invalid("face loops must be unique and rotation-canonical"));
        }
        let mut face_vertices = Vec::with_capacity(loop_ids.len());
        let mut face_edges = BTreeSet::new();
        for (ordinal, loop_id) in loop_ids.iter().enumerate() {
            let loop_index = loop_by_id
                .get(loop_id)
                .ok_or_else(|| invalid("face references an unknown loop"))?;
            let current = &loops[*loop_index];
            let next = &loops[*loop_by_id
                .get(&loop_ids[(ordinal + 1) % loop_ids.len()])
                .expect("next loop is validated above")];
            if current.face_id != face_id || current.ordinal != ordinal {
                return Err(invalid("loop face/ordinal differs from face cycle"));
            }
            let edge = &edges[*edge_by_id
                .get(&current.edge_id)
                .expect("edge was validated above")];
            let expected_start = if current.edge_forward {
                edge.vertex_ids[0].as_str()
            } else {
                edge.vertex_ids[1].as_str()
            };
            let expected_end = if current.edge_forward {
                edge.vertex_ids[1].as_str()
            } else {
                edge.vertex_ids[0].as_str()
            };
            if current.vertex_id != expected_start || next.vertex_id != expected_end {
                return Err(invalid("loop edge direction differs from face winding"));
            }
            if !face_edges.insert(current.edge_id.clone()) {
                return Err(invalid("face cannot reuse an edge"));
            }
            if !used_loops.insert(current.id.clone()) {
                return Err(invalid("loop is owned by more than one face"));
            }
            edge_incidence
                .entry(current.edge_id.clone())
                .or_default()
                .push(current.id.clone());
            used_vertices.insert(current.vertex_id.clone());
            face_vertices.push(current.vertex_id.clone());
        }
        if face_vertices.iter().collect::<BTreeSet<_>>().len() != face_vertices.len() {
            return Err(invalid("face cannot reuse a vertex"));
        }
        let mut face_vertex_set = face_vertices.clone();
        face_vertex_set.sort();
        if !seen_face_vertex_sets.insert(face_vertex_set) {
            return Err(invalid("duplicate face vertex set"));
        }
        let positions = face_vertices
            .iter()
            .map(|id| vertices[*vertex_by_id.get(id).expect("vertex was validated")].position)
            .collect::<Vec<_>>();
        for index in 1..positions.len() - 1 {
            if triangle_area(positions[0], positions[index], positions[index + 1])
                <= MIN_TRIANGLE_AREA
            {
                return Err(invalid("face contains a zero-area triangle"));
            }
        }
        faces.push(SourceFace {
            id: face_id,
            loop_ids,
        });
    }
    canonical_ids(&faces, |item| item.id.as_str(), "face")?;
    if used_loops.len() != loops.len() || used_vertices.len() != vertices.len() {
        return Err(invalid("source topology has unowned loops or vertices"));
    }

    for edge in &edges {
        let incidence = edge_incidence
            .get(&edge.id)
            .ok_or_else(|| invalid("source topology contains an unused edge"))?;
        if !(1..=2).contains(&incidence.len()) {
            return Err(invalid("non-manifold edge incidence is rejected"));
        }
        if incidence.len() == 2 {
            let left = &loops[*loop_by_id.get(&incidence[0]).expect("loop incidence")];
            let right = &loops[*loop_by_id.get(&incidence[1]).expect("loop incidence")];
            if left.face_id == right.face_id || left.edge_forward == right.edge_forward {
                return Err(invalid("edge twin must be opposite and cross-face"));
            }
        }
        let left = vertices[*vertex_by_id.get(&edge.vertex_ids[0]).expect("edge vertex")].position;
        let right = vertices[*vertex_by_id.get(&edge.vertex_ids[1]).expect("edge vertex")].position;
        if distance(left, right) <= MIN_EDGE_LENGTH_M {
            return Err(invalid("edge length is below tolerance"));
        }
    }

    Ok(SourceMesh {
        vertices,
        edges,
        loops,
        faces,
        edge_incidence,
    })
}

fn scoped_id(
    prefix: &str,
    mesh_id: &str,
    artifact_id: Option<&str>,
    kind: &str,
    source_id: &str,
) -> String {
    let hash = canonical_json_hash(&json!({
        "prefix":prefix,
        "mesh_id":mesh_id,
        "artifact_id":artifact_id,
        "kind":kind,
        "source_id":source_id,
    }));
    format!("{prefix}-{}", &hash[..56])
}

fn mesh_id(context: &AuthoringContext, source_mesh_sha256: &str) -> String {
    let hash = canonical_json_hash(&json!({
        "schema_version":"AuthoringMesh@1",
        "project_id":context.project_id,
        "candidate_id":context.candidate_id,
        "artifact_id":context.artifact_id,
        "program_sha256":context.program_sha256,
        "authoring_node_id":context.authoring_node_id,
        "part_id":context.part_id,
        "authoring_mesh_sha256":source_mesh_sha256,
    }));
    format!("mesh-{hash}")
}

fn source_mesh_sha256(parameters: &Map<String, Value>) -> String {
    canonical_json_hash(&Value::Object(parameters.clone()))
}

fn build_half_edges(
    context: &AuthoringContext,
    source_mesh: &SourceMesh,
    mesh_id: &str,
) -> Result<
    (
        Vec<HalfEdgeWork>,
        BTreeMap<String, String>,
        BTreeMap<String, String>,
    ),
    RuntimeError,
> {
    if source_mesh.loops.len() > MAX_HALF_EDGES || source_mesh.loops.len() > MAX_CORNERS {
        return Err(invalid("half-edge/corner budget exceeded"));
    }
    let mut half_edges = Vec::with_capacity(source_mesh.loops.len());
    let mut half_edge_by_loop = BTreeMap::new();
    let mut corner_by_loop = BTreeMap::new();
    for loop_item in &source_mesh.loops {
        let half_edge_id = scoped_id(
            "he",
            mesh_id,
            Some(&context.artifact_id),
            "half-edge",
            &loop_item.id,
        );
        let corner_id = scoped_id(
            "c",
            mesh_id,
            Some(&context.artifact_id),
            "corner",
            &loop_item.id,
        );
        if half_edge_by_loop
            .insert(loop_item.id.clone(), half_edge_id.clone())
            .is_some()
            || corner_by_loop
                .insert(loop_item.id.clone(), corner_id.clone())
                .is_some()
        {
            return Err(invalid("duplicate half-edge/corner identity"));
        }
        half_edges.push(HalfEdgeWork {
            source_loop_id: loop_item.id.clone(),
            source_face_id: loop_item.face_id.clone(),
            source_edge_id: loop_item.edge_id.clone(),
            source_vertex_id: loop_item.vertex_id.clone(),
            edge_forward: loop_item.edge_forward,
            half_edge_id,
            next_id: String::new(),
            prev_id: String::new(),
            twin_id: None,
            boundary: false,
        });
    }

    let index_by_loop = half_edges
        .iter()
        .enumerate()
        .map(|(index, item)| (item.source_loop_id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    for face in &source_mesh.faces {
        for (ordinal, loop_id) in face.loop_ids.iter().enumerate() {
            let current_index = *index_by_loop
                .get(loop_id)
                .ok_or_else(|| invalid("face cycle references an unknown half-edge"))?;
            let next_loop_id = &face.loop_ids[(ordinal + 1) % face.loop_ids.len()];
            let prev_loop_id =
                &face.loop_ids[(ordinal + face.loop_ids.len() - 1) % face.loop_ids.len()];
            half_edges[current_index].next_id = half_edge_by_loop
                .get(next_loop_id)
                .cloned()
                .ok_or_else(|| invalid("next half-edge reference is missing"))?;
            half_edges[current_index].prev_id = half_edge_by_loop
                .get(prev_loop_id)
                .cloned()
                .ok_or_else(|| invalid("previous half-edge reference is missing"))?;
        }
    }

    let mut index_by_half_edge = BTreeMap::new();
    for (index, item) in half_edges.iter().enumerate() {
        if index_by_half_edge
            .insert(item.half_edge_id.clone(), index)
            .is_some()
        {
            return Err(invalid("half-edge IDs are not unique"));
        }
    }
    for edge in &source_mesh.edges {
        let incidence = source_mesh
            .edge_incidence
            .get(&edge.id)
            .ok_or_else(|| invalid("edge incidence is missing"))?;
        if !(1..=2).contains(&incidence.len()) {
            return Err(invalid("edge incidence must be one or two"));
        }
        let boundary = incidence.len() == 1;
        for loop_id in incidence {
            let index = *index_by_loop
                .get(loop_id)
                .ok_or_else(|| invalid("edge incidence references an unknown loop"))?;
            half_edges[index].boundary = boundary;
        }
        if incidence.len() == 2 {
            let left_index = *index_by_loop.get(&incidence[0]).expect("left loop");
            let right_index = *index_by_loop.get(&incidence[1]).expect("right loop");
            if half_edges[left_index].edge_forward == half_edges[right_index].edge_forward
                || half_edges[left_index].source_face_id == half_edges[right_index].source_face_id
            {
                return Err(invalid("edge twins do not have opposite orientation"));
            }
            let left_id = half_edges[left_index].half_edge_id.clone();
            let right_id = half_edges[right_index].half_edge_id.clone();
            half_edges[left_index].twin_id = Some(right_id);
            half_edges[right_index].twin_id = Some(left_id);
        }
    }

    // Validate every face cycle after all links are materialized.  This is a
    // separate pass from construction so a future source producer cannot
    // accidentally make a locally plausible but incomplete cycle.
    for face in &source_mesh.faces {
        let first_loop = face
            .loop_ids
            .first()
            .ok_or_else(|| invalid("face has no first loop"))?;
        let first_id = half_edge_by_loop
            .get(first_loop)
            .ok_or_else(|| invalid("face first half-edge is missing"))?;
        let mut current_id = first_id.clone();
        let mut visited = BTreeSet::new();
        for _ in 0..=face.loop_ids.len() {
            if !visited.insert(current_id.clone()) {
                if current_id == *first_id && visited.len() == face.loop_ids.len() {
                    break;
                }
                return Err(invalid("face next cycle repeats before closure"));
            }
            let index = *index_by_half_edge
                .get(&current_id)
                .ok_or_else(|| invalid("face cycle references an unknown ID"))?;
            let next_id = half_edges[index].next_id.clone();
            let next_index = *index_by_half_edge
                .get(&next_id)
                .ok_or_else(|| invalid("face next reference is dangling"))?;
            if half_edges[next_index].prev_id != current_id
                || half_edges[index].source_face_id != half_edges[next_index].source_face_id
            {
                return Err(invalid("face next/prev links are not reciprocal"));
            }
            current_id = next_id;
        }
        if current_id != *first_id || visited.len() != face.loop_ids.len() {
            return Err(invalid("face cycle is incomplete"));
        }
    }

    // Validate twin symmetry and direction independently of edge incidence.
    for item in &half_edges {
        if let Some(twin_id) = &item.twin_id {
            let twin_index = *index_by_half_edge
                .get(twin_id)
                .ok_or_else(|| invalid("twin reference is dangling"))?;
            let twin = &half_edges[twin_index];
            if twin.twin_id.as_deref() != Some(item.half_edge_id.as_str())
                || twin.source_edge_id != item.source_edge_id
                || twin.edge_forward == item.edge_forward
                || item.boundary
                || twin.boundary
            {
                return Err(invalid("twin symmetry or orientation is invalid"));
            }
        } else if !item.boundary {
            return Err(invalid("interior half-edge is missing a twin"));
        }
    }

    Ok((half_edges, half_edge_by_loop, corner_by_loop))
}

fn identity_id(
    prefix: &str,
    mesh_id: &str,
    artifact_id: Option<&str>,
    kind: &str,
    source_id: &str,
) -> String {
    scoped_id(prefix, mesh_id, artifact_id, kind, source_id)
}

fn element_lineage(original_ids: &[String]) -> Value {
    let evaluated_ids = Vec::<String>::new();
    let correspondence_kind = "not_materialized";
    let correspondence_sha256 = canonical_json_hash(&json!({
        "original_element_ids": original_ids,
        "evaluated_element_ids": evaluated_ids,
        "correspondence_kind": correspondence_kind,
    }));
    json!({
        "original_element_ids": original_ids,
        "evaluated_element_ids": [],
        "correspondence_kind": correspondence_kind,
        "correspondence_sha256": correspondence_sha256,
    })
}

fn boundary_ring_values(
    source_mesh: &SourceMesh,
    edge_ids: &BTreeMap<String, String>,
    half_edge_by_loop: &BTreeMap<String, String>,
    mesh_id: &str,
    artifact_id: &str,
) -> Result<Vec<Value>, RuntimeError> {
    let boundary_edges = source_mesh
        .edges
        .iter()
        .filter(|edge| source_mesh.edge_incidence[&edge.id].len() == 1)
        .collect::<Vec<_>>();
    if boundary_edges.is_empty() {
        return Ok(Vec::new());
    }

    let mut edge_by_id = BTreeMap::new();
    let mut adjacency = BTreeMap::<String, Vec<String>>::new();
    for edge in boundary_edges {
        edge_by_id.insert(edge.id.clone(), edge);
        adjacency
            .entry(edge.vertex_ids[0].clone())
            .or_default()
            .push(edge.id.clone());
        adjacency
            .entry(edge.vertex_ids[1].clone())
            .or_default()
            .push(edge.id.clone());
    }
    for incident_edges in adjacency.values_mut() {
        incident_edges.sort();
    }

    let mut unseen = edge_by_id.keys().cloned().collect::<BTreeSet<_>>();
    let mut rings = Vec::new();
    while let Some(seed) = unseen.iter().next().cloned() {
        let mut queue = VecDeque::from([seed]);
        let mut component = BTreeSet::new();
        while let Some(edge_id) = queue.pop_front() {
            if !unseen.remove(&edge_id) {
                continue;
            }
            let edge = edge_by_id[&edge_id];
            component.insert(edge_id);
            for vertex_id in &edge.vertex_ids {
                if let Some(incident_edges) = adjacency.get(vertex_id) {
                    for adjacent_edge_id in incident_edges {
                        if unseen.contains(adjacent_edge_id) {
                            queue.push_back(adjacent_edge_id.clone());
                        }
                    }
                }
            }
        }

        let component_edge_ids = component.iter().cloned().collect::<Vec<_>>();
        let mut vertex_degree = BTreeMap::<String, usize>::new();
        let mut component_half_edge_ids = Vec::new();
        for edge_id in &component_edge_ids {
            let edge = edge_by_id[edge_id];
            for vertex_id in &edge.vertex_ids {
                *vertex_degree.entry(vertex_id.clone()).or_default() += 1;
            }
            if let Some(loop_id) = source_mesh.edge_incidence[edge_id].first() {
                component_half_edge_ids.push(half_edge_by_loop[loop_id].clone());
            }
        }
        component_half_edge_ids.sort();
        if component_edge_ids.len() > 128 || component_half_edge_ids.len() > 128 {
            return Err(invalid("boundary ring exceeds 128 elements"));
        }
        let closed =
            component_edge_ids.len() >= 3 && vertex_degree.values().all(|degree| *degree == 2);
        let ring_id = scoped_id(
            "ring",
            mesh_id,
            Some(artifact_id),
            "boundary",
            component_edge_ids
                .first()
                .expect("boundary component is non-empty"),
        );
        // The source mesh has no separate ring element.  Keep a bounded,
        // deterministic sample of the owning source edges as the original
        // lineage while the ring itself remains a derived projection object.
        let original_ids = component_edge_ids
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>();
        rings.push(json!({
            "ring_id": ring_id,
            "kind": "boundary",
            "edge_ids": component_edge_ids.iter().map(|id| edge_ids[id].clone()).collect::<Vec<_>>(),
            "half_edge_ids": component_half_edge_ids,
            "closed": closed,
            "boundary": true,
            "lineage": element_lineage(&original_ids),
        }));
        if rings.len() > MAX_FACES {
            return Err(invalid("ring budget exceeded"));
        }
    }
    Ok(rings)
}

fn sort_values_by_id(values: &mut [Value], key: &str) {
    values.sort_by(|left, right| {
        left.get(key)
            .and_then(Value::as_str)
            .cmp(&right.get(key).and_then(Value::as_str))
    });
}

fn projection_value(context: &AuthoringContext) -> Result<Value, RuntimeError> {
    let source_mesh = parse_source(&context.parameters)?;
    let source_mesh_sha256 = source_mesh_sha256(&context.parameters);
    let mesh_id = mesh_id(context, &source_mesh_sha256);
    let (half_edges, half_edge_by_loop, corner_by_loop) =
        build_half_edges(context, &source_mesh, &mesh_id)?;

    let vertex_ids = source_mesh
        .vertices
        .iter()
        .map(|item| {
            (
                item.id.clone(),
                scoped_id(
                    "v",
                    &mesh_id,
                    Some(&context.artifact_id),
                    "vertex",
                    &item.id,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let edge_ids = source_mesh
        .edges
        .iter()
        .map(|item| {
            (
                item.id.clone(),
                scoped_id("e", &mesh_id, Some(&context.artifact_id), "edge", &item.id),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let face_ids = source_mesh
        .faces
        .iter()
        .map(|item| {
            (
                item.id.clone(),
                scoped_id("f", &mesh_id, Some(&context.artifact_id), "face", &item.id),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let boundary_edge_count = source_mesh
        .edges
        .iter()
        .filter(|edge| source_mesh.edge_incidence[&edge.id].len() == 1)
        .count();
    let boundary_half_edge_count = half_edges.iter().filter(|item| item.boundary).count();
    let topology_status = if boundary_edge_count == 0 {
        "closed_manifold"
    } else {
        "manifold_with_boundary"
    };

    let mut lineage = json!({
        "project_id": context.project_id,
        "candidate_id": context.candidate_id,
        "artifact_id": context.artifact_id,
        "artifact_readback_sha256": context.artifact_readback_sha256,
        "program_sha256": context.program_sha256,
        "operator_catalog_sha256": context.operator_catalog_sha256,
        "readback_config_sha256": context.readback_config_sha256,
        "authoring_node_id": context.authoring_node_id,
        "part_id": context.part_id,
        "lineage_status": "candidate-program-artifact-readback-bound@1",
        "lineage_sha256": "",
    });
    lineage["lineage_sha256"] = Value::String(canonical_json_hash(&lineage));
    let lineage_sha256 = lineage["lineage_sha256"]
        .as_str()
        .expect("lineage hash is a string")
        .to_owned();
    let mesh_identity_sha256 = canonical_json_hash(&json!({
        "mesh_id": mesh_id,
        "mesh_sha256": source_mesh_sha256,
        "lineage_sha256": lineage_sha256,
        "projection_kind": "runtime-derived-read-only-projection@1",
    }));
    let original_identity = json!({
        "identity_id": identity_id("original", &mesh_id, None, "mesh", "root"),
        "identity_kind": "runtime-derived-original-authoring@1",
        "topology_sha256": source_mesh_sha256,
        "element_id_policy": "stable-within-authoring-mesh-lineage@1",
        "position_space": "authoring-local@1",
        "namespace": "original",
        "source_lineage_sha256": lineage_sha256,
    });
    let evaluated_identity = json!({
        "identity_id": identity_id("evaluated", &mesh_id, Some(&context.artifact_id), "mesh", "root"),
        "identity_kind": "runtime-derived-evaluated-artifact-readback@1",
        "artifact_id": context.artifact_id,
        "artifact_readback_sha256": context.artifact_readback_sha256,
        "element_id_policy": "artifact-local-no-authoring-bijection@1",
        "position_space": "evaluated-local@1",
        "namespace": "evaluated",
        "correspondence_policy": "non-bijective-derived-only@1",
        "source_lineage_sha256": lineage_sha256,
    });

    let mut outgoing = BTreeMap::<String, String>::new();
    for item in &half_edges {
        outgoing
            .entry(item.source_vertex_id.clone())
            .or_insert_with(|| item.half_edge_id.clone());
    }
    let boundary_vertex_ids = source_mesh
        .edges
        .iter()
        .filter(|edge| source_mesh.edge_incidence[&edge.id].len() == 1)
        .flat_map(|edge| edge.vertex_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    let mut vertex_values = Vec::with_capacity(source_mesh.vertices.len());
    for item in &source_mesh.vertices {
        vertex_values.push(json!({
            "vertex_id": vertex_ids[&item.id],
            "position_m": item.position,
            "outgoing_half_edge_id": outgoing[&item.id],
            "boundary": boundary_vertex_ids.contains(&item.id),
            "lineage": element_lineage(std::slice::from_ref(&item.id)),
        }));
    }
    sort_values_by_id(&mut vertex_values, "vertex_id");

    let mut edge_values = Vec::with_capacity(source_mesh.edges.len());
    for edge in &source_mesh.edges {
        let incidence = &source_mesh.edge_incidence[&edge.id];
        let mut incident_half_edges = incidence
            .iter()
            .map(|loop_id| half_edge_by_loop[loop_id].clone())
            .collect::<Vec<_>>();
        incident_half_edges.sort();
        edge_values.push(json!({
            "edge_id": edge_ids[&edge.id],
            "vertex_ids": [vertex_ids[&edge.vertex_ids[0]], vertex_ids[&edge.vertex_ids[1]]],
            "half_edge_ids": incident_half_edges,
            "boundary": incidence.len() == 1,
            "hard_edge": false,
            "crease": 0,
            "uv_seam": false,
            "lineage": element_lineage(std::slice::from_ref(&edge.id)),
        }));
    }
    sort_values_by_id(&mut edge_values, "edge_id");

    let mut half_edge_values = Vec::with_capacity(half_edges.len());
    for item in &half_edges {
        half_edge_values.push(json!({
            "half_edge_id": item.half_edge_id,
            "origin_vertex_id": vertex_ids[&item.source_vertex_id],
            "edge_id": edge_ids[&item.source_edge_id],
            "face_id": face_ids[&item.source_face_id],
            "corner_id": corner_by_loop[&item.source_loop_id],
            "twin_id": item.twin_id,
            "next_id": item.next_id,
            "prev_id": item.prev_id,
            "boundary": item.boundary,
            "orientation": if item.edge_forward { "forward" } else { "reverse" },
            "lineage": element_lineage(std::slice::from_ref(&item.source_loop_id)),
        }));
    }
    sort_values_by_id(&mut half_edge_values, "half_edge_id");

    let mut corner_values = Vec::with_capacity(source_mesh.loops.len());
    for item in &source_mesh.loops {
        corner_values.push(json!({
            "corner_id": corner_by_loop[&item.id],
            "face_id": face_ids[&item.face_id],
            "half_edge_id": half_edge_by_loop[&item.id],
            "vertex_id": vertex_ids[&item.vertex_id],
            "ordinal": item.ordinal,
            "lineage": element_lineage(std::slice::from_ref(&item.id)),
        }));
    }
    sort_values_by_id(&mut corner_values, "corner_id");

    let source_loop_by_id = source_mesh
        .loops
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let mut face_values = Vec::with_capacity(source_mesh.faces.len());
    for face in &source_mesh.faces {
        let boundary = face.loop_ids.iter().any(|loop_id| {
            let loop_item = source_loop_by_id[loop_id.as_str()];
            source_mesh.edge_incidence[&loop_item.edge_id].len() == 1
        });
        face_values.push(json!({
            "face_id": face_ids[&face.id],
            "first_half_edge_id": half_edge_by_loop[&face.loop_ids[0]],
            "corner_ids": face.loop_ids.iter().map(|loop_id| corner_by_loop[loop_id].clone()).collect::<Vec<_>>(),
            "degree": face.loop_ids.len(),
            "boundary": boundary,
            "lineage": element_lineage(std::slice::from_ref(&face.id)),
        }));
    }
    sort_values_by_id(&mut face_values, "face_id");

    let mut loop_values = Vec::with_capacity(source_mesh.faces.len());
    for face in &source_mesh.faces {
        let boundary = face.loop_ids.iter().any(|loop_id| {
            let loop_item = source_loop_by_id[loop_id.as_str()];
            source_mesh.edge_incidence[&loop_item.edge_id].len() == 1
        });
        loop_values.push(json!({
            "loop_id": scoped_id("loop", &mesh_id, Some(&context.artifact_id), "face-cycle", &face.id),
            "face_id": face_ids[&face.id],
            "first_half_edge_id": half_edge_by_loop[&face.loop_ids[0]],
            "half_edge_ids": face.loop_ids.iter().map(|loop_id| half_edge_by_loop[loop_id].clone()).collect::<Vec<_>>(),
            "boundary": boundary,
            "lineage": element_lineage(std::slice::from_ref(&face.id)),
        }));
    }
    sort_values_by_id(&mut loop_values, "loop_id");
    let mut ring_values = boundary_ring_values(
        &source_mesh,
        &edge_ids,
        &half_edge_by_loop,
        &mesh_id,
        &context.artifact_id,
    )?;
    sort_values_by_id(&mut ring_values, "ring_id");

    let mut value = json!({
        "schema_version": "AuthoringMesh@1",
        "mesh_id": mesh_id,
        "mesh_sha256": source_mesh_sha256,
        "scope": "single-authoring-mesh",
        "representation": "half-edge-authoring@1",
        "projection_kind": "runtime-derived-read-only-projection@1",
        "lineage": lineage,
        "mesh_identity_derivation": "runtime-derived-from-candidate-program-artifact-readback@1",
        "mesh_identity_sha256": mesh_identity_sha256,
        "identity_policy": "runtime-derived-original-evaluated-non-bijective@1",
        "original_identity": original_identity,
        "evaluated_identity": evaluated_identity,
        "cross_version_stable": false,
        "counts": {
            "vertex_count": vertex_values.len(),
            "edge_count": edge_values.len(),
            "half_edge_count": half_edge_values.len(),
            "corner_count": corner_values.len(),
            "face_count": face_values.len(),
            "loop_count": loop_values.len(),
            "ring_count": ring_values.len(),
            "boundary_edge_count": boundary_edge_count,
            "boundary_half_edge_count": boundary_half_edge_count,
            "hard_edge_count": 0,
            "crease_edge_count": 0,
            "uv_seam_count": 0,
        },
        "vertices": vertex_values,
        "edges": edge_values,
        "half_edges": half_edge_values,
        "corners": corner_values,
        "faces": face_values,
        "loops": loop_values,
        "rings": ring_values,
        "topology_policy": "bounded-half-edge-manifold-with-boundary@1",
        "topology": {
            "boundary_edge_count": boundary_edge_count,
            "boundary_half_edge_count": boundary_half_edge_count,
            "non_manifold_edge_count": 0,
            "orientation_conflict_count": 0,
            "status": topology_status,
            "validation_status": "passed",
            "rejection_policy": "fail-closed-on-non-manifold@1",
            "face_cycle_policy": "next-prev-complete-mutual@1",
            "twin_policy": "boundary-only-null-symmetric@1",
            "boundary_policy": "single-half-edge-per-boundary-edge@1",
        },
        "authoring_mesh_policy_sha256": AUTHORING_MESH_POLICY_SHA256,
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": false,
        "persistent_user_data_touched": false,
        "quality_status": "structural_only",
        "canonical_sha256": "",
    });
    value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
    let bytes = canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(invalid("AuthoringMesh response exceeds 1 MiB"));
    }
    Ok(value)
}

pub(super) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "candidate_id",
            "artifact_id",
            "artifact_readback_sha256",
            "program_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "authoring_node_id",
            "part_id",
            "authoring_mesh_policy_sha256",
            "max_response_bytes",
        ],
        "AuthoringMeshRequest@1",
    )?;
    if required_text(object, "schema_version")? != "AuthoringMeshRequest@1"
        || required_sha(object, "authoring_mesh_policy_sha256")? != AUTHORING_MESH_POLICY_SHA256
        || object.get("max_response_bytes").and_then(Value::as_u64)
            != Some(MAX_RESPONSE_BYTES as u64)
    {
        return Err(invalid("request policy or response budget differs"));
    }
    let context = authoring_topology::load_context_for_authoring_mesh(runtime, request)?;
    projection_value(&context)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad_parameters() -> Map<String, Value> {
        serde_json::from_value(json!({
            "shape":"authoring-mesh",
            "topology_policy":"triangle-quad-manifold-with-boundary@1",
            "vertices":[
                {"element_id":"v0","position_m":[0.0,0.0,0.0]},
                {"element_id":"v1","position_m":[1.0,0.0,0.0]},
                {"element_id":"v2","position_m":[1.0,1.0,0.0]},
                {"element_id":"v3","position_m":[0.0,1.0,0.0]}
            ],
            "edges":[
                {"element_id":"e01","vertex_ids":["v0","v1"]},
                {"element_id":"e03","vertex_ids":["v0","v3"]},
                {"element_id":"e12","vertex_ids":["v1","v2"]},
                {"element_id":"e23","vertex_ids":["v2","v3"]}
            ],
            "loops":[
                {"element_id":"l0","face_id":"f0","ordinal":0,"vertex_id":"v0","edge_id":"e01","edge_forward":true},
                {"element_id":"l1","face_id":"f0","ordinal":1,"vertex_id":"v1","edge_id":"e12","edge_forward":true},
                {"element_id":"l2","face_id":"f0","ordinal":2,"vertex_id":"v2","edge_id":"e23","edge_forward":true},
                {"element_id":"l3","face_id":"f0","ordinal":3,"vertex_id":"v3","edge_id":"e03","edge_forward":false}
            ],
            "faces":[{"element_id":"f0","loop_ids":["l0","l1","l2","l3"]}],
            "position_m":[0.0,0.0,0.0],
            "rotation_rad":[0.0,0.0,0.0]
        }))
        .expect("quad parameters")
    }

    #[test]
    fn source_projection_builds_boundary_cycles_and_null_twins() {
        let _source = parse_source(&quad_parameters()).expect("source mesh");
        let context = AuthoringContext {
            project_id: "p".to_owned(),
            candidate_id: "c".to_owned(),
            artifact_id: "a".repeat(64),
            artifact_readback_sha256: "b".repeat(64),
            geometry_candidate_evidence_sha256: "c".repeat(64),
            reference_id: None,
            reference_sha256: None,
            geometry_program_object_sha256: "d".repeat(64),
            program_sha256: "e".repeat(64),
            operator_catalog_sha256: "f".repeat(64),
            readback_config_sha256: "0".repeat(64),
            authoring_node_id: "n".to_owned(),
            part_id: "part".to_owned(),
            material_zone_id: "zone".to_owned(),
            solid: false,
            program: json!({}),
            node_index: 0,
            parameters: quad_parameters(),
            source_artifact_bytes: Vec::new(),
            source_triangle_count: 2,
            source_part_ids: vec!["part".to_owned()],
            source_material_zone_ids: vec!["zone".to_owned()],
            source_worker_cohort_sha256: Some("1".repeat(64)),
        };
        let value = projection_value(&context).expect("projection");
        assert_eq!(
            value["projection_kind"],
            "runtime-derived-read-only-projection@1"
        );
        assert_eq!(
            value["lineage"]["lineage_status"],
            "candidate-program-artifact-readback-bound@1"
        );
        assert_eq!(value["counts"]["loop_count"], 1);
        assert_eq!(value["counts"]["ring_count"], 1);
        assert_eq!(value["topology"]["status"], "manifold_with_boundary");
        assert_eq!(value["topology"]["boundary_edge_count"], 4);
        assert!(value["half_edges"]
            .as_array()
            .expect("half-edges")
            .iter()
            .all(|item| item["twin_id"].is_null()
                && item["face_id"].is_string()
                && item["corner_id"].is_string()
                && item["lineage"]["correspondence_kind"] == "not_materialized"
                && item["lineage"]["evaluated_element_ids"] == json!([])));
        assert_eq!(
            value["canonical_sha256"],
            canonical_json_hash(&{
                let mut copy = value.clone();
                copy["canonical_sha256"] = Value::String(String::new());
                copy
            })
        );
    }

    #[test]
    fn source_projection_rejects_non_manifold_and_zero_area() {
        let mut non_manifold = quad_parameters();
        non_manifold["faces"] = json!([
            {"element_id":"f0","loop_ids":["l0","l1","l2","l3"]},
            {"element_id":"f1","loop_ids":["l4","l5","l6","l7"]},
            {"element_id":"f2","loop_ids":["l8","l9","l10","l11"]}
        ]);
        assert!(parse_source(&non_manifold).is_err());

        let mut zero_area = quad_parameters();
        zero_area["vertices"][2]["position_m"] = json!([0.5, 0.0, 0.0]);
        assert!(parse_source(&zero_area).is_err());
    }
}
