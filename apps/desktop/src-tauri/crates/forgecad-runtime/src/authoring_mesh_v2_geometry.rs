//! Closed lowering from Runtime-owned `AuthoringMeshRevision@2` truth to the
//! existing fixed `forgecad.geometry.authoring-mesh@1` Worker operator.
//!
//! This adapter never accepts caller-owned element IDs or a GeometryProgram.
//! It first revalidates the immutable AuthoringMesh revision, then projects
//! the exact stable topology into the older triangle/quad operator surface.
//! Part/source/material ownership remains a separate Runtime concern.

use super::{authoring_mesh_v2::AuthoringMeshV2Revision, canonical_json_hash, RuntimeError};
use forgecad_contracts::{AuthoringMeshEdge, AuthoringMeshHalfEdge, AuthoringMeshRevision};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const MAX_TRANSFORM_ABS: f64 = 10.0;
const MAX_ROTATION_ABS: f64 = std::f64::consts::PI * 2.0;
const MIN_PROFILE_EDGE_SQUARED: f64 = 1.0e-12;
const MIN_PROFILE_AREA: f64 = 1.0e-12;

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

/// Convert the closed `forgecad.geometry.profile-extrude@1` source operator
/// into an editable local-space AuthoringMesh genesis.
///
/// The profile is interpreted in the local XY plane and extruded symmetrically
/// around local Z.  The input winding is normalized to counter-clockwise and
/// the first vertex is rotated to the lexicographically smallest point.  This
/// makes a profile and its reversed-winding spelling produce byte-identical
/// topology while the source-parameter hash still remains the hash of the
/// exact caller-owned parameter object.
pub(crate) fn profile_extrude_source_genesis(
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
        .filter(|value| *value == "forgecad.geometry.profile-extrude@1")
        .ok_or_else(|| invalid("source operator is not profile-extrude@1"))?;
    if object
        .get("inputs")
        .and_then(Value::as_array)
        .is_none_or(|inputs| !inputs.is_empty())
    {
        return Err(invalid("profile-extrude source must not have inputs"));
    }
    let parameters = object
        .get("parameters")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("profile-extrude parameters are missing"))?;
    let parameter_keys = ["shape", "profile", "depth_m", "position_m", "rotation_rad"];
    if parameters.len() != parameter_keys.len()
        || parameter_keys
            .iter()
            .any(|key| !parameters.contains_key(*key))
        || parameters.get("shape").and_then(Value::as_str) != Some("profile-extrude")
    {
        return Err(invalid(
            "profile-extrude parameters differ from the closed source",
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
        if result.iter().any(|value| !value.is_finite()) {
            return Err(invalid(format!("{field} contains a non-finite value")));
        }
        Ok(result)
    };
    let position_m = vector("position_m")?;
    validate_transform(position_m, "position_m")?;
    let rotation_rad = vector("rotation_rad")?;
    if rotation_rad
        .iter()
        .any(|value| value.abs() > MAX_ROTATION_ABS)
    {
        return Err(invalid("rotation_rad is outside the closed radian bound"));
    }
    let depth_m = parameters
        .get("depth_m")
        .and_then(Value::as_f64)
        .ok_or_else(|| invalid("depth_m must be a number"))?;
    if !depth_m.is_finite() || !(0.0 < depth_m && depth_m <= MAX_TRANSFORM_ABS) {
        return Err(invalid("depth_m is not positive and bounded"));
    }

    let profile_values = parameters
        .get("profile")
        .and_then(Value::as_array)
        .filter(|profile| (3..=64).contains(&profile.len()))
        .ok_or_else(|| invalid("profile must contain between 3 and 64 points"))?;
    let mut profile = profile_values
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let values = point
                .as_array()
                .filter(|values| values.len() == 2)
                .ok_or_else(|| invalid(format!("profile[{index}] must contain two values")))?;
            let point = [
                values[0]
                    .as_f64()
                    .ok_or_else(|| invalid(format!("profile[{index}][0] is invalid")))?,
                values[1]
                    .as_f64()
                    .ok_or_else(|| invalid(format!("profile[{index}][1] is invalid")))?,
            ];
            if point
                .iter()
                .any(|value| !value.is_finite() || value.abs() > MAX_TRANSFORM_ABS)
            {
                return Err(invalid(format!("profile[{index}] is outside bounds")));
            }
            Ok(point)
        })
        .collect::<Result<Vec<[f64; 2]>, RuntimeError>>()?;

    for left in 0..profile.len() {
        let right = (left + 1) % profile.len();
        if squared_distance(profile[left], profile[right]) < MIN_PROFILE_EDGE_SQUARED {
            return Err(invalid("profile contains a duplicate or degenerate edge"));
        }
    }
    for left in 0..profile.len() {
        for right in (left + 1)..profile.len() {
            if squared_distance(profile[left], profile[right]) < MIN_PROFILE_EDGE_SQUARED {
                return Err(invalid("profile contains duplicate vertices"));
            }
        }
    }

    let area = signed_area(&profile);
    if area.abs() <= MIN_PROFILE_AREA {
        return Err(invalid("profile has zero or degenerate area"));
    }
    if area < 0.0 {
        profile.reverse();
    }
    let first = profile
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            left[0]
                .total_cmp(&right[0])
                .then_with(|| left[1].total_cmp(&right[1]))
        })
        .map(|(index, _)| index)
        .ok_or_else(|| invalid("profile has no points"))?;
    profile.rotate_left(first);

    for left in 0..profile.len() {
        let left_next = (left + 1) % profile.len();
        for right in (left + 1)..profile.len() {
            let right_next = (right + 1) % profile.len();
            if left == right || left_next == right || right_next == left {
                continue;
            }
            if segments_intersect(
                profile[left],
                profile[left_next],
                profile[right],
                profile[right_next],
            ) {
                return Err(invalid("profile boundary self-intersects"));
            }
        }
    }

    let cap_triangles = ear_clip(&profile)?;
    let count = profile.len();
    let half_depth = depth_m * 0.5;
    let mut positions_m = Vec::with_capacity(count * 2);
    positions_m.extend(profile.iter().map(|[x, y]| [*x, *y, -half_depth]));
    positions_m.extend(profile.iter().map(|[x, y]| [*x, *y, half_depth]));

    let mut faces = Vec::with_capacity(count * 2 + cap_triangles.len() * 2);
    for index in 0..count {
        let next = (index + 1) % count;
        faces.push(vec![index, next, count + next, count + index]);
    }
    for [first, second, third] in &cap_triangles {
        faces.push(vec![*first, *third, *second]);
    }
    for [first, second, third] in &cap_triangles {
        faces.push(vec![count + first, count + second, count + third]);
    }

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

/// Convert a Runtime-derived `authoring-mesh@1` GeometryProgram node back to
/// an immutable AuthoringMesh genesis.  This is the closed inverse of
/// `authoring_mesh_v2_geometry_parameters`; it accepts no arbitrary operator
/// or caller element identity outside the already compiled, candidate-owned
/// GeometryProgram CAS object.
pub(crate) fn authoring_mesh_source_genesis(
    node: &Value,
    expected_node_id: &str,
) -> Result<AuthoringMeshV2SourceGenesis, RuntimeError> {
    fn exact<'a>(
        value: &'a Value,
        keys: &[&str],
        context: &str,
    ) -> Result<&'a Map<String, Value>, RuntimeError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid(format!("{context} must be an object")))?;
        if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
            return Err(invalid(format!("{context} differs from the closed shape")));
        }
        Ok(object)
    }
    let object = exact(
        node,
        &["node_id", "operator_id", "inputs", "parameters"],
        "authoring-mesh source node",
    )?;
    let node_id = object
        .get("node_id")
        .and_then(Value::as_str)
        .filter(|value| *value == expected_node_id)
        .ok_or_else(|| invalid("authoring-mesh source node identity differs"))?;
    let operator_id = object
        .get("operator_id")
        .and_then(Value::as_str)
        .filter(|value| *value == "forgecad.geometry.authoring-mesh@1")
        .ok_or_else(|| invalid("source operator is not authoring-mesh@1"))?;
    if object
        .get("inputs")
        .and_then(Value::as_array)
        .is_none_or(|inputs| !inputs.is_empty())
    {
        return Err(invalid("authoring-mesh source must not have inputs"));
    }
    let parameters = object
        .get("parameters")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("authoring-mesh parameters are missing"))?;
    let parameter_keys = [
        "shape",
        "topology_policy",
        "vertices",
        "edges",
        "loops",
        "faces",
        "position_m",
        "rotation_rad",
    ];
    if parameters.len() != parameter_keys.len()
        || parameter_keys
            .iter()
            .any(|key| !parameters.contains_key(*key))
        || parameters.get("shape").and_then(Value::as_str) != Some("authoring-mesh")
        || parameters.get("topology_policy").and_then(Value::as_str)
            != Some("triangle-quad-manifold-with-boundary@1")
    {
        return Err(invalid(
            "authoring-mesh parameters differ from the closed source",
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
    let position_m = vector("position_m")?;
    let rotation_rad = vector("rotation_rad")?;

    let vertex_values = parameters
        .get("vertices")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .ok_or_else(|| invalid("authoring-mesh vertices are empty"))?;
    let mut vertex_index = BTreeMap::<String, usize>::new();
    let mut positions_m = Vec::with_capacity(vertex_values.len());
    for vertex in vertex_values {
        let vertex = exact(vertex, &["element_id", "position_m"], "authoring vertex")?;
        let element_id = vertex
            .get("element_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty() && value.len() <= 128)
            .ok_or_else(|| invalid("authoring vertex element_id is invalid"))?;
        let values = vertex
            .get("position_m")
            .and_then(Value::as_array)
            .filter(|values| values.len() == 3)
            .ok_or_else(|| invalid("authoring vertex position_m is invalid"))?;
        let position = [
            values[0]
                .as_f64()
                .ok_or_else(|| invalid("authoring vertex X is invalid"))?,
            values[1]
                .as_f64()
                .ok_or_else(|| invalid("authoring vertex Y is invalid"))?,
            values[2]
                .as_f64()
                .ok_or_else(|| invalid("authoring vertex Z is invalid"))?,
        ];
        validate_transform(position, "authoring vertex position_m")?;
        if vertex_index
            .insert(element_id.to_owned(), positions_m.len())
            .is_some()
        {
            return Err(invalid("authoring vertex identity is duplicated"));
        }
        positions_m.push(position);
    }

    let edge_values = parameters
        .get("edges")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("authoring-mesh edges are missing"))?;
    let mut edges = BTreeMap::<String, [String; 2]>::new();
    for edge in edge_values {
        let edge = exact(edge, &["element_id", "vertex_ids"], "authoring edge")?;
        let edge_id = edge
            .get("element_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("authoring edge element_id is invalid"))?;
        let endpoints = edge
            .get("vertex_ids")
            .and_then(Value::as_array)
            .filter(|values| values.len() == 2)
            .ok_or_else(|| invalid("authoring edge vertex_ids are invalid"))?;
        let endpoint = |index: usize| -> Result<String, RuntimeError> {
            let value = endpoints[index]
                .as_str()
                .ok_or_else(|| invalid("authoring edge endpoint is invalid"))?;
            if !vertex_index.contains_key(value) {
                return Err(invalid("authoring edge references an unknown vertex"));
            }
            Ok(value.to_owned())
        };
        let pair = [endpoint(0)?, endpoint(1)?];
        if pair[0] == pair[1] || edges.insert(edge_id.to_owned(), pair).is_some() {
            return Err(invalid("authoring edge is degenerate or duplicated"));
        }
    }

    let loop_values = parameters
        .get("loops")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("authoring-mesh loops are missing"))?;
    let mut loops = BTreeMap::<String, (String, usize, String, String, bool)>::new();
    for loop_value in loop_values {
        let loop_object = exact(
            loop_value,
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
        let string = |field: &str| -> Result<String, RuntimeError> {
            loop_object
                .get(field)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| invalid(format!("authoring loop {field} is invalid")))
        };
        let loop_id = string("element_id")?;
        let entry = (
            string("face_id")?,
            loop_object
                .get("ordinal")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| invalid("authoring loop ordinal is invalid"))?,
            string("vertex_id")?,
            string("edge_id")?,
            loop_object
                .get("edge_forward")
                .and_then(Value::as_bool)
                .ok_or_else(|| invalid("authoring loop edge_forward is invalid"))?,
        );
        if !vertex_index.contains_key(&entry.2)
            || !edges.contains_key(&entry.3)
            || loops.insert(loop_id, entry).is_some()
        {
            return Err(invalid("authoring loop binding is invalid or duplicated"));
        }
    }

    let face_values = parameters
        .get("faces")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .ok_or_else(|| invalid("authoring-mesh faces are empty"))?;
    let mut faces = Vec::with_capacity(face_values.len());
    let mut used_loops = BTreeSet::new();
    for face in face_values {
        let face = exact(face, &["element_id", "loop_ids"], "authoring face")?;
        let face_id = face
            .get("element_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("authoring face element_id is invalid"))?;
        let loop_ids = face
            .get("loop_ids")
            .and_then(Value::as_array)
            .filter(|values| (3..=4).contains(&values.len()))
            .ok_or_else(|| invalid("authoring face must contain three or four loops"))?;
        let mut ordered = loop_ids
            .iter()
            .map(|value| {
                let loop_id = value
                    .as_str()
                    .ok_or_else(|| invalid("authoring face loop identity is invalid"))?;
                if !used_loops.insert(loop_id.to_owned()) {
                    return Err(invalid("authoring loop is reused by multiple faces"));
                }
                let entry = loops
                    .get(loop_id)
                    .ok_or_else(|| invalid("authoring face references an unknown loop"))?;
                if entry.0 != face_id {
                    return Err(invalid("authoring loop face binding differs"));
                }
                Ok(entry)
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        ordered.sort_by_key(|entry| entry.1);
        if ordered
            .iter()
            .enumerate()
            .any(|(ordinal, entry)| entry.1 != ordinal)
        {
            return Err(invalid("authoring loop ordinals are not contiguous"));
        }
        let mut face_vertices = Vec::with_capacity(ordered.len());
        for (ordinal, entry) in ordered.iter().enumerate() {
            let next_vertex_id = &ordered[(ordinal + 1) % ordered.len()].2;
            let edge = edges
                .get(&entry.3)
                .ok_or_else(|| invalid("authoring loop edge is unavailable"))?;
            let expected = if entry.4 {
                [&entry.2, next_vertex_id]
            } else {
                [next_vertex_id, &entry.2]
            };
            if edge[0] != *expected[0] || edge[1] != *expected[1] {
                return Err(invalid("authoring loop edge direction differs"));
            }
            face_vertices.push(
                *vertex_index
                    .get(&entry.2)
                    .ok_or_else(|| invalid("authoring face vertex is unavailable"))?,
            );
        }
        faces.push(face_vertices);
    }
    if used_loops.len() != loops.len() {
        return Err(invalid("authoring-mesh contains unreferenced loops"));
    }
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

fn squared_distance(left: [f64; 2], right: [f64; 2]) -> f64 {
    (left[0] - right[0]).mul_add(
        left[0] - right[0],
        (left[1] - right[1]) * (left[1] - right[1]),
    )
}

fn signed_area(profile: &[[f64; 2]]) -> f64 {
    profile
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let next = profile[(index + 1) % profile.len()];
            point[0] * next[1] - next[0] * point[1]
        })
        .sum::<f64>()
        * 0.5
}

fn cross(left: [f64; 2], center: [f64; 2], right: [f64; 2]) -> f64 {
    (center[0] - left[0]) * (right[1] - center[1]) - (center[1] - left[1]) * (right[0] - center[0])
}

fn orientation(left: [f64; 2], center: [f64; 2], right: [f64; 2]) -> f64 {
    (center[0] - left[0]) * (right[1] - left[1]) - (center[1] - left[1]) * (right[0] - left[0])
}

fn on_segment(left: [f64; 2], point: [f64; 2], right: [f64; 2]) -> bool {
    orientation(left, point, right).abs() <= MIN_PROFILE_AREA
        && point[0] >= left[0].min(right[0])
        && point[0] <= left[0].max(right[0])
        && point[1] >= left[1].min(right[1])
        && point[1] <= left[1].max(right[1])
}

fn segments_intersect(
    first_left: [f64; 2],
    first_right: [f64; 2],
    second_left: [f64; 2],
    second_right: [f64; 2],
) -> bool {
    let first_left_orientation = orientation(first_left, first_right, second_left);
    let first_right_orientation = orientation(first_left, first_right, second_right);
    let second_left_orientation = orientation(second_left, second_right, first_left);
    let second_right_orientation = orientation(second_left, second_right, first_right);
    if ((first_left_orientation > MIN_PROFILE_AREA && first_right_orientation < -MIN_PROFILE_AREA)
        || (first_left_orientation < -MIN_PROFILE_AREA
            && first_right_orientation > MIN_PROFILE_AREA))
        && ((second_left_orientation > MIN_PROFILE_AREA
            && second_right_orientation < -MIN_PROFILE_AREA)
            || (second_left_orientation < -MIN_PROFILE_AREA
                && second_right_orientation > MIN_PROFILE_AREA))
    {
        return true;
    }
    (first_left_orientation.abs() <= MIN_PROFILE_AREA
        && on_segment(first_left, second_left, first_right))
        || (first_right_orientation.abs() <= MIN_PROFILE_AREA
            && on_segment(first_left, second_right, first_right))
        || (second_left_orientation.abs() <= MIN_PROFILE_AREA
            && on_segment(second_left, first_left, second_right))
        || (second_right_orientation.abs() <= MIN_PROFILE_AREA
            && on_segment(second_left, first_right, second_right))
}

fn point_in_or_on_triangle(
    point: [f64; 2],
    first: [f64; 2],
    second: [f64; 2],
    third: [f64; 2],
) -> bool {
    orientation(first, second, point) >= -MIN_PROFILE_AREA
        && orientation(second, third, point) >= -MIN_PROFILE_AREA
        && orientation(third, first, point) >= -MIN_PROFILE_AREA
}

fn ear_clip(profile: &[[f64; 2]]) -> Result<Vec<[usize; 3]>, RuntimeError> {
    let mut remaining = (0..profile.len()).collect::<Vec<_>>();
    let mut triangles = Vec::with_capacity(profile.len() - 2);
    while remaining.len() > 3 {
        let mut clipped = false;
        for index in 0..remaining.len() {
            let previous = remaining[(index + remaining.len() - 1) % remaining.len()];
            let current = remaining[index];
            let next = remaining[(index + 1) % remaining.len()];
            if cross(profile[previous], profile[current], profile[next]) <= MIN_PROFILE_AREA {
                continue;
            }
            if remaining.iter().any(|candidate| {
                *candidate != previous
                    && *candidate != current
                    && *candidate != next
                    && point_in_or_on_triangle(
                        profile[*candidate],
                        profile[previous],
                        profile[current],
                        profile[next],
                    )
            }) {
                continue;
            }
            triangles.push([previous, current, next]);
            remaining.remove(index);
            clipped = true;
            break;
        }
        if !clipped {
            return Err(invalid("profile cannot be deterministically triangulated"));
        }
    }
    triangles.push([remaining[0], remaining[1], remaining[2]]);
    Ok(triangles)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authoring_mesh_v2::{AuthoringMeshV2GenesisInput, AuthoringMeshV2Revision};
    use forgecad_contracts::{AuthoringMeshId, AuthoringMeshLineageId};

    fn profile_node(profile: Value) -> Value {
        json!({
            "node_id":"dragonfang-blade-body",
            "operator_id":"forgecad.geometry.profile-extrude@1",
            "inputs":[],
            "parameters":{
                "shape":"profile-extrude",
                "profile":profile,
                "depth_m":0.12,
                "position_m":[0.0,0.0,0.0],
                "rotation_rad":[0.0,0.0,0.0]
            }
        })
    }

    #[test]
    fn profile_extrude_generates_deterministic_concave_kukri_blockout() {
        let node = profile_node(json!([
            [-2.0, -0.3],
            [0.8, -0.3],
            [1.4, -0.1],
            [1.0, 0.0],
            [1.8, 0.4],
            [0.4, 0.8],
            [-1.8, 0.4]
        ]));
        let genesis = profile_extrude_source_genesis(&node, "dragonfang-blade-body")
            .expect("concave kukri profile should triangulate");
        assert_eq!(genesis.positions_m.len(), 14);
        assert_eq!(genesis.faces.len(), 17);
        assert_eq!(
            genesis.faces.iter().filter(|face| face.len() == 4).count(),
            7
        );
        assert_eq!(
            genesis.faces.iter().filter(|face| face.len() == 3).count(),
            10
        );
        assert!(genesis
            .faces
            .iter()
            .all(|face| face.iter().all(|index| *index < genesis.positions_m.len())));
        AuthoringMeshV2Revision::genesis(AuthoringMeshV2GenesisInput {
            mesh_id: AuthoringMeshId("dragonfang-mesh".to_owned()),
            lineage_id: AuthoringMeshLineageId("dragonfang-lineage".to_owned()),
            positions_m: genesis.positions_m.clone(),
            faces: genesis.faces.clone(),
            evaluated: None,
            source_binding: None,
            foundation_source_binding: None,
        })
        .expect("profile extrusion must form a valid closed mesh");
        assert_eq!(
            genesis.source_parameters_sha256,
            canonical_json_hash(node.get("parameters").expect("parameters"))
        );
    }

    #[test]
    fn profile_extrude_accepts_the_dragonfang_live_blade_profile() {
        let node = profile_node(json!([
            [-1.18, -0.10],
            [-0.92, -0.17],
            [-0.52, -0.24],
            [-0.08, -0.30],
            [0.42, -0.31],
            [0.92, -0.24],
            [1.38, -0.12],
            [1.80, 0.05],
            [2.02, 0.18],
            [1.72, 0.25],
            [1.28, 0.34],
            [0.78, 0.42],
            [0.24, 0.40],
            [-0.30, 0.31],
            [-0.78, 0.17],
            [-1.10, 0.05]
        ]));
        let genesis = profile_extrude_source_genesis(&node, "dragonfang-blade-body")
            .expect("the live Dragonfang blade profile must remain editable");
        AuthoringMeshV2Revision::genesis(AuthoringMeshV2GenesisInput {
            mesh_id: AuthoringMeshId("dragonfang-live-mesh".to_owned()),
            lineage_id: AuthoringMeshLineageId("dragonfang-live-lineage".to_owned()),
            positions_m: genesis.positions_m,
            faces: genesis.faces,
            evaluated: None,
            source_binding: None,
            foundation_source_binding: None,
        })
        .expect("the live Dragonfang blade profile must form a closed editable mesh");
    }

    #[test]
    fn profile_extrude_normalizes_reversed_winding_without_changing_topology() {
        let forward = json!([
            [-2.0, -0.3],
            [0.8, -0.3],
            [1.4, -0.1],
            [1.0, 0.0],
            [1.8, 0.4],
            [0.4, 0.8],
            [-1.8, 0.4]
        ]);
        let reversed = Value::Array(
            forward
                .as_array()
                .expect("forward profile")
                .iter()
                .rev()
                .cloned()
                .collect(),
        );
        let first = profile_extrude_source_genesis(&profile_node(forward), "dragonfang-blade-body")
            .expect("forward profile");
        let second =
            profile_extrude_source_genesis(&profile_node(reversed), "dragonfang-blade-body")
                .expect("reversed profile");
        assert_eq!(first.positions_m, second.positions_m);
        assert_eq!(first.faces, second.faces);
        assert_ne!(
            first.source_parameters_sha256,
            second.source_parameters_sha256
        );
    }

    #[test]
    fn profile_extrude_rejects_self_intersection_and_degenerate_profiles() {
        let self_intersecting = profile_node(json!([
            [-1.0, -1.0],
            [1.0, 1.0],
            [-1.0, 1.0],
            [1.0, -1.0],
            [0.0, 0.5]
        ]));
        assert!(
            profile_extrude_source_genesis(&self_intersecting, "dragonfang-blade-body").is_err()
        );

        let degenerate = profile_node(json!([[-1.0, 0.0], [0.0, 0.0], [1.0, 0.0]]));
        assert!(profile_extrude_source_genesis(&degenerate, "dragonfang-blade-body").is_err());

        let duplicate = profile_node(json!([
            [-1.0, -1.0],
            [1.0, -1.0],
            [1.0, 1.0],
            [1.0, 1.0],
            [-1.0, 1.0]
        ]));
        assert!(profile_extrude_source_genesis(&duplicate, "dragonfang-blade-body").is_err());
    }

    #[test]
    fn primitive_box_source_regression_remains_closed_and_deterministic() {
        let node = json!({
            "node_id":"rear-stock",
            "operator_id":"forgecad.geometry.primitive@2",
            "inputs":[],
            "parameters":{
                "shape":"box",
                "size_m":[1.0,2.0,3.0],
                "position_m":[0.1,0.2,0.3],
                "rotation_rad":[0.0,0.1,0.2]
            }
        });
        let genesis = primitive_box_source_genesis(&node, "rear-stock").expect("box source");
        assert_eq!(genesis.positions_m.len(), 8);
        assert_eq!(genesis.faces.len(), 6);
        assert!(genesis.faces.iter().all(|face| face.len() == 4));
        assert_eq!(
            genesis.source_parameters_sha256,
            canonical_json_hash(node.get("parameters").expect("parameters"))
        );
    }
}
