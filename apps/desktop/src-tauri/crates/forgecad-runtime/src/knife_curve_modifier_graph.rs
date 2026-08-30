//! Runtime-owned bridge for the bounded knife Curve + ModifierGraph slice.
//!
//! This is intentionally structural. The Rust core validates and samples the
//! curves, derives the dependency graph and dirty recompute plan, and Runtime
//! persists those five canonical JSON objects. No mesh, GLB, stage transition,
//! candidate confirmation, version, export or visual-quality claim is made.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, now_string, Runtime,
    RuntimeError,
};
use forgecad_core::weaponry_dcc::{
    KnifeCurve, KnifeCurveBasis, KnifeCurveRole, MirrorAxis, ModifierGraph, ModifierKind,
    ModifierNode, Sha256Hash, StableId,
};
use forgecad_store::{
    CasObject, WeaponryCurveModifierGraphCasBundle, WeaponryCurveModifierGraphCommit,
    WeaponryCurveModifierGraphDurableRecord, WEAPONRY_CURVE_MODIFIER_GRAPH_JSON_MIME,
    WEAPONRY_CURVE_MODIFIER_GRAPH_MAX_JSON_BYTES, WEAPONRY_CURVE_MODIFIER_GRAPH_RECORD_SCHEMA,
    WEAPONRY_CURVE_MODIFIER_GRAPH_STATUS, WEAPONRY_CURVE_SET_OBJECT_KIND,
    WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND, WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND,
    WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND, WEAPONRY_SAMPLE_SET_OBJECT_KIND,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const PREPARE_SCHEMA: &str = "KnifeCurveModifierGraphPrepareRequest@1";
const GET_SCHEMA: &str = "KnifeCurveModifierGraphGetRequest@1";
const RESULT_SCHEMA: &str = "KnifeCurveModifierGraphResult@1";
const PREPARE_OPERATION: &str = "knife_curve_modifier_graph_prepare";
const GET_OPERATION: &str = "knife_curve_modifier_graph_get";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const REQUEST_CANONICALIZATION: &str = "canonical-json-sha256-excluding-input-sha256@1";
const RESULT_CANONICALIZATION: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const RECOMPUTE_POLICY: &str = "dirty-seed-dependency-closure-recompute@1";
const EVALUATION_POLICY: &str = "original-authoring-mesh-modifier-graph-deterministic@1";
const EVALUATION_STATUS: &str = "curve-sampled-modifier-recompute-planned-no-mesh@1";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const SAMPLE_COUNT: u32 = 64;
const SAMPLE_TOLERANCE_M: f64 = 0.00001;
const SAMPLE_MAX_SEGMENT_LENGTH_M: f64 = 0.01;

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "source_candidate_id",
    "source_candidate_state_sha256",
    "source_authoring_mesh_id",
    "source_authoring_mesh_lineage_id",
    "source_authoring_mesh_revision_id",
    "source_authoring_mesh_revision_index",
    "source_authoring_mesh_revision_sha256",
    "source_authoring_mesh_identity_sha256",
    "curves",
    "modifier_graph",
    "dirty_seeds",
    "recompute_policy",
    "evaluation_policy",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "source_candidate_id",
    "source_candidate_state_sha256",
    "source_authoring_mesh_id",
    "source_authoring_mesh_lineage_id",
    "source_authoring_mesh_revision_id",
    "source_authoring_mesh_revision_index",
    "source_authoring_mesh_revision_sha256",
    "source_authoring_mesh_identity_sha256",
    "curve_set_semantic_sha256",
    "sample_set_semantic_sha256",
    "modifier_graph_semantic_sha256",
    "dependency_graph_semantic_sha256",
    "recompute_plan_semantic_sha256",
    "lookup_key_sha256",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "KNIFE_CURVE_MODIFIER_GRAPH_INVALID: {}",
        message.into()
    ))
}

fn mismatch(code: &str, message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("{code}: {}", message.into()))
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{context} must be an object")))?;
    let expected = fields.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(invalid(format!(
            "{context} fields differ from the closed contract"
        )));
    }
    Ok(object)
}

fn text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{field} must be a string")))
}

fn id<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!("{field} is not an opaque identifier")));
    }
    Ok(value)
}

fn hash<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{field} is not a lowercase SHA-256")));
    }
    Ok(value)
}

fn u64_field(object: &Map<String, Value>, field: &str) -> Result<u64, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("{field} must be a non-negative integer")))
}

fn f64_field(object: &Map<String, Value>, field: &str) -> Result<f64, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid(format!("{field} must be finite")))
}

fn bool_field(object: &Map<String, Value>, field: &str) -> Result<bool, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid(format!("{field} must be boolean")))
}

fn exact_const(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), RuntimeError> {
    if text(object, field)? != expected {
        return Err(invalid(format!("{field} differs from the closed contract")));
    }
    Ok(())
}

fn max_response_bytes(object: &Map<String, Value>) -> Result<usize, RuntimeError> {
    let value = u64_field(object, "max_response_bytes")?;
    if value != MAX_RESPONSE_BYTES as u64 {
        return Err(invalid("max_response_bytes must be exactly 1048576"));
    }
    Ok(value as usize)
}

fn request_hash(request: &Value, object: &Map<String, Value>) -> Result<String, RuntimeError> {
    let supplied = hash(object, "input_sha256")?.to_owned();
    let mut canonical = request.clone();
    canonical["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&canonical) != supplied {
        return Err(invalid("input_sha256 differs from the canonical request"));
    }
    Ok(supplied)
}

fn parse_vec3(value: &Value, field: &str) -> Result<[f64; 3], RuntimeError> {
    let values = value
        .as_array()
        .filter(|values| values.len() == 3)
        .ok_or_else(|| invalid(format!("{field} must be a 3-vector")))?;
    Ok([
        values[0]
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| invalid(format!("{field}[0] must be finite")))?,
        values[1]
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| invalid(format!("{field}[1] must be finite")))?,
        values[2]
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| invalid(format!("{field}[2] must be finite")))?,
    ])
}

fn parse_hash(value: &Value, field: &str) -> Result<Sha256Hash, RuntimeError> {
    let value = value
        .as_str()
        .ok_or_else(|| invalid(format!("{field} must be a string")))?;
    Sha256Hash::new(value).map_err(|error| invalid(error.to_string()))
}

fn parse_id(value: &Value, field: &str) -> Result<StableId, RuntimeError> {
    let value = value
        .as_str()
        .ok_or_else(|| invalid(format!("{field} must be a string")))?;
    StableId::new(value).map_err(|error| invalid(error.to_string()))
}

fn parse_curve(value: &Value) -> Result<KnifeCurve, RuntimeError> {
    const FIELDS: &[&str] = &[
        "curve_id",
        "role",
        "basis",
        "degree",
        "control_points_m",
        "weights",
        "knots",
        "closed",
        "canonical_sha256",
    ];
    let object = exact_object(value, FIELDS, "knife curve")?;
    let role = match text(object, "role")? {
        "blade_spine" => KnifeCurveRole::BladeSpine,
        "blade_edge" => KnifeCurveRole::BladeEdge,
        "profile" => KnifeCurveRole::Profile,
        _ => return Err(invalid("curve role is outside the knife allowlist")),
    };
    let basis = match text(object, "basis")? {
        "bezier" => KnifeCurveBasis::Bezier,
        "nurbs_like" => KnifeCurveBasis::NurbsLike,
        _ => return Err(invalid("curve basis is outside the bounded allowlist")),
    };
    let degree = u8::try_from(u64_field(object, "degree")?)
        .map_err(|_| invalid("curve degree is too large"))?;
    let control_points = object
        .get("control_points_m")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("control_points_m must be an array"))?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_vec3(value, &format!("control_points_m[{index}]")))
        .collect::<Result<Vec<_>, _>>()?;
    let weights = object
        .get("weights")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("weights must be an array"))?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| invalid(format!("weights[{index}] must be finite")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let knots = object
        .get("knots")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("knots must be an array"))?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| invalid(format!("knots[{index}] must be finite")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let curve = KnifeCurve::new(
        id(object, "curve_id")?,
        role,
        basis,
        degree,
        control_points,
        weights,
        knots,
        bool_field(object, "closed")?,
    )
    .map_err(|error| invalid(error.to_string()))?;
    if curve
        .canonical_sha256()
        .map_err(|error| invalid(error.to_string()))?
        .as_str()
        != hash(object, "canonical_sha256")?
    {
        return Err(mismatch(
            "KNIFE_CURVE_CANONICAL_MISMATCH",
            "curve canonical_sha256 differs from the Rust typed value",
        ));
    }
    Ok(curve)
}

fn parse_modifier(value: &Value) -> Result<ModifierKind, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("modifier operator must be an object"))?;
    let operator = text(object, "operator")?;
    let exact = |fields: &[&str]| -> Result<(), RuntimeError> {
        let expected = fields.iter().copied().collect::<BTreeSet<_>>();
        let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(invalid(format!(
                "{operator} modifier fields differ from the closed contract"
            )));
        }
        Ok(())
    };
    Ok(match operator {
        "transform" => {
            exact(&["operator", "translation_m", "rotation_rad", "scale"])?;
            ModifierKind::Transform {
                translation_m: parse_vec3(&object["translation_m"], "translation_m")?,
                rotation_rad: parse_vec3(&object["rotation_rad"], "rotation_rad")?,
                scale: parse_vec3(&object["scale"], "scale")?,
            }
        }
        "mirror" => {
            exact(&["operator", "axis", "offset_m"])?;
            let axis = match text(object, "axis")? {
                "x" => MirrorAxis::X,
                "y" => MirrorAxis::Y,
                "z" => MirrorAxis::Z,
                _ => return Err(invalid("mirror axis must be x, y or z")),
            };
            ModifierKind::Mirror {
                axis,
                offset_m: f64_field(object, "offset_m")?,
            }
        }
        "array" => {
            exact(&["operator", "count", "offset_m"])?;
            ModifierKind::Array {
                count: u32::try_from(u64_field(object, "count")?)
                    .map_err(|_| invalid("array count is too large"))?,
                offset_m: parse_vec3(&object["offset_m"], "offset_m")?,
            }
        }
        "bevel" => {
            exact(&[
                "operator",
                "width_m",
                "segments",
                "profile",
                "clamp_overlap",
            ])?;
            ModifierKind::Bevel {
                width_m: f64_field(object, "width_m")?,
                segments: u8::try_from(u64_field(object, "segments")?)
                    .map_err(|_| invalid("bevel segments is too large"))?,
                profile: f64_field(object, "profile")?,
                clamp_overlap: bool_field(object, "clamp_overlap")?,
            }
        }
        "normal_policy" => {
            exact(&["operator", "crease_angle_rad"])?;
            ModifierKind::NormalPolicy {
                crease_angle_rad: f64_field(object, "crease_angle_rad")?,
            }
        }
        "curve_profile" => {
            exact(&["operator", "curve_id", "curve_sha256"])?;
            ModifierKind::CurveProfile {
                curve_id: parse_id(&object["curve_id"], "curve_id")?,
                curve_sha256: parse_hash(&object["curve_sha256"], "curve_sha256")?,
            }
        }
        _ => {
            return Err(invalid(
                "modifier operator is outside the fixed Rust allowlist",
            ))
        }
    })
}

fn parse_graph(
    value: &Value,
    source_revision_id: &str,
    source_revision_sha256: &str,
    curves: &BTreeMap<String, String>,
) -> Result<ModifierGraph, RuntimeError> {
    const GRAPH_FIELDS: &[&str] = &[
        "graph_id",
        "source_revision_id",
        "source_revision_sha256",
        "nodes",
        "output_node_ids",
        "canonical_sha256",
    ];
    const NODE_FIELDS: &[&str] = &[
        "node_id",
        "operator",
        "input_node_ids",
        "selection_query_sha256",
        "enabled",
    ];
    let object = exact_object(value, GRAPH_FIELDS, "modifier_graph")?;
    if id(object, "source_revision_id")? != source_revision_id
        || hash(object, "source_revision_sha256")? != source_revision_sha256
    {
        return Err(mismatch(
            "KNIFE_MODIFIER_GRAPH_SOURCE_MISMATCH",
            "graph source revision differs from the request",
        ));
    }
    let nodes = object
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("modifier_graph.nodes must be an array"))?
        .iter()
        .map(|node| {
            let node = exact_object(node, NODE_FIELDS, "modifier node")?;
            let operator = parse_modifier(&node["operator"])?;
            if let ModifierKind::CurveProfile {
                curve_id,
                curve_sha256,
            } = &operator
            {
                if curves.get(curve_id.as_str()).map(String::as_str) != Some(curve_sha256.as_str())
                {
                    return Err(mismatch(
                        "KNIFE_CURVE_GRAPH_BINDING_MISMATCH",
                        "curve_profile does not bind a supplied curve identity",
                    ));
                }
            }
            let inputs = node
                .get("input_node_ids")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("input_node_ids must be an array"))?
                .iter()
                .enumerate()
                .map(|(index, value)| parse_id(value, &format!("input_node_ids[{index}]")))
                .collect::<Result<Vec<_>, _>>()?;
            let selection = match node.get("selection_query_sha256") {
                Some(Value::Null) => None,
                Some(value) => Some(parse_hash(value, "selection_query_sha256")?),
                None => return Err(invalid("selection_query_sha256 is missing")),
            };
            ModifierNode::new(
                id(node, "node_id")?,
                operator,
                inputs,
                selection,
                bool_field(node, "enabled")?,
            )
            .map_err(|error| invalid(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let outputs = object
        .get("output_node_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("output_node_ids must be an array"))?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_id(value, &format!("output_node_ids[{index}]")))
        .collect::<Result<Vec<_>, _>>()?;
    let graph = ModifierGraph::new(
        id(object, "graph_id")?,
        source_revision_id,
        Sha256Hash::new(source_revision_sha256).map_err(|error| invalid(error.to_string()))?,
        nodes,
        outputs,
    )
    .map_err(|error| invalid(error.to_string()))?;
    if graph
        .canonical_sha256()
        .map_err(|error| invalid(error.to_string()))?
        .as_str()
        != hash(object, "canonical_sha256")?
    {
        return Err(mismatch(
            "KNIFE_MODIFIER_GRAPH_CANONICAL_MISMATCH",
            "modifier graph canonical_sha256 differs from the Rust typed value",
        ));
    }
    Ok(graph)
}

#[derive(Clone)]
struct StructuralObjects {
    curve_set: Value,
    curve_set_sha256: String,
    sample_set: Value,
    sample_set_sha256: String,
    modifier_graph: Value,
    modifier_graph_sha256: String,
    dependency_graph: Value,
    dependency_graph_sha256: String,
    recompute_plan: Value,
    recompute_plan_sha256: String,
    dirty_nodes: Vec<String>,
    recomputed_nodes: Vec<String>,
    reused_nodes: Vec<String>,
}

fn build_structural_objects(
    curves: &[KnifeCurve],
    graph: &ModifierGraph,
    dirty_seeds: &[String],
) -> Result<StructuralObjects, RuntimeError> {
    let curve_values = curves
        .iter()
        .map(|curve| {
            let mut value = serde_json::to_value(curve)
                .map_err(|error| invalid(format!("curve serialization failed: {error}")))?;
            value["canonical_sha256"] = Value::String(
                curve
                    .canonical_sha256()
                    .map_err(|error| invalid(error.to_string()))?
                    .as_str()
                    .to_owned(),
            );
            Ok(value)
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    let curve_set = json!({
        "schema_version":"KnifeCurveSet@1",
        "sampling_policy":"fixed-64-inclusive-parameter-samples@1",
        "curves":curve_values,
    });
    let curve_set_sha256 = canonical_json_hash(&curve_set);

    let samples = curves
        .iter()
        .map(|curve| {
            let plan = curve
                .tessellation_plan(
                    SAMPLE_COUNT,
                    SAMPLE_TOLERANCE_M,
                    SAMPLE_MAX_SEGMENT_LENGTH_M,
                )
                .map_err(|error| invalid(error.to_string()))?;
            let sample_set = curve.sample(&plan).map_err(|error| invalid(error.to_string()))?;
            Ok(json!({
                "curve_id":curve.curve_id.as_str(),
                "plan":serde_json::to_value(&plan).map_err(|error| invalid(error.to_string()))?,
                "plan_sha256":plan.canonical_sha256().map_err(|error| invalid(error.to_string()))?.as_str(),
                "samples":serde_json::to_value(&sample_set).map_err(|error| invalid(error.to_string()))?,
                "sample_set_sha256":sample_set.canonical_sha256().map_err(|error| invalid(error.to_string()))?.as_str(),
            }))
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    let sample_set = json!({
        "schema_version":"KnifeCurveSampleSetBundle@1",
        "sample_count_per_curve":SAMPLE_COUNT,
        "tolerance_m":SAMPLE_TOLERANCE_M,
        "max_segment_length_m":SAMPLE_MAX_SEGMENT_LENGTH_M,
        "samples":samples,
        "mesh_created":false,
    });
    let sample_set_sha256 = canonical_json_hash(&sample_set);

    let modifier_graph = serde_json::to_value(graph)
        .map_err(|error| invalid(format!("graph serialization failed: {error}")))?;
    let modifier_graph_sha256 = graph
        .canonical_sha256()
        .map_err(|error| invalid(error.to_string()))?
        .as_str()
        .to_owned();
    if canonical_json_hash(&modifier_graph) != modifier_graph_sha256 {
        return Err(invalid("modifier graph serialization identity drifted"));
    }
    let dependency = graph
        .dependency_graph()
        .map_err(|error| invalid(error.to_string()))?;
    let dependency_graph = serde_json::to_value(&dependency)
        .map_err(|error| invalid(format!("dependency serialization failed: {error}")))?;
    let dependency_graph_sha256 = dependency
        .canonical_sha256()
        .map_err(|error| invalid(error.to_string()))?
        .as_str()
        .to_owned();
    if canonical_json_hash(&dependency_graph) != dependency_graph_sha256 {
        return Err(invalid("dependency graph serialization identity drifted"));
    }
    let recompute = dependency
        .recompute_plan(dirty_seeds.iter().map(String::as_str))
        .map_err(|error| invalid(error.to_string()))?;
    let recompute_plan = json!({
        "schema_version":"KnifeModifierRecomputePlan@1",
        "dirty_nodes":recompute.dirty_nodes.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
        "recompute_order":recompute.recompute_order.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
        "evaluation_status":EVALUATION_STATUS,
    });
    let recompute_plan_sha256 = canonical_json_hash(&recompute_plan);
    let dirty_nodes = recompute
        .dirty_nodes
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect::<Vec<_>>();
    let recomputed_nodes = recompute
        .recompute_order
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect::<Vec<_>>();
    let recomputed = recomputed_nodes.iter().collect::<BTreeSet<_>>();
    let mut reused_nodes = dependency
        .topological_order()
        .iter()
        .map(|id| id.as_str().to_owned())
        .filter(|id| !recomputed.contains(id))
        .collect::<Vec<_>>();
    reused_nodes.sort();
    Ok(StructuralObjects {
        curve_set,
        curve_set_sha256,
        sample_set,
        sample_set_sha256,
        modifier_graph,
        modifier_graph_sha256,
        dependency_graph,
        dependency_graph_sha256,
        recompute_plan,
        recompute_plan_sha256,
        dirty_nodes,
        recomputed_nodes,
        reused_nodes,
    })
}

fn source_identity_hash(
    mesh_id: &str,
    lineage_id: &str,
    revision_id: &str,
    revision_index: u64,
    revision_sha256: &str,
) -> String {
    canonical_json_hash(&json!({
        "schema_version":"AuthoringMeshSourceIdentity@1",
        "mesh_id":mesh_id,
        "lineage_id":lineage_id,
        "revision_id":revision_id,
        "revision_index":revision_index,
        "revision_sha256":revision_sha256,
    }))
}

fn verify_source(runtime: &Runtime, object: &Map<String, Value>) -> Result<(), RuntimeError> {
    let project_id = id(object, "project_id")?;
    let candidate_id = id(object, "source_candidate_id")?;
    let candidate = runtime.store.get_candidate(candidate_id)?.ok_or_else(|| {
        mismatch(
            "KNIFE_CURVE_SOURCE_CANDIDATE_NOT_FOUND",
            "candidate is unavailable",
        )
    })?;
    if candidate.project_id != project_id
        || candidate.canonical_sha256 != hash(object, "source_candidate_state_sha256")?
    {
        return Err(mismatch(
            "KNIFE_CURVE_SOURCE_CANDIDATE_MISMATCH",
            "candidate project/state differs from durable truth",
        ));
    }
    let revision_id = id(object, "source_authoring_mesh_revision_id")?;
    let revision = runtime
        .store
        .get_authoring_mesh_v2_durable_record_by_revision(project_id, revision_id)?
        .ok_or_else(|| {
            mismatch(
                "KNIFE_CURVE_SOURCE_REVISION_NOT_FOUND",
                "revision is unavailable",
            )
        })?;
    let revision_index = u64_field(object, "source_authoring_mesh_revision_index")?;
    if revision.mesh_id != id(object, "source_authoring_mesh_id")?
        || revision.lineage_id != id(object, "source_authoring_mesh_lineage_id")?
        || revision.revision_index != revision_index
        || revision.revision_sha256 != hash(object, "source_authoring_mesh_revision_sha256")?
    {
        return Err(mismatch(
            "KNIFE_CURVE_SOURCE_REVISION_MISMATCH",
            "AuthoringMesh source identity differs from durable truth",
        ));
    }
    let identity = source_identity_hash(
        &revision.mesh_id,
        &revision.lineage_id,
        &revision.revision_id,
        revision.revision_index,
        &revision.revision_sha256,
    );
    if identity != hash(object, "source_authoring_mesh_identity_sha256")? {
        return Err(mismatch(
            "KNIFE_CURVE_SOURCE_IDENTITY_MISMATCH",
            "source AuthoringMesh identity hash differs",
        ));
    }
    Ok(())
}

fn object_bytes(value: &Value) -> Result<Vec<u8>, RuntimeError> {
    let bytes = canonical_json_bytes(value)
        .map_err(|error| invalid(format!("canonical JSON failed: {error}")))?;
    if bytes.is_empty() || bytes.len() as u64 > WEAPONRY_CURVE_MODIFIER_GRAPH_MAX_JSON_BYTES {
        return Err(invalid("structural object exceeds its bounded CAS budget"));
    }
    Ok(bytes)
}

fn stage_object(
    runtime: &Runtime,
    reservation: &forgecad_store::CasReservation,
    value: &Value,
    kind: &str,
) -> Result<CasObject, RuntimeError> {
    let mut object = runtime.store.put_object_reserved(
        reservation,
        &object_bytes(value)?,
        None,
        WEAPONRY_CURVE_MODIFIER_GRAPH_JSON_MIME,
        kind,
        &now_string(),
    )?;
    object.record = runtime
        .store
        .get_object(&object.record.sha256)?
        .ok_or_else(|| invalid("staged curve/modifier-graph CAS object is not registered"))?;
    Ok(object)
}

fn cleanup(runtime: &Runtime, reservation: &forgecad_store::CasReservation, objects: &[CasObject]) {
    for object in objects {
        let _ = runtime
            .store
            .release_cas_reservation_object(reservation, object, true);
    }
}

fn record_hash(record: &WeaponryCurveModifierGraphDurableRecord) -> Result<String, RuntimeError> {
    let mut value = serde_json::to_value(record)
        .map_err(|error| invalid(format!("record serialization failed: {error}")))?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

fn lookup_key(
    object: &Map<String, Value>,
    structural: &StructuralObjects,
) -> Result<String, RuntimeError> {
    Ok(canonical_json_hash(&json!({
        "schema_version":"KnifeCurveModifierGraphLookupKey@1",
        "project_id":id(object,"project_id")?,
        "source_candidate_id":id(object,"source_candidate_id")?,
        "source_candidate_state_sha256":hash(object,"source_candidate_state_sha256")?,
        "source_authoring_mesh_id":id(object,"source_authoring_mesh_id")?,
        "source_authoring_mesh_lineage_id":id(object,"source_authoring_mesh_lineage_id")?,
        "source_authoring_mesh_revision_id":id(object,"source_authoring_mesh_revision_id")?,
        "source_authoring_mesh_revision_index":u64_field(object,"source_authoring_mesh_revision_index")?,
        "source_authoring_mesh_revision_sha256":hash(object,"source_authoring_mesh_revision_sha256")?,
        "source_authoring_mesh_identity_sha256":hash(object,"source_authoring_mesh_identity_sha256")?,
        "curve_set_semantic_sha256":structural.curve_set_sha256,
        "sample_set_semantic_sha256":structural.sample_set_sha256,
        "modifier_graph_semantic_sha256":structural.modifier_graph_sha256,
        "dependency_graph_semantic_sha256":structural.dependency_graph_sha256,
        "recompute_plan_semantic_sha256":structural.recompute_plan_sha256,
    })))
}

fn result(
    record: &WeaponryCurveModifierGraphDurableRecord,
    structural: &StructuralObjects,
    operation: &str,
    status: &str,
    replayed: bool,
    wrote: bool,
    max_bytes: usize,
) -> Result<Value, RuntimeError> {
    let mut value = json!({
        "schema_version":RESULT_SCHEMA,
        "operation":operation,
        "request_kind":if operation == PREPARE_OPERATION { "prepare" } else { "get" },
        "status":status,
        "project_id":record.project_id,
        "source_candidate_id":record.source_candidate_id,
        "source_candidate_state_sha256":record.source_candidate_state_sha256,
        "source_authoring_mesh_id":record.source_authoring_mesh_id,
        "source_authoring_mesh_lineage_id":record.source_authoring_mesh_lineage_id,
        "source_authoring_mesh_revision_id":record.source_revision_id,
        "source_authoring_mesh_revision_index":record.source_authoring_mesh_revision_index,
        "source_authoring_mesh_revision_sha256":record.source_revision_sha256,
        "source_authoring_mesh_identity_sha256":record.source_authoring_mesh_identity_sha256,
        "curve_set_object_sha256":record.curve_set_object_sha256,
        "curve_set_semantic_sha256":record.curve_set_sha256,
        "sample_set_object_sha256":record.sample_set_object_sha256,
        "sample_set_semantic_sha256":record.sample_set_sha256,
        "modifier_graph_object_sha256":record.modifier_graph_object_sha256,
        "modifier_graph_semantic_sha256":record.modifier_graph_sha256,
        "dependency_graph_object_sha256":record.dependency_graph_object_sha256,
        "dependency_graph_semantic_sha256":record.dependency_graph_sha256,
        "recompute_plan_object_sha256":record.recompute_plan_object_sha256,
        "recompute_plan_semantic_sha256":record.recompute_plan_sha256,
        "dirty_seed_node_ids":structural.dirty_nodes,
        "recomputed_node_ids":structural.recomputed_nodes,
        "reused_node_ids":structural.reused_nodes,
        "evaluation_status":EVALUATION_STATUS,
        "evaluated_mesh_created":false,
        "geometry_artifact_created":false,
        "replayed":replayed,
        "deterministic_replay":true,
        "byte_exact_replay":true,
        "restart_hash_verified":false,
        "idempotency_key":record.idempotency_key,
        "atomicity_status":"committed",
        "store_commit_status":if wrote { "committed" } else { "not-touched" },
        "cas_commit_status":if wrote { "committed" } else { "not-touched" },
        "runtime_write_performed":wrote,
        "persistent_user_data_touched":wrote,
        "partial_result_exposed":false,
        "stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "quality_status":"structural_only",
        "visual_status":"NOT_RUN",
        "human_status":"NOT_RUN",
        "engine_status":"NOT_RUN",
        "canonicalization_policy":RESULT_CANONICALIZATION,
        "canonical_sha256":"",
    });
    value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
    let bytes = canonical_json_bytes(&value)
        .map_err(|error| invalid(format!("result canonicalization failed: {error}")))?;
    if bytes.len() > max_bytes {
        return Err(invalid("result exceeds max_response_bytes"));
    }
    Ok(value)
}

fn structural_from_store(
    runtime: &Runtime,
    record: &WeaponryCurveModifierGraphDurableRecord,
) -> Result<StructuralObjects, RuntimeError> {
    let curve_set = runtime
        .store
        .authoring_repository()
        .read_knife_curve_modifier_graph_json(
            &record.curve_set_object_sha256,
            WEAPONRY_CURVE_SET_OBJECT_KIND,
        )?;
    let sample_set = runtime
        .store
        .authoring_repository()
        .read_knife_curve_modifier_graph_json(
            &record.sample_set_object_sha256,
            WEAPONRY_SAMPLE_SET_OBJECT_KIND,
        )?;
    let modifier_graph = runtime
        .store
        .authoring_repository()
        .read_knife_curve_modifier_graph_json(
            &record.modifier_graph_object_sha256,
            WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND,
        )?;
    let dependency_graph = runtime
        .store
        .authoring_repository()
        .read_knife_curve_modifier_graph_json(
            &record.dependency_graph_object_sha256,
            WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND,
        )?;
    let recompute_plan = runtime
        .store
        .authoring_repository()
        .read_knife_curve_modifier_graph_json(
            &record.recompute_plan_object_sha256,
            WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND,
        )?;
    let checks = [
        (
            canonical_json_hash(&curve_set),
            record.curve_set_sha256.as_str(),
        ),
        (
            canonical_json_hash(&sample_set),
            record.sample_set_sha256.as_str(),
        ),
        (
            canonical_json_hash(&modifier_graph),
            record.modifier_graph_sha256.as_str(),
        ),
        (
            canonical_json_hash(&dependency_graph),
            record.dependency_graph_sha256.as_str(),
        ),
        (
            canonical_json_hash(&recompute_plan),
            record.recompute_plan_sha256.as_str(),
        ),
    ];
    if checks.iter().any(|(actual, expected)| actual != expected) {
        return Err(mismatch(
            "KNIFE_CURVE_MODIFIER_GRAPH_SEMANTIC_HASH_MISMATCH",
            "a durable CAS object no longer matches its semantic hash",
        ));
    }
    let dirty_nodes = recompute_plan
        .get("dirty_nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("stored recompute dirty_nodes are invalid"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid("stored dirty node is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let recomputed_nodes = recompute_plan
        .get("recompute_order")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("stored recompute_order is invalid"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid("stored recompute node is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let dependency_order = dependency_graph
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("stored dependency nodes are invalid"))?
        .iter()
        .filter_map(|value| value.get("node_id").and_then(Value::as_str))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let recomputed = recomputed_nodes.iter().collect::<BTreeSet<_>>();
    let mut reused_nodes = dependency_order
        .into_iter()
        .filter(|node| !recomputed.contains(node))
        .collect::<Vec<_>>();
    reused_nodes.sort();
    Ok(StructuralObjects {
        curve_set,
        curve_set_sha256: record.curve_set_sha256.clone(),
        sample_set,
        sample_set_sha256: record.sample_set_sha256.clone(),
        modifier_graph,
        modifier_graph_sha256: record.modifier_graph_sha256.clone(),
        dependency_graph,
        dependency_graph_sha256: record.dependency_graph_sha256.clone(),
        recompute_plan,
        recompute_plan_sha256: record.recompute_plan_sha256.clone(),
        dirty_nodes,
        recomputed_nodes,
        reused_nodes,
    })
}

pub(crate) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, PREPARE_FIELDS, "prepare request")?;
    exact_const(object, "schema_version", PREPARE_SCHEMA)?;
    exact_const(object, "operation", PREPARE_OPERATION)?;
    exact_const(object, "writer_policy", WRITER_POLICY)?;
    exact_const(object, "canonicalization_policy", REQUEST_CANONICALIZATION)?;
    exact_const(object, "recompute_policy", RECOMPUTE_POLICY)?;
    exact_const(object, "evaluation_policy", EVALUATION_POLICY)?;
    if bool_field(object, "runtime_write_performed")? {
        return Err(invalid(
            "runtime_write_performed must be false in a request",
        ));
    }
    let max_bytes = max_response_bytes(object)?;
    let input_sha256 = request_hash(request, object)?;
    verify_source(runtime, object)?;
    let curve_values = object
        .get("curves")
        .and_then(Value::as_array)
        .filter(|curves| !curves.is_empty() && curves.len() <= 16)
        .ok_or_else(|| invalid("curves must contain 1..=16 values"))?;
    let mut curves = curve_values
        .iter()
        .map(parse_curve)
        .collect::<Result<Vec<_>, _>>()?;
    curves.sort_by(|left, right| left.curve_id.cmp(&right.curve_id));
    if curves
        .windows(2)
        .any(|pair| pair[0].curve_id == pair[1].curve_id)
    {
        return Err(invalid("curve_id values must be unique"));
    }
    let curve_hashes = curves
        .iter()
        .map(|curve| {
            Ok((
                curve.curve_id.as_str().to_owned(),
                curve
                    .canonical_sha256()
                    .map_err(|error| invalid(error.to_string()))?
                    .as_str()
                    .to_owned(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, RuntimeError>>()?;
    let graph = parse_graph(
        object
            .get("modifier_graph")
            .ok_or_else(|| invalid("modifier_graph is missing"))?,
        id(object, "source_authoring_mesh_revision_id")?,
        hash(object, "source_authoring_mesh_revision_sha256")?,
        &curve_hashes,
    )?;
    let dirty_seeds = object
        .get("dirty_seeds")
        .and_then(Value::as_array)
        .filter(|seeds| !seeds.is_empty() && seeds.len() <= 64)
        .ok_or_else(|| invalid("dirty_seeds must contain 1..=64 node IDs"))?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            parse_id(value, &format!("dirty_seeds[{index}]")).map(|id| id.as_str().to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if dirty_seeds.iter().collect::<BTreeSet<_>>().len() != dirty_seeds.len() {
        return Err(invalid("dirty_seeds must be unique"));
    }
    let structural = build_structural_objects(&curves, &graph, &dirty_seeds)?;
    let replay = build_structural_objects(&curves, &graph, &dirty_seeds)?;
    let first_bytes = [
        object_bytes(&structural.curve_set)?,
        object_bytes(&structural.sample_set)?,
        object_bytes(&structural.modifier_graph)?,
        object_bytes(&structural.dependency_graph)?,
        object_bytes(&structural.recompute_plan)?,
    ];
    let replay_bytes = [
        object_bytes(&replay.curve_set)?,
        object_bytes(&replay.sample_set)?,
        object_bytes(&replay.modifier_graph)?,
        object_bytes(&replay.dependency_graph)?,
        object_bytes(&replay.recompute_plan)?,
    ];
    if first_bytes != replay_bytes {
        return Err(mismatch(
            "KNIFE_CURVE_MODIFIER_GRAPH_REPLAY_MISMATCH",
            "pure Rust structural evaluation was not byte exact",
        ));
    }
    let lookup_key_sha256 = lookup_key(object, &structural)?;
    let reservation = runtime.store.begin_cas_reservation();
    let mut staged = Vec::new();
    let outcome = (|| -> Result<Value, RuntimeError> {
        let curve_object = stage_object(
            runtime,
            &reservation,
            &structural.curve_set,
            WEAPONRY_CURVE_SET_OBJECT_KIND,
        )?;
        staged.push(curve_object.clone());
        let sample_object = stage_object(
            runtime,
            &reservation,
            &structural.sample_set,
            WEAPONRY_SAMPLE_SET_OBJECT_KIND,
        )?;
        staged.push(sample_object.clone());
        let graph_object = stage_object(
            runtime,
            &reservation,
            &structural.modifier_graph,
            WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND,
        )?;
        staged.push(graph_object.clone());
        let dependency_object = stage_object(
            runtime,
            &reservation,
            &structural.dependency_graph,
            WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND,
        )?;
        staged.push(dependency_object.clone());
        let recompute_object = stage_object(
            runtime,
            &reservation,
            &structural.recompute_plan,
            WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND,
        )?;
        staged.push(recompute_object.clone());
        let mut record = WeaponryCurveModifierGraphDurableRecord {
            schema_version: WEAPONRY_CURVE_MODIFIER_GRAPH_RECORD_SCHEMA.to_owned(),
            project_id: id(object, "project_id")?.to_owned(),
            source_revision_id: id(object, "source_authoring_mesh_revision_id")?.to_owned(),
            source_revision_sha256: hash(object, "source_authoring_mesh_revision_sha256")?
                .to_owned(),
            source_candidate_id: id(object, "source_candidate_id")?.to_owned(),
            source_candidate_state_sha256: hash(object, "source_candidate_state_sha256")?
                .to_owned(),
            source_authoring_mesh_id: id(object, "source_authoring_mesh_id")?.to_owned(),
            source_authoring_mesh_lineage_id: id(object, "source_authoring_mesh_lineage_id")?
                .to_owned(),
            source_authoring_mesh_revision_index: u64_field(
                object,
                "source_authoring_mesh_revision_index",
            )?,
            source_authoring_mesh_identity_sha256: hash(
                object,
                "source_authoring_mesh_identity_sha256",
            )?
            .to_owned(),
            curve_set_id: format!("knife-curves-{}", &structural.curve_set_sha256[..16]),
            curve_set_sha256: structural.curve_set_sha256.clone(),
            curve_set_object_sha256: curve_object.record.sha256.clone(),
            sample_set_id: format!("knife-samples-{}", &structural.sample_set_sha256[..16]),
            sample_set_sha256: structural.sample_set_sha256.clone(),
            sample_set_object_sha256: sample_object.record.sha256.clone(),
            modifier_graph_id: graph.graph_id.as_str().to_owned(),
            modifier_graph_sha256: structural.modifier_graph_sha256.clone(),
            modifier_graph_object_sha256: graph_object.record.sha256.clone(),
            dependency_graph_sha256: structural.dependency_graph_sha256.clone(),
            dependency_graph_object_sha256: dependency_object.record.sha256.clone(),
            recompute_plan_sha256: structural.recompute_plan_sha256.clone(),
            recompute_plan_object_sha256: recompute_object.record.sha256.clone(),
            lookup_key_sha256,
            idempotency_key: id(object, "idempotency_key")?.to_owned(),
            input_sha256,
            materialization_status: WEAPONRY_CURVE_MODIFIER_GRAPH_STATUS.to_owned(),
            canonical_sha256: String::new(),
            created_at: now_string(),
        };
        record.canonical_sha256 = record_hash(&record)?;
        let commit = WeaponryCurveModifierGraphCommit {
            record,
            cas: WeaponryCurveModifierGraphCasBundle {
                curve_set: curve_object.record.clone(),
                sample_set: sample_object.record.clone(),
                modifier_graph: graph_object.record.clone(),
                dependency_graph: dependency_object.record.clone(),
                recompute_plan: recompute_object.record.clone(),
            },
        };
        let (stored, replayed) = runtime
            .store
            .authoring_repository()
            .record_knife_curve_modifier_graph_with_replay(&commit)?;
        result(
            &stored,
            &structural,
            PREPARE_OPERATION,
            if replayed { "replayed" } else { "prepared" },
            replayed,
            !replayed,
            max_bytes,
        )
    })();
    match outcome {
        Ok(value) => {
            for object in &staged {
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, object, false)?;
            }
            Ok(value)
        }
        Err(error) => {
            cleanup(runtime, &reservation, &staged);
            Err(error)
        }
    }
}

pub(crate) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, GET_FIELDS, "get request")?;
    exact_const(object, "schema_version", GET_SCHEMA)?;
    exact_const(object, "operation", GET_OPERATION)?;
    exact_const(object, "writer_policy", WRITER_POLICY)?;
    exact_const(object, "canonicalization_policy", REQUEST_CANONICALIZATION)?;
    if bool_field(object, "runtime_write_performed")? {
        return Err(invalid("runtime_write_performed must be false"));
    }
    let max_bytes = max_response_bytes(object)?;
    request_hash(request, object)?;
    verify_source(runtime, object)?;
    let record = runtime
        .store
        .authoring_repository()
        .get_knife_curve_modifier_graph(
            id(object, "project_id")?,
            hash(object, "lookup_key_sha256")?,
        )?
        .ok_or_else(|| {
            mismatch(
                "KNIFE_CURVE_MODIFIER_GRAPH_NOT_FOUND",
                "durable record is unavailable",
            )
        })?;
    let bindings = [
        (
            record.source_candidate_id.as_str(),
            id(object, "source_candidate_id")?,
        ),
        (
            record.source_candidate_state_sha256.as_str(),
            hash(object, "source_candidate_state_sha256")?,
        ),
        (
            record.source_authoring_mesh_id.as_str(),
            id(object, "source_authoring_mesh_id")?,
        ),
        (
            record.source_authoring_mesh_lineage_id.as_str(),
            id(object, "source_authoring_mesh_lineage_id")?,
        ),
        (
            record.source_revision_id.as_str(),
            id(object, "source_authoring_mesh_revision_id")?,
        ),
        (
            record.source_revision_sha256.as_str(),
            hash(object, "source_authoring_mesh_revision_sha256")?,
        ),
        (
            record.source_authoring_mesh_identity_sha256.as_str(),
            hash(object, "source_authoring_mesh_identity_sha256")?,
        ),
        (
            record.curve_set_sha256.as_str(),
            hash(object, "curve_set_semantic_sha256")?,
        ),
        (
            record.sample_set_sha256.as_str(),
            hash(object, "sample_set_semantic_sha256")?,
        ),
        (
            record.modifier_graph_sha256.as_str(),
            hash(object, "modifier_graph_semantic_sha256")?,
        ),
        (
            record.dependency_graph_sha256.as_str(),
            hash(object, "dependency_graph_semantic_sha256")?,
        ),
        (
            record.recompute_plan_sha256.as_str(),
            hash(object, "recompute_plan_semantic_sha256")?,
        ),
        (
            record.idempotency_key.as_str(),
            id(object, "idempotency_key")?,
        ),
    ];
    if bindings.iter().any(|(left, right)| left != right)
        || record.source_authoring_mesh_revision_index
            != u64_field(object, "source_authoring_mesh_revision_index")?
    {
        return Err(mismatch(
            "KNIFE_CURVE_MODIFIER_GRAPH_LOOKUP_MISMATCH",
            "get request does not exactly bind the durable record",
        ));
    }
    let structural = structural_from_store(runtime, &record)?;
    result(
        &record,
        &structural,
        GET_OPERATION,
        "found",
        false,
        false,
        max_bytes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_identity_is_stable_and_parser_rejects_unknown_curve_fields() {
        assert_eq!(
            source_identity_hash("mesh", "lineage", "revision", 2, &"a".repeat(64)),
            source_identity_hash("mesh", "lineage", "revision", 2, &"a".repeat(64))
        );
        let mut curve = json!({
            "curve_id":"spine",
            "role":"blade_spine",
            "basis":"bezier",
            "degree":1,
            "control_points_m":[[0.0,0.0,0.0],[1.0,0.0,0.0]],
            "weights":[],
            "knots":[],
            "closed":false,
            "canonical_sha256":"0".repeat(64),
        });
        assert!(parse_curve(&curve).is_err());
        curve["script"] = json!("forbidden");
        assert!(parse_curve(&curve).is_err());
    }

    #[test]
    fn curve_sampling_and_modifier_recompute_are_byte_exact_without_mesh_output() {
        let curve = KnifeCurve::new(
            "blade-spine",
            KnifeCurveRole::BladeSpine,
            KnifeCurveBasis::Bezier,
            2,
            vec![[0.0, 0.0, 0.0], [0.4, 0.08, 0.0], [0.8, 0.0, 0.0]],
            vec![],
            vec![],
            false,
        )
        .expect("curve");
        let curve_hash = curve.canonical_sha256().expect("curve hash");
        let node = ModifierNode::new(
            "profile-node",
            ModifierKind::CurveProfile {
                curve_id: curve.curve_id.clone(),
                curve_sha256: curve_hash,
            },
            vec![],
            None,
            true,
        )
        .expect("node");
        let graph = ModifierGraph::new(
            "knife-graph",
            "revision-1",
            Sha256Hash::new("a".repeat(64)).expect("source hash"),
            vec![node],
            vec![StableId::new("profile-node").expect("output")],
        )
        .expect("graph");
        let curve_dependency = format!("__curve-{}", curve.canonical_sha256().unwrap());
        let first = build_structural_objects(&[curve.clone()], &graph, &[curve_dependency.clone()])
            .expect("first");
        let second =
            build_structural_objects(&[curve], &graph, &[curve_dependency]).expect("second");
        assert_eq!(
            object_bytes(&first.curve_set).unwrap(),
            object_bytes(&second.curve_set).unwrap()
        );
        assert_eq!(
            object_bytes(&first.sample_set).unwrap(),
            object_bytes(&second.sample_set).unwrap()
        );
        assert_eq!(first.modifier_graph_sha256, second.modifier_graph_sha256);
        assert_eq!(
            first.dependency_graph_sha256,
            second.dependency_graph_sha256
        );
        assert_eq!(first.recompute_plan_sha256, second.recompute_plan_sha256);
        assert_eq!(first.sample_set["mesh_created"], false);
        assert!(first
            .recomputed_nodes
            .iter()
            .any(|node| node == "profile-node"));
    }
}
