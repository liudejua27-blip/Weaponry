//! Runtime-owned bridge for the bounded knife curve EvaluatedMesh slice.
//!
//! The structural Curve/ModifierGraph record is the only source of curves. The
//! Runtime reads it back from Store/CAS, reconstructs a closed Core sweep plan,
//! evaluates disposable geometry twice, and commits four immutable JSON roots.
//! AuthoringMesh, GLB, High/Low/UV/Bake and candidate/version state are not
//! touched by this module.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, now_string, Runtime,
    RuntimeError,
};
use forgecad_core::weaponry_dcc::{
    EvaluatedMeshGeometry, EvaluatedMeshLink, KnifeBladeSweepPlan, KnifeCurve, KnifeCurveBasis,
    KnifeCurveRole, KnifeThicknessAxis, ModifierGraph, ModifierKind, ModifierNode, Sha256Hash,
    StableId,
};
use forgecad_store::{
    CasObject, KnifeCurveEvaluatedMeshCasBundle, KnifeCurveEvaluatedMeshCommit,
    KnifeCurveEvaluatedMeshDurableRecord, WeaponryCurveModifierGraphDurableRecord,
    WEAPONRY_CURVE_EVALUATED_MESH_JSON_MIME, WEAPONRY_CURVE_EVALUATED_MESH_MAX_JSON_BYTES,
    WEAPONRY_CURVE_EVALUATED_MESH_RECORD_SCHEMA, WEAPONRY_CURVE_EVALUATED_MESH_STATUS,
    WEAPONRY_CURVE_EVALUATION_PLAN_OBJECT_KIND, WEAPONRY_CURVE_SET_OBJECT_KIND,
    WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND, WEAPONRY_EVALUATED_MESH_IDENTITY_OBJECT_KIND,
    WEAPONRY_EVALUATED_MESH_LINK_OBJECT_KIND, WEAPONRY_EVALUATED_MESH_OBJECT_KIND,
    WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND, WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND,
    WEAPONRY_SAMPLE_SET_OBJECT_KIND,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const PREPARE_SCHEMA: &str = "KnifeCurveEvaluatedMeshPrepareRequest@1";
const GET_SCHEMA: &str = "KnifeCurveEvaluatedMeshGetRequest@1";
const RESULT_SCHEMA: &str = "KnifeCurveEvaluatedMeshResult@1";
const PREPARE_OPERATION: &str = "knife_curve_evaluated_mesh_prepare";
const GET_OPERATION: &str = "knife_curve_evaluated_mesh_get";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const REQUEST_CANONICALIZATION: &str = "canonical-json-sha256-excluding-input-sha256@1";
const RESULT_CANONICALIZATION: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const PLAN_SCHEMA: &str = "KnifeBladeProfileSweepLoftPlan@1";
const PLAN_TRIANGULATION: &str = "station-ring-fixed-diagonal@1";
const PLAN_LINEAGE: &str = "source-curve-modifier-graph-evaluated-mesh@1";
const MESH_READBACK_STATUS: &str = "strict-evaluated-mesh-readback@1";
const EVALUATION_STATUS: &str = "curve-sweep-loft-evaluated-mesh-created-no-geometry-artifact@1";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const STATION_COUNT: u32 = 32;
const PLAN_TOLERANCE_M: f64 = 1.0e-4;
const PLAN_MAX_SEGMENT_LENGTH_M: f64 = 1.0;
const SAMPLE_COUNT: u64 = 64;
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
    "source_modifier_graph_id",
    "source_modifier_graph_sha256",
    "curve_set_semantic_sha256",
    "sample_set_semantic_sha256",
    "modifier_graph_semantic_sha256",
    "dependency_graph_semantic_sha256",
    "recompute_plan_semantic_sha256",
    "evaluation_plan",
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
    "source_modifier_graph_id",
    "source_modifier_graph_sha256",
    "curve_set_semantic_sha256",
    "sample_set_semantic_sha256",
    "modifier_graph_semantic_sha256",
    "dependency_graph_semantic_sha256",
    "recompute_plan_semantic_sha256",
    "evaluated_mesh_lookup_key_sha256",
    "evaluation_id",
    "evaluation_plan_object_sha256",
    "evaluation_plan_semantic_sha256",
    "evaluated_mesh_id",
    "evaluated_mesh_object_sha256",
    "evaluated_mesh_semantic_sha256",
    "evaluated_mesh_identity_sha256",
    "evaluated_mesh_link_sha256",
    "vertex_count",
    "triangle_count",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

const PLAN_FIELDS: &[&str] = &[
    "schema_version",
    "evaluation_id",
    "spine_curve_id",
    "spine_curve_sha256",
    "edge_curve_id",
    "edge_curve_sha256",
    "station_count",
    "thickness_axis",
    "thickness_m",
    "root_cap",
    "tip_cap",
    "stable_triangulation",
    "stable_lineage_policy",
    "canonical_sha256",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "KNIFE_CURVE_EVALUATED_MESH_INVALID: {}",
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

fn canonical_hash_without(value: &Value, field: &str) -> Result<String, RuntimeError> {
    let mut value = value.clone();
    value
        .as_object_mut()
        .ok_or_else(|| invalid("canonical hash input must be an object"))?
        .remove(field);
    Ok(canonical_json_hash(&value))
}

fn request_hash(request: &Value, object: &Map<String, Value>) -> Result<String, RuntimeError> {
    let supplied = hash(object, "input_sha256")?.to_owned();
    let expected = canonical_hash_without(request, "input_sha256")?;
    if supplied != expected {
        return Err(invalid("input_sha256 differs from the canonical request"));
    }
    Ok(supplied)
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

fn parse_vec3(value: &Value, field: &str) -> Result<[f64; 3], RuntimeError> {
    let values = value
        .as_array()
        .filter(|values| values.len() == 3)
        .ok_or_else(|| invalid(format!("{field} must be a 3-vector")))?;
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        result[index] = value
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| invalid(format!("{field}[{index}] must be finite")))?;
    }
    Ok(result)
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
    let object = exact_object(value, FIELDS, "stored knife curve")?;
    let role = match text(object, "role")? {
        "blade_spine" => KnifeCurveRole::BladeSpine,
        "blade_edge" => KnifeCurveRole::BladeEdge,
        "profile" => KnifeCurveRole::Profile,
        _ => return Err(invalid("stored curve role is outside the knife allowlist")),
    };
    let basis = match text(object, "basis")? {
        "bezier" => KnifeCurveBasis::Bezier,
        "nurbs_like" => KnifeCurveBasis::NurbsLike,
        _ => {
            return Err(invalid(
                "stored curve basis is outside the bounded allowlist",
            ))
        }
    };
    let degree = u8::try_from(u64_field(object, "degree")?)
        .map_err(|_| invalid("stored curve degree is too large"))?;
    let points = object
        .get("control_points_m")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("stored control_points_m must be an array"))?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_vec3(value, &format!("control_points_m[{index}]")))
        .collect::<Result<Vec<_>, _>>()?;
    let weights = object
        .get("weights")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("stored weights must be an array"))?
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
        .ok_or_else(|| invalid("stored knots must be an array"))?
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
        points,
        weights,
        knots,
        bool_field(object, "closed")?,
    )
    .map_err(|error| invalid(error.to_string()))?;
    let expected = curve
        .canonical_sha256()
        .map_err(|error| invalid(error.to_string()))?;
    if expected.as_str() != hash(object, "canonical_sha256")? {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_CURVE_CANONICAL_MISMATCH",
            "stored curve canonical_sha256 differs from the typed curve",
        ));
    }
    Ok(curve)
}

fn parse_modifier(value: &Value) -> Result<ModifierKind, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("stored modifier operator must be an object"))?;
    let operator = object
        .get("operator")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("stored modifier operator is missing"))?;
    let require = |fields: &[&str]| -> Result<(), RuntimeError> {
        let expected = fields.iter().copied().collect::<BTreeSet<_>>();
        let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(invalid(format!("{operator} modifier fields differ")));
        }
        Ok(())
    };
    Ok(match operator {
        "transform" => {
            require(&["operator", "translation_m", "rotation_rad", "scale"])?;
            ModifierKind::Transform {
                translation_m: parse_vec3(&object["translation_m"], "translation_m")?,
                rotation_rad: parse_vec3(&object["rotation_rad"], "rotation_rad")?,
                scale: parse_vec3(&object["scale"], "scale")?,
            }
        }
        "mirror" => {
            require(&["operator", "axis", "offset_m"])?;
            let axis = match text(object, "axis")? {
                "x" => forgecad_core::weaponry_dcc::MirrorAxis::X,
                "y" => forgecad_core::weaponry_dcc::MirrorAxis::Y,
                "z" => forgecad_core::weaponry_dcc::MirrorAxis::Z,
                _ => return Err(invalid("stored mirror axis is invalid")),
            };
            ModifierKind::Mirror {
                axis,
                offset_m: object["offset_m"]
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| invalid("stored mirror offset_m must be finite"))?,
            }
        }
        "array" => {
            require(&["operator", "count", "offset_m"])?;
            ModifierKind::Array {
                count: u32::try_from(u64_field(object, "count")?)
                    .map_err(|_| invalid("stored array count is too large"))?,
                offset_m: parse_vec3(&object["offset_m"], "offset_m")?,
            }
        }
        "bevel" => {
            require(&[
                "operator",
                "width_m",
                "segments",
                "profile",
                "clamp_overlap",
            ])?;
            ModifierKind::Bevel {
                width_m: object["width_m"]
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| invalid("stored bevel width_m must be finite"))?,
                segments: u8::try_from(u64_field(object, "segments")?)
                    .map_err(|_| invalid("stored bevel segments is too large"))?,
                profile: object["profile"]
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| invalid("stored bevel profile must be finite"))?,
                clamp_overlap: bool_field(object, "clamp_overlap")?,
            }
        }
        "normal_policy" => {
            require(&["operator", "crease_angle_rad"])?;
            ModifierKind::NormalPolicy {
                crease_angle_rad: object["crease_angle_rad"]
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| invalid("stored crease_angle_rad must be finite"))?,
            }
        }
        "curve_profile" => {
            require(&["operator", "curve_id", "curve_sha256"])?;
            ModifierKind::CurveProfile {
                curve_id: parse_id(&object["curve_id"], "curve_id")?,
                curve_sha256: parse_hash(&object["curve_sha256"], "curve_sha256")?,
            }
        }
        _ => {
            return Err(invalid(
                "stored modifier operator is outside the Rust allowlist",
            ))
        }
    })
}

fn parse_modifier_graph(value: &Value) -> Result<ModifierGraph, RuntimeError> {
    const GRAPH_FIELDS: &[&str] = &[
        "graph_id",
        "source_revision_id",
        "source_revision_sha256",
        "nodes",
        "output_node_ids",
    ];
    const NODE_FIELDS: &[&str] = &[
        "node_id",
        "operator",
        "input_node_ids",
        "selection_query_sha256",
        "enabled",
    ];
    let object = exact_object(value, GRAPH_FIELDS, "stored modifier_graph")?;
    let nodes = object
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("stored modifier_graph.nodes must be an array"))?
        .iter()
        .map(|value| {
            let node = exact_object(value, NODE_FIELDS, "stored modifier node")?;
            let inputs = node
                .get("input_node_ids")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("stored input_node_ids must be an array"))?
                .iter()
                .enumerate()
                .map(|(index, value)| parse_id(value, &format!("input_node_ids[{index}]")))
                .collect::<Result<Vec<_>, _>>()?;
            let selection = match node.get("selection_query_sha256") {
                Some(Value::Null) => None,
                Some(value) => Some(parse_hash(value, "selection_query_sha256")?),
                None => return Err(invalid("stored selection_query_sha256 is missing")),
            };
            ModifierNode::new(
                id(node, "node_id")?,
                parse_modifier(&node["operator"])?,
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
        .ok_or_else(|| invalid("stored output_node_ids must be an array"))?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_id(value, &format!("output_node_ids[{index}]")))
        .collect::<Result<Vec<_>, _>>()?;
    ModifierGraph::new(
        id(object, "graph_id")?,
        id(object, "source_revision_id")?,
        parse_hash(&object["source_revision_sha256"], "source_revision_sha256")?,
        nodes,
        outputs,
    )
    .map_err(|error| invalid(error.to_string()))
}

fn parse_plan(value: &Value) -> Result<(Value, String), RuntimeError> {
    let object = exact_object(value, PLAN_FIELDS, "evaluation_plan")?;
    exact_const(object, "schema_version", PLAN_SCHEMA)?;
    if u64_field(object, "station_count")? != STATION_COUNT as u64 {
        return Err(invalid("evaluation_plan station_count must be exactly 32"));
    }
    if !bool_field(object, "root_cap")? || !bool_field(object, "tip_cap")? {
        return Err(invalid("evaluation_plan root_cap and tip_cap must be true"));
    }
    exact_const(object, "stable_triangulation", PLAN_TRIANGULATION)?;
    exact_const(object, "stable_lineage_policy", PLAN_LINEAGE)?;
    let thickness = object
        .get("thickness_m")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid("evaluation_plan thickness_m must be finite"))?;
    if !(0.0001 < thickness && thickness <= 0.25) {
        return Err(invalid(
            "evaluation_plan thickness_m is outside the closed bound",
        ));
    }
    if !matches!(
        text(object, "thickness_axis")?,
        "local_normal" | "world_x" | "world_y" | "world_z"
    ) {
        return Err(invalid(
            "evaluation_plan thickness_axis is outside the Core enum",
        ));
    }
    let plan = value.clone();
    let expected_canonical = canonical_hash_without(&plan, "canonical_sha256")?;
    if expected_canonical != hash(object, "canonical_sha256")? {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_PLAN_CANONICAL_MISMATCH",
            "evaluation_plan canonical_sha256 differs from its canonical payload",
        ));
    }
    Ok((plan, expected_canonical))
}

fn bound_plan(
    plan: &Value,
    spine: &KnifeCurve,
    edge: &KnifeCurve,
) -> Result<(KnifeBladeSweepPlan, String), RuntimeError> {
    let object = exact_object(plan, PLAN_FIELDS, "evaluation_plan")?;
    let axis = match text(object, "thickness_axis")? {
        "local_normal" => KnifeThicknessAxis::LocalNormal,
        "world_x" => KnifeThicknessAxis::WorldX,
        "world_y" => KnifeThicknessAxis::WorldY,
        "world_z" => KnifeThicknessAxis::WorldZ,
        _ => {
            return Err(invalid(
                "evaluation_plan thickness_axis is outside the Core enum",
            ))
        }
    };
    if text(object, "spine_curve_id")? != spine.curve_id.as_str()
        || text(object, "edge_curve_id")? != edge.curve_id.as_str()
    {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_PLAN_CURVE_BINDING_MISMATCH",
            "evaluation_plan curve IDs do not bind structural BladeSpine/BladeEdge",
        ));
    }
    let spine_hash = spine
        .canonical_sha256()
        .map_err(|error| invalid(error.to_string()))?;
    let edge_hash = edge
        .canonical_sha256()
        .map_err(|error| invalid(error.to_string()))?;
    if text(object, "spine_curve_sha256")? != spine_hash.as_str()
        || text(object, "edge_curve_sha256")? != edge_hash.as_str()
    {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_PLAN_CURVE_HASH_MISMATCH",
            "evaluation_plan curve hashes do not bind structural curves",
        ));
    }
    let thickness = object
        .get("thickness_m")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid("evaluation_plan thickness_m must be finite"))?;
    let typed = KnifeBladeSweepPlan::new(
        spine,
        edge,
        STATION_COUNT,
        PLAN_TOLERANCE_M,
        PLAN_MAX_SEGMENT_LENGTH_M,
        axis,
        thickness,
    )
    .map_err(|error| invalid(error.to_string()))?;
    let semantic = canonical_hash_without(plan, "canonical_sha256")?;
    Ok((typed, semantic))
}

#[derive(Clone)]
struct StructuralSource {
    record: WeaponryCurveModifierGraphDurableRecord,
    curves: BTreeMap<String, KnifeCurve>,
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

fn verify_runtime_source(
    runtime: &Runtime,
    object: &Map<String, Value>,
) -> Result<(), RuntimeError> {
    let project_id = id(object, "project_id")?;
    let candidate_id = id(object, "source_candidate_id")?;
    let candidate = runtime.store.get_candidate(candidate_id)?.ok_or_else(|| {
        mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_SOURCE_CANDIDATE_NOT_FOUND",
            "candidate is unavailable",
        )
    })?;
    if candidate.project_id != project_id
        || candidate.canonical_sha256 != hash(object, "source_candidate_state_sha256")?
    {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_SOURCE_CANDIDATE_MISMATCH",
            "candidate project/state differs from durable truth",
        ));
    }
    let revision_id = id(object, "source_authoring_mesh_revision_id")?;
    let revision = runtime
        .store
        .get_authoring_mesh_v2_durable_record_by_revision(project_id, revision_id)?
        .ok_or_else(|| {
            mismatch(
                "KNIFE_CURVE_EVALUATED_MESH_SOURCE_REVISION_NOT_FOUND",
                "AuthoringMesh revision is unavailable",
            )
        })?;
    if revision.mesh_id != id(object, "source_authoring_mesh_id")?
        || revision.lineage_id != id(object, "source_authoring_mesh_lineage_id")?
        || revision.revision_index != u64_field(object, "source_authoring_mesh_revision_index")?
        || revision.revision_sha256 != hash(object, "source_authoring_mesh_revision_sha256")?
    {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_SOURCE_REVISION_MISMATCH",
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
            "KNIFE_CURVE_EVALUATED_MESH_SOURCE_IDENTITY_MISMATCH",
            "source AuthoringMesh identity hash differs",
        ));
    }
    Ok(())
}

fn load_structural(
    runtime: &Runtime,
    object: &Map<String, Value>,
) -> Result<StructuralSource, RuntimeError> {
    let project_id = id(object, "project_id")?;
    let source_revision_sha256 = hash(object, "source_authoring_mesh_revision_sha256")?;
    let modifier_graph_sha256 = hash(object, "modifier_graph_semantic_sha256")?;
    let record = runtime
        .store
        .authoring_repository()
        .get_knife_curve_modifier_graph_by_source_revision_and_modifier_graph(
            project_id,
            source_revision_sha256,
            modifier_graph_sha256,
            hash(object, "curve_set_semantic_sha256")?,
            hash(object, "sample_set_semantic_sha256")?,
            hash(object, "dependency_graph_semantic_sha256")?,
            hash(object, "recompute_plan_semantic_sha256")?,
        )?
        .ok_or_else(|| {
            mismatch(
                "KNIFE_CURVE_EVALUATED_MESH_STRUCTURAL_RECORD_NOT_FOUND",
                "curve graph structural record is unavailable",
            )
        })?;
    let source_pairs = [
        ("source_candidate_id", record.source_candidate_id.as_str()),
        (
            "source_authoring_mesh_id",
            record.source_authoring_mesh_id.as_str(),
        ),
        (
            "source_authoring_mesh_lineage_id",
            record.source_authoring_mesh_lineage_id.as_str(),
        ),
        (
            "source_authoring_mesh_revision_id",
            record.source_revision_id.as_str(),
        ),
        (
            "source_modifier_graph_id",
            record.modifier_graph_id.as_str(),
        ),
    ];
    for (field, expected) in source_pairs {
        if id(object, field)? != expected {
            return Err(mismatch(
                "KNIFE_CURVE_EVALUATED_MESH_STRUCTURAL_SOURCE_MISMATCH",
                format!("{field} differs from structural record"),
            ));
        }
    }
    let hash_pairs = [
        (
            "source_candidate_state_sha256",
            record.source_candidate_state_sha256.as_str(),
        ),
        (
            "source_authoring_mesh_revision_sha256",
            record.source_revision_sha256.as_str(),
        ),
        (
            "source_authoring_mesh_identity_sha256",
            record.source_authoring_mesh_identity_sha256.as_str(),
        ),
        (
            "source_modifier_graph_sha256",
            record.modifier_graph_sha256.as_str(),
        ),
        (
            "curve_set_semantic_sha256",
            record.curve_set_sha256.as_str(),
        ),
        (
            "sample_set_semantic_sha256",
            record.sample_set_sha256.as_str(),
        ),
        (
            "modifier_graph_semantic_sha256",
            record.modifier_graph_sha256.as_str(),
        ),
        (
            "dependency_graph_semantic_sha256",
            record.dependency_graph_sha256.as_str(),
        ),
        (
            "recompute_plan_semantic_sha256",
            record.recompute_plan_sha256.as_str(),
        ),
    ];
    for (field, expected) in hash_pairs {
        if hash(object, field)? != expected {
            return Err(mismatch(
                "KNIFE_CURVE_EVALUATED_MESH_STRUCTURAL_HASH_MISMATCH",
                format!("{field} differs from structural record"),
            ));
        }
    }
    if record.source_authoring_mesh_revision_index
        != u64_field(object, "source_authoring_mesh_revision_index")?
    {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_STRUCTURAL_REVISION_INDEX_MISMATCH",
            "source revision index differs from structural record",
        ));
    }
    let roots = [
        (
            record.curve_set_object_sha256.as_str(),
            WEAPONRY_CURVE_SET_OBJECT_KIND,
            record.curve_set_sha256.as_str(),
        ),
        (
            record.sample_set_object_sha256.as_str(),
            WEAPONRY_SAMPLE_SET_OBJECT_KIND,
            record.sample_set_sha256.as_str(),
        ),
        (
            record.modifier_graph_object_sha256.as_str(),
            WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND,
            record.modifier_graph_sha256.as_str(),
        ),
        (
            record.dependency_graph_object_sha256.as_str(),
            WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND,
            record.dependency_graph_sha256.as_str(),
        ),
        (
            record.recompute_plan_object_sha256.as_str(),
            WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND,
            record.recompute_plan_sha256.as_str(),
        ),
    ];
    let values = roots
        .iter()
        .map(|(sha, kind, semantic)| {
            let value = runtime
                .store
                .authoring_repository()
                .read_knife_curve_modifier_graph_json(sha, kind)?;
            if canonical_json_hash(&value) != *semantic {
                return Err(mismatch(
                    "KNIFE_CURVE_EVALUATED_MESH_STRUCTURAL_SEMANTIC_MISMATCH",
                    format!("structural CAS root {kind} differs from its semantic hash"),
                ));
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    let curve_set = &values[0];
    let curve_set_object = exact_object(
        curve_set,
        &["schema_version", "sampling_policy", "curves"],
        "stored curve_set",
    )?;
    exact_const(curve_set_object, "schema_version", "KnifeCurveSet@1")?;
    exact_const(
        curve_set_object,
        "sampling_policy",
        "fixed-64-inclusive-parameter-samples@1",
    )?;
    let curves = curve_set_object
        .get("curves")
        .and_then(Value::as_array)
        .filter(|curves| !curves.is_empty() && curves.len() <= 16)
        .ok_or_else(|| invalid("stored curve_set must contain 1..=16 curves"))?
        .iter()
        .map(parse_curve)
        .collect::<Result<Vec<_>, _>>()?;
    let mut curve_map = BTreeMap::new();
    for curve in curves {
        if curve_map
            .insert(curve.curve_id.as_str().to_owned(), curve)
            .is_some()
        {
            return Err(invalid("stored curve_set repeats curve_id"));
        }
    }
    let sample_object = exact_object(
        &values[1],
        &[
            "schema_version",
            "sample_count_per_curve",
            "tolerance_m",
            "max_segment_length_m",
            "samples",
            "mesh_created",
        ],
        "stored sample_set",
    )?;
    exact_const(
        sample_object,
        "schema_version",
        "KnifeCurveSampleSetBundle@1",
    )?;
    if u64_field(sample_object, "sample_count_per_curve")? != SAMPLE_COUNT
        || sample_object["tolerance_m"].as_f64() != Some(SAMPLE_TOLERANCE_M)
        || sample_object["max_segment_length_m"].as_f64() != Some(SAMPLE_MAX_SEGMENT_LENGTH_M)
        || sample_object["mesh_created"] != Value::Bool(false)
    {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_STRUCTURAL_SAMPLE_MISMATCH",
            "stored sample_set policy differs from the structural curve contract",
        ));
    }
    let graph = parse_modifier_graph(&values[2])?;
    if graph.source_revision_id.as_str() != record.source_revision_id
        || graph.source_revision_sha256.as_str() != record.source_revision_sha256
        || graph.graph_id.as_str() != record.modifier_graph_id
        || graph
            .canonical_sha256()
            .map_err(|error| invalid(error.to_string()))?
            .as_str()
            != record.modifier_graph_sha256
    {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_MODIFIER_GRAPH_MISMATCH",
            "stored ModifierGraph does not bind the structural source",
        ));
    }
    for node in &graph.nodes {
        if let ModifierKind::CurveProfile {
            curve_id,
            curve_sha256,
        } = &node.operator
        {
            let curve = curve_map.get(curve_id.as_str()).ok_or_else(|| {
                mismatch(
                    "KNIFE_CURVE_EVALUATED_MESH_CURVE_GRAPH_BINDING_MISMATCH",
                    "stored curve profile references a missing curve",
                )
            })?;
            if curve
                .canonical_sha256()
                .map_err(|error| invalid(error.to_string()))?
                != *curve_sha256
            {
                return Err(mismatch(
                    "KNIFE_CURVE_EVALUATED_MESH_CURVE_GRAPH_BINDING_MISMATCH",
                    "stored curve profile hash differs from curve_set",
                ));
            }
        }
    }
    Ok(StructuralSource {
        record,
        curves: curve_map,
    })
}

fn verify_plan_curves(
    structural: &StructuralSource,
    plan: &Value,
) -> Result<(KnifeCurve, KnifeCurve, KnifeBladeSweepPlan, String), RuntimeError> {
    let plan_object = exact_object(plan, PLAN_FIELDS, "evaluation_plan")?;
    let spine_id = id(plan_object, "spine_curve_id")?;
    let edge_id = id(plan_object, "edge_curve_id")?;
    if spine_id == edge_id {
        return Err(invalid("evaluation_plan spine and edge curves must differ"));
    }
    let spine = structural.curves.get(spine_id).cloned().ok_or_else(|| {
        mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_SPINE_NOT_FOUND",
            "evaluation_plan BladeSpine is absent from curve_set",
        )
    })?;
    let edge = structural.curves.get(edge_id).cloned().ok_or_else(|| {
        mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_EDGE_NOT_FOUND",
            "evaluation_plan BladeEdge is absent from curve_set",
        )
    })?;
    if spine.role != KnifeCurveRole::BladeSpine || edge.role != KnifeCurveRole::BladeEdge {
        return Err(invalid(
            "evaluation_plan curves have the wrong semantic roles",
        ));
    }
    let (typed, semantic) = bound_plan(plan, &spine, &edge)?;
    Ok((spine, edge, typed, semantic))
}

fn object_bytes(value: &Value) -> Result<Vec<u8>, RuntimeError> {
    let bytes = canonical_json_bytes(value)
        .map_err(|error| invalid(format!("canonical JSON failed: {error}")))?;
    if bytes.is_empty() || bytes.len() as u64 > WEAPONRY_CURVE_EVALUATED_MESH_MAX_JSON_BYTES {
        return Err(invalid(
            "evaluated mesh CAS object exceeds its bounded budget",
        ));
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
        WEAPONRY_CURVE_EVALUATED_MESH_JSON_MIME,
        kind,
        &now_string(),
    )?;
    object.record = runtime
        .store
        .get_object(&object.record.sha256)?
        .ok_or_else(|| invalid("staged evaluated-mesh CAS object is not registered"))?;
    Ok(object)
}

fn cleanup(runtime: &Runtime, reservation: &forgecad_store::CasReservation, objects: &[CasObject]) {
    for object in objects {
        let _ = runtime
            .store
            .release_cas_reservation_object(reservation, object, true);
    }
}

fn input_evaluation_hashes(
    structural: &StructuralSource,
    evaluation_plan_semantic_sha256: &str,
) -> Result<Vec<Sha256Hash>, RuntimeError> {
    [
        structural.record.curve_set_sha256.as_str(),
        structural.record.sample_set_sha256.as_str(),
        structural.record.dependency_graph_sha256.as_str(),
        structural.record.recompute_plan_sha256.as_str(),
        evaluation_plan_semantic_sha256,
    ]
    .into_iter()
    .map(|value| Sha256Hash::new(value).map_err(|error| invalid(error.to_string())))
    .collect()
}

fn lookup_key(
    structural: &StructuralSource,
    evaluation_id: &str,
    plan_semantic: &str,
    mesh_object_sha256: &str,
    mesh_semantic: &str,
    identity_sha256: &str,
    link_sha256: &str,
) -> String {
    canonical_json_hash(&json!({
        "schema_version":"KnifeCurveEvaluatedMeshLookupKey@1",
        "source_candidate_id":structural.record.source_candidate_id,
        "source_authoring_mesh_revision_sha256":structural.record.source_revision_sha256,
        "source_modifier_graph_sha256":structural.record.modifier_graph_sha256,
        "curve_graph_lookup_key_sha256":structural.record.lookup_key_sha256,
        "evaluation_id":evaluation_id,
        "evaluation_plan_semantic_sha256":plan_semantic,
        "evaluated_mesh_identity_sha256":identity_sha256,
        "evaluated_mesh_link_sha256":link_sha256,
        "evaluated_mesh_object_sha256":mesh_object_sha256,
        "evaluated_mesh_semantic_sha256":mesh_semantic,
    }))
}

fn record_canonical(record: &KnifeCurveEvaluatedMeshDurableRecord) -> Result<String, RuntimeError> {
    Ok(forgecad_store::weaponry_curve_evaluated_mesh_record_canonical_sha256(record)?)
}

fn result(
    record: &KnifeCurveEvaluatedMeshDurableRecord,
    plan: &Value,
    operation: &str,
    status: &str,
    request_idempotency_key: &str,
    replayed: bool,
    wrote: bool,
    restart_hash_verified: bool,
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
        "source_authoring_mesh_revision_id":record.source_authoring_mesh_revision_id,
        "source_authoring_mesh_revision_index":record.source_authoring_mesh_revision_index,
        "source_authoring_mesh_revision_sha256":record.source_authoring_mesh_revision_sha256,
        "source_authoring_mesh_identity_sha256":record.source_authoring_mesh_identity_sha256,
        "source_modifier_graph_id":record.source_modifier_graph_id,
        "source_modifier_graph_sha256":record.source_modifier_graph_sha256,
        "curve_set_semantic_sha256":record.curve_set_semantic_sha256,
        "sample_set_semantic_sha256":record.sample_set_semantic_sha256,
        "modifier_graph_semantic_sha256":record.modifier_graph_semantic_sha256,
        "dependency_graph_semantic_sha256":record.dependency_graph_semantic_sha256,
        "recompute_plan_semantic_sha256":record.recompute_plan_semantic_sha256,
        "curve_graph_lookup_key_sha256":record.curve_graph_lookup_key_sha256,
        "evaluated_mesh_lookup_key_sha256":record.evaluated_mesh_lookup_key_sha256,
        "evaluation_plan":plan,
        "evaluation_plan_object_sha256":record.evaluation_plan_object_sha256,
        "evaluation_plan_semantic_sha256":record.evaluation_plan_semantic_sha256,
        "evaluated_mesh_id":record.evaluated_mesh_id,
        "evaluated_mesh_object_sha256":record.evaluated_mesh_object_sha256,
        "evaluated_mesh_semantic_sha256":record.evaluated_mesh_semantic_sha256,
        "evaluated_mesh_identity_sha256":record.evaluated_mesh_identity_sha256,
        "evaluated_mesh_link_sha256":record.evaluated_mesh_link_sha256,
        "vertex_count":record.vertex_count,
        "triangle_count":record.triangle_count,
        "closed_two_manifold":record.closed_two_manifold,
        "zero_degenerate_triangles":record.zero_degenerate_triangles,
        "mesh_readback_status":MESH_READBACK_STATUS,
        "evaluation_status":EVALUATION_STATUS,
        "evaluated_mesh_created":true,
        "geometry_artifact_created":false,
        "replayed":replayed,
        "deterministic_replay":true,
        "byte_exact_replay":true,
        "restart_hash_verified":restart_hash_verified,
        "idempotency_key":request_idempotency_key,
        "atomicity_status":if wrote { "committed" } else { "not-touched" },
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
        "high_status":"NOT_RUN",
        "uv_status":"NOT_RUN",
        "bake_status":"NOT_RUN",
        "visual_status":"NOT_RUN",
        "human_status":"NOT_RUN",
        "engine_status":"NOT_RUN",
        "canonicalization_policy":RESULT_CANONICALIZATION,
        "canonical_sha256":"",
    });
    value["canonical_sha256"] = Value::String(canonical_hash_without(&value, "canonical_sha256")?);
    let bytes = canonical_json_bytes(&value)
        .map_err(|error| invalid(format!("result canonicalization failed: {error}")))?;
    if bytes.len() > max_bytes {
        return Err(invalid("result exceeds max_response_bytes"));
    }
    Ok(value)
}

fn compare_source_request_to_record(
    object: &Map<String, Value>,
    record: &KnifeCurveEvaluatedMeshDurableRecord,
) -> Result<(), RuntimeError> {
    let pairs = [
        ("source_candidate_id", record.source_candidate_id.as_str()),
        (
            "source_authoring_mesh_id",
            record.source_authoring_mesh_id.as_str(),
        ),
        (
            "source_authoring_mesh_lineage_id",
            record.source_authoring_mesh_lineage_id.as_str(),
        ),
        (
            "source_authoring_mesh_revision_id",
            record.source_authoring_mesh_revision_id.as_str(),
        ),
        (
            "source_modifier_graph_id",
            record.source_modifier_graph_id.as_str(),
        ),
        (
            "source_candidate_state_sha256",
            record.source_candidate_state_sha256.as_str(),
        ),
        (
            "source_authoring_mesh_revision_sha256",
            record.source_authoring_mesh_revision_sha256.as_str(),
        ),
        (
            "source_authoring_mesh_identity_sha256",
            record.source_authoring_mesh_identity_sha256.as_str(),
        ),
        (
            "source_modifier_graph_sha256",
            record.source_modifier_graph_sha256.as_str(),
        ),
        (
            "curve_set_semantic_sha256",
            record.curve_set_semantic_sha256.as_str(),
        ),
        (
            "sample_set_semantic_sha256",
            record.sample_set_semantic_sha256.as_str(),
        ),
        (
            "modifier_graph_semantic_sha256",
            record.modifier_graph_semantic_sha256.as_str(),
        ),
        (
            "dependency_graph_semantic_sha256",
            record.dependency_graph_semantic_sha256.as_str(),
        ),
        (
            "recompute_plan_semantic_sha256",
            record.recompute_plan_semantic_sha256.as_str(),
        ),
    ];
    for (field, expected) in pairs {
        let actual = if field.starts_with("source_") && field.ends_with("_id")
            || field == "source_candidate_id"
            || field == "source_modifier_graph_id"
        {
            id(object, field)?
        } else {
            hash(object, field)?
        };
        if actual != expected {
            return Err(mismatch(
                "KNIFE_CURVE_EVALUATED_MESH_RECORD_SOURCE_MISMATCH",
                format!("{field} differs from durable evaluated-mesh truth"),
            ));
        }
    }
    if u64_field(object, "source_authoring_mesh_revision_index")?
        != record.source_authoring_mesh_revision_index
    {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_RECORD_SOURCE_MISMATCH",
            "source_authoring_mesh_revision_index differs from durable truth",
        ));
    }
    Ok(())
}

fn parse_and_validate_derived(
    structural: &StructuralSource,
    record: &KnifeCurveEvaluatedMeshDurableRecord,
    plan: &Value,
    mesh_value: &Value,
    identity_value: &Value,
    link_value: &Value,
) -> Result<(), RuntimeError> {
    // Reapply the closed public plan policy on readback before binding it to
    // Core rails. Store verifies the CAS bytes and semantic hash; Runtime
    // additionally owns the fixed 32-station/tolerance/axis contract.
    parse_plan(plan)?;
    let (spine, edge, typed_plan, plan_semantic) = verify_plan_curves(structural, plan)?;
    if plan_semantic != record.evaluation_plan_semantic_sha256
        || canonical_json_hash(plan) != record.evaluation_plan_object_sha256
    {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_PLAN_HASH_MISMATCH",
            "evaluation plan object/semantic hash differs from durable truth",
        ));
    }
    let mesh: EvaluatedMeshGeometry =
        serde_json::from_value(mesh_value.clone()).map_err(|error| {
            invalid(format!(
                "evaluated mesh JSON is not typed Core geometry: {error}"
            ))
        })?;
    mesh.validate()
        .map_err(|error| invalid(error.to_string()))?;
    if mesh.semantic_sha256.as_str() != record.evaluated_mesh_semantic_sha256
        || canonical_json_hash(mesh_value) != record.evaluated_mesh_object_sha256
        || mesh.plan_sha256
            != typed_plan
                .canonical_sha256()
                .map_err(|error| invalid(error.to_string()))?
    {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_OUTPUT_HASH_MISMATCH",
            "evaluated mesh payload does not match plan/object/semantic bindings",
        ));
    }
    if mesh.vertices.len() as u64 != record.vertex_count
        || mesh.triangles.len() as u64 != record.triangle_count
    {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_COUNT_MISMATCH",
            "evaluated mesh counts differ from durable truth",
        ));
    }
    let input_hashes = input_evaluation_hashes(structural, &plan_semantic)?;
    let identity = mesh
        .evaluated_mesh_identity(
            Sha256Hash::new(record.source_authoring_mesh_revision_sha256.as_str())
                .map_err(|error| invalid(error.to_string()))?,
            Sha256Hash::new(record.source_modifier_graph_sha256.as_str())
                .map_err(|error| invalid(error.to_string()))?,
            input_hashes,
        )
        .map_err(|error| invalid(error.to_string()))?;
    let expected_identity = serde_json::to_value(&identity)
        .map_err(|error| invalid(format!("identity serialization failed: {error}")))?;
    if expected_identity != *identity_value
        || canonical_json_hash(identity_value) != record.evaluated_mesh_identity_sha256
    {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_IDENTITY_HASH_MISMATCH",
            "EvaluatedMeshIdentity does not bind real source/graph/input/output hashes",
        ));
    }
    let link = EvaluatedMeshLink::new(identity);
    let expected_link = serde_json::to_value(&link)
        .map_err(|error| invalid(format!("link serialization failed: {error}")))?;
    if expected_link != *link_value
        || canonical_json_hash(link_value) != record.evaluated_mesh_link_sha256
    {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_LINK_HASH_MISMATCH",
            "EvaluatedMeshLink does not exactly bind EvaluatedMeshIdentity",
        ));
    }
    // Re-evaluate the disposable mesh during readback. This checks that a
    // reopened Runtime sees the same deterministic bytes, while the public
    // result deliberately remains restart_hash_verified=false.
    let replay = typed_plan
        .evaluate(&spine, &edge)
        .map_err(|error| invalid(error.to_string()))?;
    let replay_bytes = canonical_json_bytes(
        &serde_json::to_value(&replay)
            .map_err(|error| invalid(format!("replay serialization failed: {error}")))?,
    )
    .map_err(|error| invalid(format!("replay canonicalization failed: {error}")))?;
    let stored_bytes = canonical_json_bytes(mesh_value)
        .map_err(|error| invalid(format!("mesh canonicalization failed: {error}")))?;
    if replay_bytes != stored_bytes {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_REPLAY_MISMATCH",
            "stored mesh differs from deterministic Core replay",
        ));
    }
    Ok(())
}

pub(crate) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, PREPARE_FIELDS, "prepare request")?;
    exact_const(object, "schema_version", PREPARE_SCHEMA)?;
    exact_const(object, "operation", PREPARE_OPERATION)?;
    exact_const(object, "writer_policy", WRITER_POLICY)?;
    exact_const(object, "canonicalization_policy", REQUEST_CANONICALIZATION)?;
    if bool_field(object, "runtime_write_performed")? {
        return Err(invalid(
            "runtime_write_performed must be false in a request",
        ));
    }
    let max_bytes = max_response_bytes(object)?;
    let input_sha256 = request_hash(request, object)?;
    verify_runtime_source(runtime, object)?;
    let structural = load_structural(runtime, object)?;
    let plan_value = object
        .get("evaluation_plan")
        .ok_or_else(|| invalid("evaluation_plan is missing"))?;
    parse_plan(plan_value)?;
    let (spine, edge, typed_plan, plan_semantic) = verify_plan_curves(&structural, plan_value)?;
    let first = typed_plan
        .evaluate(&spine, &edge)
        .map_err(|error| invalid(error.to_string()))?;
    let second = typed_plan
        .evaluate(&spine, &edge)
        .map_err(|error| invalid(error.to_string()))?;
    let first_value = serde_json::to_value(&first)
        .map_err(|error| invalid(format!("evaluated mesh serialization failed: {error}")))?;
    let second_value = serde_json::to_value(&second).map_err(|error| {
        invalid(format!(
            "evaluated mesh replay serialization failed: {error}"
        ))
    })?;
    let first_bytes = canonical_json_bytes(&first_value)
        .map_err(|error| invalid(format!("evaluated mesh canonicalization failed: {error}")))?;
    let second_bytes = canonical_json_bytes(&second_value).map_err(|error| {
        invalid(format!(
            "evaluated mesh replay canonicalization failed: {error}"
        ))
    })?;
    if first_bytes != second_bytes {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_REPLAY_MISMATCH",
            "pure Core evaluation was not byte exact",
        ));
    }
    let identity = first
        .evaluated_mesh_identity(
            Sha256Hash::new(structural.record.source_revision_sha256.as_str())
                .map_err(|error| invalid(error.to_string()))?,
            Sha256Hash::new(structural.record.modifier_graph_sha256.as_str())
                .map_err(|error| invalid(error.to_string()))?,
            input_evaluation_hashes(&structural, &plan_semantic)?,
        )
        .map_err(|error| invalid(error.to_string()))?;
    let identity_value = serde_json::to_value(&identity)
        .map_err(|error| invalid(format!("identity serialization failed: {error}")))?;
    let link = EvaluatedMeshLink::new(identity.clone());
    let link_value = serde_json::to_value(&link)
        .map_err(|error| invalid(format!("link serialization failed: {error}")))?;
    let mesh_semantic = first.semantic_sha256.as_str().to_owned();
    let identity_sha256 = canonical_json_hash(&identity_value);
    let link_sha256 = canonical_json_hash(&link_value);
    let evaluated_mesh_id = format!("knife-evaluated-mesh-{mesh_semantic}");
    let reservation = runtime.store.begin_cas_reservation();
    let mut staged = Vec::new();
    let outcome = (|| -> Result<Value, RuntimeError> {
        let plan_object = stage_object(
            runtime,
            &reservation,
            plan_value,
            WEAPONRY_CURVE_EVALUATION_PLAN_OBJECT_KIND,
        )?;
        staged.push(plan_object.clone());
        let mesh_object = stage_object(
            runtime,
            &reservation,
            &first_value,
            WEAPONRY_EVALUATED_MESH_OBJECT_KIND,
        )?;
        staged.push(mesh_object.clone());
        let identity_object = stage_object(
            runtime,
            &reservation,
            &identity_value,
            WEAPONRY_EVALUATED_MESH_IDENTITY_OBJECT_KIND,
        )?;
        staged.push(identity_object.clone());
        let link_object = stage_object(
            runtime,
            &reservation,
            &link_value,
            WEAPONRY_EVALUATED_MESH_LINK_OBJECT_KIND,
        )?;
        staged.push(link_object.clone());
        let evaluated_lookup = lookup_key(
            &structural,
            id(
                exact_object(plan_value, PLAN_FIELDS, "evaluation_plan")?,
                "evaluation_id",
            )?,
            &plan_semantic,
            &mesh_object.record.sha256,
            &mesh_semantic,
            &identity_sha256,
            &link_sha256,
        );
        let mut record = KnifeCurveEvaluatedMeshDurableRecord {
            schema_version: WEAPONRY_CURVE_EVALUATED_MESH_RECORD_SCHEMA.to_owned(),
            project_id: structural.record.project_id.clone(),
            curve_graph_lookup_key_sha256: structural.record.lookup_key_sha256.clone(),
            source_candidate_id: structural.record.source_candidate_id.clone(),
            source_candidate_state_sha256: structural.record.source_candidate_state_sha256.clone(),
            source_authoring_mesh_id: structural.record.source_authoring_mesh_id.clone(),
            source_authoring_mesh_lineage_id: structural
                .record
                .source_authoring_mesh_lineage_id
                .clone(),
            source_authoring_mesh_revision_id: structural.record.source_revision_id.clone(),
            source_authoring_mesh_revision_index: structural
                .record
                .source_authoring_mesh_revision_index,
            source_authoring_mesh_revision_sha256: structural.record.source_revision_sha256.clone(),
            source_authoring_mesh_identity_sha256: structural
                .record
                .source_authoring_mesh_identity_sha256
                .clone(),
            source_modifier_graph_id: structural.record.modifier_graph_id.clone(),
            source_modifier_graph_sha256: structural.record.modifier_graph_sha256.clone(),
            curve_set_semantic_sha256: structural.record.curve_set_sha256.clone(),
            curve_set_object_sha256: structural.record.curve_set_object_sha256.clone(),
            sample_set_semantic_sha256: structural.record.sample_set_sha256.clone(),
            sample_set_object_sha256: structural.record.sample_set_object_sha256.clone(),
            modifier_graph_semantic_sha256: structural.record.modifier_graph_sha256.clone(),
            modifier_graph_object_sha256: structural.record.modifier_graph_object_sha256.clone(),
            dependency_graph_semantic_sha256: structural.record.dependency_graph_sha256.clone(),
            dependency_graph_object_sha256: structural
                .record
                .dependency_graph_object_sha256
                .clone(),
            recompute_plan_semantic_sha256: structural.record.recompute_plan_sha256.clone(),
            recompute_plan_object_sha256: structural.record.recompute_plan_object_sha256.clone(),
            evaluation_id: id(
                exact_object(plan_value, PLAN_FIELDS, "evaluation_plan")?,
                "evaluation_id",
            )?
            .to_owned(),
            evaluation_plan_semantic_sha256: plan_semantic.clone(),
            evaluation_plan_object_sha256: plan_object.record.sha256.clone(),
            evaluated_mesh_id,
            evaluated_mesh_semantic_sha256: mesh_semantic,
            evaluated_mesh_object_sha256: mesh_object.record.sha256.clone(),
            evaluated_mesh_identity_sha256: identity_sha256,
            evaluated_mesh_identity_object_sha256: identity_object.record.sha256.clone(),
            evaluated_mesh_link_sha256: link_sha256,
            evaluated_mesh_link_object_sha256: link_object.record.sha256.clone(),
            vertex_count: first.vertices.len() as u64,
            triangle_count: first.triangles.len() as u64,
            closed_two_manifold: true,
            zero_degenerate_triangles: true,
            evaluated_mesh_lookup_key_sha256: evaluated_lookup,
            idempotency_key: id(object, "idempotency_key")?.to_owned(),
            input_sha256,
            materialization_status: WEAPONRY_CURVE_EVALUATED_MESH_STATUS.to_owned(),
            canonical_sha256: String::new(),
            created_at: now_string(),
        };
        record.canonical_sha256 = record_canonical(&record)?;
        let commit = KnifeCurveEvaluatedMeshCommit {
            record,
            cas: KnifeCurveEvaluatedMeshCasBundle {
                evaluation_plan: plan_object.record.clone(),
                evaluated_mesh: mesh_object.record.clone(),
                evaluated_mesh_identity: identity_object.record.clone(),
                evaluated_mesh_link: link_object.record.clone(),
            },
        };
        let (stored, replayed) = runtime
            .store
            .record_knife_curve_evaluated_mesh_with_replay(&commit)?;
        result(
            &stored,
            plan_value,
            PREPARE_OPERATION,
            if replayed { "replayed" } else { "prepared" },
            &stored.idempotency_key,
            replayed,
            !replayed,
            false,
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
    verify_runtime_source(runtime, object)?;
    let record = runtime
        .store
        .get_knife_curve_evaluated_mesh(
            id(object, "project_id")?,
            hash(object, "evaluated_mesh_lookup_key_sha256")?,
        )?
        .ok_or_else(|| {
            mismatch(
                "KNIFE_CURVE_EVALUATED_MESH_NOT_FOUND",
                "evaluated mesh durable record is unavailable",
            )
        })?;
    // Bind every caller-supplied source/derived field to the evaluated row
    // before following its structural parent. This keeps lookup namespaces
    // separate and makes a mismatched GET fail on the requested evaluated
    // identity rather than on an unrelated parent lookup.
    compare_source_request_to_record(object, &record)?;
    if id(object, "evaluation_id")? != record.evaluation_id
        || hash(object, "evaluation_plan_object_sha256")? != record.evaluation_plan_object_sha256
        || hash(object, "evaluation_plan_semantic_sha256")?
            != record.evaluation_plan_semantic_sha256
        || id(object, "evaluated_mesh_id")? != record.evaluated_mesh_id
        || hash(object, "evaluated_mesh_object_sha256")? != record.evaluated_mesh_object_sha256
        || hash(object, "evaluated_mesh_semantic_sha256")? != record.evaluated_mesh_semantic_sha256
        || hash(object, "evaluated_mesh_identity_sha256")? != record.evaluated_mesh_identity_sha256
        || hash(object, "evaluated_mesh_link_sha256")? != record.evaluated_mesh_link_sha256
        || u64_field(object, "vertex_count")? != record.vertex_count
        || u64_field(object, "triangle_count")? != record.triangle_count
    {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_LOOKUP_MISMATCH",
            "get request does not exactly bind evaluated mesh durable truth",
        ));
    }
    // Get's closed contract intentionally carries only the evaluated lookup
    // key. Reconstruct the structural-parent request from the durable record
    // instead of requiring curve_graph_lookup_key_sha256 from the caller.
    let structural_request = json!({
        "project_id": record.project_id,
        "source_candidate_id": record.source_candidate_id,
        "source_candidate_state_sha256": record.source_candidate_state_sha256,
        "source_authoring_mesh_id": record.source_authoring_mesh_id,
        "source_authoring_mesh_lineage_id": record.source_authoring_mesh_lineage_id,
        "source_authoring_mesh_revision_id": record.source_authoring_mesh_revision_id,
        "source_authoring_mesh_revision_index": record.source_authoring_mesh_revision_index,
        "source_authoring_mesh_revision_sha256": record.source_authoring_mesh_revision_sha256,
        "source_authoring_mesh_identity_sha256": record.source_authoring_mesh_identity_sha256,
        "source_modifier_graph_id": record.source_modifier_graph_id,
        "source_modifier_graph_sha256": record.source_modifier_graph_sha256,
        "curve_set_semantic_sha256": record.curve_set_semantic_sha256,
        "sample_set_semantic_sha256": record.sample_set_semantic_sha256,
        "modifier_graph_semantic_sha256": record.modifier_graph_semantic_sha256,
        "dependency_graph_semantic_sha256": record.dependency_graph_semantic_sha256,
        "recompute_plan_semantic_sha256": record.recompute_plan_semantic_sha256,
    });
    let structural_object = structural_request
        .as_object()
        .ok_or_else(|| invalid("internal structural lookup is not an object"))?;
    let structural = load_structural(runtime, structural_object)?;
    if record.curve_graph_lookup_key_sha256 != structural.record.lookup_key_sha256 {
        return Err(mismatch(
            "KNIFE_CURVE_EVALUATED_MESH_CURVE_GRAPH_LOOKUP_MISMATCH",
            "evaluated mesh does not bind the requested structural record",
        ));
    }
    let plan = runtime
        .store
        .authoring_repository()
        .read_knife_curve_evaluated_mesh_json(
            &record.evaluation_plan_object_sha256,
            WEAPONRY_CURVE_EVALUATION_PLAN_OBJECT_KIND,
        )?;
    let mesh = runtime
        .store
        .authoring_repository()
        .read_knife_curve_evaluated_mesh_json(
            &record.evaluated_mesh_object_sha256,
            WEAPONRY_EVALUATED_MESH_OBJECT_KIND,
        )?;
    let identity = runtime
        .store
        .authoring_repository()
        .read_knife_curve_evaluated_mesh_json(
            &record.evaluated_mesh_identity_object_sha256,
            WEAPONRY_EVALUATED_MESH_IDENTITY_OBJECT_KIND,
        )?;
    let link = runtime
        .store
        .authoring_repository()
        .read_knife_curve_evaluated_mesh_json(
            &record.evaluated_mesh_link_object_sha256,
            WEAPONRY_EVALUATED_MESH_LINK_OBJECT_KIND,
        )?;
    parse_and_validate_derived(&structural, &record, &plan, &mesh, &identity, &link)?;
    result(
        &record,
        &plan,
        GET_OPERATION,
        "found",
        id(object, "idempotency_key")?,
        false,
        false,
        false,
        max_bytes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_contracts::CandidateRecord;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    const TEST_MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

    fn curve_value(curve_id: &str, role: &str, control_points_m: Vec<[f64; 3]>) -> Value {
        let curve = KnifeCurve::new(
            curve_id,
            match role {
                "blade_spine" => KnifeCurveRole::BladeSpine,
                "blade_edge" => KnifeCurveRole::BladeEdge,
                _ => panic!("test curve role"),
            },
            KnifeCurveBasis::Bezier,
            3,
            control_points_m.clone(),
            Vec::new(),
            Vec::new(),
            false,
        )
        .expect("test curve");
        json!({
            "curve_id": curve_id,
            "role": role,
            "basis": "bezier",
            "degree": 3,
            "control_points_m": control_points_m,
            "weights": [],
            "knots": [],
            "closed": false,
            "canonical_sha256": curve.canonical_sha256().expect("curve hash"),
        })
    }

    fn authoring_genesis_request(project_id: &str) -> Value {
        let mut request = json!({
            "schema_version": "AuthoringMeshV2DurablePrepareRequest@1",
            "project_id": project_id,
            "operation": "genesis",
            "mesh_id": "knife-authoring-mesh",
            "lineage_id": "knife-authoring-lineage",
            "parent_revision_id": null,
            "operation_id": null,
            "edge_id": null,
            "split_ratio_milli": null,
            "vertex_ids": null,
            "delta_m": null,
            "operation_lineage_sha256": null,
            "positions_m": [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0]
            ],
            "faces": [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
            "evaluated": null,
            "idempotency_key": "knife-authoring-genesis",
            "max_response_bytes": TEST_MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
            "input_sha256": "",
        });
        let input_sha256 = canonical_json_hash(&request);
        request["input_sha256"] = Value::String(input_sha256);
        request
    }

    fn graph_value(source_revision_id: &str, source_revision_sha256: &str, spine: &Value) -> Value {
        let spine_curve = parse_curve(spine).expect("spine curve");
        let graph = ModifierGraph::new(
            "knife-curve-graph",
            source_revision_id,
            Sha256Hash::new(source_revision_sha256).expect("source hash"),
            vec![ModifierNode::new(
                "profile-node",
                ModifierKind::curve_profile(&spine_curve).expect("curve profile"),
                Vec::new(),
                None,
                true,
            )
            .expect("modifier node")],
            vec![StableId::new("profile-node").expect("output id")],
        )
        .expect("modifier graph");
        json!({
            "graph_id": "knife-curve-graph",
            "source_revision_id": source_revision_id,
            "source_revision_sha256": source_revision_sha256,
            "nodes": [{
                "node_id": "profile-node",
                "operator": {
                    "operator": "curve_profile",
                    "curve_id": spine["curve_id"],
                    "curve_sha256": spine["canonical_sha256"],
                },
                "input_node_ids": [],
                "selection_query_sha256": null,
                "enabled": true,
            }],
            "output_node_ids": ["profile-node"],
            "canonical_sha256": graph.canonical_sha256().expect("graph hash"),
        })
    }

    fn structural_request(
        project_id: &str,
        candidate_state_sha256: &str,
        source: &Value,
        spine: &Value,
        edge: &Value,
        graph: &Value,
    ) -> Value {
        let mut request = json!({
            "schema_version": "KnifeCurveModifierGraphPrepareRequest@1",
            "operation": "knife_curve_modifier_graph_prepare",
            "project_id": project_id,
            "source_candidate_id": "knife-source-candidate",
            "source_candidate_state_sha256": candidate_state_sha256,
            "source_authoring_mesh_id": source["mesh_id"],
            "source_authoring_mesh_lineage_id": source["lineage_id"],
            "source_authoring_mesh_revision_id": source["revision_id"],
            "source_authoring_mesh_revision_index": source["revision_index"],
            "source_authoring_mesh_revision_sha256": source["revision_sha256"],
            "source_authoring_mesh_identity_sha256": source["identity_sha256"],
            "curves": [spine, edge],
            "modifier_graph": graph,
            "dirty_seeds": ["profile-node"],
            "recompute_policy": "dirty-seed-dependency-closure-recompute@1",
            "evaluation_policy": "original-authoring-mesh-modifier-graph-deterministic@1",
            "idempotency_key": "knife-curve-structural",
            "max_response_bytes": TEST_MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": "",
        });
        request["input_sha256"] = Value::String(canonical_hash_with_empty_input(&request));
        request
    }

    fn canonical_hash_with_empty_input(value: &Value) -> String {
        let mut value = value.clone();
        value["input_sha256"] = Value::String(String::new());
        canonical_json_hash(&value)
    }

    fn source_fixture(runtime: &Runtime) -> (String, Value) {
        let project = runtime
            .create_project("knife evaluated mesh test", json!({"scope": "test"}))
            .expect("project");
        let candidate_state_sha256 = "c".repeat(64);
        runtime
            .insert_candidate(&CandidateRecord {
                schema_version: "Candidate@1".to_owned(),
                candidate_id: "knife-source-candidate".to_owned(),
                project_id: project.project_id.clone(),
                base_version_id: None,
                source_version_id: None,
                prepared_object_id: None,
                prepared_object_sha256: None,
                state: "prepared".to_owned(),
                request_sha256: "d".repeat(64),
                manifest_hash: None,
                quality_report_id: None,
                quality_hard_gate_passed: false,
                canonical_sha256: candidate_state_sha256.clone(),
                error_code: None,
                created_at: "1".to_owned(),
                updated_at: "1".to_owned(),
            })
            .expect("candidate");
        let authoring = runtime
            .authoring_mesh_v2_durable_prepare(&authoring_genesis_request(&project.project_id))
            .expect("AuthoringMesh genesis");
        let revision_id = authoring["revision_id"].as_str().expect("revision id");
        let revision_sha256 = authoring["revision_sha256"]
            .as_str()
            .expect("revision hash");
        let revision_index = authoring["revision_index"]
            .as_u64()
            .expect("revision index");
        let source = json!({
            "mesh_id": authoring["mesh_id"],
            "lineage_id": authoring["lineage_id"],
            "revision_id": revision_id,
            "revision_index": revision_index,
            "revision_sha256": revision_sha256,
            "identity_sha256": source_identity_hash(
                authoring["mesh_id"].as_str().expect("mesh id"),
                authoring["lineage_id"].as_str().expect("lineage id"),
                revision_id,
                revision_index,
                revision_sha256,
            ),
        });
        (
            project.project_id,
            json!({"candidate_state_sha256": candidate_state_sha256, "source": source}),
        )
    }

    fn evaluation_fixture(runtime: &Runtime) -> (String, Value, Value) {
        let (project_id, fixture) = source_fixture(runtime);
        let candidate_state_sha256 = fixture["candidate_state_sha256"].as_str().unwrap();
        let source = &fixture["source"];
        let spine = curve_value(
            "blade-spine",
            "blade_spine",
            vec![
                [0.0, 0.0, 0.0],
                [0.0, 0.2, 0.4],
                [0.0, 0.6, 0.8],
                [0.0, 1.0, 1.0],
            ],
        );
        let edge = curve_value(
            "blade-edge",
            "blade_edge",
            vec![
                [0.42, 0.0, 0.0],
                [0.42, 0.2, 0.0],
                [0.34, 0.65, 0.0],
                [0.0, 1.0, 0.0],
            ],
        );
        let graph = graph_value(
            source["revision_id"].as_str().unwrap(),
            source["revision_sha256"].as_str().unwrap(),
            &spine,
        );
        let structural = runtime
            .knife_curve_modifier_graph_prepare(&structural_request(
                &project_id,
                candidate_state_sha256,
                source,
                &spine,
                &edge,
                &graph,
            ))
            .expect("structural prepare");
        let plan_without_hash = json!({
            "schema_version": PLAN_SCHEMA,
            "evaluation_id": "knife-evaluation",
            "spine_curve_id": spine["curve_id"],
            "spine_curve_sha256": spine["canonical_sha256"],
            "edge_curve_id": edge["curve_id"],
            "edge_curve_sha256": edge["canonical_sha256"],
            "station_count": STATION_COUNT,
            "thickness_axis": "local_normal",
            "thickness_m": 0.06,
            "root_cap": true,
            "tip_cap": true,
            "stable_triangulation": PLAN_TRIANGULATION,
            "stable_lineage_policy": PLAN_LINEAGE,
            "canonical_sha256": "",
        });
        let mut plan = plan_without_hash;
        plan["canonical_sha256"] =
            Value::String(canonical_hash_without(&plan, "canonical_sha256").expect("plan hash"));
        let mut request = json!({
            "schema_version": PREPARE_SCHEMA,
            "operation": PREPARE_OPERATION,
            "project_id": project_id,
            "source_candidate_id": "knife-source-candidate",
            "source_candidate_state_sha256": candidate_state_sha256,
            "source_authoring_mesh_id": source["mesh_id"],
            "source_authoring_mesh_lineage_id": source["lineage_id"],
            "source_authoring_mesh_revision_id": source["revision_id"],
            "source_authoring_mesh_revision_index": source["revision_index"],
            "source_authoring_mesh_revision_sha256": source["revision_sha256"],
            "source_authoring_mesh_identity_sha256": source["identity_sha256"],
            "source_modifier_graph_id": "knife-curve-graph",
            "source_modifier_graph_sha256": structural["modifier_graph_semantic_sha256"],
            "curve_set_semantic_sha256": structural["curve_set_semantic_sha256"],
            "sample_set_semantic_sha256": structural["sample_set_semantic_sha256"],
            "modifier_graph_semantic_sha256": structural["modifier_graph_semantic_sha256"],
            "dependency_graph_semantic_sha256": structural["dependency_graph_semantic_sha256"],
            "recompute_plan_semantic_sha256": structural["recompute_plan_semantic_sha256"],
            "evaluation_plan": plan,
            "idempotency_key": "knife-evaluation",
            "max_response_bytes": TEST_MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": "",
        });
        request["input_sha256"] = Value::String(
            canonical_hash_without(&request, "input_sha256").expect("evaluation request hash"),
        );
        (project_id, request, structural)
    }

    fn get_request_from_prepare(result: &Value) -> Value {
        let mut request = json!({
            "schema_version": GET_SCHEMA,
            "operation": GET_OPERATION,
            "project_id": result["project_id"],
            "source_candidate_id": result["source_candidate_id"],
            "source_candidate_state_sha256": result["source_candidate_state_sha256"],
            "source_authoring_mesh_id": result["source_authoring_mesh_id"],
            "source_authoring_mesh_lineage_id": result["source_authoring_mesh_lineage_id"],
            "source_authoring_mesh_revision_id": result["source_authoring_mesh_revision_id"],
            "source_authoring_mesh_revision_index": result["source_authoring_mesh_revision_index"],
            "source_authoring_mesh_revision_sha256": result["source_authoring_mesh_revision_sha256"],
            "source_authoring_mesh_identity_sha256": result["source_authoring_mesh_identity_sha256"],
            "source_modifier_graph_id": result["source_modifier_graph_id"],
            "source_modifier_graph_sha256": result["source_modifier_graph_sha256"],
            "curve_set_semantic_sha256": result["curve_set_semantic_sha256"],
            "sample_set_semantic_sha256": result["sample_set_semantic_sha256"],
            "modifier_graph_semantic_sha256": result["modifier_graph_semantic_sha256"],
            "dependency_graph_semantic_sha256": result["dependency_graph_semantic_sha256"],
            "recompute_plan_semantic_sha256": result["recompute_plan_semantic_sha256"],
            "evaluated_mesh_lookup_key_sha256": result["evaluated_mesh_lookup_key_sha256"],
            "evaluation_id": result["evaluation_plan"]["evaluation_id"],
            "evaluation_plan_object_sha256": result["evaluation_plan_object_sha256"],
            "evaluation_plan_semantic_sha256": result["evaluation_plan_semantic_sha256"],
            "evaluated_mesh_id": result["evaluated_mesh_id"],
            "evaluated_mesh_object_sha256": result["evaluated_mesh_object_sha256"],
            "evaluated_mesh_semantic_sha256": result["evaluated_mesh_semantic_sha256"],
            "evaluated_mesh_identity_sha256": result["evaluated_mesh_identity_sha256"],
            "evaluated_mesh_link_sha256": result["evaluated_mesh_link_sha256"],
            "vertex_count": result["vertex_count"],
            "triangle_count": result["triangle_count"],
            "idempotency_key": result["idempotency_key"],
            "max_response_bytes": TEST_MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": "",
        });
        request["input_sha256"] = Value::String(
            canonical_hash_without(&request, "input_sha256").expect("get request hash"),
        );
        request
    }

    fn cas_file_count(runtime: &Runtime) -> usize {
        runtime
            .store
            .cas()
            .list_objects()
            .expect("CAS listing")
            .len()
    }

    fn file_backed_paths(label: &str) -> (PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("forgecad-runtime-{label}-{}", Uuid::new_v4()));
        (root.join("runtime.sqlite"), root.join("cas"))
    }

    #[test]
    fn plan_is_closed_and_excludes_only_its_canonical_field() {
        let plan = json!({
            "schema_version": PLAN_SCHEMA,
            "evaluation_id": "evaluation-1",
            "spine_curve_id": "spine",
            "spine_curve_sha256": "a".repeat(64),
            "edge_curve_id": "edge",
            "edge_curve_sha256": "b".repeat(64),
            "station_count": STATION_COUNT,
            "thickness_axis": "local_normal",
            "thickness_m": 0.012,
            "root_cap": true,
            "tip_cap": true,
            "stable_triangulation": PLAN_TRIANGULATION,
            "stable_lineage_policy": PLAN_LINEAGE,
            "canonical_sha256": "",
        });
        let expected = canonical_hash_without(&plan, "canonical_sha256").unwrap();
        let mut plan = plan;
        plan["canonical_sha256"] = Value::String(expected.clone());
        let (_, semantic) = parse_plan(&plan).unwrap();
        assert_eq!(semantic, expected);
    }

    #[test]
    fn public_prepare_replays_exactly_and_rejects_same_key_conflict() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let (_, request, _) = evaluation_fixture(&runtime);
        let first = runtime
            .knife_curve_evaluated_mesh_prepare(&request)
            .expect("first prepare");
        assert_eq!(first["status"], "prepared");
        assert_eq!(first["replayed"], false);
        assert_eq!(first["runtime_write_performed"], true);
        assert_eq!(first["evaluated_mesh_created"], true);
        assert_eq!(first["geometry_artifact_created"], false);

        let replay = runtime
            .knife_curve_evaluated_mesh_prepare(&request)
            .expect("exact replay");
        assert_eq!(replay["status"], "replayed");
        assert_eq!(replay["replayed"], true);
        assert_eq!(replay["runtime_write_performed"], false);
        assert_eq!(
            replay["evaluated_mesh_lookup_key_sha256"],
            first["evaluated_mesh_lookup_key_sha256"]
        );
        assert_eq!(
            replay["evaluated_mesh_semantic_sha256"],
            first["evaluated_mesh_semantic_sha256"]
        );

        let mut conflict = request.clone();
        conflict["evaluation_plan"]["thickness_m"] = json!(0.061);
        let conflict_plan_hash =
            canonical_hash_without(&conflict["evaluation_plan"], "canonical_sha256")
                .expect("conflict plan hash");
        conflict["evaluation_plan"]["canonical_sha256"] = Value::String(conflict_plan_hash);
        conflict["input_sha256"] = Value::String(
            canonical_hash_without(&conflict, "input_sha256").expect("conflict request hash"),
        );
        let error = runtime
            .knife_curve_evaluated_mesh_prepare(&conflict)
            .expect_err("same-key conflict");
        assert!(
            error.to_string().contains("IDEMPOTENCY_CONFLICT"),
            "{error}"
        );
        let stored = runtime
            .store
            .get_knife_curve_evaluated_mesh(
                first["project_id"].as_str().unwrap(),
                first["evaluated_mesh_lookup_key_sha256"].as_str().unwrap(),
            )
            .expect("stored row")
            .expect("durable row");
        assert_eq!(
            stored.evaluated_mesh_semantic_sha256,
            first["evaluated_mesh_semantic_sha256"]
        );
    }

    #[test]
    fn public_prepare_rejects_tampered_plan_without_new_row_or_cas_root() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let (_, request, _) = evaluation_fixture(&runtime);
        let first = runtime
            .knife_curve_evaluated_mesh_prepare(&request)
            .expect("first prepare");
        let before_cas = cas_file_count(&runtime);
        let before = runtime
            .store
            .get_knife_curve_evaluated_mesh(
                first["project_id"].as_str().unwrap(),
                first["evaluated_mesh_lookup_key_sha256"].as_str().unwrap(),
            )
            .expect("row lookup")
            .expect("row");

        let mut malformed = request.clone();
        malformed["evaluation_plan"]["station_count"] = json!(31);
        malformed["evaluation_plan"]["canonical_sha256"] = Value::String(
            canonical_hash_without(&malformed["evaluation_plan"], "canonical_sha256")
                .expect("malformed plan hash"),
        );
        malformed["input_sha256"] = Value::String(
            canonical_hash_without(&malformed, "input_sha256").expect("malformed request hash"),
        );
        let error = runtime
            .knife_curve_evaluated_mesh_prepare(&malformed)
            .expect_err("malformed plan");
        assert!(error.to_string().contains("station_count"), "{error}");
        assert_eq!(cas_file_count(&runtime), before_cas);
        let after = runtime
            .store
            .get_knife_curve_evaluated_mesh(
                first["project_id"].as_str().unwrap(),
                first["evaluated_mesh_lookup_key_sha256"].as_str().unwrap(),
            )
            .expect("row lookup")
            .expect("row");
        assert_eq!(after, before);
    }

    #[test]
    fn public_get_reloads_structural_parent_after_file_backed_runtime_reopen() {
        let (database, cas) = file_backed_paths("knife-evaluated-get");
        let request = {
            let runtime = Runtime::open_with_cas(&database, &cas).expect("runtime");
            let fixture = evaluation_fixture(&runtime);
            let prepared = runtime
                .knife_curve_evaluated_mesh_prepare(&fixture.1)
                .expect("prepare");
            let get_request = get_request_from_prepare(&prepared);
            assert_eq!(
                runtime
                    .knife_curve_evaluated_mesh_get(&get_request)
                    .expect("pre-reopen get")["status"],
                "found"
            );
            get_request
        };
        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopened runtime");
        let result = reopened
            .knife_curve_evaluated_mesh_get(&request)
            .expect("reopened get");
        assert_eq!(result["status"], "found");
        assert_eq!(result["runtime_write_performed"], false);
        assert_eq!(result["persistent_user_data_touched"], false);
        assert_eq!(result["restart_hash_verified"], false);
        assert_eq!(result["geometry_artifact_created"], false);
        assert_eq!(
            result["evaluated_mesh_lookup_key_sha256"],
            request["evaluated_mesh_lookup_key_sha256"]
        );
        assert_eq!(
            result["evaluated_mesh_semantic_sha256"],
            request["evaluated_mesh_semantic_sha256"]
        );
        assert_eq!(result["vertex_count"], request["vertex_count"]);
        assert_eq!(result["triangle_count"], request["triangle_count"]);
        assert_eq!(result["high_status"], "NOT_RUN");
        assert_eq!(result["uv_status"], "NOT_RUN");
        assert_eq!(result["bake_status"], "NOT_RUN");
        assert_eq!(result["visual_status"], "NOT_RUN");
        assert_eq!(result["human_status"], "NOT_RUN");
        assert_eq!(result["engine_status"], "NOT_RUN");
        drop(reopened);
        let _ = fs::remove_dir_all(database.parent().expect("test root"));
    }
}
