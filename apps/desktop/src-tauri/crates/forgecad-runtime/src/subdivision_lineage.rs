//! Worker-produced control-cage -> evaluated topology lineage.
//!
//! This projection is deliberately read-only and program-bound. It describes
//! the fixed regular-quad evaluator's topology IDs, not GLB accessor indices,
//! persistent authoring IDs, influence weights, a limit surface or visual
//! quality. The Runtime validates the complete Worker sidecar before exposing
//! it to MCP and never writes it to SQLite/CAS in this slice.

use super::{
    canonical_json_bytes, canonical_json_hash, exact_object, geometry_worker, is_opaque_id,
    is_sha256, operator_catalog_sha256, verify_output_canonical_hash, Runtime, RuntimeError,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const REQUEST_SCHEMA: &str = "SubdivisionTopologyLineageRequest@1";
const RESULT_SCHEMA: &str = "SubdivisionTopologyLineage@1";
const MAX_LINEAGE_ELEMENTS: u64 = 25_000;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const LIMITATIONS: [&str; 8] = [
    "REGULAR_RECTANGULAR_OPEN_QUAD_GRID_ONLY",
    "INTEGER_EDGE_SHARPNESS_LEVELS_1_TO_2_ONLY",
    "ELEMENT_IDS_CHANGE_WHEN_PROGRAM_OR_EVALUATION_CHANGES",
    "EVALUATED_QUAD_IDS_ARE_NOT_GLTF_TRIANGLE_OR_DEDUPLICATED_VERTEX_IDS",
    "ROOT_ANCESTRY_ONLY_NO_INFLUENCE_WEIGHTS_OR_CORNER_DOMAIN",
    "PREVIEW_NOT_ARTIFACT_OR_READBACK_BOUND",
    "LINEAGE_NOT_PERSISTED_IN_CURRENT_GLB",
    "STRUCTURAL_LINEAGE_DOES_NOT_PROVE_VISUAL_QUALITY",
];

#[derive(Debug, Clone)]
struct ExpectedLineage {
    program_sha256: String,
    node_id: String,
    u_points: u64,
    v_points: u64,
    levels: u64,
    control_vertices: u64,
    control_edges: u64,
    control_quads: u64,
    evaluated_vertices: u64,
    evaluated_edges: u64,
    evaluated_quads: u64,
    evaluated_triangles: u64,
    lineage_elements: u64,
    crease_edges: Vec<(u64, u64)>,
    full_lineage: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RootOrigin {
    ControlVertex(u64),
    ControlEdge(u64),
    ControlQuad(u64),
}

impl RootOrigin {
    fn wire(self) -> Value {
        match self {
            Self::ControlVertex(id) => json!(["control_vertex", id]),
            Self::ControlEdge(id) => json!(["control_edge", id]),
            Self::ControlQuad(id) => json!(["control_quad", id]),
        }
    }
}

impl Runtime {
    pub fn subdivision_topology_lineage_preview(
        &self,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        let object = exact_object(
            &request,
            &[
                "schema_version",
                "geometry_program",
                "subdivision_node_id",
                "max_lineage_elements",
                "canonical_sha256",
            ],
            REQUEST_SCHEMA,
        )?;
        if object.get("schema_version").and_then(Value::as_str) != Some(REQUEST_SCHEMA) {
            return Err(lineage_error("request schema_version differs"));
        }
        verify_output_canonical_hash(&request, REQUEST_SCHEMA)?;
        let program = object
            .get("geometry_program")
            .filter(|value| value.is_object())
            .ok_or_else(|| lineage_error("geometry_program must be an object"))?;
        verify_geometry_program_canonical_hash(program)?;
        let subdivision_node_id = required_id(object, "subdivision_node_id")?;
        let max_lineage_elements = object
            .get("max_lineage_elements")
            .and_then(Value::as_u64)
            .filter(|value| (1..=MAX_LINEAGE_ELEMENTS).contains(value))
            .ok_or_else(|| lineage_error("max_lineage_elements is outside 1..25000"))?;
        let expected = expected_lineage(program, &subdivision_node_id)?;
        if expected.lineage_elements > max_lineage_elements {
            return Err(lineage_error(
                "complete lineage exceeds max_lineage_elements without truncation",
            ));
        }

        let result = execute_worker(program, &subdivision_node_id, max_lineage_elements)
            .map_err(|error| lineage_error(&error.to_string()))?;
        validate_result(&result, max_lineage_elements, &expected)?;
        let bytes = canonical_json_bytes(&result)
            .map_err(|error| lineage_error(&format!("result serialization failed: {error}")))?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(lineage_error("result exceeds 1 MiB"));
        }
        Ok(result)
    }
}

fn execute_worker(
    program: &Value,
    subdivision_node_id: &str,
    max_lineage_elements: u64,
) -> Result<Value, geometry_worker::GeometryWorkerError> {
    let result = geometry_worker::subdivision_topology_lineage(
        program,
        subdivision_node_id,
        max_lineage_elements,
    );
    #[cfg(any(test, feature = "test-geometry-worker-fallback"))]
    if matches!(
        result,
        Err(geometry_worker::GeometryWorkerError::Unavailable)
    ) {
        return geometry_worker::subdivision_topology_lineage_test_fallback(
            program,
            subdivision_node_id,
            max_lineage_elements,
        );
    }
    result
}

fn validate_result(
    value: &Value,
    max_lineage_elements: u64,
    expected: &ExpectedLineage,
) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "program_sha256",
            "operator_catalog_sha256",
            "subdivision_node_id",
            "lineage_kind",
            "lineage_space",
            "id_scope",
            "complete",
            "completeness_scope",
            "cross_version_stable",
            "artifact_binding_status",
            "max_lineage_elements",
            "lineage_element_count",
            "lineage",
            "lineage_sha256",
            "materialization_status",
            "runtime_write_performed",
            "quality_status",
            "limitations",
            "canonical_sha256",
        ],
        RESULT_SCHEMA,
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some(RESULT_SCHEMA)
        || object.get("program_sha256").and_then(Value::as_str)
            != Some(expected.program_sha256.as_str())
        || object
            .get("operator_catalog_sha256")
            .and_then(Value::as_str)
            != Some(operator_catalog_sha256().as_str())
        || object.get("subdivision_node_id").and_then(Value::as_str)
            != Some(expected.node_id.as_str())
        || object.get("lineage_kind").and_then(Value::as_str)
            != Some("control-root-to-evaluated-quad-topology@1")
        || object.get("lineage_space").and_then(Value::as_str) != Some("evaluated-quad-topology@1")
        || object.get("id_scope").and_then(Value::as_str) != Some("program-and-evaluation-bound")
        || object.get("complete") != Some(&Value::Bool(true))
        || object.get("completeness_scope").and_then(Value::as_str)
            != Some("all-root-mappings-within-declared-preview-lineage")
        || object.get("cross_version_stable") != Some(&Value::Bool(false))
        || object
            .get("artifact_binding_status")
            .and_then(Value::as_str)
            != Some("unavailable-preview-only")
        || object.get("max_lineage_elements").and_then(Value::as_u64) != Some(max_lineage_elements)
        || object.get("lineage_element_count").and_then(Value::as_u64)
            != Some(expected.lineage_elements)
        || object.get("materialization_status").and_then(Value::as_str)
            != Some("preview-only-not-persisted-in-glb")
        || object.get("runtime_write_performed") != Some(&Value::Bool(false))
        || object.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || object.get("limitations") != Some(&json!(LIMITATIONS))
    {
        return Err(lineage_error("result constants, counts or scope differ"));
    }
    let lineage = object
        .get("lineage")
        .ok_or_else(|| lineage_error("lineage is missing"))?;
    validate_lineage(lineage, expected)?;
    if object.get("lineage_sha256").and_then(Value::as_str)
        != Some(canonical_json_hash(lineage).as_str())
    {
        return Err(lineage_error("lineage hash differs"));
    }
    verify_output_canonical_hash(value, RESULT_SCHEMA)
}

fn validate_lineage(value: &Value, expected: &ExpectedLineage) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "control_dimensions",
            "subdivision_levels",
            "control_counts",
            "evaluated_counts",
            "control_vertex_to_evaluated_vertex_ids",
            "control_edge_to_evaluated_edge_ids",
            "control_quad_descendant_ranges",
            "control_crease_edge_chains",
            "evaluated_vertex_root_origins",
            "evaluated_edge_root_origins",
            "evaluated_quad_control_quad_ids",
            "quad_triangulation",
        ],
        "SubdivisionTopologyLineage@1.lineage",
    )?;
    validate_counts(
        object.get("control_dimensions"),
        &[
            ("u_points", expected.u_points),
            ("v_points", expected.v_points),
        ],
        "control_dimensions",
    )?;
    if object.get("subdivision_levels").and_then(Value::as_u64) != Some(expected.levels) {
        return Err(lineage_error("subdivision level differs"));
    }
    validate_counts(
        object.get("control_counts"),
        &[
            ("vertex_count", expected.control_vertices),
            ("edge_count", expected.control_edges),
            ("quad_count", expected.control_quads),
        ],
        "control_counts",
    )?;
    validate_counts(
        object.get("evaluated_counts"),
        &[
            ("vertex_count", expected.evaluated_vertices),
            ("edge_count", expected.evaluated_edges),
            ("quad_count", expected.evaluated_quads),
            ("triangle_count", expected.evaluated_triangles),
        ],
        "evaluated_counts",
    )?;
    if object.get("quad_triangulation").and_then(Value::as_str) != Some("0-1-2_0-2-3") {
        return Err(lineage_error("quad triangulation policy differs"));
    }

    let control_vertices = required_array(
        object.get("control_vertex_to_evaluated_vertex_ids"),
        expected.control_vertices,
        "control vertex descendants",
    )?;
    for (index, value) in control_vertices.iter().enumerate() {
        if value.as_u64() != Some(index as u64) {
            return Err(lineage_error("control vertex descendant ID differs"));
        }
    }

    let vertex_origins = required_array(
        object.get("evaluated_vertex_root_origins"),
        expected.evaluated_vertices,
        "evaluated vertex origins",
    )?;
    for (index, value) in vertex_origins.iter().enumerate() {
        let (kind, root) = origin(value, expected)?;
        if index < expected.control_vertices as usize
            && (kind != "control_vertex" || root != index as u64)
        {
            return Err(lineage_error("retained control vertex origin differs"));
        }
    }

    let edge_origins = required_array(
        object.get("evaluated_edge_root_origins"),
        expected.evaluated_edges,
        "evaluated edge origins",
    )?;
    let edge_chains = required_array(
        object.get("control_edge_to_evaluated_edge_ids"),
        expected.control_edges,
        "control edge descendants",
    )?;
    let chain_length = 1u64 << expected.levels;
    let mut claimed_edges = BTreeSet::<u64>::new();
    for (control_edge_id, chain) in edge_chains.iter().enumerate() {
        let chain = required_array(Some(chain), chain_length, "control edge descendant chain")?;
        for value in chain {
            let evaluated_edge_id = value
                .as_u64()
                .filter(|id| *id < expected.evaluated_edges)
                .ok_or_else(|| lineage_error("control edge descendant is invalid"))?;
            if !claimed_edges.insert(evaluated_edge_id) {
                return Err(lineage_error("control edge descendant is duplicated"));
            }
            let (kind, root) = origin(&edge_origins[evaluated_edge_id as usize], expected)?;
            if kind != "control_edge" || root != control_edge_id as u64 {
                return Err(lineage_error("control edge descendant origin differs"));
            }
        }
    }
    for (evaluated_edge_id, value) in edge_origins.iter().enumerate() {
        let (kind, _) = origin(value, expected)?;
        if (kind == "control_edge") != claimed_edges.contains(&(evaluated_edge_id as u64)) {
            return Err(lineage_error("evaluated edge origin coverage differs"));
        }
    }

    let descendants_per_quad = 4u64.pow(expected.levels as u32);
    let quad_ranges = required_array(
        object.get("control_quad_descendant_ranges"),
        expected.control_quads,
        "control quad descendant ranges",
    )?;
    for (control_quad_id, value) in quad_ranges.iter().enumerate() {
        let range = exact_object(
            value,
            &[
                "evaluated_quad_start",
                "evaluated_quad_count",
                "evaluated_triangle_start",
                "evaluated_triangle_count",
            ],
            "control quad descendant range",
        )?;
        let start = control_quad_id as u64 * descendants_per_quad;
        if range.get("evaluated_quad_start").and_then(Value::as_u64) != Some(start)
            || range.get("evaluated_quad_count").and_then(Value::as_u64)
                != Some(descendants_per_quad)
            || range
                .get("evaluated_triangle_start")
                .and_then(Value::as_u64)
                != Some(start * 2)
            || range
                .get("evaluated_triangle_count")
                .and_then(Value::as_u64)
                != Some(descendants_per_quad * 2)
        {
            return Err(lineage_error("control quad descendant range differs"));
        }
    }
    let quad_roots = required_array(
        object.get("evaluated_quad_control_quad_ids"),
        expected.evaluated_quads,
        "evaluated quad roots",
    )?;
    for (evaluated_quad_id, root) in quad_roots.iter().enumerate() {
        if root.as_u64() != Some(evaluated_quad_id as u64 / descendants_per_quad) {
            return Err(lineage_error("evaluated quad root differs"));
        }
    }

    let crease_chains = required_array(
        object.get("control_crease_edge_chains"),
        expected.crease_edges.len() as u64,
        "control crease chains",
    )?;
    for (index, value) in crease_chains.iter().enumerate() {
        let chain = exact_object(
            value,
            &["control_edge_id", "sharpness_levels", "evaluated_edge_ids"],
            "control crease chain",
        )?;
        let (expected_edge, expected_sharpness) = expected.crease_edges[index];
        if chain.get("control_edge_id").and_then(Value::as_u64) != Some(expected_edge)
            || chain.get("sharpness_levels").and_then(Value::as_u64) != Some(expected_sharpness)
            || chain.get("evaluated_edge_ids") != edge_chains.get(expected_edge as usize)
        {
            return Err(lineage_error("control crease chain differs"));
        }
    }
    if value != &expected.full_lineage {
        return Err(lineage_error(
            "full independently reconstructed root topology differs",
        ));
    }
    Ok(())
}

fn expected_lineage(program: &Value, node_id: &str) -> Result<ExpectedLineage, RuntimeError> {
    let object = program
        .as_object()
        .ok_or_else(|| lineage_error("geometry_program must be an object"))?;
    let program_sha256 = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| lineage_error("geometry_program canonical hash is invalid"))?
        .to_owned();
    let nodes = object
        .get("nodes")
        .and_then(Value::as_array)
        .filter(|nodes| !nodes.is_empty() && nodes.len() <= 4096)
        .ok_or_else(|| lineage_error("geometry_program nodes are invalid"))?;
    let node = nodes
        .iter()
        .find(|node| node.get("node_id").and_then(Value::as_str) == Some(node_id))
        .ok_or_else(|| lineage_error("subdivision_node_id is unavailable"))?;
    let node = node
        .as_object()
        .ok_or_else(|| lineage_error("subdivision node is invalid"))?;
    if node.get("operator_id").and_then(Value::as_str) != Some("forgecad.geometry.subd-cage@2")
        || node
            .get("inputs")
            .and_then(Value::as_array)
            .is_none_or(|inputs| !inputs.is_empty())
    {
        return Err(lineage_error(
            "subdivision_node_id must select input-free subd-cage@2",
        ));
    }
    let parameters = node
        .get("parameters")
        .and_then(Value::as_object)
        .ok_or_else(|| lineage_error("subdivision parameters are invalid"))?;
    let u_points = bounded_count(parameters, "u_points", 3, 16)?;
    let v_points = bounded_count(parameters, "v_points", 3, 16)?;
    let levels = bounded_count(parameters, "subdivision_levels", 1, 2)?;
    if parameters.get("crease_method").and_then(Value::as_str)
        != Some("uniform-integer-level-decay@1")
    {
        return Err(lineage_error("subdivision crease method differs"));
    }
    let control_vertices = u_points * v_points;
    if parameters
        .get("control_points")
        .and_then(Value::as_array)
        .is_none_or(|points| points.len() as u64 != control_vertices)
    {
        return Err(lineage_error("control point inventory differs"));
    }
    let control_edges = v_points * (u_points - 1) + (v_points - 1) * u_points;
    let control_quads = (u_points - 1) * (v_points - 1);
    let scale = 1u64 << levels;
    let evaluated_u = (u_points - 1) * scale + 1;
    let evaluated_v = (v_points - 1) * scale + 1;
    let evaluated_vertices = evaluated_u * evaluated_v;
    let evaluated_edges = evaluated_v * (evaluated_u - 1) + (evaluated_v - 1) * evaluated_u;
    let evaluated_quads = (evaluated_u - 1) * (evaluated_v - 1);
    let evaluated_triangles = evaluated_quads * 2;
    let lineage_elements = control_vertices
        + control_edges
        + control_quads
        + evaluated_vertices
        + evaluated_edges
        + evaluated_quads
        + evaluated_triangles;
    let crease_values = parameters
        .get("crease_edges")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 128)
        .ok_or_else(|| lineage_error("crease edge inventory is invalid"))?;
    let mut crease_edges = Vec::with_capacity(crease_values.len());
    for value in crease_values {
        let edge = value
            .as_object()
            .ok_or_else(|| lineage_error("crease edge is invalid"))?;
        let a = edge
            .get("vertex_a")
            .and_then(Value::as_u64)
            .ok_or_else(|| lineage_error("crease vertex_a is invalid"))?;
        let b = edge
            .get("vertex_b")
            .and_then(Value::as_u64)
            .ok_or_else(|| lineage_error("crease vertex_b is invalid"))?;
        let sharpness = edge
            .get("sharpness_levels")
            .and_then(Value::as_u64)
            .filter(|value| (1..=2).contains(value))
            .ok_or_else(|| lineage_error("crease sharpness is invalid"))?;
        let edge_id = control_edge_id(u_points, v_points, a, b)
            .ok_or_else(|| lineage_error("crease edge is not a control-grid edge"))?;
        crease_edges.push((edge_id, sharpness));
    }
    let full_lineage = reconstruct_expected_lineage(u_points, v_points, levels, &crease_edges)?;
    Ok(ExpectedLineage {
        program_sha256,
        node_id: node_id.to_owned(),
        u_points,
        v_points,
        levels,
        control_vertices,
        control_edges,
        control_quads,
        evaluated_vertices,
        evaluated_edges,
        evaluated_quads,
        evaluated_triangles,
        lineage_elements,
        crease_edges,
        full_lineage,
    })
}

fn reconstruct_expected_lineage(
    u_points: u64,
    v_points: u64,
    levels: u64,
    crease_edges: &[(u64, u64)],
) -> Result<Value, RuntimeError> {
    let control_vertices = u_points * v_points;
    let horizontal_edge_count = v_points * (u_points - 1);
    let control_edges = horizontal_edge_count + (v_points - 1) * u_points;
    let control_quads = (u_points - 1) * (v_points - 1);

    let mut faces = Vec::<[u64; 4]>::with_capacity(control_quads as usize);
    for v_index in 0..v_points - 1 {
        for u_index in 0..u_points - 1 {
            let a = v_index * u_points + u_index;
            let b = a + 1;
            let d = a + u_points;
            let c = d + 1;
            faces.push([a, b, c, d]);
        }
    }
    let mut vertex_roots = (0..control_vertices)
        .map(RootOrigin::ControlVertex)
        .collect::<Vec<_>>();
    let mut face_roots = (0..control_quads).collect::<Vec<_>>();
    let mut edge_roots = BTreeMap::<(u64, u64), RootOrigin>::new();
    for v_index in 0..v_points {
        for u_index in 0..u_points - 1 {
            let a = v_index * u_points + u_index;
            edge_roots.insert(
                (a, a + 1),
                RootOrigin::ControlEdge(v_index * (u_points - 1) + u_index),
            );
        }
    }
    for v_index in 0..v_points - 1 {
        for u_index in 0..u_points {
            let a = v_index * u_points + u_index;
            edge_roots.insert(
                (a, a + u_points),
                RootOrigin::ControlEdge(horizontal_edge_count + v_index * u_points + u_index),
            );
        }
    }
    if edge_roots.len() as u64 != control_edges {
        return Err(lineage_error("independent control edge inventory differs"));
    }

    for _ in 0..levels {
        let mut edge_lookup = BTreeMap::<(u64, u64), usize>::new();
        let mut input_edges = Vec::<(u64, u64)>::new();
        let mut face_edges = Vec::<[usize; 4]>::with_capacity(faces.len());
        for face in &faces {
            let pairs = [
                (face[0], face[1]),
                (face[1], face[2]),
                (face[2], face[3]),
                (face[3], face[0]),
            ];
            let mut ids = [0usize; 4];
            for (slot, (left, right)) in pairs.into_iter().enumerate() {
                let key = (left.min(right), left.max(right));
                let id = if let Some(id) = edge_lookup.get(&key).copied() {
                    id
                } else {
                    let id = input_edges.len();
                    edge_lookup.insert(key, id);
                    input_edges.push(key);
                    id
                };
                ids[slot] = id;
            }
            face_edges.push(ids);
        }

        let vertex_count = vertex_roots.len() as u64;
        let edge_offset = vertex_count;
        let face_offset = edge_offset + input_edges.len() as u64;
        let mut next_vertex_roots = vertex_roots.clone();
        for key in &input_edges {
            next_vertex_roots.push(
                *edge_roots
                    .get(key)
                    .ok_or_else(|| lineage_error("independent input edge root is missing"))?,
            );
        }
        next_vertex_roots.extend(face_roots.iter().copied().map(RootOrigin::ControlQuad));

        let mut next_edge_roots = BTreeMap::<(u64, u64), RootOrigin>::new();
        for (edge_index, (a, b)) in input_edges.iter().copied().enumerate() {
            let root = *edge_roots
                .get(&(a, b))
                .ok_or_else(|| lineage_error("independent input edge root is missing"))?;
            let edge_point = edge_offset + edge_index as u64;
            for endpoint in [a, b] {
                let key = (endpoint.min(edge_point), endpoint.max(edge_point));
                if next_edge_roots.insert(key, root).is_some() {
                    return Err(lineage_error(
                        "independent child edge inventory is ambiguous",
                    ));
                }
            }
        }

        for (face_index, edge_ids) in face_edges.iter().enumerate() {
            let face_point = face_offset + face_index as u64;
            let root = RootOrigin::ControlQuad(face_roots[face_index]);
            for edge_index in edge_ids {
                let edge_point = edge_offset + *edge_index as u64;
                let key = (edge_point.min(face_point), edge_point.max(face_point));
                match next_edge_roots.insert(key, root) {
                    None => {}
                    Some(existing) if existing == root => {}
                    Some(_) => {
                        return Err(lineage_error("independent internal edge root conflicts"));
                    }
                }
            }
        }

        let mut next_faces = Vec::<[u64; 4]>::with_capacity(faces.len() * 4);
        for (face_index, face) in faces.iter().enumerate() {
            let [edge_ab, edge_bc, edge_cd, edge_da] = face_edges[face_index];
            let face_point = face_offset + face_index as u64;
            next_faces.extend([
                [
                    face[0],
                    edge_offset + edge_ab as u64,
                    face_point,
                    edge_offset + edge_da as u64,
                ],
                [
                    face[1],
                    edge_offset + edge_bc as u64,
                    face_point,
                    edge_offset + edge_ab as u64,
                ],
                [
                    face[2],
                    edge_offset + edge_cd as u64,
                    face_point,
                    edge_offset + edge_bc as u64,
                ],
                [
                    face[3],
                    edge_offset + edge_da as u64,
                    face_point,
                    edge_offset + edge_cd as u64,
                ],
            ]);
        }
        let mut next_face_roots = Vec::with_capacity(face_roots.len() * 4);
        for root in &face_roots {
            next_face_roots.extend([*root; 4]);
        }
        faces = next_faces;
        face_roots = next_face_roots;
        vertex_roots = next_vertex_roots;
        edge_roots = next_edge_roots;
    }

    let mut evaluated_edge_keys = BTreeSet::<(u64, u64)>::new();
    for face in &faces {
        for (left, right) in [
            (face[0], face[1]),
            (face[1], face[2]),
            (face[2], face[3]),
            (face[3], face[0]),
        ] {
            evaluated_edge_keys.insert((left.min(right), left.max(right)));
        }
    }
    if evaluated_edge_keys.len() != edge_roots.len() {
        return Err(lineage_error(
            "independent evaluated edge inventory differs",
        ));
    }
    let mut control_edge_descendants = vec![Vec::<u64>::new(); control_edges as usize];
    let mut evaluated_edge_origins = Vec::<RootOrigin>::with_capacity(evaluated_edge_keys.len());
    for (evaluated_edge_id, key) in evaluated_edge_keys.iter().enumerate() {
        let root = *edge_roots
            .get(key)
            .ok_or_else(|| lineage_error("independent final edge root is missing"))?;
        if let RootOrigin::ControlEdge(control_edge_id) = root {
            control_edge_descendants[control_edge_id as usize].push(evaluated_edge_id as u64);
        }
        evaluated_edge_origins.push(root);
    }
    let expected_chain_length = 1usize << levels;
    if control_edge_descendants
        .iter()
        .any(|chain| chain.len() != expected_chain_length)
    {
        return Err(lineage_error(
            "independent control edge chain length differs",
        ));
    }

    let descendants_per_quad = 4u64.pow(levels as u32);
    let control_quad_ranges = (0..control_quads)
        .map(|control_quad_id| {
            let start = control_quad_id * descendants_per_quad;
            json!({
                "evaluated_quad_start":start,
                "evaluated_quad_count":descendants_per_quad,
                "evaluated_triangle_start":start * 2,
                "evaluated_triangle_count":descendants_per_quad * 2
            })
        })
        .collect::<Vec<_>>();
    let control_crease_edge_chains = crease_edges
        .iter()
        .map(|(control_edge_id, sharpness_levels)| {
            json!({
                "control_edge_id":control_edge_id,
                "sharpness_levels":sharpness_levels,
                "evaluated_edge_ids":control_edge_descendants[*control_edge_id as usize]
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "control_dimensions":{"u_points":u_points,"v_points":v_points},
        "subdivision_levels":levels,
        "control_counts":{"vertex_count":control_vertices,"edge_count":control_edges,"quad_count":control_quads},
        "evaluated_counts":{"vertex_count":vertex_roots.len(),"edge_count":evaluated_edge_keys.len(),"quad_count":faces.len(),"triangle_count":faces.len() * 2},
        "control_vertex_to_evaluated_vertex_ids":(0..control_vertices).collect::<Vec<_>>(),
        "control_edge_to_evaluated_edge_ids":control_edge_descendants,
        "control_quad_descendant_ranges":control_quad_ranges,
        "control_crease_edge_chains":control_crease_edge_chains,
        "evaluated_vertex_root_origins":vertex_roots.into_iter().map(RootOrigin::wire).collect::<Vec<_>>(),
        "evaluated_edge_root_origins":evaluated_edge_origins.into_iter().map(RootOrigin::wire).collect::<Vec<_>>(),
        "evaluated_quad_control_quad_ids":face_roots,
        "quad_triangulation":"0-1-2_0-2-3"
    }))
}

fn verify_geometry_program_canonical_hash(program: &Value) -> Result<(), RuntimeError> {
    let object = program
        .as_object()
        .ok_or_else(|| lineage_error("geometry_program must be an object"))?;
    if object.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2") {
        return Err(lineage_error("geometry_program schema_version differs"));
    }
    if object
        .get("operator_catalog_sha256")
        .and_then(Value::as_str)
        != Some(operator_catalog_sha256().as_str())
    {
        return Err(lineage_error("geometry_program operator catalog differs"));
    }
    let actual = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| lineage_error("geometry_program canonical_sha256 is invalid"))?;
    let mut binding = program.clone();
    binding
        .as_object_mut()
        .expect("program is an object")
        .remove("canonical_sha256");
    if actual != canonical_json_hash(&binding) {
        return Err(lineage_error("geometry_program canonical hash differs"));
    }
    Ok(())
}

fn validate_counts(
    value: Option<&Value>,
    expected: &[(&str, u64)],
    label: &str,
) -> Result<(), RuntimeError> {
    let value = value
        .and_then(Value::as_object)
        .ok_or_else(|| lineage_error(&format!("{label} is invalid")))?;
    if value.len() != expected.len()
        || expected
            .iter()
            .any(|(key, count)| value.get(*key).and_then(Value::as_u64) != Some(*count))
    {
        return Err(lineage_error(&format!("{label} differs")));
    }
    Ok(())
}

fn origin<'a>(
    value: &'a Value,
    expected: &ExpectedLineage,
) -> Result<(&'a str, u64), RuntimeError> {
    let pair = value
        .as_array()
        .filter(|pair| pair.len() == 2)
        .ok_or_else(|| lineage_error("element origin is invalid"))?;
    let kind = pair[0]
        .as_str()
        .ok_or_else(|| lineage_error("element origin kind is invalid"))?;
    let root = pair[1]
        .as_u64()
        .ok_or_else(|| lineage_error("element origin ID is invalid"))?;
    let bound = match kind {
        "control_vertex" => expected.control_vertices,
        "control_edge" => expected.control_edges,
        "control_quad" => expected.control_quads,
        _ => return Err(lineage_error("element origin kind differs")),
    };
    if root >= bound {
        return Err(lineage_error(
            "element origin ID is outside its control bound",
        ));
    }
    Ok((kind, root))
}

fn required_array<'a>(
    value: Option<&'a Value>,
    expected_len: u64,
    label: &str,
) -> Result<&'a Vec<Value>, RuntimeError> {
    value
        .and_then(Value::as_array)
        .filter(|values| values.len() as u64 == expected_len)
        .ok_or_else(|| lineage_error(&format!("{label} length differs")))
}

fn bounded_count(
    object: &Map<String, Value>,
    key: &str,
    minimum: u64,
    maximum: u64,
) -> Result<u64, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .filter(|value| (minimum..=maximum).contains(value))
        .ok_or_else(|| lineage_error(&format!("{key} is outside its bound")))
}

fn control_edge_id(u_points: u64, v_points: u64, a: u64, b: u64) -> Option<u64> {
    if a >= u_points * v_points || b >= u_points * v_points || a >= b {
        return None;
    }
    let a_row = a / u_points;
    let a_column = a % u_points;
    let b_row = b / u_points;
    let b_column = b % u_points;
    if a_row == b_row && b_column == a_column + 1 {
        return Some(a_row * (u_points - 1) + a_column);
    }
    if a_column == b_column && b_row == a_row + 1 {
        return Some(v_points * (u_points - 1) + a_row * u_points + a_column);
    }
    None
}

fn required_id(object: &Map<String, Value>, key: &str) -> Result<String, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .map(str::to_owned)
        .ok_or_else(|| lineage_error(&format!("{key} is invalid")))
}

fn lineage_error(detail: &str) -> RuntimeError {
    RuntimeError::InvalidInput(format!("SUBDIVISION_TOPOLOGY_LINEAGE_INVALID: {detail}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program() -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"subdivision-lineage-runtime",
            "representation_plan_sha256":"a".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":4,"max_triangles":128,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[{
                "node_id":"cage",
                "operator_id":"forgecad.geometry.subd-cage@2",
                "inputs":[],
                "parameters":{
                    "shape":"subd-cage",
                    "control_points":[[-1.0,-1.0,0.0],[0.0,-1.0,0.0],[1.0,-1.0,0.0],[-1.0,0.0,0.0],[0.0,0.0,1.0],[1.0,0.0,0.0],[-1.0,1.0,0.0],[0.0,1.0,0.0],[1.0,1.0,0.0]],
                    "u_points":3,"v_points":3,"subdivision_levels":2,
                    "crease_method":"uniform-integer-level-decay@1",
                    "crease_edges":[{"vertex_a":3,"vertex_b":4,"sharpness_levels":2},{"vertex_a":4,"vertex_b":5,"sharpness_levels":2}],
                    "position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{"part_id":"cage","input_node_ids":["cage"],"material_zone_id":"zone-white-shell","solid":false}]
        });
        program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
        program
    }

    fn request(program: Value, max_lineage_elements: u64) -> Value {
        let mut request = json!({
            "schema_version":REQUEST_SCHEMA,
            "geometry_program":program,
            "subdivision_node_id":"cage",
            "max_lineage_elements":max_lineage_elements,
            "canonical_sha256":""
        });
        request["canonical_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    fn rehash_worker_result(result: &mut Value) {
        result["lineage_sha256"] = Value::String(canonical_json_hash(&result["lineage"]));
        result
            .as_object_mut()
            .expect("Worker result object")
            .remove("canonical_sha256");
        result["canonical_sha256"] = Value::String(canonical_json_hash(result));
    }

    #[test]
    fn runtime_validates_complete_worker_lineage_without_writing() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let before = json!({
            "projects":runtime.projects().unwrap(),
            "candidates":runtime.store.list_candidates("subdivision-lineage-runtime").unwrap(),
            "versions":runtime.store.list_versions(None).unwrap(),
            "cas":runtime.store.cas().list_objects().unwrap()
        });
        let first = runtime
            .subdivision_topology_lineage_preview(request(program(), 25_000))
            .expect("runtime subdivision lineage");
        let second = runtime
            .subdivision_topology_lineage_preview(request(program(), 25_000))
            .expect("deterministic runtime subdivision lineage");
        assert_eq!(first, second);
        assert_eq!(first["lineage_element_count"], 442);
        assert!(serde_json::to_vec(&first).unwrap().len() < MAX_RESPONSE_BYTES);
        let after = json!({
            "projects":runtime.projects().unwrap(),
            "candidates":runtime.store.list_candidates("subdivision-lineage-runtime").unwrap(),
            "versions":runtime.store.list_versions(None).unwrap(),
            "cas":runtime.store.cas().list_objects().unwrap()
        });
        assert_eq!(before, after);
    }

    #[test]
    fn runtime_rejects_wrong_version_hash_budget_and_unknown_fields() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let program = program();
        assert!(runtime
            .subdivision_topology_lineage_preview(request(program.clone(), 441))
            .is_err());
        let mut wrong_version = program.clone();
        wrong_version["nodes"][0]["operator_id"] = json!("forgecad.geometry.subd-cage@1");
        wrong_version["canonical_sha256"] = json!("");
        wrong_version["canonical_sha256"] = Value::String(canonical_json_hash(&wrong_version));
        assert!(runtime
            .subdivision_topology_lineage_preview(request(wrong_version, 25_000))
            .is_err());
        let mut stale_program = program.clone();
        stale_program["nodes"][0]["parameters"]["u_points"] = json!(4);
        assert!(runtime
            .subdivision_topology_lineage_preview(request(stale_program, 25_000))
            .is_err());
        let mut unknown = request(program, 25_000);
        unknown["python"] = json!("forbidden");
        unknown["canonical_sha256"] = json!("");
        unknown["canonical_sha256"] = Value::String(canonical_json_hash(&unknown));
        assert!(runtime
            .subdivision_topology_lineage_preview(unknown)
            .is_err());
    }

    #[test]
    fn runtime_rejects_tampered_root_coverage_and_keeps_topology_hash_semantics_explicit() {
        let base_program = program();
        let expected = expected_lineage(&base_program, "cage").expect("expected lineage");
        let result = geometry_worker::subdivision_topology_lineage_test_fallback(
            &base_program,
            "cage",
            25_000,
        )
        .expect("Worker lineage fixture");
        validate_result(&result, 25_000, &expected).expect("valid Worker lineage");

        let mut out_of_range = result.clone();
        out_of_range["lineage"]["evaluated_vertex_root_origins"][0] = json!(["control_vertex", 9]);
        assert!(validate_result(&out_of_range, 25_000, &expected).is_err());

        let mut duplicate_edge = result.clone();
        duplicate_edge["lineage"]["control_edge_to_evaluated_edge_ids"][1][0] =
            duplicate_edge["lineage"]["control_edge_to_evaluated_edge_ids"][0][0].clone();
        assert!(validate_result(&duplicate_edge, 25_000, &expected).is_err());

        let mut wrong_quad_root = result.clone();
        wrong_quad_root["lineage"]["evaluated_quad_control_quad_ids"][16] = json!(0);
        assert!(validate_result(&wrong_quad_root, 25_000, &expected).is_err());

        let mut legal_but_wrong_vertex_root = result.clone();
        legal_but_wrong_vertex_root["lineage"]["evaluated_vertex_root_origins"][9] =
            json!(["control_edge", 1]);
        rehash_worker_result(&mut legal_but_wrong_vertex_root);
        assert!(validate_result(&legal_but_wrong_vertex_root, 25_000, &expected).is_err());

        let mut legal_but_wrong_internal_edge_root = result.clone();
        let internal_edge_index = legal_but_wrong_internal_edge_root["lineage"]
            ["evaluated_edge_root_origins"]
            .as_array()
            .expect("evaluated edge origins")
            .iter()
            .position(|origin| origin == &json!(["control_quad", 0]))
            .expect("control quad edge root");
        legal_but_wrong_internal_edge_root["lineage"]["evaluated_edge_root_origins"]
            [internal_edge_index] = json!(["control_quad", 1]);
        rehash_worker_result(&mut legal_but_wrong_internal_edge_root);
        assert!(validate_result(&legal_but_wrong_internal_edge_root, 25_000, &expected).is_err());

        let mut reordered_control_edge_chain = result.clone();
        reordered_control_edge_chain["lineage"]["control_edge_to_evaluated_edge_ids"][0]
            .as_array_mut()
            .expect("control edge chain")
            .swap(0, 1);
        rehash_worker_result(&mut reordered_control_edge_chain);
        assert!(validate_result(&reordered_control_edge_chain, 25_000, &expected).is_err());

        let runtime = Runtime::ephemeral().expect("runtime");
        let original = runtime
            .subdivision_topology_lineage_preview(request(base_program.clone(), 25_000))
            .expect("original topology lineage");
        let mut edited_program = base_program;
        edited_program["nodes"][0]["parameters"]["control_points"][4][2] = json!(0.5);
        edited_program
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        edited_program["canonical_sha256"] = Value::String(canonical_json_hash(&edited_program));
        let edited = runtime
            .subdivision_topology_lineage_preview(request(edited_program, 25_000))
            .expect("edited topology lineage");
        assert_ne!(original["program_sha256"], edited["program_sha256"]);
        assert_ne!(original["canonical_sha256"], edited["canonical_sha256"]);
        assert_eq!(
            original["lineage_sha256"], edited["lineage_sha256"],
            "control-point positions change evaluation geometry but not regular-grid root topology"
        );
    }
}
