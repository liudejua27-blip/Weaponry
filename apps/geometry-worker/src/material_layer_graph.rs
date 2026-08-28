//! Closed, deterministic execution planning for commercial material authoring.
//!
//! This is deliberately not a shader language. It validates a fixed set of
//! data-only nodes, rejects unknown fields and executable payloads, proves the
//! graph is acyclic, and emits a deterministic topological execution plan.
//! Texture evaluation/lowering remains a separate bounded Worker stage.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const SCHEMA_VERSION: &str = "MaterialLayerGraph@1";
pub const MAX_NODES: usize = 256;
pub const MAX_INPUTS_PER_NODE: usize = 4;
pub const MAX_OUTPUTS: usize = 12;

const GRAPH_KEYS: &[&str] = &[
    "schema_version",
    "graph_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "low_artifact_sha256",
    "hero_uv_layout_sha256",
    "bake_set_sha256",
    "material_pack_manifest_sha256",
    "nodes",
    "budget",
    "canonical_sha256",
];

const COMMON_NODE_KEYS: &[&str] = &["node_id", "kind", "inputs", "ownership"];
const NODE_KINDS: &[&str] = &[
    "Source",
    "Constant",
    "Anchor",
    "Generator",
    "Mask",
    "Filter",
    "Transform",
    "Blend",
    "NormalCombine",
    "RoughnessRemap",
    "Decal",
    "Trim",
    "ChannelPack",
    "Output",
];
const OUTPUT_CHANNELS: &[&str] = &[
    "base_color",
    "normal",
    "metallic_roughness",
    "ao",
    "emissive",
    "clearcoat",
    "clearcoat_roughness",
    "object_id",
    "material_id",
    "part_id",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialLayerExecutionNode {
    pub node_id: String,
    pub kind: String,
    pub input_node_ids: Vec<String>,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialLayerExecutionPlan {
    pub graph_id: String,
    pub graph_canonical_sha256: String,
    pub ordered_nodes: Vec<MaterialLayerExecutionNode>,
    pub output_nodes: BTreeMap<String, String>,
    pub max_resolution: u64,
    pub max_output_bytes: u64,
    pub max_runtime_ms: u64,
    pub promotion_eligible: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum MaterialLayerGraphError {
    #[error("MaterialLayerGraph is invalid: {0}")]
    Invalid(String),
}

pub fn compile_material_layer_graph_plan(
    value: &Value,
) -> Result<MaterialLayerExecutionPlan, MaterialLayerGraphError> {
    let graph = object(value, "graph")?;
    exact_keys(graph, GRAPH_KEYS, "graph")?;
    if text(graph, "schema_version")? != SCHEMA_VERSION {
        return invalid("schema_version must be MaterialLayerGraph@1");
    }
    let graph_id = id(text(graph, "graph_id")?, "graph_id")?.to_owned();
    for field in ["project_id", "candidate_id"] {
        id(text(graph, field)?, field)?;
    }
    for field in [
        "candidate_state_sha256",
        "low_artifact_sha256",
        "hero_uv_layout_sha256",
        "bake_set_sha256",
        "material_pack_manifest_sha256",
    ] {
        sha256(text(graph, field)?, field)?;
    }
    let declared_hash = sha256(text(graph, "canonical_sha256")?, "canonical_sha256")?;
    let mut preimage = graph.clone();
    preimage.remove("canonical_sha256");
    let actual_hash = canonical_hash(&Value::Object(preimage));
    if declared_hash != actual_hash {
        return invalid("canonical_sha256 does not match the closed graph payload");
    }

    let (max_resolution, max_output_bytes, max_runtime_ms) = validate_budget(graph)?;
    let nodes = graph
        .get("nodes")
        .and_then(Value::as_array)
        .filter(|nodes| !nodes.is_empty() && nodes.len() <= MAX_NODES)
        .ok_or_else(|| error("nodes must contain 1..=256 entries"))?;

    let mut node_values = BTreeMap::<String, &Map<String, Value>>::new();
    let mut inputs_by_id = BTreeMap::<String, Vec<String>>::new();
    let mut node_hashes = BTreeMap::<String, String>::new();
    let mut output_nodes = BTreeMap::<String, String>::new();
    for node_value in nodes {
        let node = object(node_value, "node")?;
        let node_id = id(text(node, "node_id")?, "node_id")?.to_owned();
        if node_values.insert(node_id.clone(), node).is_some() {
            return invalid("node_id values must be unique");
        }
        let kind = text(node, "kind")?;
        if !NODE_KINDS.contains(&kind) {
            return invalid("node kind is outside the closed allowlist");
        }
        let inputs = validate_node(node, kind)?;
        if kind == "Output" {
            let channel = text(node, "channel")?.to_owned();
            if output_nodes.insert(channel, node_id.clone()).is_some() {
                return invalid("each output channel may be authored only once");
            }
        }
        inputs_by_id.insert(node_id.clone(), inputs);
        node_hashes.insert(node_id, canonical_hash(node_value));
    }
    if output_nodes.is_empty() || output_nodes.len() > MAX_OUTPUTS {
        return invalid("graph requires 1..=12 unique Output nodes");
    }

    let ordered_ids = topological_order(&inputs_by_id)?;
    let ordered_nodes = ordered_ids
        .into_iter()
        .map(|node_id| {
            let node = node_values[&node_id];
            MaterialLayerExecutionNode {
                kind: text(node, "kind").expect("validated kind").to_owned(),
                input_node_ids: inputs_by_id[&node_id].clone(),
                canonical_sha256: node_hashes[&node_id].clone(),
                node_id,
            }
        })
        .collect();

    Ok(MaterialLayerExecutionPlan {
        graph_id,
        graph_canonical_sha256: actual_hash,
        ordered_nodes,
        output_nodes,
        max_resolution,
        max_output_bytes,
        max_runtime_ms,
        // A compiled source plan is not a baked texture set, engine receipt,
        // or human material review.
        promotion_eligible: false,
    })
}

pub fn compile_material_layer_graph_result(
    value: &Value,
) -> Result<Value, MaterialLayerGraphError> {
    let plan = compile_material_layer_graph_plan(value)?;
    let ordered_nodes = plan
        .ordered_nodes
        .iter()
        .map(|node| {
            serde_json::json!({
                "node_id": node.node_id,
                "kind": node.kind,
                "input_node_ids": node.input_node_ids,
                "canonical_sha256": node.canonical_sha256,
            })
        })
        .collect::<Vec<_>>();
    let mut result = serde_json::json!({
        "schema_version":"MaterialLayerGraphPlanResult@1",
        "graph_id":plan.graph_id,
        "graph_canonical_sha256":plan.graph_canonical_sha256,
        "ordered_nodes":ordered_nodes,
        "output_nodes":plan.output_nodes,
        "budget":{
            "max_resolution":plan.max_resolution,
            "max_output_bytes":plan.max_output_bytes,
            "max_runtime_ms":plan.max_runtime_ms,
        },
        "execution_status":"VALIDATED_PLAN_NOT_EVALUATED",
        "promotion_eligible":plan.promotion_eligible,
        "runtime_write_performed":false,
        "canonical_sha256":"",
    });
    let canonical = canonical_hash(&without_canonical_hash(&result));
    result["canonical_sha256"] = Value::String(canonical);
    Ok(result)
}

fn validate_budget(graph: &Map<String, Value>) -> Result<(u64, u64, u64), MaterialLayerGraphError> {
    let budget = graph
        .get("budget")
        .and_then(Value::as_object)
        .ok_or_else(|| error("budget is required"))?;
    exact_keys(
        budget,
        &[
            "max_resolution",
            "max_nodes",
            "max_output_textures",
            "max_output_bytes",
            "max_runtime_ms",
        ],
        "budget",
    )?;
    let resolution = integer(budget, "max_resolution", 2048, 4096)?;
    if resolution != 2048 && resolution != 4096 {
        return invalid("max_resolution must be 2048 or 4096");
    }
    if integer(budget, "max_nodes", 1, MAX_NODES as u64)? != MAX_NODES as u64 {
        return invalid("max_nodes must use the fixed 256-node product budget");
    }
    integer(budget, "max_output_textures", 1, MAX_OUTPUTS as u64)?;
    let max_output_bytes = integer(budget, "max_output_bytes", 1, 512 * 1024 * 1024)?;
    let max_runtime_ms = integer(budget, "max_runtime_ms", 1, 120_000)?;
    Ok((resolution, max_output_bytes, max_runtime_ms))
}

fn validate_node(
    node: &Map<String, Value>,
    kind: &str,
) -> Result<Vec<String>, MaterialLayerGraphError> {
    let extra = match kind {
        "Source" => &["source_channel", "source_object_sha256", "data_class"][..],
        "Constant" => &["value", "data_class", "color_space"][..],
        "Anchor" => &["anchor_kind", "intensity", "evidence_sha256"][..],
        "Generator" => &[
            "generator_kind",
            "seed",
            "domain",
            "scale",
            "max_samples",
            "provenance_sha256",
        ][..],
        "Mask" => &["combine", "threshold", "feather"][..],
        "Filter" => &["filter_kind", "radius", "amount"][..],
        "Transform" => &["scale", "offset", "rotation_degrees"][..],
        "Blend" => &["blend_mode", "opacity"][..],
        "NormalCombine" => &["method", "strength"][..],
        "RoughnessRemap" => &["input_range", "output_range"][..],
        "Decal" => &["decal_object_sha256", "opacity", "color_space"][..],
        "Trim" => &["trim_id", "uv_range"][..],
        "ChannelPack" => &["layout", "data_class"][..],
        "Output" => &["channel", "data_class", "color_space"][..],
        _ => return invalid("unknown node kind"),
    };
    let allowed = COMMON_NODE_KEYS
        .iter()
        .copied()
        .chain(extra.iter().copied())
        .collect::<Vec<_>>();
    exact_keys(node, &allowed, "node")?;
    id(text(node, "node_id")?, "node_id")?;
    validate_ownership(node.get("ownership"))?;
    let inputs = node
        .get("inputs")
        .and_then(Value::as_array)
        .ok_or_else(|| error("node inputs must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| error("node input must be an id"))
                .and_then(|value| id(value, "input").map(str::to_owned))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if inputs.len() > MAX_INPUTS_PER_NODE
        || inputs.iter().collect::<BTreeSet<_>>().len() != inputs.len()
    {
        return invalid("node inputs exceed the fixed limit or contain duplicates");
    }
    validate_kind_fields(node, kind, inputs.len())?;
    Ok(inputs)
}

fn validate_kind_fields(
    node: &Map<String, Value>,
    kind: &str,
    input_count: usize,
) -> Result<(), MaterialLayerGraphError> {
    let exact_inputs = |expected: usize| {
        if input_count == expected {
            Ok(())
        } else {
            invalid("node input arity is invalid for its kind")
        }
    };
    match kind {
        "Source" => {
            exact_inputs(0)?;
            one_of(
                text(node, "source_channel")?,
                OUTPUT_CHANNELS,
                "source_channel",
            )?;
            sha256(text(node, "source_object_sha256")?, "source_object_sha256")?;
            data_class(node)?;
        }
        "Constant" => {
            exact_inputs(0)?;
            vec_numbers(node, "value", 1, 4, -65_504.0, 65_504.0)?;
            data_class(node)?;
            color_space(node)?;
        }
        "Anchor" => {
            exact_inputs(0)?;
            one_of(
                text(node, "anchor_kind")?,
                &["contact", "friction", "heat", "maintenance", "art_directed"],
                "anchor_kind",
            )?;
            number(node, "intensity", 0.0, 1.0)?;
            sha256(text(node, "evidence_sha256")?, "evidence_sha256")?;
        }
        "Generator" => {
            if input_count > 1 {
                return invalid("Generator accepts zero or one input");
            }
            one_of(
                text(node, "generator_kind")?,
                &[
                    "edge_curvature",
                    "ao_cavity",
                    "directional_grain",
                    "macro_variation",
                    "microdetail",
                ],
                "generator_kind",
            )?;
            integer(node, "seed", 0, u32::MAX as u64)?;
            one_of(text(node, "domain")?, &["uv0", "object", "part"], "domain")?;
            number(node, "scale", 0.0001, 4096.0)?;
            integer(node, "max_samples", 1, 64)?;
            sha256(text(node, "provenance_sha256")?, "provenance_sha256")?;
        }
        "Mask" => {
            if !(1..=4).contains(&input_count) {
                return invalid("Mask requires 1..=4 inputs");
            }
            one_of(
                text(node, "combine")?,
                &["multiply", "min", "max", "invert"],
                "combine",
            )?;
            number(node, "threshold", 0.0, 1.0)?;
            number(node, "feather", 0.0, 1.0)?;
        }
        "Filter" => {
            exact_inputs(1)?;
            one_of(
                text(node, "filter_kind")?,
                &["levels", "blur", "sharpen", "dilate", "erode"],
                "filter_kind",
            )?;
            integer(node, "radius", 0, 32)?;
            number(node, "amount", 0.0, 4.0)?;
        }
        "Transform" => {
            exact_inputs(1)?;
            vec_numbers(node, "scale", 2, 2, 0.0001, 4096.0)?;
            vec_numbers(node, "offset", 2, 2, -16.0, 16.0)?;
            number(node, "rotation_degrees", -360.0, 360.0)?;
        }
        "Blend" => {
            if input_count != 2 && input_count != 3 {
                return invalid("Blend requires base/layer and an optional mask");
            }
            one_of(
                text(node, "blend_mode")?,
                &[
                    "normal", "multiply", "add", "screen", "overlay", "max", "min",
                ],
                "blend_mode",
            )?;
            number(node, "opacity", 0.0, 1.0)?;
        }
        "NormalCombine" => {
            exact_inputs(2)?;
            one_of(text(node, "method")?, &["reoriented", "whiteout"], "method")?;
            number(node, "strength", 0.0, 2.0)?;
        }
        "RoughnessRemap" => {
            exact_inputs(1)?;
            let input = vec_numbers(node, "input_range", 2, 2, 0.0, 1.0)?;
            let output = vec_numbers(node, "output_range", 2, 2, 0.0, 1.0)?;
            if input[0] >= input[1] || output[0] > output[1] {
                return invalid("roughness ranges must be ordered");
            }
        }
        "Decal" => {
            if input_count > 1 {
                return invalid("Decal accepts zero or one mask input");
            }
            sha256(text(node, "decal_object_sha256")?, "decal_object_sha256")?;
            number(node, "opacity", 0.0, 1.0)?;
            color_space(node)?;
        }
        "Trim" => {
            exact_inputs(1)?;
            id(text(node, "trim_id")?, "trim_id")?;
            vec_numbers(node, "uv_range", 4, 4, 0.0, 1.0)?;
        }
        "ChannelPack" => {
            if !(2..=4).contains(&input_count) {
                return invalid("ChannelPack requires 2..=4 inputs");
            }
            one_of(text(node, "layout")?, &["orm", "rma", "rgba"], "layout")?;
            if text(node, "data_class")? != "data" {
                return invalid("ChannelPack is always data, never color");
            }
        }
        "Output" => {
            exact_inputs(1)?;
            let channel = one_of(text(node, "channel")?, OUTPUT_CHANNELS, "channel")?;
            let class = data_class(node)?;
            let space = color_space(node)?;
            let color_output = channel == "base_color" || channel == "emissive";
            if color_output && class != "color" || !color_output && class != "data" {
                return invalid("Output data_class does not match channel semantics");
            }
            if color_output && space != "srgb" || !color_output && space != "data" {
                return invalid("Output color_space does not match channel semantics");
            }
        }
        _ => return invalid("unknown node kind"),
    }
    Ok(())
}

fn validate_ownership(value: Option<&Value>) -> Result<(), MaterialLayerGraphError> {
    let ownership = value
        .and_then(Value::as_object)
        .ok_or_else(|| error("ownership is required"))?;
    exact_keys(ownership, &["part_ids", "material_zone_ids"], "ownership")?;
    for key in ["part_ids", "material_zone_ids"] {
        let values = ownership
            .get(key)
            .and_then(Value::as_array)
            .filter(|values| !values.is_empty() && values.len() <= 512)
            .ok_or_else(|| error("ownership lists must contain 1..=512 ids"))?;
        let mut unique = BTreeSet::new();
        for value in values {
            let value = id(
                value
                    .as_str()
                    .ok_or_else(|| error("ownership id is invalid"))?,
                key,
            )?;
            if !unique.insert(value) {
                return invalid("ownership ids must be unique");
            }
        }
    }
    Ok(())
}

fn topological_order(
    inputs_by_id: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<String>, MaterialLayerGraphError> {
    let mut indegree = inputs_by_id
        .iter()
        .map(|(id, inputs)| (id.clone(), inputs.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<String, Vec<String>>::new();
    for (node_id, inputs) in inputs_by_id {
        for input in inputs {
            if input == node_id || !inputs_by_id.contains_key(input) {
                return invalid("node input is missing or self-referential");
            }
            dependents
                .entry(input.clone())
                .or_default()
                .push(node_id.clone());
        }
    }
    for values in dependents.values_mut() {
        values.sort();
    }
    let mut queue = indegree
        .iter()
        .filter_map(|(id, degree)| (*degree == 0).then_some(id.clone()))
        .collect::<VecDeque<_>>();
    let mut ordered = Vec::with_capacity(inputs_by_id.len());
    while let Some(node_id) = queue.pop_front() {
        ordered.push(node_id.clone());
        if let Some(children) = dependents.get(&node_id) {
            for child in children {
                let degree = indegree.get_mut(child).expect("known dependent");
                *degree -= 1;
                if *degree == 0 {
                    queue.push_back(child.clone());
                }
            }
        }
    }
    if ordered.len() != inputs_by_id.len() {
        return invalid("material graph contains a cycle");
    }
    Ok(ordered)
}

fn exact_keys(
    value: &Map<String, Value>,
    allowed: &[&str],
    label: &str,
) -> Result<(), MaterialLayerGraphError> {
    if value.len() != allowed.len() || value.keys().any(|key| !allowed.contains(&key.as_str())) {
        return invalid(&format!("{label} field set is not closed"));
    }
    Ok(())
}

fn object<'a>(
    value: &'a Value,
    label: &str,
) -> Result<&'a Map<String, Value>, MaterialLayerGraphError> {
    value
        .as_object()
        .ok_or_else(|| error(&format!("{label} must be an object")))
}

fn text<'a>(value: &'a Map<String, Value>, key: &str) -> Result<&'a str, MaterialLayerGraphError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| error(&format!("{key} must be text")))
}

fn id<'a>(value: &'a str, label: &str) -> Result<&'a str, MaterialLayerGraphError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
    {
        return invalid(&format!("{label} is not a bounded identifier"));
    }
    Ok(value)
}

fn sha256<'a>(value: &'a str, label: &str) -> Result<&'a str, MaterialLayerGraphError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return invalid(&format!("{label} must be lowercase SHA-256"));
    }
    Ok(value)
}

fn one_of<'a>(
    value: &'a str,
    values: &[&str],
    label: &str,
) -> Result<&'a str, MaterialLayerGraphError> {
    if !values.contains(&value) {
        return invalid(&format!("{label} is outside the closed allowlist"));
    }
    Ok(value)
}

fn integer(
    value: &Map<String, Value>,
    key: &str,
    min: u64,
    max: u64,
) -> Result<u64, MaterialLayerGraphError> {
    let number = value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| error(&format!("{key} must be an integer")))?;
    if !(min..=max).contains(&number) {
        return invalid(&format!("{key} is outside the bounded domain"));
    }
    Ok(number)
}

fn number(
    value: &Map<String, Value>,
    key: &str,
    min: f64,
    max: f64,
) -> Result<f64, MaterialLayerGraphError> {
    let number = value
        .get(key)
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
        .ok_or_else(|| error(&format!("{key} must be finite")))?;
    if !(min..=max).contains(&number) {
        return invalid(&format!("{key} is outside the bounded domain"));
    }
    Ok(number)
}

fn vec_numbers(
    value: &Map<String, Value>,
    key: &str,
    min_len: usize,
    max_len: usize,
    min: f64,
    max: f64,
) -> Result<Vec<f64>, MaterialLayerGraphError> {
    let values = value
        .get(key)
        .and_then(Value::as_array)
        .filter(|values| (min_len..=max_len).contains(&values.len()))
        .ok_or_else(|| error(&format!("{key} has invalid length")))?;
    values
        .iter()
        .map(|value| {
            value
                .as_f64()
                .filter(|number| number.is_finite() && (min..=max).contains(number))
                .ok_or_else(|| error(&format!("{key} contains an invalid number")))
        })
        .collect()
}

fn data_class<'a>(node: &'a Map<String, Value>) -> Result<&'a str, MaterialLayerGraphError> {
    one_of(text(node, "data_class")?, &["color", "data"], "data_class")
}

fn color_space<'a>(node: &'a Map<String, Value>) -> Result<&'a str, MaterialLayerGraphError> {
    one_of(
        text(node, "color_space")?,
        &["srgb", "linear", "data"],
        "color_space",
    )
}

fn canonical_hash(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).expect("JSON value is serializable");
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn without_canonical_hash(value: &Value) -> Value {
    let mut value = value.clone();
    if let Some(object) = value.as_object_mut() {
        object.remove("canonical_sha256");
    }
    value
}

fn error(message: &str) -> MaterialLayerGraphError {
    MaterialLayerGraphError::Invalid(message.to_owned())
}

fn invalid<T>(message: &str) -> Result<T, MaterialLayerGraphError> {
    Err(error(message))
}
