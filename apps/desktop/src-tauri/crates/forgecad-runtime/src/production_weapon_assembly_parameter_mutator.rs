//! Runtime-owned, closed aggregate parameter mutation for the FPS weapon form.
//!
//! This is deliberately smaller than a GeometryProgram editor.  It accepts a
//! canonical, already-authorized `GeometryProgram@2` value in memory and
//! changes only eight product-owned semantic parameters.  There is no JSON
//! pointer, expression, script, path, Worker, CAS, candidate, or Store path in
//! this module.  The returned value is a GeometryProgram draft: its canonical
//! field is removed so the normal hash/prepare boundary remains responsible
//! for re-hashing it.  The two muzzle envelope ratios scale local station and
//! Y/Z section coordinates together while leaving `position_m` unchanged.

use super::{canonical_json_hash, is_opaque_id, is_sha256, RuntimeError};
use forgecad_worker_protocol::operator_catalog_sha256;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const GEOMETRY_PROGRAM_SCHEMA_VERSION: &str = "GeometryProgram@2";
const LONGITUDINAL_SECTION_LOFT_OPERATOR: &str = "forgecad.geometry.longitudinal-section-loft@1";
const PRIMITIVE_OPERATOR: &str = "forgecad.geometry.primitive@2";
const PROFILE_EXTRUDE_OPERATOR: &str = "forgecad.geometry.profile-extrude@1";
const PROFILE_LOFT_V2_OPERATOR: &str = "forgecad.geometry.profile-loft@2";
const AUTHORING_MESH_OPERATOR: &str = "forgecad.geometry.authoring-mesh@1";

const RECEIVER_PART_IDS: [&str; 3] = ["receiver-main", "receiver-upper", "receiver-lower"];
const MUZZLE_SHROUD_PART_ID: &str = "muzzle-shroud";
const MUZZLE_EMITTER_PART_ID: &str = "muzzle-emitter";
const MUZZLE_CORE_PART_ID: &str = "muzzle-core";
const STOCK_PART_ID: &str = "rear-stock";
const STOCK_UPPER_NODE_ID: &str = "rear-stock";
const STOCK_LOWER_NODE_ID: &str = "rear-stock-lower-beam";
const STOCK_UPPER_DIAGNOSTIC_PART_ID: &str = "rear-stock-upper-diagnostic";
const STOCK_LOWER_DIAGNOSTIC_PART_ID: &str = "rear-stock-lower-diagnostic";
const TRIGGER_GUARD_PART_ID: &str = "trigger-guard";
const TRIGGER_GUARD_NODE_ID: &str = "trigger-guard";
const TRIGGER_GUARD_APERTURE_PROFILE_ID: &str = "trigger-guard-side-aperture-xy@1";
const REAR_STOCK_OWNER_VOID_HALF_Y_FLAT_Z_PROFILE_ID: &str =
    "registered-boundary-bridge-half-y-flat-z-owner-void@1";
const REAR_STOCK_BRIDGE_STATION_RATIOS: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
const REAR_STOCK_BRIDGE_CURRENT_Y_OFFSETS_M: [f64; 5] = [0.0, -0.003, -0.0045, -0.003, 0.0];
const REAR_STOCK_BRIDGE_TARGET_Y_OFFSETS_M: [f64; 5] = [0.0, -0.006, -0.009, -0.006, 0.0];
const REAR_STOCK_BRIDGE_HALF_DEPTH_M: f64 = 0.43;

const SUPPORTED_PARAMETER_IDS: [&str; 8] = [
    "receiver-envelope-width",
    "receiver-envelope-height",
    "receiver-envelope-shoulder",
    "muzzle-axis-shroud-envelope",
    "muzzle-axis-emitter-envelope",
    "muzzle-axis-core-aperture",
    "stock-open-frame-clearance",
    "stock-open-frame-angle",
];

const UNSUPPORTED_PARAMETER_IDS: [&str; 4] = [
    "trigger-void-clearance",
    "trigger-void-centroid",
    "rail-spine-continuity",
    "rail-spine-offset",
];

const RATIO_MIN: f64 = 0.8;
const RATIO_MAX: f64 = 1.2;
const SHOULDER_DELTA_MIN_M: f64 = -0.12;
const SHOULDER_DELTA_MAX_M: f64 = 0.12;
const STOCK_CLEARANCE_MIN_M: f64 = 0.10;
const STOCK_CLEARANCE_MAX_M: f64 = 0.50;
const STOCK_ANGLE_MIN_RAD: f64 = -0.20;
const STOCK_ANGLE_MAX_RAD: f64 = 0.40;
const STOCK_PLANE_POSITION_DELTA_MIN_M: f64 = -0.20;
const STOCK_PLANE_POSITION_DELTA_MAX_M: f64 = 0.20;
const STOCK_PLANE_POSITION_SEPARATION_M: f64 = 0.03;
const STOCK_UPPER_INNER_SPAN_MIN_M: f64 = 0.70;
const STOCK_UPPER_INNER_SPAN_MAX_M: f64 = 0.95;
const STOCK_UPPER_PROFILE_VARIANTS_M: [f64; 2] = [0.85, 0.75];
const STOCK_UPPER_PROFILE_LIP_VARIANTS_M: [f64; 2] = [-0.035, -0.075];
const STOCK_UPPER_PROFILE_04V_LIP_VARIANTS_M: [f64; 2] = [-0.015, 0.005];
const STOCK_UPPER_PROFILE_04W_LIP_VARIANTS_M: [f64; 2] = [0.025, 0.045];
const STOCK_UPPER_PROFILE_04X_BOUNDARY_TRANSLATION_VARIANTS_M: [f64; 2] = [0.020, 0.040];
const STOCK_UPPER_PROFILE_04Z_STATION_DELTA_M: f64 = 0.020;
const STOCK_UPPER_PROFILE_SHOULDER_VARIANTS_M: [f64; 2] = [-0.085, -0.065];
const STOCK_UPPER_PROFILE_CAP_LIP_VARIANTS_M: [f64; 2] = [-0.405, -0.395];
const ABSOLUTE_ART_BOUND_M: f64 = 10.0;
const EPSILON: f64 = 1.0e-9;
const AXIS_ROTATION_TOLERANCE_RAD: f64 = 1.0e-4;

/// Apply one closed assembly parameter to an in-memory canonical program.
///
/// The caller supplies a ratio for width/height/envelope/radius parameters and
/// a metre delta for the receiver shoulder parameter.  All receiver lofts are
/// linked as one aggregate; each muzzle envelope owns exactly one PartOutput;
/// the aperture is bound only to the `muzzle-core` primitive radius.  The
/// function is pure with respect to Runtime state and returns a draft without
/// `canonical_sha256`.
pub(crate) fn production_weapon_assembly_parameter_mutate(
    program: &Value,
    parameter_id: &str,
    value: f64,
) -> Result<Value, RuntimeError> {
    mutate_with_expected_hash(program, None, parameter_id, value)
}

/// Variant for a GeometryProgram draft read from a persisted envelope that
/// intentionally omits `canonical_sha256`.  The caller must supply the
/// already-bound program hash; the mutator never invents that binding.
pub(crate) fn production_weapon_assembly_parameter_mutate_bound(
    program: &Value,
    expected_program_sha256: &str,
    parameter_id: &str,
    value: f64,
) -> Result<Value, RuntimeError> {
    if !is_sha256(expected_program_sha256) {
        return Err(invalid("ASSEMBLY_PARAMETER_EXPECTED_PROGRAM_HASH_INVALID"));
    }
    mutate_with_expected_hash(program, Some(expected_program_sha256), parameter_id, value)
}

/// Apply the diagnostic-only stock-plane position trial.
///
/// This is intentionally not a semantic parameter sink: it has no parameter
/// ID, descriptor, availability entry, or registry binding.  It only moves the
/// two already-resolved stock nodes together along X and returns a fully
/// rehashed GeometryProgram for an isolated trial.
pub(crate) fn production_weapon_stock_plane_position_trial_mutate(
    program: &Value,
    delta_x_m: f64,
) -> Result<Value, RuntimeError> {
    if !delta_x_m.is_finite() {
        return Err(invalid("ASSEMBLY_PARAMETER_STOCK_PLANE_DELTA_NONFINITE"));
    }
    if !(STOCK_PLANE_POSITION_DELTA_MIN_M..=STOCK_PLANE_POSITION_DELTA_MAX_M).contains(&delta_x_m) {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_PLANE_DELTA_OUT_OF_BOUNDS",
        ));
    }

    let index = ProgramIndex::parse_with_expected_hash(program, None)?;
    resolve_stock_open_frame_binding(&index, BindingKind::StockOpenFrameClearanceM)?;
    let upper_position = stock_vec3(&index, STOCK_UPPER_NODE_ID, "position_m")?;
    let lower_position = stock_vec3(&index, STOCK_LOWER_NODE_ID, "position_m")?;
    let separation_m = upper_position[0] - lower_position[0];
    if !separation_m.is_finite()
        || (separation_m - STOCK_PLANE_POSITION_SEPARATION_M).abs() > EPSILON
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_PLANE_POSITION_SEPARATION_INVALID",
        ));
    }

    let mut trial = program.clone();
    let nodes = trial
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_NODES_MISSING"))?;
    let mut node_positions = BTreeMap::new();
    for (position, node) in nodes.iter().enumerate() {
        let node_id = node
            .get("node_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_NODE_ID_INVALID"))?;
        node_positions.insert(node_id.to_owned(), position);
    }
    let upper_index = *node_positions
        .get(STOCK_UPPER_NODE_ID)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
    let lower_index = *node_positions
        .get(STOCK_LOWER_NODE_ID)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
    set_box_position_x(&mut nodes[upper_index], upper_position[0] + delta_x_m)?;
    set_box_position_x(&mut nodes[lower_index], lower_position[0] + delta_x_m)?;

    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&trial);
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical_sha256),
        );
    ProgramIndex::parse_with_expected_hash(&trial, None)?;
    Ok(trial)
}

/// Replace the D1 trigger-guard box with one fixed concave side-profile
/// extrusion. The opening lies in the weapon longitudinal/up plane (X/Y) and
/// is extruded only through depth (Z), which is the orientation required by
/// the reviewed left/right trigger-void contours. This is a product-owned
/// screen profile: callers cannot provide points, axes, depth, transforms or
/// raw GeometryProgram patches.
///
/// The helper deliberately remains outside the public parameter registry
/// until a real six-view proposal proves that the fixed aperture improves the
/// reviewed negative space without regressing the other views. It preserves
/// the stable node ID, PartOutput, material zone and every non-target node.
pub(crate) fn production_weapon_trigger_guard_aperture_trial_mutate(
    program: &Value,
) -> Result<Value, RuntimeError> {
    let index = ProgramIndex::parse_with_expected_hash(program, None)?;
    let sources = index
        .part_outputs
        .get(TRIGGER_GUARD_PART_ID)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_TRIGGER_GUARD_PART_UNAVAILABLE"))?;
    if sources.as_slice() != [TRIGGER_GUARD_NODE_ID] {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_TRIGGER_GUARD_NODE_BINDING_AMBIGUOUS",
        ));
    }
    let source = index
        .nodes
        .get(TRIGGER_GUARD_NODE_ID)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_TRIGGER_GUARD_NODE_MISSING"))?;
    if source.get("operator_id").and_then(Value::as_str) != Some(PRIMITIVE_OPERATOR) {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_TRIGGER_GUARD_OPERATOR_MISMATCH",
        ));
    }
    let parameters = source
        .get("parameters")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_TRIGGER_GUARD_PARAMETERS_INVALID"))?;
    if parameters.get("shape").and_then(Value::as_str) != Some("box") {
        return Err(invalid("ASSEMBLY_PARAMETER_TRIGGER_GUARD_SHAPE_INVALID"));
    }
    validate_primitive_box_parameters(parameters)?;
    let position = stock_vec3(&index, TRIGGER_GUARD_NODE_ID, "position_m")?;
    let rotation = stock_vec3(&index, TRIGGER_GUARD_NODE_ID, "rotation_rad")?;
    let size = stock_vec3(&index, TRIGGER_GUARD_NODE_ID, "size_m")?;
    if rotation
        .iter()
        .any(|value| value.abs() > AXIS_ROTATION_TOLERANCE_RAD)
        || size[0] < 0.20
        || size[0] > 0.80
        || size[1] < 0.08
        || size[1] > 0.40
        || size[2] < 0.10
        || size[2] > 1.00
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_TRIGGER_GUARD_SOURCE_ENVELOPE_INVALID",
        ));
    }

    let half_x = size[0] * 0.5;
    let half_y = size[1] * 0.5;
    let inner_half_x = half_x * 0.625;
    let inner_floor_y = -half_y * 0.20;
    let profile = serde_json::json!([
        [-half_x, half_y],
        [-inner_half_x, half_y],
        [-inner_half_x, inner_floor_y],
        [inner_half_x, inner_floor_y],
        [inner_half_x, half_y],
        [half_x, half_y],
        [half_x, -half_y],
        [-half_x, -half_y]
    ]);

    let mut trial = program.clone();
    let target = trial
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .and_then(|nodes| {
            nodes.iter_mut().find(|node| {
                node.get("node_id").and_then(Value::as_str) == Some(TRIGGER_GUARD_NODE_ID)
            })
        })
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_TRIGGER_GUARD_NODE_MISSING"))?;
    *target = serde_json::json!({
        "node_id":TRIGGER_GUARD_NODE_ID,
        "operator_id":PROFILE_EXTRUDE_OPERATOR,
        "inputs":[],
        "parameters":{
            "shape":"profile-extrude",
            "profile":profile,
            "depth_m":size[2],
            "position_m":position,
            "rotation_rad":rotation
        }
    });

    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&trial);
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical_sha256),
        );
    ProgramIndex::parse_with_expected_hash(&trial, None)?;

    let before_nodes = index.nodes;
    let after = ProgramIndex::parse_with_expected_hash(&trial, None)?;
    if before_nodes.keys().collect::<BTreeSet<_>>() != after.nodes.keys().collect::<BTreeSet<_>>()
        || index.part_outputs != after.part_outputs
        || before_nodes.iter().any(|(node_id, node)| {
            node_id != TRIGGER_GUARD_NODE_ID && after.nodes.get(node_id) != Some(node)
        })
        || after
            .nodes
            .get(TRIGGER_GUARD_NODE_ID)
            .and_then(|node| node.get("operator_id"))
            .and_then(Value::as_str)
            != Some(PROFILE_EXTRUDE_OPERATOR)
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_TRIGGER_GUARD_APERTURE_LINEAGE_CHANGED",
        ));
    }
    Ok(trial)
}

pub(crate) fn production_weapon_trigger_guard_aperture_profile_id() -> &'static str {
    TRIGGER_GUARD_APERTURE_PROFILE_ID
}

/// Execute the evidence-ranked 04BE-E owner-void repair over the exact 04BB
/// rear-stock authoring node carried by the cumulative 04BE-C candidate.
///
/// The profile is deliberately closed: callers provide no vertex IDs, points,
/// deltas, transforms or mesh payload. Runtime first proves the registered
/// five-station, flat-Z source profile, then moves only the six non-endpoint
/// inner-boundary vertices from the quarter-Y curve to the half-Y curve.
/// Endpoints, outer boundary, depth, topology, lower beam, rear cap, every
/// other Part and all PartOutputs remain byte-identical.
pub(crate) fn production_weapon_rear_stock_owner_void_half_y_flat_z_mutate(
    program: &Value,
) -> Result<Value, RuntimeError> {
    let before = ProgramIndex::parse_with_expected_hash(program, None)?;
    let sources = before
        .part_outputs
        .get(STOCK_PART_ID)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_REAR_STOCK_PART_UNAVAILABLE"))?;
    if sources.as_slice() != [STOCK_UPPER_NODE_ID, STOCK_LOWER_NODE_ID] {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_REAR_STOCK_NODE_BINDING_AMBIGUOUS",
        ));
    }
    let source = before
        .nodes
        .get(STOCK_UPPER_NODE_ID)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
    if source.get("operator_id").and_then(Value::as_str) != Some(AUTHORING_MESH_OPERATOR) {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_REAR_STOCK_AUTHORING_OPERATOR_MISMATCH",
        ));
    }
    let source_vertices = source
        .get("parameters")
        .and_then(Value::as_object)
        .and_then(|parameters| parameters.get("vertices"))
        .and_then(Value::as_array)
        .filter(|vertices| vertices.len() == 20)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_REAR_STOCK_BRIDGE_VERTICES_INVALID"))?;

    let mut inner = Vec::<(String, [f64; 3])>::new();
    for vertex in source_vertices {
        let element_id = vertex
            .get("element_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_REAR_STOCK_VERTEX_ID_INVALID"))?;
        let position = vertex
            .get("position_m")
            .and_then(Value::as_array)
            .filter(|values| values.len() == 3)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_REAR_STOCK_VERTEX_POSITION_INVALID"))?;
        let position = [
            position[0]
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_REAR_STOCK_VERTEX_POSITION_INVALID"))?,
            position[1]
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_REAR_STOCK_VERTEX_POSITION_INVALID"))?,
            position[2]
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_REAR_STOCK_VERTEX_POSITION_INVALID"))?,
        ];
        if position[1] < 0.0 {
            inner.push((element_id.to_owned(), position));
        }
    }
    if inner.len() != 10 {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_REAR_STOCK_INNER_BOUNDARY_COUNT_INVALID",
        ));
    }
    let min_x = inner
        .iter()
        .map(|(_, position)| position[0])
        .fold(f64::INFINITY, f64::min);
    let max_x = inner
        .iter()
        .map(|(_, position)| position[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let span_x = max_x - min_x;
    if !span_x.is_finite() || span_x <= EPSILON {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_REAR_STOCK_STATION_SPAN_INVALID",
        ));
    }
    let endpoint_y = inner
        .iter()
        .filter(|(_, position)| {
            (position[0] - min_x).abs() <= EPSILON || (position[0] - max_x).abs() <= EPSILON
        })
        .map(|(_, position)| position[1])
        .collect::<Vec<_>>();
    if endpoint_y.len() != 4
        || endpoint_y
            .iter()
            .any(|value| (*value - endpoint_y[0]).abs() > EPSILON)
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_REAR_STOCK_ENDPOINT_PLANE_INVALID",
        ));
    }
    let endpoint_y = endpoint_y[0];
    let mut target_y_by_id = BTreeMap::<String, f64>::new();
    for station_index in 0..REAR_STOCK_BRIDGE_STATION_RATIOS.len() {
        let station_x = min_x + span_x * REAR_STOCK_BRIDGE_STATION_RATIOS[station_index];
        let station = inner
            .iter()
            .filter(|(_, position)| (position[0] - station_x).abs() <= EPSILON)
            .collect::<Vec<_>>();
        let expected_y = endpoint_y + REAR_STOCK_BRIDGE_CURRENT_Y_OFFSETS_M[station_index];
        if station.len() != 2
            || station.iter().any(|(_, position)| {
                (position[1] - expected_y).abs() > EPSILON
                    || (position[2].abs() - REAR_STOCK_BRIDGE_HALF_DEPTH_M).abs() > EPSILON
            })
            || (station[0].1[2] + station[1].1[2]).abs() > EPSILON
        {
            return Err(invalid(format!(
                "ASSEMBLY_PARAMETER_REAR_STOCK_CURRENT_PROFILE_STATION_{station_index}_INVALID"
            )));
        }
        let target_y = endpoint_y + REAR_STOCK_BRIDGE_TARGET_Y_OFFSETS_M[station_index];
        for (element_id, _) in station {
            target_y_by_id.insert(element_id.clone(), target_y);
        }
    }

    let mut trial = program.clone();
    let target_vertices = trial
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .and_then(|nodes| {
            nodes.iter_mut().find(|node| {
                node.get("node_id").and_then(Value::as_str) == Some(STOCK_UPPER_NODE_ID)
            })
        })
        .and_then(|node| node.pointer_mut("/parameters/vertices"))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_REAR_STOCK_BRIDGE_VERTICES_INVALID"))?;
    let mut changed_ids = BTreeSet::new();
    for vertex in target_vertices {
        let element_id = vertex
            .get("element_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_REAR_STOCK_VERTEX_ID_INVALID"))?
            .to_owned();
        let Some(target_y) = target_y_by_id.get(&element_id).copied() else {
            continue;
        };
        let position = vertex
            .get_mut("position_m")
            .and_then(Value::as_array_mut)
            .filter(|values| values.len() == 3)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_REAR_STOCK_VERTEX_POSITION_INVALID"))?;
        let current_y = position[1]
            .as_f64()
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_REAR_STOCK_VERTEX_POSITION_INVALID"))?;
        if (current_y - target_y).abs() > EPSILON {
            position[1] = number(target_y)?;
            changed_ids.insert(element_id);
        }
    }
    if changed_ids.len() != 6 {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_REAR_STOCK_REPAIR_VERTEX_COUNT_INVALID",
        ));
    }

    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&trial);
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical_sha256),
        );
    let after = ProgramIndex::parse_with_expected_hash(&trial, None)?;
    if before.nodes.keys().collect::<BTreeSet<_>>() != after.nodes.keys().collect::<BTreeSet<_>>()
        || before.part_outputs != after.part_outputs
        || before.nodes.iter().any(|(node_id, node)| {
            node_id != STOCK_UPPER_NODE_ID && after.nodes.get(node_id) != Some(node)
        })
        || before.nodes.get(STOCK_UPPER_NODE_ID) == after.nodes.get(STOCK_UPPER_NODE_ID)
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_REAR_STOCK_OWNER_VOID_LINEAGE_CHANGED",
        ));
    }
    Ok(trial)
}

pub(crate) fn production_weapon_rear_stock_owner_void_half_y_flat_z_profile_id() -> &'static str {
    REAR_STOCK_OWNER_VOID_HALF_Y_FLAT_Z_PROFILE_ID
}

/// Shorten only the upper stock beam from its receiver-facing inner edge.
///
/// This private screen parameter owns the coupled X position/size derivation:
/// callers cannot patch either raw field independently.  The cap-facing
/// endpoint is held exact while lower beam, rear-cap and all Y/Z/rotation
/// fields remain unchanged.  It is intentionally absent from the public
/// parameter registry and availability surface.
pub(crate) fn production_weapon_stock_upper_inner_span_trial_mutate(
    program: &Value,
    target_span_m: f64,
) -> Result<Value, RuntimeError> {
    if !target_span_m.is_finite()
        || !(STOCK_UPPER_INNER_SPAN_MIN_M..=STOCK_UPPER_INNER_SPAN_MAX_M).contains(&target_span_m)
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_UPPER_INNER_SPAN_OUT_OF_BOUNDS",
        ));
    }
    let index = ProgramIndex::parse_with_expected_hash(program, None)?;
    resolve_stock_open_frame_binding(&index, BindingKind::StockOpenFrameClearanceM)?;
    let upper_position = stock_vec3(&index, STOCK_UPPER_NODE_ID, "position_m")?;
    let upper_size = stock_vec3(&index, STOCK_UPPER_NODE_ID, "size_m")?;
    if target_span_m > upper_size[0] + EPSILON {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_UPPER_INNER_SPAN_EXPANSION_BLOCKED",
        ));
    }
    let cap_facing_endpoint_x = upper_position[0] + upper_size[0] * 0.5;
    let target_position_x = cap_facing_endpoint_x - target_span_m * 0.5;

    let mut trial = program.clone();
    let upper = trial
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .and_then(|nodes| {
            nodes.iter_mut().find(|node| {
                node.get("node_id").and_then(Value::as_str) == Some(STOCK_UPPER_NODE_ID)
            })
        })
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
    let parameters = upper
        .get_mut("parameters")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_PARAMETERS_INVALID"))?;
    let position = parameters
        .get_mut("position_m")
        .and_then(Value::as_array_mut)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_POSITION_INVALID"))?;
    position[0] = number(target_position_x)?;
    let size = parameters
        .get_mut("size_m")
        .and_then(Value::as_array_mut)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_SIZE_INVALID"))?;
    size[0] = number(target_span_m)?;
    validate_primitive_box_parameters(parameters)?;

    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&trial);
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical_sha256),
        );
    ProgramIndex::parse_with_expected_hash(&trial, None)?;
    Ok(trial)
}

/// Replace the upper stock box with one closed concave profile-loft screen.
///
/// The two admitted variants are complete product-owned profiles, not raw
/// point edits.  They inherit the box envelope and depth extent, keep the
/// stable node/PartOutput identity, and leave lower stock plus rear-cap exact.
/// The inherited depth remains an UNKNOWN design choice and this helper has no
/// public parameter/registry/availability surface.
pub(crate) fn production_weapon_stock_upper_profile_trial_mutate(
    program: &Value,
    inner_span_m: f64,
) -> Result<Value, RuntimeError> {
    if !inner_span_m.is_finite()
        || !STOCK_UPPER_PROFILE_VARIANTS_M
            .iter()
            .any(|variant| (inner_span_m - variant).abs() <= EPSILON)
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_VARIANT_UNAVAILABLE",
        ));
    }
    let index = ProgramIndex::parse_with_expected_hash(program, None)?;
    resolve_stock_open_frame_binding(&index, BindingKind::StockOpenFrameClearanceM)?;
    let upper_position = stock_vec3(&index, STOCK_UPPER_NODE_ID, "position_m")?;
    let upper_size = stock_vec3(&index, STOCK_UPPER_NODE_ID, "size_m")?;
    let upper_rotation = stock_vec3(&index, STOCK_UPPER_NODE_ID, "rotation_rad")?;
    if upper_rotation.iter().any(|value| value.abs() > EPSILON)
        || inner_span_m >= upper_size[0] - EPSILON
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_BASELINE_INVALID",
        ));
    }
    let half_x = upper_size[0] * 0.5;
    let half_y = upper_size[1] * 0.5;
    let half_depth = upper_size[2] * 0.5;
    let half_inner = inner_span_m * 0.5;
    let lip_y = -half_y * 0.5;
    let points = serde_json::json!([
        [-half_y, half_inner],
        [lip_y, half_inner],
        [lip_y, -half_inner],
        [-half_y, -half_inner],
        [-half_y, -half_x],
        [half_y, -half_x],
        [half_y, half_x],
        [-half_y, half_x]
    ]);
    let profiles = serde_json::json!([
        {
            "station_m":-half_depth,
            "points":points,
            "corner_indices":[0,1,2,3,4,5,6,7]
        },
        {
            "station_m":half_depth,
            "points":points,
            "corner_indices":[0,1,2,3,4,5,6,7]
        }
    ]);

    let mut trial = program.clone();
    let upper = trial
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .and_then(|nodes| {
            nodes.iter_mut().find(|node| {
                node.get("node_id").and_then(Value::as_str) == Some(STOCK_UPPER_NODE_ID)
            })
        })
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
    *upper = serde_json::json!({
        "node_id":STOCK_UPPER_NODE_ID,
        "operator_id":PROFILE_LOFT_V2_OPERATOR,
        "inputs":[],
        "parameters":{
            "shape":"profile-loft-v2",
            "profiles":profiles,
            "resample_points":8,
            "interpolation":"linear",
            "interpolation_rings":0,
            "preserve_corners":true,
            "position_m":upper_position,
            "rotation_rad":[0.0,std::f64::consts::FRAC_PI_2,0.0]
        }
    });

    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&trial);
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical_sha256),
        );
    ProgramIndex::parse_with_expected_hash(&trial, None)?;
    Ok(trial)
}

/// Reconstruct the upper rear-stock source node as one closed, product-owned
/// profile.  Unlike the private screen helpers, this is the production
/// materializer used by `rear-stock-profile-reconstruction-v1`: callers can
/// express five bounded art-direction controls, but never raw profile points,
/// arbitrary JSON, or a replacement GeometryProgram.
///
/// The two end stations retain the outer envelope and receive independent
/// receiver/cap inner-junction edits.  A fixed centre station adds only a
/// small inward depth contour, giving rear-three-quarter evidence a real
/// shape signal without moving either end silhouette.  The stable
/// `rear-stock` node and aggregate PartOutput ownership are preserved exactly.
pub(crate) fn production_weapon_stock_profile_reconstruction_mutate(
    program: &Value,
    inner_receiver_delta_y_m: f64,
    inner_cap_delta_y_m: f64,
    receiver_inner_x_delta_m: f64,
    cap_inner_x_delta_m: f64,
    depth_center_inner_delta_y_m: f64,
) -> Result<Value, RuntimeError> {
    if ![
        inner_receiver_delta_y_m,
        inner_cap_delta_y_m,
        receiver_inner_x_delta_m,
        cap_inner_x_delta_m,
        depth_center_inner_delta_y_m,
    ]
    .iter()
    .all(|value| value.is_finite())
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_PROFILE_RECONSTRUCTION_NONFINITE",
        ));
    }
    if !(0.0..=0.07).contains(&inner_receiver_delta_y_m)
        || !(0.0..=0.07).contains(&inner_cap_delta_y_m)
        || !(-0.01..=0.01).contains(&receiver_inner_x_delta_m)
        || !(-0.01..=0.01).contains(&cap_inner_x_delta_m)
        || !(0.0..=0.01).contains(&depth_center_inner_delta_y_m)
        || -0.055 + inner_receiver_delta_y_m + depth_center_inner_delta_y_m > 0.025
        || -0.055 + inner_cap_delta_y_m + depth_center_inner_delta_y_m > 0.025
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_PROFILE_RECONSTRUCTION_OUT_OF_BOUNDS",
        ));
    }

    // Start from the already validated closed profile topology.  This keeps
    // the production path on the same active profile-loft@2 compiler surface
    // as the real D1 screens while replacing every art-sensitive coordinate
    // from the typed controls below.
    let mut proposal = production_weapon_stock_upper_profile_trial_mutate(program, 0.85)?;
    let upper = proposal
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .and_then(|nodes| {
            nodes.iter_mut().find(|node| {
                node.get("node_id").and_then(Value::as_str) == Some(STOCK_UPPER_NODE_ID)
            })
        })
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
    let profiles = upper
        .get_mut("parameters")
        .and_then(Value::as_object_mut)
        .and_then(|parameters| parameters.get_mut("profiles"))
        .and_then(Value::as_array_mut)
        .filter(|profiles| profiles.len() == 2)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
    let end_profile = profiles[0].clone();
    *profiles = vec![end_profile.clone(), end_profile.clone(), end_profile];

    for (station_index, profile) in profiles.iter_mut().enumerate() {
        profile["station_m"] = number(match station_index {
            0 => -0.43,
            1 => 0.0,
            _ => 0.43,
        })?;
        let centre_delta = if station_index == 1 {
            depth_center_inner_delta_y_m
        } else {
            0.0
        };
        let points = profile
            .get_mut("points")
            .and_then(Value::as_array_mut)
            .filter(|points| points.len() == 8)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
        for (point_index, first, second) in [
            (
                1_usize,
                -0.055 + inner_cap_delta_y_m + centre_delta,
                0.425 + cap_inner_x_delta_m,
            ),
            (
                2_usize,
                -0.055 + inner_receiver_delta_y_m + centre_delta,
                -0.425 + receiver_inner_x_delta_m,
            ),
        ] {
            let point = points[point_index]
                .as_array_mut()
                .filter(|point| point.len() == 2)
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
            point[0] = number(first)?;
            point[1] = number(second)?;
        }
    }
    upper["parameters"]["interpolation_rings"] = Value::from(0);

    proposal
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&proposal);
    proposal
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical_sha256),
        );
    ProgramIndex::parse_with_expected_hash(&proposal, None)?;
    validate_rear_stock_source_node_lineage(program, &proposal)?;
    Ok(proposal)
}

/// Keep the reconstruction a true one-source-node proposal.  This check lives
/// beside the pure materializer so callers that use the typed Runtime seam get
/// the same lineage guarantee as the ActionRun validator: all non-target
/// nodes, PartOutputs, and top-level program metadata remain byte-equivalent
/// under canonical JSON, while only `rear-stock` is replaced.
fn validate_rear_stock_source_node_lineage(
    baseline: &Value,
    proposed: &Value,
) -> Result<(), RuntimeError> {
    let baseline_object = baseline
        .as_object()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?;
    let proposed_object = proposed
        .as_object()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?;
    let metadata_keys = baseline_object
        .keys()
        .chain(proposed_object.keys())
        .filter(|key| key.as_str() != "canonical_sha256")
        .collect::<BTreeSet<_>>();
    for key in metadata_keys {
        if key.as_str() != "nodes" && baseline_object.get(key) != proposed_object.get(key) {
            return Err(invalid(
                "ASSEMBLY_PARAMETER_STOCK_PROFILE_RECONSTRUCTION_METADATA_CHANGED",
            ));
        }
    }
    let node_map = |value: &Value| -> Result<BTreeMap<String, Value>, RuntimeError> {
        let nodes = value
            .get("nodes")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_NODES_MISSING"))?;
        let mut result = BTreeMap::new();
        for node in nodes {
            let node_id = node
                .get("node_id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_NODE_ID_INVALID"))?;
            if result.insert(node_id.to_owned(), node.clone()).is_some() {
                return Err(invalid("ASSEMBLY_PARAMETER_NODE_ID_DUPLICATE"));
            }
        }
        Ok(result)
    };
    let baseline_nodes = node_map(baseline)?;
    let proposed_nodes = node_map(proposed)?;
    if baseline_nodes.keys().collect::<BTreeSet<_>>()
        != proposed_nodes.keys().collect::<BTreeSet<_>>()
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_PROFILE_RECONSTRUCTION_NODE_SET_CHANGED",
        ));
    }
    let changed = baseline_nodes
        .iter()
        .filter_map(|(node_id, before)| {
            (proposed_nodes.get(node_id) != Some(before)).then_some(node_id.as_str())
        })
        .collect::<BTreeSet<_>>();
    if changed != BTreeSet::from([STOCK_UPPER_NODE_ID])
        || baseline_nodes
            .get(STOCK_UPPER_NODE_ID)
            .and_then(|node| node.get("operator_id"))
            .and_then(Value::as_str)
            != Some(PRIMITIVE_OPERATOR)
        || proposed_nodes
            .get(STOCK_UPPER_NODE_ID)
            .and_then(|node| node.get("operator_id"))
            .and_then(Value::as_str)
            != Some(PROFILE_LOFT_V2_OPERATOR)
        || baseline.get("part_outputs") != proposed.get("part_outputs")
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_PROFILE_RECONSTRUCTION_LINEAGE_CHANGED",
        ));
    }
    Ok(())
}

/// Change only the two inner-lip coordinates of the fixed 0.85 m upper
/// profile proposal.  The caller selects one of two complete bounded variants;
/// arbitrary profile points remain unavailable.
pub(crate) fn production_weapon_stock_upper_profile_lip_trial_mutate(
    program: &Value,
    lip_y_m: f64,
) -> Result<Value, RuntimeError> {
    if !lip_y_m.is_finite()
        || !STOCK_UPPER_PROFILE_LIP_VARIANTS_M
            .iter()
            .any(|variant| (lip_y_m - variant).abs() <= EPSILON)
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_LIP_VARIANT_UNAVAILABLE",
        ));
    }
    let mut trial = production_weapon_stock_upper_profile_trial_mutate(program, 0.85)?;
    let upper = trial
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .and_then(|nodes| {
            nodes.iter_mut().find(|node| {
                node.get("node_id").and_then(Value::as_str) == Some(STOCK_UPPER_NODE_ID)
            })
        })
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
    let profiles = upper
        .get_mut("parameters")
        .and_then(Value::as_object_mut)
        .and_then(|parameters| parameters.get_mut("profiles"))
        .and_then(Value::as_array_mut)
        .filter(|profiles| profiles.len() == 2)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
    for profile in profiles {
        let points = profile
            .get_mut("points")
            .and_then(Value::as_array_mut)
            .filter(|points| points.len() == 8)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
        for point_index in [1_usize, 2_usize] {
            let point = points[point_index]
                .as_array_mut()
                .filter(|point| point.len() == 2)
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
            point[0] = number(lip_y_m)?;
        }
    }
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&trial);
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical_sha256),
        );
    ProgramIndex::parse_with_expected_hash(&trial, None)?;
    Ok(trial)
}

/// Change only the two inner-lip coordinates of the fixed 04Q@0.85 upper
/// profile for the private 04V extrapolation screen.  This is intentionally a
/// separate closed mutator: the earlier 04R lip variants remain unchanged and
/// these two forward extrapolations never become a public parameter sink.
pub(crate) fn production_weapon_stock_upper_profile_04v_lip_trial_mutate(
    program: &Value,
    lip_y_m: f64,
) -> Result<Value, RuntimeError> {
    if !lip_y_m.is_finite()
        || !STOCK_UPPER_PROFILE_04V_LIP_VARIANTS_M
            .iter()
            .any(|variant| (lip_y_m - variant).abs() <= EPSILON)
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_04V_LIP_VARIANT_UNAVAILABLE",
        ));
    }
    let mut trial = production_weapon_stock_upper_profile_trial_mutate(program, 0.85)?;
    let upper = trial
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .and_then(|nodes| {
            nodes.iter_mut().find(|node| {
                node.get("node_id").and_then(Value::as_str) == Some(STOCK_UPPER_NODE_ID)
            })
        })
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
    let profiles = upper
        .get_mut("parameters")
        .and_then(Value::as_object_mut)
        .and_then(|parameters| parameters.get_mut("profiles"))
        .and_then(Value::as_array_mut)
        .filter(|profiles| profiles.len() == 2)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
    for profile in profiles {
        let points = profile
            .get_mut("points")
            .and_then(Value::as_array_mut)
            .filter(|points| points.len() == 8)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
        for point_index in [1_usize, 2_usize] {
            let point = points[point_index]
                .as_array_mut()
                .filter(|point| point.len() == 2)
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
            point[0] = number(lip_y_m)?;
        }
    }
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&trial);
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical_sha256),
        );
    ProgramIndex::parse_with_expected_hash(&trial, None)?;
    Ok(trial)
}

/// Change only the two inner-lip coordinates of the fixed 04Q@0.85 upper
/// profile for the private 04W continuation screen.  This is a separate
/// closed mutator from 04V so neither exploration set becomes a public
/// parameter sink or silently widens the other set's whitelist.
pub(crate) fn production_weapon_stock_upper_profile_04w_lip_trial_mutate(
    program: &Value,
    lip_y_m: f64,
) -> Result<Value, RuntimeError> {
    if !lip_y_m.is_finite()
        || !STOCK_UPPER_PROFILE_04W_LIP_VARIANTS_M
            .iter()
            .any(|variant| (lip_y_m - variant).abs() <= EPSILON)
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_04W_LIP_VARIANT_UNAVAILABLE",
        ));
    }
    let mut trial = production_weapon_stock_upper_profile_trial_mutate(program, 0.85)?;
    let upper = trial
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .and_then(|nodes| {
            nodes.iter_mut().find(|node| {
                node.get("node_id").and_then(Value::as_str) == Some(STOCK_UPPER_NODE_ID)
            })
        })
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
    let profiles = upper
        .get_mut("parameters")
        .and_then(Value::as_object_mut)
        .and_then(|parameters| parameters.get_mut("profiles"))
        .and_then(Value::as_array_mut)
        .filter(|profiles| profiles.len() == 2)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
    for profile in profiles {
        let points = profile
            .get_mut("points")
            .and_then(Value::as_array_mut)
            .filter(|points| points.len() == 8)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
        for point_index in [1_usize, 2_usize] {
            let point = points[point_index]
                .as_array_mut()
                .filter(|point| point.len() == 2)
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
            point[0] = number(lip_y_m)?;
        }
    }
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&trial);
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical_sha256),
        );
    ProgramIndex::parse_with_expected_hash(&trial, None)?;
    Ok(trial)
}

/// Translate the complete inner boundary of the fixed 04Q@0.85 upper profile
/// by one closed first-coordinate delta.  Points 0..3 move together at both
/// stations; the outer envelope points 4..7, all second coordinates, station
/// positions, pose, lower stock, and rear-cap remain locked.  This is a
/// private diagnostic-only screen and is intentionally absent from every
/// public parameter sink or registry.
pub(crate) fn production_weapon_stock_upper_profile_04x_boundary_translation_trial_mutate(
    program: &Value,
    delta_x_m: f64,
) -> Result<Value, RuntimeError> {
    if !delta_x_m.is_finite()
        || !STOCK_UPPER_PROFILE_04X_BOUNDARY_TRANSLATION_VARIANTS_M
            .iter()
            .any(|variant| (delta_x_m - variant).abs() <= EPSILON)
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_04X_BOUNDARY_TRANSLATION_VARIANT_UNAVAILABLE",
        ));
    }
    let mut trial = production_weapon_stock_upper_profile_trial_mutate(program, 0.85)?;
    let upper = trial
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .and_then(|nodes| {
            nodes.iter_mut().find(|node| {
                node.get("node_id").and_then(Value::as_str) == Some(STOCK_UPPER_NODE_ID)
            })
        })
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
    let profiles = upper
        .get_mut("parameters")
        .and_then(Value::as_object_mut)
        .and_then(|parameters| parameters.get_mut("profiles"))
        .and_then(Value::as_array_mut)
        .filter(|profiles| profiles.len() == 2)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
    for profile in profiles {
        let points = profile
            .get_mut("points")
            .and_then(Value::as_array_mut)
            .filter(|points| points.len() == 8)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
        for point_index in 0_usize..=3_usize {
            let point = points[point_index]
                .as_array_mut()
                .filter(|point| point.len() == 2)
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
            let baseline_x = point[0]
                .as_f64()
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
            point[0] = number(baseline_x + delta_x_m)?;
        }
    }
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&trial);
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical_sha256),
        );
    ProgramIndex::parse_with_expected_hash(&trial, None)?;
    Ok(trial)
}

/// Move only the inner boundary of one authored upper-profile station.
///
/// This is the closed 04Z station-attribution screen used after the 04Y
/// registration preflight.  It is deliberately not a public parameter sink:
/// the caller can select only one of the two fixed profile stations, and the
/// mutator changes only points 0..=3 first coordinates by the fixed +0.020 m
/// diagnostic delta.  The other station, outer points, second coordinates,
/// station positions, pose, lower stock, rear-cap and PartOutput bindings are
/// inherited unchanged from the bounded 04Q@0.85 profile.
pub(crate) fn production_weapon_stock_upper_profile_04z_station_isolation_trial_mutate(
    program: &Value,
    station_index: usize,
) -> Result<Value, RuntimeError> {
    if station_index >= 2 {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_04Z_STATION_INDEX_UNAVAILABLE",
        ));
    }

    let mut trial = production_weapon_stock_upper_profile_trial_mutate(program, 0.85)?;
    let upper = trial
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .and_then(|nodes| {
            nodes.iter_mut().find(|node| {
                node.get("node_id").and_then(Value::as_str) == Some(STOCK_UPPER_NODE_ID)
            })
        })
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
    let profiles = upper
        .get_mut("parameters")
        .and_then(Value::as_object_mut)
        .and_then(|parameters| parameters.get_mut("profiles"))
        .and_then(Value::as_array_mut)
        .filter(|profiles| profiles.len() == 2)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
    let profile = profiles
        .get_mut(station_index)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
    let points = profile
        .get_mut("points")
        .and_then(Value::as_array_mut)
        .filter(|points| points.len() == 8)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
    for point_index in 0_usize..=3_usize {
        let point = points[point_index]
            .as_array_mut()
            .filter(|point| point.len() == 2)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
        let baseline_x = point[0]
            .as_f64()
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
        point[0] = number(baseline_x + STOCK_UPPER_PROFILE_04Z_STATION_DELTA_M)?;
    }

    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&trial);
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical_sha256),
        );
    ProgramIndex::parse_with_expected_hash(&trial, None)?;
    Ok(trial)
}

/// Change only the two outer-shoulder coordinates of the fixed 0.85 m upper
/// profile proposal.  The lip remains the 04Q baseline (-0.055 m), and the
/// caller selects one of two complete bounded shoulder variants.  Arbitrary
/// profile points remain unavailable.
pub(crate) fn production_weapon_stock_upper_profile_shoulder_trial_mutate(
    program: &Value,
    shoulder_y_m: f64,
) -> Result<Value, RuntimeError> {
    if !shoulder_y_m.is_finite()
        || !STOCK_UPPER_PROFILE_SHOULDER_VARIANTS_M
            .iter()
            .any(|variant| (shoulder_y_m - variant).abs() <= EPSILON)
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_SHOULDER_VARIANT_UNAVAILABLE",
        ));
    }
    let mut trial = production_weapon_stock_upper_profile_trial_mutate(program, 0.85)?;
    let upper = trial
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .and_then(|nodes| {
            nodes.iter_mut().find(|node| {
                node.get("node_id").and_then(Value::as_str) == Some(STOCK_UPPER_NODE_ID)
            })
        })
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
    let profiles = upper
        .get_mut("parameters")
        .and_then(Value::as_object_mut)
        .and_then(|parameters| parameters.get_mut("profiles"))
        .and_then(Value::as_array_mut)
        .filter(|profiles| profiles.len() == 2)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
    for profile in profiles {
        let points = profile
            .get_mut("points")
            .and_then(Value::as_array_mut)
            .filter(|points| points.len() == 8)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
        for point_index in [0_usize, 3_usize] {
            let point = points[point_index]
                .as_array_mut()
                .filter(|point| point.len() == 2)
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
            point[0] = number(shoulder_y_m)?;
        }
    }
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&trial);
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical_sha256),
        );
    ProgramIndex::parse_with_expected_hash(&trial, None)?;
    Ok(trial)
}

/// Change only the cap-facing longitudinal coordinate of the fixed 0.85 m
/// upper profile.  The two profile stations are changed together and only
/// the second coordinate of point 2 is admitted.  Point 1's longitudinal
/// coordinate, every first coordinate, all other points, station positions,
/// pose, lower stock and rear-cap remain locked.  This is a private,
/// diagnostic-only screen and is intentionally absent from all public sinks.
pub(crate) fn production_weapon_stock_upper_profile_cap_lip_trial_mutate(
    program: &Value,
    cap_lip_longitudinal_m: f64,
) -> Result<Value, RuntimeError> {
    if !cap_lip_longitudinal_m.is_finite()
        || !STOCK_UPPER_PROFILE_CAP_LIP_VARIANTS_M
            .iter()
            .any(|variant| (cap_lip_longitudinal_m - variant).abs() <= EPSILON)
    {
        return Err(invalid(
            "ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_CAP_LIP_VARIANT_UNAVAILABLE",
        ));
    }
    let mut trial = production_weapon_stock_upper_profile_trial_mutate(program, 0.85)?;
    let upper = trial
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .and_then(|nodes| {
            nodes.iter_mut().find(|node| {
                node.get("node_id").and_then(Value::as_str) == Some(STOCK_UPPER_NODE_ID)
            })
        })
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
    let profiles = upper
        .get_mut("parameters")
        .and_then(Value::as_object_mut)
        .and_then(|parameters| parameters.get_mut("profiles"))
        .and_then(Value::as_array_mut)
        .filter(|profiles| profiles.len() == 2)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
    for profile in profiles {
        let points = profile
            .get_mut("points")
            .and_then(Value::as_array_mut)
            .filter(|points| points.len() == 8)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
        let point_one_y = points[1]
            .as_array()
            .filter(|point| point.len() == 2)
            .and_then(|point| point[1].as_f64())
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
        let point_two_coordinates = points[2]
            .as_array()
            .filter(|point| point.len() == 2)
            .map(|point| (point[0].as_f64(), point[1].as_f64()))
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
        if (point_one_y - 0.425).abs() > EPSILON
            || point_two_coordinates
                .0
                .is_none_or(|value| (value + 0.055).abs() > EPSILON)
            || point_two_coordinates
                .1
                .is_none_or(|value| (value + 0.425).abs() > EPSILON)
        {
            return Err(invalid(
                "ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_CAP_LIP_BASELINE_INVALID",
            ));
        }
        let point_two = points[2]
            .as_array_mut()
            .filter(|point| point.len() == 2)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_UPPER_PROFILE_INVALID"))?;
        point_two[1] = number(cap_lip_longitudinal_m)?;
    }
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&trial);
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical_sha256),
        );
    ProgramIndex::parse_with_expected_hash(&trial, None)?;
    Ok(trial)
}

/// Split the aggregate stock PartOutput into two ephemeral diagnostic
/// outputs, without changing any GeometryProgram node or geometry parameter.
///
/// This is deliberately not a semantic parameter sink: it has no parameter
/// ID, descriptor, availability entry, or registry binding.  The source
/// aggregate must bind exactly both stock nodes and must carry valid material
/// and solid metadata.  The two diagnostic outputs inherit that metadata and
/// replace the aggregate at the same array position; all other outputs and
/// every node remain byte-for-byte equivalent after canonical JSON ordering.
pub(crate) fn production_weapon_stock_split_output_diagnostic(
    program: &Value,
) -> Result<Value, RuntimeError> {
    let index = ProgramIndex::parse_with_expected_hash(program, None)?;
    resolve_stock_open_frame_binding(&index, BindingKind::StockOpenFrameClearanceM)?;

    let outputs = program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PART_OUTPUTS_INVALID"))?;
    let mut stock_position = None;
    let mut stock_material_zone_id = None;
    let mut stock_solid = None;
    for (position, output) in outputs.iter().enumerate() {
        let part_id = output.get("part_id").and_then(Value::as_str);
        if matches!(
            part_id,
            Some(STOCK_UPPER_DIAGNOSTIC_PART_ID | STOCK_LOWER_DIAGNOSTIC_PART_ID)
        ) {
            return Err(invalid(
                "ASSEMBLY_PARAMETER_STOCK_DIAGNOSTIC_PART_ALREADY_PRESENT",
            ));
        }
        if part_id != Some(STOCK_PART_ID) {
            continue;
        }
        if stock_position.replace(position).is_some() {
            return Err(invalid("ASSEMBLY_PARAMETER_STOCK_PART_DUPLICATE"));
        }
        let source_ids = output
            .get("input_node_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_PART_INPUTS_INVALID"))?;
        let exact_source_binding = source_ids.len() == 2
            && source_ids[0].as_str() == Some(STOCK_UPPER_NODE_ID)
            && source_ids[1].as_str() == Some(STOCK_LOWER_NODE_ID);
        if !exact_source_binding {
            return Err(invalid(
                "ASSEMBLY_PARAMETER_STOCK_SPLIT_SOURCE_BINDING_INVALID",
            ));
        }
        let material_zone_id = output
            .get("material_zone_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_PART_MATERIAL_ZONE_INVALID"))?;
        let solid = output
            .get("solid")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_PART_SOLID_INVALID"))?;
        stock_material_zone_id = Some(material_zone_id.to_owned());
        stock_solid = Some(solid);
    }

    let stock_position =
        stock_position.ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_PART_UNAVAILABLE"))?;
    let stock_material_zone_id = stock_material_zone_id
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_PART_MATERIAL_ZONE_INVALID"))?;
    let stock_solid =
        stock_solid.ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_PART_SOLID_INVALID"))?;
    let source_output = outputs
        .get(stock_position)
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_PART_OUTPUT_INVALID"))?;

    let mut upper_output = source_output.clone();
    upper_output.insert(
        "part_id".to_owned(),
        Value::String(STOCK_UPPER_DIAGNOSTIC_PART_ID.to_owned()),
    );
    upper_output.insert(
        "input_node_ids".to_owned(),
        Value::Array(vec![Value::String(STOCK_UPPER_NODE_ID.to_owned())]),
    );
    upper_output.insert(
        "material_zone_id".to_owned(),
        Value::String(stock_material_zone_id.clone()),
    );
    upper_output.insert("solid".to_owned(), Value::Bool(stock_solid));

    let mut lower_output = source_output.clone();
    lower_output.insert(
        "part_id".to_owned(),
        Value::String(STOCK_LOWER_DIAGNOSTIC_PART_ID.to_owned()),
    );
    lower_output.insert(
        "input_node_ids".to_owned(),
        Value::Array(vec![Value::String(STOCK_LOWER_NODE_ID.to_owned())]),
    );
    lower_output.insert(
        "material_zone_id".to_owned(),
        Value::String(stock_material_zone_id),
    );
    lower_output.insert("solid".to_owned(), Value::Bool(stock_solid));

    let mut trial = program.clone();
    let trial_outputs = trial
        .get_mut("part_outputs")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PART_OUTPUTS_INVALID"))?;
    trial_outputs.remove(stock_position);
    trial_outputs.insert(stock_position, Value::Object(upper_output));
    trial_outputs.insert(stock_position + 1, Value::Object(lower_output));

    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&trial);
    trial
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical_sha256),
        );
    ProgramIndex::parse_with_expected_hash(&trial, None)?;
    Ok(trial)
}

fn mutate_with_expected_hash(
    program: &Value,
    expected_program_sha256: Option<&str>,
    parameter_id: &str,
    value: f64,
) -> Result<Value, RuntimeError> {
    let index = ProgramIndex::parse_with_expected_hash(program, expected_program_sha256)?;
    let binding = resolve_binding(&index, parameter_id)?;
    validate_value(binding.kind, value)?;

    let mut draft = program.clone();
    let nodes = draft
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_NODES_MISSING"))?;
    let mut node_positions = BTreeMap::new();
    for (position, node) in nodes.iter().enumerate() {
        let node_id = node
            .get("node_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_NODE_ID_INVALID"))?;
        node_positions.insert(node_id.to_owned(), position);
    }

    // The resolver has already proven the exact node/PartOutput binding.  The
    // open-frame clearance is an aggregate relation between its two beams;
    // every other parameter remains a closed per-node mutation.
    if binding.kind == BindingKind::StockOpenFrameClearanceM {
        apply_stock_open_frame_clearance(nodes, &node_positions, value)?;
    } else {
        for node_id in &binding.node_ids {
            let position = *node_positions
                .get(node_id)
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_NODE_BINDING_MISSING"))?;
            let node = nodes
                .get_mut(position)
                .and_then(Value::as_object_mut)
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_NODE_INVALID"))?;
            let parameters = node
                .get_mut("parameters")
                .and_then(Value::as_object_mut)
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_NODE_PARAMETERS_INVALID"))?;
            apply_to_parameters(parameters, binding.kind, value)?;
        }
    }

    // Only the targeted `parameters` objects may differ.  The canonical field
    // is deliberately removed as this is a draft, not a hash claim.
    draft
        .as_object_mut()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_INVALID"))?
        .remove("canonical_sha256");

    // Re-run the same closed resolver over the derived draft.  This catches a
    // scale that would erase clearance or a station delta that would violate
    // strict order after the aggregate has been applied.  The temporary hash
    // exists only for readback validation and is never returned or persisted.
    let mut revalidated = draft.clone();
    let derived_hash = canonical_json_hash(&revalidated);
    revalidated["canonical_sha256"] = Value::String(derived_hash);
    let derived_index = ProgramIndex::parse_with_expected_hash(&revalidated, None)?;
    resolve_binding(&derived_index, parameter_id)?;
    Ok(draft)
}

/// The exact eight semantic IDs supported by this mutator.
pub(crate) fn production_weapon_assembly_parameter_supported(parameter_id: &str) -> bool {
    SUPPORTED_PARAMETER_IDS.contains(&parameter_id)
}

/// The four current assembly IDs that intentionally remain unavailable.  They
/// have no unambiguous typed GeometryProgram field in the current contract.
pub(crate) fn production_weapon_assembly_parameter_unavailable(parameter_id: &str) -> bool {
    UNSUPPORTED_PARAMETER_IDS.contains(&parameter_id)
}

/// Closed descriptor emitted by the read-only sink projection.  The
/// descriptor is derived from the same resolver used by the pure mutator; it
/// never exposes a JSON path or a caller-selected field name.
#[derive(Debug, Clone)]
pub(crate) struct ProductionWeaponAssemblyParameterDescriptor {
    pub(crate) parameter_id: String,
    pub(crate) group_id: String,
    pub(crate) mutator_id: String,
    pub(crate) current: f64,
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) step: f64,
    pub(crate) unit: String,
    pub(crate) target_part_ids: Vec<String>,
    pub(crate) source_node_ids: Vec<String>,
    pub(crate) operator_ids: Vec<String>,
    pub(crate) evidence_requirements: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProductionWeaponAssemblyParameterDescriptorReport {
    pub(crate) available: Vec<ProductionWeaponAssemblyParameterDescriptor>,
    pub(crate) unavailable_parameter_ids: Vec<String>,
}

/// Resolve all eight product-owned semantics in one validated program pass.
/// A structurally valid program may still lack one or more exact bindings;
/// those semantic IDs are returned as unavailable rather than guessed.
pub(crate) fn production_weapon_assembly_parameter_descriptors(
    program: &Value,
    expected_program_sha256: &str,
) -> Result<ProductionWeaponAssemblyParameterDescriptorReport, RuntimeError> {
    if !is_sha256(expected_program_sha256) {
        return Err(invalid("ASSEMBLY_PARAMETER_EXPECTED_PROGRAM_HASH_INVALID"));
    }
    let index = ProgramIndex::parse_with_expected_hash(program, Some(expected_program_sha256))?;
    let mut available = Vec::new();
    let mut unavailable_parameter_ids = Vec::new();
    for parameter_id in SUPPORTED_PARAMETER_IDS {
        match resolve_binding(&index, parameter_id) {
            Ok(binding) => available.push(descriptor_from_binding(&index, parameter_id, binding)?),
            Err(_) => unavailable_parameter_ids.push(parameter_id.to_owned()),
        }
    }
    Ok(ProductionWeaponAssemblyParameterDescriptorReport {
        available,
        unavailable_parameter_ids,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindingKind {
    ReceiverWidthRatio,
    ReceiverHeightRatio,
    ReceiverShoulderStationDelta,
    MuzzleShroudEnvelopeRatio,
    MuzzleEmitterEnvelopeRatio,
    MuzzleCoreRadiusRatio,
    StockOpenFrameClearanceM,
    StockOpenFrameAngleRad,
}

fn binding_kind(parameter_id: &str) -> Option<BindingKind> {
    match parameter_id {
        "receiver-envelope-width" => Some(BindingKind::ReceiverWidthRatio),
        "receiver-envelope-height" => Some(BindingKind::ReceiverHeightRatio),
        "receiver-envelope-shoulder" => Some(BindingKind::ReceiverShoulderStationDelta),
        "muzzle-axis-shroud-envelope" => Some(BindingKind::MuzzleShroudEnvelopeRatio),
        "muzzle-axis-emitter-envelope" => Some(BindingKind::MuzzleEmitterEnvelopeRatio),
        "muzzle-axis-core-aperture" => Some(BindingKind::MuzzleCoreRadiusRatio),
        "stock-open-frame-clearance" => Some(BindingKind::StockOpenFrameClearanceM),
        "stock-open-frame-angle" => Some(BindingKind::StockOpenFrameAngleRad),
        _ => None,
    }
}

#[derive(Debug, Clone)]
struct Binding {
    kind: BindingKind,
    node_ids: Vec<String>,
}

fn descriptor_from_binding(
    index: &ProgramIndex,
    parameter_id: &str,
    binding: Binding,
) -> Result<ProductionWeaponAssemblyParameterDescriptor, RuntimeError> {
    let (group_id, mutator_id, current, min, max, step, unit, target_part_ids) = match binding.kind
    {
        BindingKind::ReceiverWidthRatio => (
            "receiver-envelope",
            "forgecad.assembly.mutator.receiver-envelope@1",
            1.0,
            RATIO_MIN,
            RATIO_MAX,
            0.01,
            "ratio",
            RECEIVER_PART_IDS.as_slice(),
        ),
        BindingKind::ReceiverHeightRatio => (
            "receiver-envelope",
            "forgecad.assembly.mutator.receiver-envelope@1",
            1.0,
            RATIO_MIN,
            RATIO_MAX,
            0.01,
            "ratio",
            RECEIVER_PART_IDS.as_slice(),
        ),
        BindingKind::ReceiverShoulderStationDelta => (
            "receiver-envelope",
            "forgecad.assembly.mutator.receiver-envelope@1",
            0.0,
            SHOULDER_DELTA_MIN_M,
            SHOULDER_DELTA_MAX_M,
            0.01,
            "meter",
            RECEIVER_PART_IDS.as_slice(),
        ),
        BindingKind::MuzzleShroudEnvelopeRatio => (
            "muzzle-axis",
            "forgecad.assembly.mutator.muzzle-axis@1",
            1.0,
            RATIO_MIN,
            RATIO_MAX,
            0.01,
            "ratio",
            &[MUZZLE_SHROUD_PART_ID][..],
        ),
        BindingKind::MuzzleEmitterEnvelopeRatio => (
            "muzzle-axis",
            "forgecad.assembly.mutator.muzzle-axis@1",
            1.0,
            RATIO_MIN,
            RATIO_MAX,
            0.01,
            "ratio",
            &[MUZZLE_EMITTER_PART_ID][..],
        ),
        BindingKind::MuzzleCoreRadiusRatio => (
            "muzzle-axis",
            "forgecad.assembly.mutator.muzzle-axis@1",
            1.0,
            RATIO_MIN,
            RATIO_MAX,
            0.01,
            "ratio",
            &[MUZZLE_CORE_PART_ID][..],
        ),
        BindingKind::StockOpenFrameClearanceM => (
            "stock-open-frame",
            "forgecad.assembly.mutator.stock-open-frame@1",
            stock_open_frame_clearance(index)?,
            STOCK_CLEARANCE_MIN_M,
            STOCK_CLEARANCE_MAX_M,
            0.01,
            "meter",
            &[STOCK_PART_ID][..],
        ),
        BindingKind::StockOpenFrameAngleRad => (
            "stock-open-frame",
            "forgecad.assembly.mutator.stock-open-frame@1",
            stock_open_frame_angle(index)?,
            STOCK_ANGLE_MIN_RAD,
            STOCK_ANGLE_MAX_RAD,
            0.01,
            "radian",
            &[STOCK_PART_ID][..],
        ),
    };
    let mut operator_ids = BTreeSet::new();
    for node_id in &binding.node_ids {
        let node = index
            .nodes
            .get(node_id)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_NODE_BINDING_MISSING"))?;
        let operator_id = node
            .get("operator_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_OPERATOR_INVALID"))?;
        operator_ids.insert(operator_id.to_owned());
    }
    Ok(ProductionWeaponAssemblyParameterDescriptor {
        parameter_id: parameter_id.to_owned(),
        group_id: group_id.to_owned(),
        mutator_id: mutator_id.to_owned(),
        current,
        min,
        max,
        step,
        unit: unit.to_owned(),
        target_part_ids: target_part_ids
            .iter()
            .map(|part_id| (*part_id).to_owned())
            .collect(),
        source_node_ids: binding.node_ids,
        operator_ids: operator_ids.into_iter().collect(),
        evidence_requirements: vec![
            "assembly-registry".to_owned(),
            "geometry-program".to_owned(),
            "operator-catalog".to_owned(),
            "artifact-readback".to_owned(),
            "candidate-state".to_owned(),
        ],
    })
}

#[derive(Debug, Clone)]
struct ProgramIndex {
    nodes: BTreeMap<String, Map<String, Value>>,
    part_outputs: BTreeMap<String, Vec<String>>,
}

impl ProgramIndex {
    fn parse_with_expected_hash(
        program: &Value,
        expected_program_sha256: Option<&str>,
    ) -> Result<Self, RuntimeError> {
        let object = program
            .as_object()
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_NOT_OBJECT"))?;
        let has_canonical = object.contains_key("canonical_sha256");
        let root_fields_with_hash = [
            "schema_version",
            "project_id",
            "representation_plan_sha256",
            "operator_catalog_sha256",
            "units",
            "budgets",
            "nodes",
            "part_outputs",
            "canonical_sha256",
        ];
        let root_fields_without_hash = [
            "schema_version",
            "project_id",
            "representation_plan_sha256",
            "operator_catalog_sha256",
            "units",
            "budgets",
            "nodes",
            "part_outputs",
        ];
        require_exact_keys(
            object,
            if has_canonical {
                &root_fields_with_hash
            } else {
                &root_fields_without_hash
            },
            "ASSEMBLY_PARAMETER_PROGRAM",
        )?;
        if object.get("schema_version").and_then(Value::as_str)
            != Some(GEOMETRY_PROGRAM_SCHEMA_VERSION)
        {
            return Err(invalid("ASSEMBLY_PARAMETER_PROGRAM_SCHEMA_INVALID"));
        }
        require_id(object, "project_id")?;
        require_hash(object, "representation_plan_sha256")?;
        let active_catalog_sha256 = operator_catalog_sha256();
        if object
            .get("operator_catalog_sha256")
            .and_then(Value::as_str)
            != Some(active_catalog_sha256.as_str())
        {
            return Err(invalid("ASSEMBLY_PARAMETER_OPERATOR_CATALOG_MISMATCH"));
        }
        let mut without_hash = object.clone();
        without_hash.remove("canonical_sha256");
        let computed_hash = canonical_json_hash(&Value::Object(without_hash));
        match (
            object.get("canonical_sha256").and_then(Value::as_str),
            expected_program_sha256,
        ) {
            (Some(declared), Some(expected))
                if is_sha256(declared) && declared == expected && declared == computed_hash => {}
            (Some(declared), None) if is_sha256(declared) && declared == computed_hash => {}
            (None, Some(expected)) if expected == computed_hash => {}
            (Some(_), _) => return Err(invalid("ASSEMBLY_PARAMETER_PROGRAM_CANONICAL_MISMATCH")),
            (None, None) => return Err(invalid("ASSEMBLY_PARAMETER_PROGRAM_CANONICAL_REQUIRED")),
            (None, Some(_)) => return Err(invalid("ASSEMBLY_PARAMETER_PROGRAM_HASH_MISMATCH")),
        }
        validate_units(object.get("units"))?;
        validate_budgets(object.get("budgets"))?;
        reject_forbidden_keys(program)?;

        let node_values = object
            .get("nodes")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PROGRAM_NODES_INVALID"))?;
        if node_values.is_empty() || node_values.len() > 512 {
            return Err(invalid("ASSEMBLY_PARAMETER_PROGRAM_NODES_BOUNDS"));
        }
        let mut nodes = BTreeMap::new();
        let mut declared_inputs = BTreeSet::new();
        for node_value in node_values {
            let node = node_value
                .as_object()
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_NODE_INVALID"))?;
            require_exact_keys(
                node,
                &["node_id", "operator_id", "inputs", "parameters"],
                "ASSEMBLY_PARAMETER_NODE",
            )?;
            let node_id = node
                .get("node_id")
                .and_then(Value::as_str)
                .filter(|id| is_opaque_id(id))
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_NODE_ID_INVALID"))?;
            let operator_id = node
                .get("operator_id")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_OPERATOR_INVALID"))?;
            if !catalog_has_active_operator(operator_id) {
                return Err(invalid("ASSEMBLY_PARAMETER_OPERATOR_NOT_IN_CATALOG"));
            }
            let inputs = node
                .get("inputs")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_NODE_INPUTS_INVALID"))?;
            let mut seen_inputs = BTreeSet::new();
            for input in inputs {
                let input_id = input
                    .as_str()
                    .filter(|id| is_opaque_id(id))
                    .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_INPUT_ID_INVALID"))?;
                if !seen_inputs.insert(input_id.to_owned()) {
                    return Err(invalid("ASSEMBLY_PARAMETER_INPUT_DUPLICATE"));
                }
                declared_inputs.insert(input_id.to_owned());
            }
            if node.get("parameters").and_then(Value::as_object).is_none() {
                return Err(invalid("ASSEMBLY_PARAMETER_NODE_PARAMETERS_INVALID"));
            }
            if nodes.insert(node_id.to_owned(), node.clone()).is_some() {
                return Err(invalid("ASSEMBLY_PARAMETER_NODE_DUPLICATE"));
            }
        }
        if declared_inputs
            .iter()
            .any(|input| !nodes.contains_key(input))
        {
            return Err(invalid("ASSEMBLY_PARAMETER_INPUT_UNKNOWN"));
        }

        let output_values = object
            .get("part_outputs")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PART_OUTPUTS_INVALID"))?;
        if output_values.is_empty() || output_values.len() > 512 {
            return Err(invalid("ASSEMBLY_PARAMETER_PART_OUTPUTS_BOUNDS"));
        }
        let mut part_outputs = BTreeMap::new();
        for output_value in output_values {
            let output = output_value
                .as_object()
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PART_OUTPUT_INVALID"))?;
            require_exact_keys(
                output,
                &["part_id", "input_node_ids", "material_zone_id", "solid"],
                "ASSEMBLY_PARAMETER_PART_OUTPUT",
            )?;
            let part_id = output
                .get("part_id")
                .and_then(Value::as_str)
                .filter(|id| is_opaque_id(id))
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PART_ID_INVALID"))?;
            let input_node_ids = output
                .get("input_node_ids")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PART_INPUTS_INVALID"))?;
            if input_node_ids.is_empty() {
                return Err(invalid("ASSEMBLY_PARAMETER_PART_INPUTS_EMPTY"));
            }
            let mut node_ids = Vec::with_capacity(input_node_ids.len());
            let mut seen = BTreeSet::new();
            for node_id in input_node_ids {
                let node_id = node_id
                    .as_str()
                    .filter(|id| is_opaque_id(id))
                    .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PART_NODE_ID_INVALID"))?;
                if !nodes.contains_key(node_id) || !seen.insert(node_id.to_owned()) {
                    return Err(invalid("ASSEMBLY_PARAMETER_PART_NODE_BINDING_INVALID"));
                }
                node_ids.push(node_id.to_owned());
            }
            if output
                .get("material_zone_id")
                .and_then(Value::as_str)
                .filter(|id| is_opaque_id(id))
                .is_none()
                || output.get("solid").and_then(Value::as_bool).is_none()
            {
                return Err(invalid("ASSEMBLY_PARAMETER_PART_OUTPUT_METADATA_INVALID"));
            }
            if part_outputs.insert(part_id.to_owned(), node_ids).is_some() {
                return Err(invalid("ASSEMBLY_PARAMETER_PART_DUPLICATE"));
            }
        }
        Ok(Self {
            nodes,
            part_outputs,
        })
    }
}

fn resolve_binding(index: &ProgramIndex, parameter_id: &str) -> Result<Binding, RuntimeError> {
    let kind = binding_kind(parameter_id).ok_or_else(|| {
        if UNSUPPORTED_PARAMETER_IDS.contains(&parameter_id) {
            invalid("ASSEMBLY_PARAMETER_UNAVAILABLE_TYPED_FIELD")
        } else {
            invalid("ASSEMBLY_PARAMETER_ID_UNKNOWN")
        }
    })?;
    let part_ids = match kind {
        BindingKind::ReceiverWidthRatio
        | BindingKind::ReceiverHeightRatio
        | BindingKind::ReceiverShoulderStationDelta => RECEIVER_PART_IDS.as_slice(),
        BindingKind::MuzzleShroudEnvelopeRatio => &[MUZZLE_SHROUD_PART_ID][..],
        BindingKind::MuzzleEmitterEnvelopeRatio => &[MUZZLE_EMITTER_PART_ID][..],
        BindingKind::MuzzleCoreRadiusRatio => &[MUZZLE_CORE_PART_ID][..],
        BindingKind::StockOpenFrameClearanceM | BindingKind::StockOpenFrameAngleRad => {
            return resolve_stock_open_frame_binding(index, kind);
        }
    };

    let mut node_ids = Vec::new();
    for part_id in part_ids {
        let source_ids = index
            .part_outputs
            .get(*part_id)
            .ok_or_else(|| invalid(format!("ASSEMBLY_PARAMETER_PART_UNAVAILABLE: {part_id}")))?;
        if source_ids.len() != 1 {
            return Err(invalid("ASSEMBLY_PARAMETER_PART_NODE_AMBIGUOUS"));
        }
        let node_id = &source_ids[0];
        let node = index
            .nodes
            .get(node_id)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_NODE_BINDING_MISSING"))?;
        let operator_id = node
            .get("operator_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_OPERATOR_INVALID"))?;
        let parameters = node
            .get("parameters")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_NODE_PARAMETERS_INVALID"))?;
        match kind {
            BindingKind::ReceiverWidthRatio | BindingKind::ReceiverHeightRatio => {
                validate_loft_or_box(operator_id, parameters, false)?;
            }
            BindingKind::ReceiverShoulderStationDelta => {
                if operator_id != LONGITUDINAL_SECTION_LOFT_OPERATOR {
                    return Err(invalid("ASSEMBLY_PARAMETER_OPERATOR_BINDING_MISMATCH"));
                }
                validate_longitudinal_parameters(parameters, true)?;
            }
            BindingKind::MuzzleShroudEnvelopeRatio => {
                validate_loft_or_box(operator_id, parameters, false)?;
            }
            BindingKind::MuzzleEmitterEnvelopeRatio => {
                validate_loft_or_cylinder(operator_id, parameters)?;
            }
            BindingKind::MuzzleCoreRadiusRatio => {
                if operator_id != PRIMITIVE_OPERATOR
                    || parameters.get("shape").and_then(Value::as_str) != Some("cylinder")
                {
                    return Err(invalid("ASSEMBLY_PARAMETER_OPERATOR_BINDING_MISMATCH"));
                }
                validate_primitive_radius_parameters(parameters)?;
            }
            BindingKind::StockOpenFrameClearanceM | BindingKind::StockOpenFrameAngleRad => {
                return Err(invalid("ASSEMBLY_PARAMETER_STOCK_BINDING_ROUTE_INVALID"));
            }
        }
        node_ids.push(node_id.clone());
    }

    if matches!(
        kind,
        BindingKind::ReceiverWidthRatio
            | BindingKind::ReceiverHeightRatio
            | BindingKind::ReceiverShoulderStationDelta
    ) {
        validate_receiver_axis(index, &node_ids)?;
    }
    if matches!(
        kind,
        BindingKind::MuzzleShroudEnvelopeRatio
            | BindingKind::MuzzleEmitterEnvelopeRatio
            | BindingKind::MuzzleCoreRadiusRatio
    ) {
        validate_muzzle_axis_and_clearance(index)?;
    }
    Ok(Binding { kind, node_ids })
}

fn resolve_stock_open_frame_binding(
    index: &ProgramIndex,
    kind: BindingKind,
) -> Result<Binding, RuntimeError> {
    let stock_sources = index
        .part_outputs
        .get(STOCK_PART_ID)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_PART_UNAVAILABLE"))?;
    if stock_sources.as_slice() != [STOCK_UPPER_NODE_ID, STOCK_LOWER_NODE_ID] {
        return Err(invalid("ASSEMBLY_PARAMETER_STOCK_NODE_BINDING_AMBIGUOUS"));
    }
    let cap_sources = index
        .part_outputs
        .get("rear-cap")
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_CAP_UNAVAILABLE"))?;
    if cap_sources.as_slice() != ["rear-cap"] {
        return Err(invalid("ASSEMBLY_PARAMETER_STOCK_CAP_BINDING_AMBIGUOUS"));
    }
    for node_id in [STOCK_UPPER_NODE_ID, STOCK_LOWER_NODE_ID, "rear-cap"] {
        let node = index
            .nodes
            .get(node_id)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
        if node.get("operator_id").and_then(Value::as_str) != Some(PRIMITIVE_OPERATOR) {
            return Err(invalid("ASSEMBLY_PARAMETER_STOCK_OPERATOR_MISMATCH"));
        }
        let parameters = node
            .get("parameters")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_PARAMETERS_INVALID"))?;
        if parameters.get("shape").and_then(Value::as_str) != Some("box") {
            return Err(invalid("ASSEMBLY_PARAMETER_STOCK_SHAPE_INVALID"));
        }
        validate_primitive_box_parameters(parameters)?;
    }
    let upper_rotation = stock_vec3(index, STOCK_UPPER_NODE_ID, "rotation_rad")?;
    let lower_rotation = stock_vec3(index, STOCK_LOWER_NODE_ID, "rotation_rad")?;
    if upper_rotation
        .iter()
        .any(|value| value.abs() > AXIS_ROTATION_TOLERANCE_RAD)
        || lower_rotation[0].abs() > AXIS_ROTATION_TOLERANCE_RAD
        || lower_rotation[1].abs() > AXIS_ROTATION_TOLERANCE_RAD
        || !(STOCK_ANGLE_MIN_RAD..=STOCK_ANGLE_MAX_RAD).contains(&lower_rotation[2])
    {
        return Err(invalid("ASSEMBLY_PARAMETER_STOCK_FRAME_AXIS_INVALID"));
    }
    let clearance = stock_open_frame_clearance(index)?;
    if !(STOCK_CLEARANCE_MIN_M..=STOCK_CLEARANCE_MAX_M).contains(&clearance) {
        return Err(invalid("ASSEMBLY_PARAMETER_STOCK_CLEARANCE_INVALID"));
    }
    Ok(Binding {
        kind,
        node_ids: if kind == BindingKind::StockOpenFrameAngleRad {
            vec![STOCK_LOWER_NODE_ID.to_owned()]
        } else {
            vec![
                STOCK_UPPER_NODE_ID.to_owned(),
                STOCK_LOWER_NODE_ID.to_owned(),
            ]
        },
    })
}

fn stock_vec3(index: &ProgramIndex, node_id: &str, field: &str) -> Result<[f64; 3], RuntimeError> {
    let values = index
        .nodes
        .get(node_id)
        .and_then(|node| node.get("parameters"))
        .and_then(Value::as_object)
        .and_then(|parameters| parameters.get(field))
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_VECTOR_INVALID"))?;
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        result[index] = value
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_VECTOR_INVALID"))?;
    }
    Ok(result)
}

fn stock_open_frame_angle(index: &ProgramIndex) -> Result<f64, RuntimeError> {
    Ok(stock_vec3(index, STOCK_LOWER_NODE_ID, "rotation_rad")?[2])
}

fn stock_open_frame_clearance(index: &ProgramIndex) -> Result<f64, RuntimeError> {
    let upper_position = stock_vec3(index, STOCK_UPPER_NODE_ID, "position_m")?;
    let upper_size = stock_vec3(index, STOCK_UPPER_NODE_ID, "size_m")?;
    let lower_position = stock_vec3(index, STOCK_LOWER_NODE_ID, "position_m")?;
    let lower_size = stock_vec3(index, STOCK_LOWER_NODE_ID, "size_m")?;
    let angle = stock_open_frame_angle(index)?;
    stock_clearance_from_components(
        upper_position,
        upper_size,
        lower_position,
        lower_size,
        angle,
    )
}

fn stock_clearance_from_components(
    upper_position: [f64; 3],
    upper_size: [f64; 3],
    lower_position: [f64; 3],
    lower_size: [f64; 3],
    lower_angle: f64,
) -> Result<f64, RuntimeError> {
    let upper_lower_edge = upper_position[1] - upper_size[1] * 0.5;
    let lower_y_extent = lower_angle.sin().abs() * lower_size[0] * 0.5
        + lower_angle.cos().abs() * lower_size[1] * 0.5;
    let clearance = upper_lower_edge - (lower_position[1] + lower_y_extent);
    if !clearance.is_finite() || clearance <= 0.0 {
        return Err(invalid("ASSEMBLY_PARAMETER_STOCK_CLEARANCE_INVALID"));
    }
    Ok(clearance)
}

fn validate_value(kind: BindingKind, value: f64) -> Result<(), RuntimeError> {
    if !value.is_finite() {
        return Err(invalid("ASSEMBLY_PARAMETER_VALUE_NONFINITE"));
    }
    match kind {
        BindingKind::ReceiverShoulderStationDelta => {
            if !(SHOULDER_DELTA_MIN_M..=SHOULDER_DELTA_MAX_M).contains(&value) {
                Err(invalid("ASSEMBLY_PARAMETER_SHOULDER_DELTA_OUT_OF_BOUNDS"))
            } else {
                Ok(())
            }
        }
        BindingKind::StockOpenFrameClearanceM => {
            if !(STOCK_CLEARANCE_MIN_M..=STOCK_CLEARANCE_MAX_M).contains(&value) {
                Err(invalid("ASSEMBLY_PARAMETER_STOCK_CLEARANCE_OUT_OF_BOUNDS"))
            } else {
                Ok(())
            }
        }
        BindingKind::StockOpenFrameAngleRad => {
            if !(STOCK_ANGLE_MIN_RAD..=STOCK_ANGLE_MAX_RAD).contains(&value) {
                Err(invalid("ASSEMBLY_PARAMETER_STOCK_ANGLE_OUT_OF_BOUNDS"))
            } else {
                Ok(())
            }
        }
        _ => {
            if !(RATIO_MIN..=RATIO_MAX).contains(&value) {
                Err(invalid("ASSEMBLY_PARAMETER_RATIO_OUT_OF_BOUNDS"))
            } else {
                Ok(())
            }
        }
    }
}

fn apply_to_parameters(
    parameters: &mut Map<String, Value>,
    kind: BindingKind,
    value: f64,
) -> Result<(), RuntimeError> {
    match kind {
        BindingKind::ReceiverWidthRatio => match parameters.get("shape").and_then(Value::as_str) {
            Some("longitudinal-section-loft") => scale_longitudinal_points(parameters, 1, value),
            Some("box") => scale_box_axis(parameters, 2, value),
            _ => Err(invalid("ASSEMBLY_PARAMETER_OPERATOR_BINDING_MISMATCH")),
        },
        BindingKind::ReceiverHeightRatio => match parameters.get("shape").and_then(Value::as_str) {
            Some("longitudinal-section-loft") => scale_longitudinal_points(parameters, 0, value),
            Some("box") => scale_box_axis(parameters, 1, value),
            _ => Err(invalid("ASSEMBLY_PARAMETER_OPERATOR_BINDING_MISMATCH")),
        },
        BindingKind::ReceiverShoulderStationDelta => shift_shoulder_stations(parameters, value),
        BindingKind::MuzzleShroudEnvelopeRatio => {
            match parameters.get("shape").and_then(Value::as_str) {
                Some("longitudinal-section-loft") => {
                    scale_longitudinal_points_coupled(parameters, value)
                }
                Some("box") => scale_box_envelope(parameters, value),
                _ => Err(invalid("ASSEMBLY_PARAMETER_OPERATOR_BINDING_MISMATCH")),
            }
        }
        BindingKind::MuzzleEmitterEnvelopeRatio => {
            match parameters.get("shape").and_then(Value::as_str) {
                Some("longitudinal-section-loft") => {
                    scale_longitudinal_points_coupled(parameters, value)
                }
                Some("cylinder") => scale_cylinder_envelope(parameters, value),
                _ => Err(invalid("ASSEMBLY_PARAMETER_OPERATOR_BINDING_MISMATCH")),
            }
        }
        BindingKind::MuzzleCoreRadiusRatio => {
            let radius = parameters
                .get("radius_m")
                .and_then(Value::as_f64)
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_RADIUS_MISSING"))?;
            let next = radius * value;
            if !next.is_finite() || !(0.0 < next && next <= 5.0) {
                return Err(invalid("ASSEMBLY_PARAMETER_RADIUS_OUT_OF_BOUNDS"));
            }
            parameters.insert("radius_m".to_owned(), number(next)?);
            Ok(())
        }
        BindingKind::StockOpenFrameAngleRad => set_stock_lower_angle(parameters, value),
        BindingKind::StockOpenFrameClearanceM => {
            Err(invalid("ASSEMBLY_PARAMETER_AGGREGATE_APPLICATION_REQUIRED"))
        }
    }
}

fn apply_stock_open_frame_clearance(
    nodes: &mut [Value],
    node_positions: &BTreeMap<String, usize>,
    target_clearance_m: f64,
) -> Result<(), RuntimeError> {
    let upper_index = *node_positions
        .get(STOCK_UPPER_NODE_ID)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
    let lower_index = *node_positions
        .get(STOCK_LOWER_NODE_ID)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_NODE_MISSING"))?;
    let read_vec3 = |node: &Value, field: &str| -> Result<[f64; 3], RuntimeError> {
        let values = node
            .get("parameters")
            .and_then(Value::as_object)
            .and_then(|parameters| parameters.get(field))
            .and_then(Value::as_array)
            .filter(|values| values.len() == 3)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_VECTOR_INVALID"))?;
        let mut result = [0.0; 3];
        for (index, value) in values.iter().enumerate() {
            result[index] = value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_VECTOR_INVALID"))?;
        }
        Ok(result)
    };
    let upper_position = read_vec3(&nodes[upper_index], "position_m")?;
    let upper_size = read_vec3(&nodes[upper_index], "size_m")?;
    let lower_position = read_vec3(&nodes[lower_index], "position_m")?;
    let lower_size = read_vec3(&nodes[lower_index], "size_m")?;
    let lower_rotation = read_vec3(&nodes[lower_index], "rotation_rad")?;
    let current_clearance = stock_clearance_from_components(
        upper_position,
        upper_size,
        lower_position,
        lower_size,
        lower_rotation[2],
    )?;
    let half_delta = (target_clearance_m - current_clearance) * 0.5;
    set_box_position_y(&mut nodes[upper_index], upper_position[1] + half_delta)?;
    set_box_position_y(&mut nodes[lower_index], lower_position[1] - half_delta)?;
    Ok(())
}

fn set_box_position_y(node: &mut Value, value: f64) -> Result<(), RuntimeError> {
    let parameters = node
        .get_mut("parameters")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_PARAMETERS_INVALID"))?;
    let position = parameters
        .get_mut("position_m")
        .and_then(Value::as_array_mut)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_POSITION_INVALID"))?;
    position[1] = number(value)?;
    validate_primitive_box_parameters(parameters)
}

fn set_box_position_x(node: &mut Value, value: f64) -> Result<(), RuntimeError> {
    let parameters = node
        .get_mut("parameters")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_PARAMETERS_INVALID"))?;
    let position = parameters
        .get_mut("position_m")
        .and_then(Value::as_array_mut)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_POSITION_INVALID"))?;
    position[0] = number(value)?;
    validate_primitive_box_parameters(parameters)
}

fn set_stock_lower_angle(
    parameters: &mut Map<String, Value>,
    angle_rad: f64,
) -> Result<(), RuntimeError> {
    let rotation = parameters
        .get_mut("rotation_rad")
        .and_then(Value::as_array_mut)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STOCK_ROTATION_INVALID"))?;
    rotation[2] = number(angle_rad)?;
    validate_primitive_box_parameters(parameters)
}

fn validate_loft_or_box(
    operator_id: &str,
    parameters: &Map<String, Value>,
    require_inner_shoulder_stations: bool,
) -> Result<(), RuntimeError> {
    match (operator_id, parameters.get("shape").and_then(Value::as_str)) {
        (LONGITUDINAL_SECTION_LOFT_OPERATOR, Some("longitudinal-section-loft")) => {
            validate_longitudinal_parameters(parameters, require_inner_shoulder_stations)
        }
        (PRIMITIVE_OPERATOR, Some("box")) if !require_inner_shoulder_stations => {
            validate_primitive_box_parameters(parameters)
        }
        _ => Err(invalid("ASSEMBLY_PARAMETER_OPERATOR_BINDING_MISMATCH")),
    }
}

fn validate_loft_or_cylinder(
    operator_id: &str,
    parameters: &Map<String, Value>,
) -> Result<(), RuntimeError> {
    match (operator_id, parameters.get("shape").and_then(Value::as_str)) {
        (LONGITUDINAL_SECTION_LOFT_OPERATOR, Some("longitudinal-section-loft")) => {
            validate_longitudinal_parameters(parameters, false)
        }
        (PRIMITIVE_OPERATOR, Some("cylinder")) => validate_primitive_radius_parameters(parameters),
        _ => Err(invalid("ASSEMBLY_PARAMETER_OPERATOR_BINDING_MISMATCH")),
    }
}

fn scale_box_axis(
    parameters: &mut Map<String, Value>,
    axis: usize,
    ratio: f64,
) -> Result<(), RuntimeError> {
    let size = parameters
        .get_mut("size_m")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SIZE_MISSING"))?;
    let current = size
        .get(axis)
        .and_then(Value::as_f64)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SIZE_INVALID"))?;
    size[axis] = number(current * ratio)?;
    validate_primitive_box_parameters(parameters)
}

fn scale_box_envelope(parameters: &mut Map<String, Value>, ratio: f64) -> Result<(), RuntimeError> {
    let size = parameters
        .get_mut("size_m")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SIZE_MISSING"))?;
    for axis in 0..3 {
        let current = size
            .get(axis)
            .and_then(Value::as_f64)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SIZE_INVALID"))?;
        size[axis] = number(current * ratio)?;
    }
    validate_primitive_box_parameters(parameters)
}

fn scale_cylinder_envelope(
    parameters: &mut Map<String, Value>,
    ratio: f64,
) -> Result<(), RuntimeError> {
    for key in ["radius_m", "height_m"] {
        let current = parameters
            .get(key)
            .and_then(Value::as_f64)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_CYLINDER_ENVELOPE_INVALID"))?;
        parameters.insert(key.to_owned(), number(current * ratio)?);
    }
    validate_primitive_radius_parameters(parameters)
}

fn scale_longitudinal_points(
    parameters: &mut Map<String, Value>,
    axis: usize,
    ratio: f64,
) -> Result<(), RuntimeError> {
    let sections = parameters
        .get_mut("sections")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SECTIONS_MISSING"))?;
    for section in sections {
        let points = section
            .get_mut("points")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_POINTS_MISSING"))?;
        for point in points {
            let point_values = point
                .as_array_mut()
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_POINT_INVALID"))?;
            let current = point_values
                .get(axis)
                .and_then(Value::as_f64)
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_POINT_COORDINATE_INVALID"))?;
            point_values[axis] = number(current * ratio)?;
        }
    }
    validate_longitudinal_parameters(parameters, false)
}

fn scale_longitudinal_points_coupled(
    parameters: &mut Map<String, Value>,
    ratio: f64,
) -> Result<(), RuntimeError> {
    let sections = parameters
        .get_mut("sections")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SECTIONS_MISSING"))?;
    for section in sections {
        let station = section
            .get("station_m")
            .and_then(Value::as_f64)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STATION_INVALID"))?;
        section["station_m"] = number(station * ratio)?;
        let points = section
            .get_mut("points")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_POINTS_MISSING"))?;
        for point in points {
            let point_values = point
                .as_array_mut()
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_POINT_INVALID"))?;
            for axis in 0..=1 {
                let current = point_values
                    .get(axis)
                    .and_then(Value::as_f64)
                    .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_POINT_COORDINATE_INVALID"))?;
                point_values[axis] = number(current * ratio)?;
            }
        }
    }
    validate_longitudinal_parameters(parameters, false)
}

fn shift_shoulder_stations(
    parameters: &mut Map<String, Value>,
    delta_m: f64,
) -> Result<(), RuntimeError> {
    let sections = parameters
        .get_mut("sections")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SECTIONS_MISSING"))?;
    if sections.len() < 3 {
        return Err(invalid("ASSEMBLY_PARAMETER_SHOULDER_STATIONS_UNAVAILABLE"));
    }
    let first_inner = 1usize;
    let last_inner = sections.len() - 2;
    let indices = if first_inner == last_inner {
        vec![first_inner]
    } else {
        vec![first_inner, last_inner]
    };
    let original = sections
        .iter()
        .map(|section| {
            section
                .get("station_m")
                .and_then(Value::as_f64)
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_STATION_INVALID"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut next_stations = original.clone();
    for index in indices {
        next_stations[index] += delta_m;
    }
    for index in 0..next_stations.len() {
        if !next_stations[index].is_finite()
            || next_stations[index].abs() > ABSOLUTE_ART_BOUND_M
            || (index > 0 && next_stations[index] <= next_stations[index - 1] + EPSILON)
        {
            return Err(invalid("ASSEMBLY_PARAMETER_SHOULDER_STATION_ORDER_INVALID"));
        }
    }
    for (section, station) in sections.iter_mut().zip(next_stations) {
        section["station_m"] = number(station)?;
    }
    validate_longitudinal_parameters(parameters, true)
}

fn validate_longitudinal_parameters(
    parameters: &Map<String, Value>,
    require_inner_shoulder_stations: bool,
) -> Result<(), RuntimeError> {
    require_exact_keys(
        parameters,
        &["shape", "sections", "position_m", "rotation_rad"],
        "ASSEMBLY_PARAMETER_LONGITUDINAL_PARAMETERS",
    )?;
    if parameters.get("shape").and_then(Value::as_str) != Some("longitudinal-section-loft") {
        return Err(invalid("ASSEMBLY_PARAMETER_LONGITUDINAL_SHAPE_INVALID"));
    }
    let sections = parameters
        .get("sections")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SECTIONS_MISSING"))?;
    if sections.len() < 2 || sections.len() > 16 {
        return Err(invalid("ASSEMBLY_PARAMETER_SECTIONS_BOUNDS"));
    }
    if require_inner_shoulder_stations && sections.len() < 3 {
        return Err(invalid("ASSEMBLY_PARAMETER_SHOULDER_STATIONS_UNAVAILABLE"));
    }
    let mut previous_station = f64::NEG_INFINITY;
    let mut expected_point_count = None;
    for section in sections {
        let section = section
            .as_object()
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SECTION_INVALID"))?;
        require_exact_keys(
            section,
            &["station_m", "points"],
            "ASSEMBLY_PARAMETER_SECTION",
        )?;
        let station = finite_coordinate(section.get("station_m"), "station_m")?;
        if station <= previous_station + EPSILON {
            return Err(invalid("ASSEMBLY_PARAMETER_STATIONS_NOT_STRICT"));
        }
        previous_station = station;
        let points = section
            .get("points")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_POINTS_MISSING"))?;
        if points.len() < 3 || points.len() > 64 {
            return Err(invalid("ASSEMBLY_PARAMETER_POINT_COUNT_BOUNDS"));
        }
        if let Some(expected) = expected_point_count {
            if expected != points.len() {
                return Err(invalid("ASSEMBLY_PARAMETER_SECTION_POINT_COUNT_MISMATCH"));
            }
        } else {
            expected_point_count = Some(points.len());
        }
        let mut polygon = Vec::with_capacity(points.len());
        for point in points {
            let point = point
                .as_array()
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_POINT_INVALID"))?;
            if point.len() != 2 {
                return Err(invalid("ASSEMBLY_PARAMETER_POINT_ARITY_INVALID"));
            }
            let y = finite_coordinate(point.first(), "point-y")?;
            let z = finite_coordinate(point.get(1), "point-z")?;
            polygon.push([y, z]);
        }
        if signed_area(&polygon).abs() <= EPSILON || polygon_self_intersects(&polygon) {
            return Err(invalid("ASSEMBLY_PARAMETER_PROFILE_INVALID"));
        }
    }
    validate_vec3(parameters.get("position_m"), "position_m", false)?;
    validate_vec3(parameters.get("rotation_rad"), "rotation_rad", true)?;
    validate_longitudinal_absolute_bounds(parameters)?;
    Ok(())
}

fn validate_primitive_radius_parameters(
    parameters: &Map<String, Value>,
) -> Result<(), RuntimeError> {
    let shape = parameters
        .get("shape")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_PRIMITIVE_SHAPE_INVALID"))?;
    let required = match shape {
        "cylinder" => [
            "shape",
            "radius_m",
            "height_m",
            "radial_segments",
            "position_m",
            "rotation_rad",
        ]
        .as_slice(),
        "sphere" => [
            "shape",
            "radius_m",
            "longitude_segments",
            "latitude_segments",
            "position_m",
            "rotation_rad",
        ]
        .as_slice(),
        _ => return Err(invalid("ASSEMBLY_PARAMETER_PRIMITIVE_RADIUS_UNAVAILABLE")),
    };
    require_exact_keys(
        parameters,
        required,
        "ASSEMBLY_PARAMETER_PRIMITIVE_PARAMETERS",
    )?;
    let radius = parameters
        .get("radius_m")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 5.0)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_RADIUS_INVALID"))?;
    if shape == "cylinder" {
        let height = parameters
            .get("height_m")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && *value > 0.0 && *value <= 10.0)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_HEIGHT_INVALID"))?;
        let segments = parameters
            .get("radial_segments")
            .and_then(Value::as_u64)
            .filter(|value| (8..=64).contains(value))
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SEGMENTS_INVALID"))?;
        let _ = (height, segments);
    } else {
        for key in ["longitude_segments", "latitude_segments"] {
            let value = parameters
                .get(key)
                .and_then(Value::as_u64)
                .filter(|value| {
                    (if key == "longitude_segments" {
                        8..=64
                    } else {
                        4..=64
                    })
                    .contains(value)
                })
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SEGMENTS_INVALID"))?;
            let _ = value;
        }
    }
    validate_vec3(parameters.get("position_m"), "position_m", false)?;
    validate_vec3(parameters.get("rotation_rad"), "rotation_rad", true)?;
    if radius.is_sign_negative() {
        return Err(invalid("ASSEMBLY_PARAMETER_RADIUS_INVALID"));
    }
    validate_primitive_absolute_bounds(parameters, shape, radius)?;
    Ok(())
}

fn validate_primitive_box_parameters(parameters: &Map<String, Value>) -> Result<(), RuntimeError> {
    require_exact_keys(
        parameters,
        &["shape", "size_m", "position_m", "rotation_rad"],
        "ASSEMBLY_PARAMETER_PRIMITIVE_BOX_PARAMETERS",
    )?;
    if parameters.get("shape").and_then(Value::as_str) != Some("box") {
        return Err(invalid("ASSEMBLY_PARAMETER_PRIMITIVE_BOX_UNAVAILABLE"));
    }
    let size = vec3(parameters.get("size_m"), "size_m", false)?;
    if size
        .iter()
        .any(|coordinate| *coordinate <= EPSILON || *coordinate > ABSOLUTE_ART_BOUND_M)
    {
        return Err(invalid("ASSEMBLY_PARAMETER_SIZE_INVALID"));
    }
    let position = vec3(parameters.get("position_m"), "position_m", false)?;
    validate_vec3(parameters.get("rotation_rad"), "rotation_rad", true)?;
    if position
        .iter()
        .zip(size)
        .any(|(origin, extent)| origin.abs() + extent / 2.0 > ABSOLUTE_ART_BOUND_M)
    {
        return Err(invalid("ASSEMBLY_PARAMETER_PRIMITIVE_ABSOLUTE_BOUNDS"));
    }
    Ok(())
}

fn validate_receiver_axis(index: &ProgramIndex, node_ids: &[String]) -> Result<(), RuntimeError> {
    let mut axis_z: Option<f64> = None;
    let mut axis_rotation: Option<[f64; 3]> = None;
    for node_id in node_ids {
        let node = index
            .nodes
            .get(node_id)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_NODE_BINDING_MISSING"))?;
        let parameters = node
            .get("parameters")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_NODE_PARAMETERS_INVALID"))?;
        let position = vec3(parameters.get("position_m"), "position_m", false)?;
        let rotation = vec3(parameters.get("rotation_rad"), "rotation_rad", true)?;
        if let Some(expected_z) = axis_z {
            if (expected_z - position[2]).abs() > EPSILON {
                return Err(invalid("ASSEMBLY_PARAMETER_RECEIVER_AXIS_NOT_SHARED"));
            }
        } else {
            axis_z = Some(position[2]);
        }
        if let Some(expected_rotation) = axis_rotation {
            if expected_rotation
                .iter()
                .zip(rotation)
                .any(|(expected, actual)| (expected - actual).abs() > EPSILON)
            {
                return Err(invalid("ASSEMBLY_PARAMETER_RECEIVER_AXIS_NOT_SHARED"));
            }
        } else {
            axis_rotation = Some(rotation);
        }
        let _ = position[1];
    }
    Ok(())
}

fn validate_muzzle_axis_and_clearance(index: &ProgramIndex) -> Result<(), RuntimeError> {
    let shroud_nodes = exact_part_nodes(index, MUZZLE_SHROUD_PART_ID)?;
    let emitter_nodes = exact_part_nodes(index, MUZZLE_EMITTER_PART_ID)?;
    let core_nodes = exact_part_nodes(index, MUZZLE_CORE_PART_ID)?;
    let shroud = index
        .nodes
        .get(&shroud_nodes[0])
        .and_then(|node| node.get("parameters"))
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_MUZZLE_SHROUD_UNAVAILABLE"))?;
    let emitter = index
        .nodes
        .get(&emitter_nodes[0])
        .and_then(|node| node.get("parameters"))
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_MUZZLE_EMITTER_UNAVAILABLE"))?;
    let core = index
        .nodes
        .get(&core_nodes[0])
        .and_then(|node| node.get("parameters"))
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_MUZZLE_CORE_UNAVAILABLE"))?;
    let shroud_position = vec3(shroud.get("position_m"), "position_m", false)?;
    let emitter_position = vec3(emitter.get("position_m"), "position_m", false)?;
    let core_position = vec3(core.get("position_m"), "position_m", false)?;
    let shroud_rotation = vec3(shroud.get("rotation_rad"), "rotation_rad", true)?;
    let emitter_rotation = vec3(emitter.get("rotation_rad"), "rotation_rad", true)?;
    let core_rotation = vec3(core.get("rotation_rad"), "rotation_rad", true)?;
    for axis in 1..=2 {
        if (shroud_position[axis] - emitter_position[axis]).abs() > EPSILON
            || (shroud_position[axis] - core_position[axis]).abs() > EPSILON
        {
            return Err(invalid("ASSEMBLY_PARAMETER_MUZZLE_AXIS_NOT_COAXIAL"));
        }
    }
    let shroud_shape = shroud.get("shape").and_then(Value::as_str);
    let emitter_shape = emitter.get("shape").and_then(Value::as_str);
    let legacy_shared_rotation = shroud_shape == Some("longitudinal-section-loft")
        && emitter_shape == Some("longitudinal-section-loft")
        && rotations_match(shroud_rotation, emitter_rotation, EPSILON)
        && rotations_match(shroud_rotation, core_rotation, EPSILON);
    let mixed_d1_axis = shroud_shape == Some("box")
        && emitter_shape == Some("cylinder")
        && shroud_rotation
            .iter()
            .all(|coordinate| coordinate.abs() <= AXIS_ROTATION_TOLERANCE_RAD)
        && rotations_match(emitter_rotation, core_rotation, AXIS_ROTATION_TOLERANCE_RAD)
        && emitter_rotation[0].abs() <= AXIS_ROTATION_TOLERANCE_RAD
        && emitter_rotation[1].abs() <= AXIS_ROTATION_TOLERANCE_RAD
        && (emitter_rotation[2].abs() - std::f64::consts::FRAC_PI_2).abs()
            <= AXIS_ROTATION_TOLERANCE_RAD;
    if (!legacy_shared_rotation && !mixed_d1_axis)
        || core.get("shape").and_then(Value::as_str) != Some("cylinder")
    {
        return Err(invalid("ASSEMBLY_PARAMETER_MUZZLE_AXIS_NOT_COAXIAL"));
    }
    let core_radius = core
        .get("radius_m")
        .and_then(Value::as_f64)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_MUZZLE_CORE_RADIUS_MISSING"))?;
    let envelope_radius = [shroud, emitter]
        .into_iter()
        .map(max_cross_section_radius)
        .collect::<Result<Vec<_>, _>>()?;
    if envelope_radius
        .iter()
        .any(|radius| core_radius >= *radius - EPSILON)
    {
        return Err(invalid("ASSEMBLY_PARAMETER_MUZZLE_CLEARANCE_INVALID"));
    }
    Ok(())
}

fn rotations_match(expected: [f64; 3], actual: [f64; 3], tolerance: f64) -> bool {
    expected
        .iter()
        .zip(actual)
        .all(|(expected, actual)| (expected - actual).abs() <= tolerance)
}

fn exact_part_nodes(index: &ProgramIndex, part_id: &str) -> Result<Vec<String>, RuntimeError> {
    let node_ids = index
        .part_outputs
        .get(part_id)
        .ok_or_else(|| invalid(format!("ASSEMBLY_PARAMETER_PART_UNAVAILABLE: {part_id}")))?;
    if node_ids.len() != 1 {
        return Err(invalid("ASSEMBLY_PARAMETER_PART_NODE_AMBIGUOUS"));
    }
    Ok(node_ids.clone())
}

fn max_cross_section_radius(parameters: &Map<String, Value>) -> Result<f64, RuntimeError> {
    let maximum =
        match parameters.get("shape").and_then(Value::as_str) {
            Some("longitudinal-section-loft") => {
                validate_longitudinal_parameters(parameters, false)?;
                let sections = parameters
                    .get("sections")
                    .and_then(Value::as_array)
                    .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SECTIONS_MISSING"))?;
                let mut maximum: f64 = 0.0;
                for section in sections {
                    let points = section
                        .get("points")
                        .and_then(Value::as_array)
                        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_POINTS_MISSING"))?;
                    for point in points {
                        let point = point
                            .as_array()
                            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_POINT_INVALID"))?;
                        let y = point.first().and_then(Value::as_f64).ok_or_else(|| {
                            invalid("ASSEMBLY_PARAMETER_POINT_COORDINATE_INVALID")
                        })?;
                        let z = point.get(1).and_then(Value::as_f64).ok_or_else(|| {
                            invalid("ASSEMBLY_PARAMETER_POINT_COORDINATE_INVALID")
                        })?;
                        maximum = maximum.max((y.powi(2) + z.powi(2)).sqrt());
                    }
                }
                maximum
            }
            Some("box") => {
                validate_primitive_box_parameters(parameters)?;
                let size = vec3(parameters.get("size_m"), "size_m", false)?;
                ((size[1] / 2.0).powi(2) + (size[2] / 2.0).powi(2)).sqrt()
            }
            Some("cylinder") => {
                validate_primitive_radius_parameters(parameters)?;
                parameters
                    .get("radius_m")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_RADIUS_MISSING"))?
            }
            _ => return Err(invalid("ASSEMBLY_PARAMETER_MUZZLE_ENVELOPE_INVALID")),
        };
    if maximum <= EPSILON {
        Err(invalid("ASSEMBLY_PARAMETER_MUZZLE_ENVELOPE_INVALID"))
    } else {
        Ok(maximum)
    }
}

fn validate_longitudinal_absolute_bounds(
    parameters: &Map<String, Value>,
) -> Result<(), RuntimeError> {
    let position = vec3(parameters.get("position_m"), "position_m", false)?;
    let sections = parameters
        .get("sections")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SECTIONS_MISSING"))?;
    for section in sections {
        let station = finite_coordinate(section.get("station_m"), "station_m")?;
        let points = section
            .get("points")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_POINTS_MISSING"))?;
        for point in points {
            let values = point
                .as_array()
                .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_POINT_INVALID"))?;
            let y = finite_coordinate(values.first(), "point-y")?;
            let z = finite_coordinate(values.get(1), "point-z")?;
            if position[0].abs() + station.abs() > ABSOLUTE_ART_BOUND_M + EPSILON
                || position[1].abs() + y.abs() > ABSOLUTE_ART_BOUND_M + EPSILON
                || position[2].abs() + z.abs() > ABSOLUTE_ART_BOUND_M + EPSILON
            {
                return Err(invalid("ASSEMBLY_PARAMETER_ABSOLUTE_ART_BOUNDS"));
            }
        }
    }
    Ok(())
}

fn validate_primitive_absolute_bounds(
    parameters: &Map<String, Value>,
    shape: &str,
    radius: f64,
) -> Result<(), RuntimeError> {
    let position = vec3(parameters.get("position_m"), "position_m", false)?;
    let extra = if shape == "cylinder" {
        parameters
            .get("height_m")
            .and_then(Value::as_f64)
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_HEIGHT_INVALID"))?
    } else {
        radius
    };
    for axis in 0..=2 {
        if position[axis].abs() + radius + extra > ABSOLUTE_ART_BOUND_M + EPSILON {
            return Err(invalid("ASSEMBLY_PARAMETER_ABSOLUTE_ART_BOUNDS"));
        }
    }
    Ok(())
}

fn validate_units(value: Option<&Value>) -> Result<(), RuntimeError> {
    let units = value
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_UNITS_INVALID"))?;
    require_exact_keys(
        units,
        &["length", "angle", "coordinate_system"],
        "ASSEMBLY_PARAMETER_UNITS",
    )?;
    if units.get("length").and_then(Value::as_str) != Some("meter")
        || units.get("angle").and_then(Value::as_str) != Some("radian")
        || units.get("coordinate_system").and_then(Value::as_str) != Some("right-handed-y-up")
    {
        return Err(invalid("ASSEMBLY_PARAMETER_UNITS_INVALID"));
    }
    Ok(())
}

fn validate_budgets(value: Option<&Value>) -> Result<(), RuntimeError> {
    let budgets = value
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_BUDGETS_INVALID"))?;
    require_exact_keys(
        budgets,
        &[
            "max_nodes",
            "max_triangles",
            "max_glb_bytes",
            "max_worker_memory_bytes",
            "max_runtime_ms",
        ],
        "ASSEMBLY_PARAMETER_BUDGETS",
    )?;
    let bounds = [
        ("max_nodes", 1, 512),
        ("max_triangles", 1, 250_000),
        ("max_glb_bytes", 1, 67_108_864),
        ("max_worker_memory_bytes", 1, 536_870_912),
        ("max_runtime_ms", 1, 10_000),
    ];
    for (key, minimum, maximum) in bounds {
        let value = budgets
            .get(key)
            .and_then(Value::as_u64)
            .filter(|value| (*value >= minimum) && (*value <= maximum))
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_BUDGET_INVALID"))?;
        let _ = value;
    }
    Ok(())
}

fn validate_vec3(value: Option<&Value>, label: &str, rotation: bool) -> Result<(), RuntimeError> {
    let _ = vec3(value, label, rotation)?;
    Ok(())
}

fn vec3(value: Option<&Value>, label: &str, rotation: bool) -> Result<[f64; 3], RuntimeError> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| invalid(format!("ASSEMBLY_PARAMETER_{label}_INVALID")))?;
    let maximum = if rotation {
        std::f64::consts::TAU
    } else {
        ABSOLUTE_ART_BOUND_M
    };
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        let value = value
            .as_f64()
            .filter(|value| value.is_finite() && value.abs() <= maximum)
            .ok_or_else(|| invalid(format!("ASSEMBLY_PARAMETER_{label}_INVALID")))?;
        result[index] = value;
    }
    Ok(result)
}

fn finite_coordinate(value: Option<&Value>, label: &str) -> Result<f64, RuntimeError> {
    value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && value.abs() <= ABSOLUTE_ART_BOUND_M)
        .ok_or_else(|| invalid(format!("ASSEMBLY_PARAMETER_{label}_INVALID")))
}

fn signed_area(points: &[[f64; 2]]) -> f64 {
    points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let next = points[(index + 1) % points.len()];
            point[0] * next[1] - next[0] * point[1]
        })
        .sum::<f64>()
        * 0.5
}

fn polygon_self_intersects(points: &[[f64; 2]]) -> bool {
    for left in 0..points.len() {
        let left_next = (left + 1) % points.len();
        for right in (left + 1)..points.len() {
            let right_next = (right + 1) % points.len();
            if left == right || left_next == right || right_next == left {
                continue;
            }
            if segments_intersect(
                points[left],
                points[left_next],
                points[right],
                points[right_next],
            ) {
                return true;
            }
        }
    }
    false
}

fn segments_intersect(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let ab_c = cross(a, b, c);
    let ab_d = cross(a, b, d);
    let cd_a = cross(c, d, a);
    let cd_b = cross(c, d, b);
    if ab_c.abs() <= EPSILON && between(a, b, c)
        || ab_d.abs() <= EPSILON && between(a, b, d)
        || cd_a.abs() <= EPSILON && between(c, d, a)
        || cd_b.abs() <= EPSILON && between(c, d, b)
    {
        return true;
    }
    (ab_c > 0.0) != (ab_d > 0.0) && (cd_a > 0.0) != (cd_b > 0.0)
}

fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn between(a: [f64; 2], b: [f64; 2], point: [f64; 2]) -> bool {
    point[0] >= a[0].min(b[0]) - EPSILON
        && point[0] <= a[0].max(b[0]) + EPSILON
        && point[1] >= a[1].min(b[1]) - EPSILON
        && point[1] <= a[1].max(b[1]) + EPSILON
}

fn catalog_has_active_operator(operator_id: &str) -> bool {
    forgecad_worker_protocol::operator_catalog()
        .get("operators")
        .and_then(Value::as_array)
        .is_some_and(|operators| {
            operators.iter().any(|operator| {
                operator.get("operator_id").and_then(Value::as_str) == Some(operator_id)
                    && operator.get("status").and_then(Value::as_str) == Some("active")
            })
        })
}

fn reject_forbidden_keys(value: &Value) -> Result<(), RuntimeError> {
    const FORBIDDEN: [&str; 10] = [
        "script",
        "scripts",
        "expression",
        "pointer",
        "json_pointer",
        "path",
        "url",
        "uri",
        "file_path",
        "env",
    ];
    match value {
        Value::Object(object) => {
            if object.keys().any(|key| FORBIDDEN.contains(&key.as_str())) {
                return Err(invalid("ASSEMBLY_PARAMETER_ARBITRARY_FIELD_REJECTED"));
            }
            for child in object.values() {
                reject_forbidden_keys(child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                reject_forbidden_keys(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn require_exact_keys(
    object: &Map<String, Value>,
    fields: &[&str],
    label: &str,
) -> Result<(), RuntimeError> {
    if object.len() != fields.len()
        || object.keys().any(|key| !fields.contains(&key.as_str()))
        || fields.iter().any(|field| !object.contains_key(*field))
    {
        return Err(invalid(format!("{label}_FIELDS_INVALID")));
    }
    Ok(())
}

fn require_id(object: &Map<String, Value>, field: &str) -> Result<(), RuntimeError> {
    if object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .is_none()
    {
        return Err(invalid(format!("ASSEMBLY_PARAMETER_{field}_INVALID")));
    }
    Ok(())
}

fn require_hash(object: &Map<String, Value>, field: &str) -> Result<(), RuntimeError> {
    if object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .is_none()
    {
        return Err(invalid(format!("ASSEMBLY_PARAMETER_{field}_INVALID")));
    }
    Ok(())
}

fn number(value: f64) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(invalid("ASSEMBLY_PARAMETER_NUMBER_NONFINITE"));
    }
    // Product-owned ratio/delta mutators must produce a representation that
    // survives the Runtime -> sibling Worker JSON boundary without changing
    // the GeometryProgram canonical hash.  Raw binary multiplication can
    // otherwise retain tails such as 0.18000000000000002 in an in-memory
    // `Number` whose canonical form differs after a serialize/parse cycle.
    // Twelve decimal places are well below the typed metre tolerances while
    // remaining deterministic for every currently bounded assembly value.
    const CANONICAL_DECIMAL_SCALE: f64 = 1_000_000_000_000.0;
    let rounded = (value * CANONICAL_DECIMAL_SCALE).round() / CANONICAL_DECIMAL_SCALE;
    let stable = if rounded == 0.0 { 0.0 } else { rounded };
    serde_json::Number::from_f64(stable)
        .map(Value::Number)
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_NUMBER_NONFINITE"))
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(message.into())
}

#[cfg(test)]
pub(crate) fn production_weapon_assembly_parameter_test_fixture() -> Value {
    tests::fixture()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn longitudinal_node(node_id: &str, position_x: f64) -> Value {
        json!({
            "node_id":node_id,
            "operator_id":LONGITUDINAL_SECTION_LOFT_OPERATOR,
            "inputs":[],
            "parameters":{
                "shape":"longitudinal-section-loft",
                "sections":[
                    {"station_m":-1.0,"points":[[-0.5,-0.35],[0.5,-0.35],[0.5,0.35],[-0.5,0.35]]},
                    {"station_m":-0.25,"points":[[-0.45,-0.30],[0.45,-0.30],[0.45,0.30],[-0.45,0.30]]},
                    {"station_m":0.25,"points":[[-0.42,-0.28],[0.42,-0.28],[0.42,0.28],[-0.42,0.28]]},
                    {"station_m":1.0,"points":[[-0.38,-0.25],[0.38,-0.25],[0.38,0.25],[-0.38,0.25]]}
                ],
                "position_m":[position_x,1.0,0.0],
                "rotation_rad":[0.0,0.0,0.0]
            }
        })
    }

    fn primitive_node() -> Value {
        json!({
            "node_id":"muzzle-core-node",
            "operator_id":PRIMITIVE_OPERATOR,
            "inputs":[],
            "parameters":{
                "shape":"cylinder",
                "radius_m":0.1,
                "height_m":0.4,
                "radial_segments":16,
                "position_m":[5.0,1.0,0.0],
                "rotation_rad":[0.0,0.0,0.0]
            }
        })
    }

    pub(super) fn fixture() -> Value {
        let nodes = vec![
            longitudinal_node("receiver-main-node", 0.0),
            longitudinal_node("receiver-upper-node", 0.8),
            longitudinal_node("receiver-lower-node", -0.8),
            longitudinal_node("muzzle-shroud-node", 4.0),
            longitudinal_node("muzzle-emitter-node", 4.5),
            primitive_node(),
            json!({
                "node_id":STOCK_UPPER_NODE_ID,"operator_id":PRIMITIVE_OPERATOR,"inputs":[],
                "parameters":{"shape":"box","size_m":[0.95,0.22,0.86],
                    "position_m":[2.05,1.68,0.0],"rotation_rad":[0.0,0.0,0.0]}
            }),
            json!({
                "node_id":STOCK_LOWER_NODE_ID,"operator_id":PRIMITIVE_OPERATOR,"inputs":[],
                "parameters":{"shape":"box","size_m":[0.9,0.16,0.72],
                    "position_m":[2.02,1.15,0.0],"rotation_rad":[0.0,0.0,0.16]}
            }),
            json!({
                "node_id":"rear-cap","operator_id":PRIMITIVE_OPERATOR,"inputs":[],
                "parameters":{"shape":"box","size_m":[0.2,0.98,0.92],
                    "position_m":[2.57,1.4,0.0],"rotation_rad":[0.0,0.0,0.0]}
            }),
            json!({
                "node_id":"locked-node",
                "operator_id":PRIMITIVE_OPERATOR,
                "inputs":[],
                "parameters":{
                    "shape":"sphere","radius_m":0.2,
                    "longitude_segments":16,"latitude_segments":8,
                    "position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]
                }
            }),
        ];
        let mut outputs = Vec::new();
        for (part_id, node_id) in [
            ("receiver-main", "receiver-main-node"),
            ("receiver-upper", "receiver-upper-node"),
            ("receiver-lower", "receiver-lower-node"),
            ("muzzle-shroud", "muzzle-shroud-node"),
            ("muzzle-emitter", "muzzle-emitter-node"),
            ("muzzle-core", "muzzle-core-node"),
            ("locked-part", "locked-node"),
        ] {
            outputs.push(json!({
                "part_id":part_id,"input_node_ids":[node_id],
                "material_zone_id":"zone-mechanical","solid":true
            }));
        }
        outputs.push(json!({
            "part_id":STOCK_PART_ID,"input_node_ids":[STOCK_UPPER_NODE_ID,STOCK_LOWER_NODE_ID],
            "material_zone_id":"zone-white-shell","solid":true
        }));
        outputs.push(json!({
            "part_id":"rear-cap","input_node_ids":["rear-cap"],
            "material_zone_id":"zone-gold-accent","solid":true
        }));
        let mut program = json!({
            "schema_version":GEOMETRY_PROGRAM_SCHEMA_VERSION,
            "project_id":"project-d1-fixture",
            "representation_plan_sha256":"a".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":32,"max_triangles":50000,"max_glb_bytes":16777216,"max_worker_memory_bytes":134217728,"max_runtime_ms":5000},
            "nodes":nodes,
            "part_outputs":outputs
        });
        let hash = canonical_json_hash(&program);
        program["canonical_sha256"] = Value::String(hash);
        program
    }

    fn mixed_d1_fixture() -> Value {
        let mut program = fixture();
        let nodes = program["nodes"].as_array_mut().unwrap();
        *nodes
            .iter_mut()
            .find(|node| node["node_id"] == "receiver-upper-node")
            .unwrap() = json!({
            "node_id":"receiver-upper-node","operator_id":PRIMITIVE_OPERATOR,"inputs":[],
            "parameters":{"shape":"box","size_m":[2.85,0.2,0.92],
                "position_m":[0.8,1.0,0.0],"rotation_rad":[0.0,0.0,0.0]}
        });
        *nodes
            .iter_mut()
            .find(|node| node["node_id"] == "muzzle-shroud-node")
            .unwrap() = json!({
            "node_id":"muzzle-shroud-node","operator_id":PRIMITIVE_OPERATOR,"inputs":[],
            "parameters":{"shape":"box","size_m":[0.72,0.62,0.9],
                "position_m":[4.0,1.0,0.0],"rotation_rad":[0.0,0.0,0.0]}
        });
        *nodes
            .iter_mut()
            .find(|node| node["node_id"] == "muzzle-emitter-node")
            .unwrap() = json!({
            "node_id":"muzzle-emitter-node","operator_id":PRIMITIVE_OPERATOR,"inputs":[],
            "parameters":{"shape":"cylinder","radius_m":0.3,"height_m":0.48,
                "radial_segments":20,"position_m":[4.5,1.0,0.0],
                "rotation_rad":[0.0,0.0,1.5708]}
        });
        nodes
            .iter_mut()
            .find(|node| node["node_id"] == "muzzle-core-node")
            .unwrap()["parameters"]["rotation_rad"] = json!([0.0, 0.0, 1.5708]);
        rehash(&mut program);
        program
    }

    fn node<'a>(program: &'a Value, node_id: &str) -> &'a Value {
        program["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["node_id"] == node_id)
            .unwrap()
    }

    fn node_mut<'a>(program: &'a mut Value, node_id: &str) -> &'a mut Value {
        program["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["node_id"] == node_id)
            .unwrap()
    }

    fn part_output_mut<'a>(program: &'a mut Value, part_id: &str) -> &'a mut Value {
        program["part_outputs"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|output| output["part_id"] == part_id)
            .unwrap()
    }

    fn rehash(program: &mut Value) {
        program.as_object_mut().unwrap().remove("canonical_sha256");
        program["canonical_sha256"] = Value::String(canonical_json_hash(program));
    }

    #[test]
    fn d1_trigger_guard_aperture_is_one_fixed_xy_profile_and_worker_compiles() {
        let source = crate::production_weapon_d1_seed::materialize("project-trigger-aperture")
            .expect("closed D1 source");
        let proposal = production_weapon_trigger_guard_aperture_trial_mutate(&source)
            .expect("fixed trigger-guard aperture proposal");
        assert_eq!(
            production_weapon_trigger_guard_aperture_profile_id(),
            "trigger-guard-side-aperture-xy@1"
        );
        assert_eq!(
            node(&proposal, TRIGGER_GUARD_NODE_ID)["operator_id"],
            PROFILE_EXTRUDE_OPERATOR
        );
        assert_eq!(
            node(&proposal, TRIGGER_GUARD_NODE_ID)["parameters"]["profile"]
                .as_array()
                .map(Vec::len),
            Some(8)
        );
        assert_eq!(proposal["part_outputs"], source["part_outputs"]);
        for source_node in source["nodes"].as_array().expect("source nodes") {
            let node_id = source_node["node_id"].as_str().expect("source node id");
            if node_id != TRIGGER_GUARD_NODE_ID {
                assert_eq!(node(&proposal, node_id), source_node);
            }
        }
        let artifact = forgecad_geometry_worker::compile_geometry_program(&proposal)
            .expect("fixed trigger aperture compiles through Geometry Worker");
        assert_eq!(artifact.part_ids.len(), 23);
        assert!(artifact
            .part_ids
            .iter()
            .any(|part_id| part_id == TRIGGER_GUARD_PART_ID));
    }

    #[test]
    fn d1_fixture_eight_supported_parameters_apply_as_typed_aggregates() {
        let program = fixture();
        for parameter_id in SUPPORTED_PARAMETER_IDS {
            let value = match parameter_id {
                "receiver-envelope-shoulder" => 0.1,
                "stock-open-frame-clearance" => 0.30,
                "stock-open-frame-angle" => 0.12,
                _ => 1.1,
            };
            let draft = production_weapon_assembly_parameter_mutate(&program, parameter_id, value)
                .expect(parameter_id);
            assert!(draft.get("canonical_sha256").is_none());
            assert_eq!(draft["project_id"], program["project_id"]);
            assert_eq!(draft["part_outputs"], program["part_outputs"]);
            assert_eq!(node(&draft, "locked-node"), node(&program, "locked-node"));
        }
        let width =
            production_weapon_assembly_parameter_mutate(&program, "receiver-envelope-width", 1.1)
                .unwrap();
        assert_eq!(
            node(&width, "receiver-main-node")["parameters"]["sections"][0]["points"][0][1],
            json!(-0.385)
        );
        assert_eq!(
            node(&width, "receiver-main-node")["parameters"]["sections"][0]["points"][0][0],
            json!(-0.5)
        );

        let height =
            production_weapon_assembly_parameter_mutate(&program, "receiver-envelope-height", 1.1)
                .unwrap();
        assert_eq!(
            node(&height, "receiver-main-node")["parameters"]["sections"][0]["points"][0][0],
            json!(-0.55)
        );

        let shoulder = production_weapon_assembly_parameter_mutate(
            &program,
            "receiver-envelope-shoulder",
            0.1,
        )
        .unwrap();
        assert_eq!(
            node(&shoulder, "receiver-main-node")["parameters"]["sections"][0]["station_m"],
            json!(-1.0)
        );
        assert_eq!(
            node(&shoulder, "receiver-main-node")["parameters"]["sections"][1]["station_m"],
            json!(-0.15)
        );
        assert_eq!(
            node(&shoulder, "receiver-main-node")["parameters"]["sections"][2]["station_m"],
            json!(0.35)
        );
        assert_eq!(
            node(&shoulder, "receiver-main-node")["parameters"]["sections"][3]["station_m"],
            json!(1.0)
        );

        let shroud = production_weapon_assembly_parameter_mutate(
            &program,
            "muzzle-axis-shroud-envelope",
            1.1,
        )
        .unwrap();
        assert_eq!(
            node(&shroud, "muzzle-shroud-node")["parameters"]["sections"][0]["points"][0],
            json!([-0.55, -0.385])
        );
        assert_eq!(
            node(&shroud, "muzzle-shroud-node")["parameters"]["sections"][0]["station_m"],
            json!(-1.1)
        );
        assert_eq!(
            node(&shroud, "muzzle-shroud-node")["parameters"]["position_m"],
            node(&program, "muzzle-shroud-node")["parameters"]["position_m"]
        );

        let emitter = production_weapon_assembly_parameter_mutate(
            &program,
            "muzzle-axis-emitter-envelope",
            1.1,
        )
        .unwrap();
        assert_eq!(
            node(&emitter, "muzzle-emitter-node")["parameters"]["sections"][0]["points"][0],
            json!([-0.55, -0.385])
        );

        let core =
            production_weapon_assembly_parameter_mutate(&program, "muzzle-axis-core-aperture", 1.1)
                .unwrap();
        let radius = node(&core, "muzzle-core-node")["parameters"]["radius_m"]
            .as_f64()
            .unwrap();
        assert!((radius - 0.11).abs() <= EPSILON);

        let clearance = production_weapon_assembly_parameter_mutate(
            &program,
            "stock-open-frame-clearance",
            0.30,
        )
        .unwrap();
        assert_eq!(
            node(&clearance, STOCK_UPPER_NODE_ID)["parameters"]["size_m"][2],
            node(&program, STOCK_UPPER_NODE_ID)["parameters"]["size_m"][2]
        );
        assert_eq!(
            node(&clearance, STOCK_LOWER_NODE_ID)["parameters"]["size_m"][2],
            node(&program, STOCK_LOWER_NODE_ID)["parameters"]["size_m"][2]
        );
        let mut rebound = clearance.clone();
        rehash(&mut rebound);
        let rebound_index = ProgramIndex::parse_with_expected_hash(&rebound, None).unwrap();
        assert!((stock_open_frame_clearance(&rebound_index).unwrap() - 0.30).abs() <= EPSILON);

        let angle =
            production_weapon_assembly_parameter_mutate(&program, "stock-open-frame-angle", 0.12)
                .unwrap();
        assert_eq!(
            node(&angle, STOCK_LOWER_NODE_ID)["parameters"]["rotation_rad"][2],
            json!(0.12)
        );
    }

    #[test]
    fn ratio_mutation_is_canonical_across_worker_json_roundtrip() {
        let program = mixed_d1_fixture();
        let draft =
            production_weapon_assembly_parameter_mutate(&program, "receiver-envelope-width", 0.9)
                .expect("receiver width mutation");
        let encoded = serde_json::to_vec(&draft).expect("serialize mutated draft");
        let roundtrip: Value = serde_json::from_slice(&encoded).expect("parse mutated draft");
        assert_eq!(roundtrip, draft);
        assert_eq!(canonical_json_hash(&roundtrip), canonical_json_hash(&draft));
        assert_eq!(
            node(&draft, "receiver-upper-node")["parameters"]["size_m"][2],
            json!(0.828)
        );
    }

    #[test]
    fn stock_open_frame_mutation_is_canonical_and_depth_preserving() {
        let program = mixed_d1_fixture();
        let draft = production_weapon_assembly_parameter_mutate(
            &program,
            "stock-open-frame-clearance",
            0.22,
        )
        .expect("stock clearance mutation");
        let roundtrip: Value =
            serde_json::from_slice(&serde_json::to_vec(&draft).expect("serialize stock draft"))
                .expect("parse stock draft");
        assert_eq!(roundtrip, draft);
        assert_eq!(canonical_json_hash(&roundtrip), canonical_json_hash(&draft));
        for node_id in [STOCK_UPPER_NODE_ID, STOCK_LOWER_NODE_ID] {
            assert_eq!(
                node(&draft, node_id)["parameters"]["position_m"][2],
                node(&program, node_id)["parameters"]["position_m"][2]
            );
            assert_eq!(
                node(&draft, node_id)["parameters"]["size_m"][2],
                node(&program, node_id)["parameters"]["size_m"][2]
            );
        }
        assert_eq!(node(&draft, "rear-cap"), node(&program, "rear-cap"));
        let mut rebound = draft.clone();
        rehash(&mut rebound);
        let rebound_index = ProgramIndex::parse_with_expected_hash(&rebound, None).unwrap();
        assert!((stock_open_frame_clearance(&rebound_index).unwrap() - 0.22).abs() <= EPSILON);
    }

    #[test]
    fn stock_plane_position_trial_moves_both_x_coordinates_and_rehashes() {
        let program = mixed_d1_fixture();
        let delta_x_m = 0.12;
        let trial = production_weapon_stock_plane_position_trial_mutate(&program, delta_x_m)
            .expect("stock plane position trial");

        let mut without_hash = trial.clone();
        without_hash
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        assert_eq!(
            trial["canonical_sha256"],
            canonical_json_hash(&without_hash)
        );
        ProgramIndex::parse_with_expected_hash(&trial, None).expect("rehashed trial validates");

        for node_id in [STOCK_UPPER_NODE_ID, STOCK_LOWER_NODE_ID] {
            let before_position = node(&program, node_id)["parameters"]["position_m"]
                .as_array()
                .unwrap();
            let after_position = node(&trial, node_id)["parameters"]["position_m"]
                .as_array()
                .unwrap();
            assert!(
                (after_position[0].as_f64().unwrap()
                    - before_position[0].as_f64().unwrap()
                    - delta_x_m)
                    .abs()
                    <= EPSILON
            );
            assert_eq!(after_position[1], before_position[1]);
            assert_eq!(after_position[2], before_position[2]);
        }
        let upper_x = node(&trial, STOCK_UPPER_NODE_ID)["parameters"]["position_m"][0]
            .as_f64()
            .unwrap();
        let lower_x = node(&trial, STOCK_LOWER_NODE_ID)["parameters"]["position_m"][0]
            .as_f64()
            .unwrap();
        assert!((upper_x - lower_x - STOCK_PLANE_POSITION_SEPARATION_M).abs() <= EPSILON);
        assert_eq!(node(&trial, "rear-cap"), node(&program, "rear-cap"));
        assert_eq!(trial["part_outputs"], program["part_outputs"]);

        // Restore only the two intentionally changed coordinates.  Equality
        // of the remaining canonical JSON proves all other fields are stable.
        let mut normalized_trial = trial.clone();
        let mut normalized_program = program.clone();
        for node_id in [STOCK_UPPER_NODE_ID, STOCK_LOWER_NODE_ID] {
            let baseline_x = node(&program, node_id)["parameters"]["position_m"][0].clone();
            node_mut(&mut normalized_trial, node_id)["parameters"]["position_m"][0] = baseline_x;
        }
        normalized_trial
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        normalized_program
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        assert_eq!(normalized_trial, normalized_program);
    }

    #[test]
    fn stock_plane_position_trial_is_bounded_and_fail_closed() {
        let program = fixture();
        let baseline = program.clone();
        for delta_x_m in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            STOCK_PLANE_POSITION_DELTA_MIN_M - 0.000_001,
            STOCK_PLANE_POSITION_DELTA_MAX_M + 0.000_001,
        ] {
            assert!(
                production_weapon_stock_plane_position_trial_mutate(&program, delta_x_m).is_err(),
                "delta must fail closed: {delta_x_m:?}"
            );
        }

        let mut missing_node = program.clone();
        missing_node["nodes"]
            .as_array_mut()
            .unwrap()
            .retain(|node| node["node_id"] != STOCK_LOWER_NODE_ID);
        rehash(&mut missing_node);
        assert!(production_weapon_stock_plane_position_trial_mutate(&missing_node, 0.1).is_err());

        let mut wrong_separation = program.clone();
        node_mut(&mut wrong_separation, STOCK_LOWER_NODE_ID)["parameters"]["position_m"][0] =
            json!(2.01);
        rehash(&mut wrong_separation);
        assert!(
            production_weapon_stock_plane_position_trial_mutate(&wrong_separation, 0.1).is_err()
        );

        let mut malformed_position = program.clone();
        node_mut(&mut malformed_position, STOCK_UPPER_NODE_ID)["parameters"]["position_m"][0] =
            json!("not-a-finite-number");
        rehash(&mut malformed_position);
        assert!(
            production_weapon_stock_plane_position_trial_mutate(&malformed_position, 0.1).is_err()
        );
        assert_eq!(program, baseline);
    }

    #[test]
    fn stock_upper_inner_span_trial_keeps_cap_endpoint_and_isolates_upper_x() {
        let program = mixed_d1_fixture();
        let trial = production_weapon_stock_upper_inner_span_trial_mutate(&program, 0.75)
            .expect("upper inner-span trial");
        let before_upper = node(&program, STOCK_UPPER_NODE_ID);
        let after_upper = node(&trial, STOCK_UPPER_NODE_ID);
        assert_eq!(after_upper["parameters"]["position_m"][0], json!(2.15));
        assert_eq!(after_upper["parameters"]["size_m"][0], json!(0.75));
        let before_endpoint = before_upper["parameters"]["position_m"][0]
            .as_f64()
            .unwrap()
            + before_upper["parameters"]["size_m"][0].as_f64().unwrap() * 0.5;
        let after_endpoint = after_upper["parameters"]["position_m"][0].as_f64().unwrap()
            + after_upper["parameters"]["size_m"][0].as_f64().unwrap() * 0.5;
        assert!((before_endpoint - after_endpoint).abs() <= EPSILON);
        assert_eq!(
            after_upper["parameters"]["position_m"][1],
            before_upper["parameters"]["position_m"][1]
        );
        assert_eq!(
            after_upper["parameters"]["position_m"][2],
            before_upper["parameters"]["position_m"][2]
        );
        assert_eq!(
            after_upper["parameters"]["size_m"][1],
            before_upper["parameters"]["size_m"][1]
        );
        assert_eq!(
            after_upper["parameters"]["size_m"][2],
            before_upper["parameters"]["size_m"][2]
        );
        assert_eq!(
            after_upper["parameters"]["rotation_rad"],
            before_upper["parameters"]["rotation_rad"]
        );
        assert_eq!(
            node(&trial, STOCK_LOWER_NODE_ID),
            node(&program, STOCK_LOWER_NODE_ID)
        );
        assert_eq!(node(&trial, "rear-cap"), node(&program, "rear-cap"));
        assert_eq!(trial["part_outputs"], program["part_outputs"]);

        let mut normalized_trial = trial.clone();
        node_mut(&mut normalized_trial, STOCK_UPPER_NODE_ID)["parameters"]["position_m"][0] =
            before_upper["parameters"]["position_m"][0].clone();
        node_mut(&mut normalized_trial, STOCK_UPPER_NODE_ID)["parameters"]["size_m"][0] =
            before_upper["parameters"]["size_m"][0].clone();
        normalized_trial
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        let mut normalized_program = program.clone();
        normalized_program
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        assert_eq!(normalized_trial, normalized_program);
    }

    #[test]
    fn stock_upper_inner_span_trial_is_bounded_and_shrink_only() {
        let program = mixed_d1_fixture();
        for span_m in [
            f64::NAN,
            f64::INFINITY,
            STOCK_UPPER_INNER_SPAN_MIN_M - 0.000_001,
            STOCK_UPPER_INNER_SPAN_MAX_M + 0.000_001,
        ] {
            assert!(
                production_weapon_stock_upper_inner_span_trial_mutate(&program, span_m).is_err(),
                "span must fail closed: {span_m:?}"
            );
        }
        let contracted =
            production_weapon_stock_upper_inner_span_trial_mutate(&program, 0.85).unwrap();
        assert!(production_weapon_stock_upper_inner_span_trial_mutate(&contracted, 0.90).is_err());
    }

    #[test]
    fn stock_upper_profile_trial_is_closed_typed_and_isolates_upper_node() {
        let program = mixed_d1_fixture();
        let trial = production_weapon_stock_upper_profile_trial_mutate(&program, 0.85)
            .expect("upper profile trial");
        let upper = node(&trial, STOCK_UPPER_NODE_ID);
        assert_eq!(upper["operator_id"], PROFILE_LOFT_V2_OPERATOR);
        assert_eq!(upper["inputs"], json!([]));
        assert_eq!(upper["parameters"]["shape"], "profile-loft-v2");
        assert_eq!(upper["parameters"]["resample_points"], 8);
        assert_eq!(upper["parameters"]["interpolation"], "linear");
        assert_eq!(upper["parameters"]["interpolation_rings"], 0);
        assert_eq!(upper["parameters"]["preserve_corners"], true);
        assert_eq!(upper["parameters"]["position_m"], json!([2.05, 1.68, 0.0]));
        assert_eq!(
            upper["parameters"]["rotation_rad"],
            json!([0.0, std::f64::consts::FRAC_PI_2, 0.0])
        );
        let profiles = upper["parameters"]["profiles"].as_array().unwrap();
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0]["station_m"], json!(-0.43));
        assert_eq!(profiles[1]["station_m"], json!(0.43));
        assert_eq!(profiles[0]["points"], profiles[1]["points"]);
        assert_eq!(profiles[0]["points"].as_array().unwrap().len(), 8);
        assert_eq!(
            profiles[0]["corner_indices"],
            json!([0, 1, 2, 3, 4, 5, 6, 7])
        );
        assert_eq!(
            node(&trial, STOCK_LOWER_NODE_ID),
            node(&program, STOCK_LOWER_NODE_ID)
        );
        assert_eq!(node(&trial, "rear-cap"), node(&program, "rear-cap"));
        assert_eq!(trial["part_outputs"], program["part_outputs"]);
        for candidate_node in trial["nodes"].as_array().unwrap() {
            if candidate_node["node_id"] == STOCK_UPPER_NODE_ID {
                continue;
            }
            assert_eq!(
                candidate_node,
                program["nodes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|baseline| baseline["node_id"] == candidate_node["node_id"])
                    .unwrap()
            );
        }
        ProgramIndex::parse_with_expected_hash(&trial, None).expect("profile trial validates");
    }

    #[test]
    fn stock_upper_profile_trial_admits_only_two_closed_variants() {
        let program = mixed_d1_fixture();
        for inner_span_m in [f64::NAN, 0.70, 0.80, 0.90, 0.95] {
            assert!(
                production_weapon_stock_upper_profile_trial_mutate(&program, inner_span_m).is_err(),
                "variant must fail closed: {inner_span_m:?}"
            );
        }
        for inner_span_m in STOCK_UPPER_PROFILE_VARIANTS_M {
            assert!(
                production_weapon_stock_upper_profile_trial_mutate(&program, inner_span_m).is_ok()
            );
        }
    }

    #[test]
    fn stock_upper_profile_lip_trial_changes_only_two_inner_points_per_station() {
        let program = mixed_d1_fixture();
        let baseline_profile =
            production_weapon_stock_upper_profile_trial_mutate(&program, 0.85).unwrap();
        let trial =
            production_weapon_stock_upper_profile_lip_trial_mutate(&program, -0.035).unwrap();
        let baseline_upper = node(&baseline_profile, STOCK_UPPER_NODE_ID);
        let trial_upper = node(&trial, STOCK_UPPER_NODE_ID);
        let baseline_profiles = baseline_upper["parameters"]["profiles"].as_array().unwrap();
        let trial_profiles = trial_upper["parameters"]["profiles"].as_array().unwrap();
        for (baseline_station, trial_station) in baseline_profiles.iter().zip(trial_profiles) {
            assert_eq!(baseline_station["station_m"], trial_station["station_m"]);
            assert_eq!(
                baseline_station["corner_indices"],
                trial_station["corner_indices"]
            );
            let baseline_points = baseline_station["points"].as_array().unwrap();
            let trial_points = trial_station["points"].as_array().unwrap();
            for point_index in 0..8 {
                if matches!(point_index, 1 | 2) {
                    assert_eq!(trial_points[point_index][0], json!(-0.035));
                    assert_eq!(
                        trial_points[point_index][1],
                        baseline_points[point_index][1]
                    );
                } else {
                    assert_eq!(trial_points[point_index], baseline_points[point_index]);
                }
            }
        }
        let mut normalized_trial = trial.clone();
        let normalized_upper = node_mut(&mut normalized_trial, STOCK_UPPER_NODE_ID);
        normalized_upper["parameters"]["profiles"] =
            baseline_upper["parameters"]["profiles"].clone();
        normalized_trial
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        let mut normalized_baseline = baseline_profile.clone();
        normalized_baseline
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        assert_eq!(normalized_trial, normalized_baseline);
    }

    #[test]
    fn stock_upper_profile_lip_trial_admits_only_two_variants() {
        let program = mixed_d1_fixture();
        for lip_y_m in [f64::NAN, -0.11, -0.055, 0.0] {
            assert!(
                production_weapon_stock_upper_profile_lip_trial_mutate(&program, lip_y_m).is_err(),
                "lip variant must fail closed: {lip_y_m:?}"
            );
        }
        for lip_y_m in STOCK_UPPER_PROFILE_LIP_VARIANTS_M {
            assert!(
                production_weapon_stock_upper_profile_lip_trial_mutate(&program, lip_y_m).is_ok()
            );
        }
    }

    #[test]
    fn stock_upper_profile_04v_lip_trial_changes_only_two_inner_points_per_station() {
        let program = mixed_d1_fixture();
        let baseline_profile =
            production_weapon_stock_upper_profile_trial_mutate(&program, 0.85).unwrap();
        let baseline_upper = node(&baseline_profile, STOCK_UPPER_NODE_ID);
        for lip_y_m in STOCK_UPPER_PROFILE_04V_LIP_VARIANTS_M {
            let trial =
                production_weapon_stock_upper_profile_04v_lip_trial_mutate(&program, lip_y_m)
                    .unwrap();
            let trial_upper = node(&trial, STOCK_UPPER_NODE_ID);
            assert_eq!(trial_upper["operator_id"], PROFILE_LOFT_V2_OPERATOR);
            assert_eq!(
                trial_upper["parameters"]["position_m"],
                baseline_upper["parameters"]["position_m"]
            );
            assert_eq!(
                trial_upper["parameters"]["rotation_rad"],
                baseline_upper["parameters"]["rotation_rad"]
            );
            let baseline_profiles = baseline_upper["parameters"]["profiles"].as_array().unwrap();
            let trial_profiles = trial_upper["parameters"]["profiles"].as_array().unwrap();
            assert_eq!(trial_profiles.len(), 2);
            for (baseline_station, trial_station) in baseline_profiles.iter().zip(trial_profiles) {
                assert_eq!(baseline_station["station_m"], trial_station["station_m"]);
                assert_eq!(
                    baseline_station["corner_indices"],
                    trial_station["corner_indices"]
                );
                let baseline_points = baseline_station["points"].as_array().unwrap();
                let trial_points = trial_station["points"].as_array().unwrap();
                for point_index in 0..8 {
                    if matches!(point_index, 1 | 2) {
                        assert_eq!(trial_points[point_index][0], json!(lip_y_m));
                        assert_eq!(
                            trial_points[point_index][1],
                            baseline_points[point_index][1]
                        );
                    } else {
                        assert_eq!(trial_points[point_index], baseline_points[point_index]);
                    }
                }
            }
            let mut normalized_trial = trial.clone();
            let normalized_upper = node_mut(&mut normalized_trial, STOCK_UPPER_NODE_ID);
            normalized_upper["parameters"]["profiles"] =
                baseline_upper["parameters"]["profiles"].clone();
            normalized_trial
                .as_object_mut()
                .unwrap()
                .remove("canonical_sha256");
            let mut normalized_baseline = baseline_profile.clone();
            normalized_baseline
                .as_object_mut()
                .unwrap()
                .remove("canonical_sha256");
            assert_eq!(normalized_trial, normalized_baseline);
        }
    }

    #[test]
    fn stock_upper_profile_04v_lip_trial_rejects_unlisted_or_nonfinite_values() {
        let program = mixed_d1_fixture();
        for lip_y_m in [
            f64::NAN,
            f64::INFINITY,
            -0.11,
            -0.075,
            -0.055,
            -0.035,
            0.0,
            0.02,
        ] {
            assert!(
                production_weapon_stock_upper_profile_04v_lip_trial_mutate(&program, lip_y_m)
                    .is_err(),
                "04V lip variant must fail closed: {lip_y_m:?}"
            );
        }
        for lip_y_m in STOCK_UPPER_PROFILE_04V_LIP_VARIANTS_M {
            assert!(
                production_weapon_stock_upper_profile_04v_lip_trial_mutate(&program, lip_y_m)
                    .is_ok()
            );
        }
    }

    #[test]
    fn stock_upper_profile_04w_lip_trial_changes_only_two_inner_points_per_station() {
        let program = mixed_d1_fixture();
        let baseline_profile =
            production_weapon_stock_upper_profile_trial_mutate(&program, 0.85).unwrap();
        let baseline_upper = node(&baseline_profile, STOCK_UPPER_NODE_ID);
        for lip_y_m in STOCK_UPPER_PROFILE_04W_LIP_VARIANTS_M {
            let trial =
                production_weapon_stock_upper_profile_04w_lip_trial_mutate(&program, lip_y_m)
                    .unwrap();
            let trial_upper = node(&trial, STOCK_UPPER_NODE_ID);
            assert_eq!(trial_upper["operator_id"], PROFILE_LOFT_V2_OPERATOR);
            assert_eq!(
                trial_upper["parameters"]["position_m"],
                baseline_upper["parameters"]["position_m"]
            );
            assert_eq!(
                trial_upper["parameters"]["rotation_rad"],
                baseline_upper["parameters"]["rotation_rad"]
            );
            let baseline_profiles = baseline_upper["parameters"]["profiles"].as_array().unwrap();
            let trial_profiles = trial_upper["parameters"]["profiles"].as_array().unwrap();
            assert_eq!(trial_profiles.len(), 2);
            for (baseline_station, trial_station) in baseline_profiles.iter().zip(trial_profiles) {
                assert_eq!(baseline_station["station_m"], trial_station["station_m"]);
                assert_eq!(
                    baseline_station["corner_indices"],
                    trial_station["corner_indices"]
                );
                let baseline_points = baseline_station["points"].as_array().unwrap();
                let trial_points = trial_station["points"].as_array().unwrap();
                for point_index in 0..8 {
                    if matches!(point_index, 1 | 2) {
                        assert_eq!(trial_points[point_index][0], json!(lip_y_m));
                        assert_eq!(
                            trial_points[point_index][1],
                            baseline_points[point_index][1]
                        );
                    } else {
                        assert_eq!(trial_points[point_index], baseline_points[point_index]);
                    }
                }
            }
            let mut normalized_trial = trial.clone();
            let normalized_upper = node_mut(&mut normalized_trial, STOCK_UPPER_NODE_ID);
            normalized_upper["parameters"]["profiles"] =
                baseline_upper["parameters"]["profiles"].clone();
            normalized_trial
                .as_object_mut()
                .unwrap()
                .remove("canonical_sha256");
            let mut normalized_baseline = baseline_profile.clone();
            normalized_baseline
                .as_object_mut()
                .unwrap()
                .remove("canonical_sha256");
            assert_eq!(normalized_trial, normalized_baseline);
        }
    }

    #[test]
    fn stock_upper_profile_04w_lip_trial_rejects_unlisted_or_nonfinite_values() {
        let program = mixed_d1_fixture();
        for lip_y_m in [f64::NAN, f64::INFINITY, -0.015, 0.005, 0.0, 0.065, 0.11] {
            assert!(
                production_weapon_stock_upper_profile_04w_lip_trial_mutate(&program, lip_y_m)
                    .is_err(),
                "04W lip variant must fail closed: {lip_y_m:?}"
            );
        }
        for lip_y_m in STOCK_UPPER_PROFILE_04W_LIP_VARIANTS_M {
            assert!(
                production_weapon_stock_upper_profile_04w_lip_trial_mutate(&program, lip_y_m)
                    .is_ok()
            );
        }
    }

    #[test]
    fn stock_upper_profile_04x_boundary_translation_moves_only_inner_boundary_per_station() {
        let program = mixed_d1_fixture();
        let baseline_profile =
            production_weapon_stock_upper_profile_trial_mutate(&program, 0.85).unwrap();
        let baseline_upper = node(&baseline_profile, STOCK_UPPER_NODE_ID);
        for delta_x_m in STOCK_UPPER_PROFILE_04X_BOUNDARY_TRANSLATION_VARIANTS_M {
            let trial =
                production_weapon_stock_upper_profile_04x_boundary_translation_trial_mutate(
                    &program, delta_x_m,
                )
                .unwrap();
            let trial_upper = node(&trial, STOCK_UPPER_NODE_ID);
            assert_eq!(trial_upper["operator_id"], PROFILE_LOFT_V2_OPERATOR);
            assert_eq!(
                trial_upper["parameters"]["position_m"],
                baseline_upper["parameters"]["position_m"]
            );
            assert_eq!(
                trial_upper["parameters"]["rotation_rad"],
                baseline_upper["parameters"]["rotation_rad"]
            );
            let baseline_profiles = baseline_upper["parameters"]["profiles"].as_array().unwrap();
            let trial_profiles = trial_upper["parameters"]["profiles"].as_array().unwrap();
            assert_eq!(trial_profiles.len(), 2);
            for (baseline_station, trial_station) in baseline_profiles.iter().zip(trial_profiles) {
                assert_eq!(baseline_station["station_m"], trial_station["station_m"]);
                assert_eq!(
                    baseline_station["corner_indices"],
                    trial_station["corner_indices"]
                );
                let baseline_points = baseline_station["points"].as_array().unwrap();
                let trial_points = trial_station["points"].as_array().unwrap();
                for point_index in 0..8 {
                    if point_index <= 3 {
                        let baseline_x = baseline_points[point_index][0].as_f64().unwrap();
                        assert_eq!(trial_points[point_index][0], json!(baseline_x + delta_x_m));
                        assert_eq!(
                            trial_points[point_index][1],
                            baseline_points[point_index][1]
                        );
                    } else {
                        assert_eq!(trial_points[point_index], baseline_points[point_index]);
                    }
                }
            }
            let mut normalized_trial = trial.clone();
            let normalized_upper = node_mut(&mut normalized_trial, STOCK_UPPER_NODE_ID);
            normalized_upper["parameters"]["profiles"] =
                baseline_upper["parameters"]["profiles"].clone();
            normalized_trial
                .as_object_mut()
                .unwrap()
                .remove("canonical_sha256");
            let mut normalized_baseline = baseline_profile.clone();
            normalized_baseline
                .as_object_mut()
                .unwrap()
                .remove("canonical_sha256");
            assert_eq!(normalized_trial, normalized_baseline);
        }
    }

    #[test]
    fn stock_upper_profile_04x_boundary_translation_rejects_unlisted_or_nonfinite_values() {
        let program = mixed_d1_fixture();
        for delta_x_m in [f64::NAN, f64::INFINITY, -0.02, 0.0, 0.015, 0.045, 0.11] {
            assert!(
                production_weapon_stock_upper_profile_04x_boundary_translation_trial_mutate(
                    &program, delta_x_m
                )
                .is_err(),
                "04X boundary translation must fail closed: {delta_x_m:?}"
            );
        }
        for delta_x_m in STOCK_UPPER_PROFILE_04X_BOUNDARY_TRANSLATION_VARIANTS_M {
            assert!(
                production_weapon_stock_upper_profile_04x_boundary_translation_trial_mutate(
                    &program, delta_x_m
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn stock_upper_profile_04z_station_isolation_changes_only_selected_station_inner_boundary() {
        let program = mixed_d1_fixture();
        let baseline_profile =
            production_weapon_stock_upper_profile_trial_mutate(&program, 0.85).unwrap();
        let baseline_upper = node(&baseline_profile, STOCK_UPPER_NODE_ID);
        let baseline_profiles = baseline_upper["parameters"]["profiles"].as_array().unwrap();

        for station_index in [0_usize, 1_usize] {
            let trial = production_weapon_stock_upper_profile_04z_station_isolation_trial_mutate(
                &program,
                station_index,
            )
            .expect("04Z station-isolation trial");
            let trial_upper = node(&trial, STOCK_UPPER_NODE_ID);
            assert_eq!(trial_upper["operator_id"], PROFILE_LOFT_V2_OPERATOR);
            assert_eq!(
                trial_upper["parameters"]["position_m"],
                baseline_upper["parameters"]["position_m"]
            );
            assert_eq!(
                trial_upper["parameters"]["rotation_rad"],
                baseline_upper["parameters"]["rotation_rad"]
            );
            let trial_profiles = trial_upper["parameters"]["profiles"].as_array().unwrap();
            assert_eq!(trial_profiles.len(), 2);
            for (profile_index, (baseline_station, trial_station)) in
                baseline_profiles.iter().zip(trial_profiles).enumerate()
            {
                assert_eq!(baseline_station["station_m"], trial_station["station_m"]);
                assert_eq!(
                    baseline_station["corner_indices"],
                    trial_station["corner_indices"]
                );
                let baseline_points = baseline_station["points"].as_array().unwrap();
                let trial_points = trial_station["points"].as_array().unwrap();
                for point_index in 0..8 {
                    if profile_index == station_index && point_index <= 3 {
                        let baseline_x = baseline_points[point_index][0].as_f64().unwrap();
                        assert_eq!(
                            trial_points[point_index][0],
                            json!(baseline_x + STOCK_UPPER_PROFILE_04Z_STATION_DELTA_M)
                        );
                        assert_eq!(
                            trial_points[point_index][1],
                            baseline_points[point_index][1]
                        );
                    } else {
                        assert_eq!(trial_points[point_index], baseline_points[point_index]);
                    }
                }
            }
            assert_eq!(
                node(&trial, STOCK_LOWER_NODE_ID),
                node(&program, STOCK_LOWER_NODE_ID)
            );
            assert_eq!(node(&trial, "rear-cap"), node(&program, "rear-cap"));
            assert_eq!(trial["part_outputs"], program["part_outputs"]);
            ProgramIndex::parse_with_expected_hash(&trial, None)
                .expect("04Z trial must remain a valid hash-bound program");
        }
    }

    #[test]
    fn stock_upper_profile_04z_station_isolation_rejects_unlisted_station() {
        let program = mixed_d1_fixture();
        for station_index in [2_usize, usize::MAX] {
            assert!(
                production_weapon_stock_upper_profile_04z_station_isolation_trial_mutate(
                    &program,
                    station_index,
                )
                .is_err(),
                "04Z station index must fail closed: {station_index}"
            );
        }
    }

    #[test]
    fn stock_upper_profile_cap_lip_trial_changes_only_point_two_longitudinal_coordinate() {
        let program = mixed_d1_fixture();
        let baseline_profile =
            production_weapon_stock_upper_profile_trial_mutate(&program, 0.85).unwrap();
        let trial = production_weapon_stock_upper_profile_cap_lip_trial_mutate(&program, -0.405)
            .expect("cap-facing inner-lip trial");
        let baseline_upper = node(&baseline_profile, STOCK_UPPER_NODE_ID);
        let trial_upper = node(&trial, STOCK_UPPER_NODE_ID);
        assert_eq!(trial_upper["operator_id"], PROFILE_LOFT_V2_OPERATOR);
        assert_eq!(
            trial_upper["parameters"]["position_m"],
            baseline_upper["parameters"]["position_m"]
        );
        assert_eq!(
            trial_upper["parameters"]["rotation_rad"],
            baseline_upper["parameters"]["rotation_rad"]
        );
        let baseline_profiles = baseline_upper["parameters"]["profiles"].as_array().unwrap();
        let trial_profiles = trial_upper["parameters"]["profiles"].as_array().unwrap();
        assert_eq!(trial_profiles.len(), 2);
        for (baseline_station, trial_station) in baseline_profiles.iter().zip(trial_profiles) {
            assert_eq!(baseline_station["station_m"], trial_station["station_m"]);
            assert_eq!(
                baseline_station["corner_indices"],
                trial_station["corner_indices"]
            );
            let baseline_points = baseline_station["points"].as_array().unwrap();
            let trial_points = trial_station["points"].as_array().unwrap();
            for point_index in 0..8 {
                if point_index == 2 {
                    assert_eq!(
                        trial_points[point_index][0],
                        baseline_points[point_index][0]
                    );
                    assert_eq!(trial_points[point_index][1], json!(-0.405));
                } else {
                    assert_eq!(trial_points[point_index], baseline_points[point_index]);
                }
            }
            assert_eq!(trial_points[1][1], json!(0.425));
            assert_eq!(trial_points[2][0], baseline_points[2][0]);
        }
        assert_eq!(
            node(&trial, STOCK_LOWER_NODE_ID),
            node(&program, STOCK_LOWER_NODE_ID)
        );
        assert_eq!(node(&trial, "rear-cap"), node(&program, "rear-cap"));
        assert_eq!(trial["part_outputs"], program["part_outputs"]);

        let mut normalized_trial = trial.clone();
        let normalized_upper = node_mut(&mut normalized_trial, STOCK_UPPER_NODE_ID);
        normalized_upper["parameters"]["profiles"] =
            baseline_upper["parameters"]["profiles"].clone();
        normalized_trial
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        let mut normalized_baseline = baseline_profile.clone();
        normalized_baseline
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        assert_eq!(normalized_trial, normalized_baseline);
        ProgramIndex::parse_with_expected_hash(&trial, None).expect("cap-lip trial validates");
    }

    #[test]
    fn stock_upper_profile_cap_lip_trial_admits_only_two_variants() {
        let program = mixed_d1_fixture();
        for cap_lip_longitudinal_m in [f64::NAN, -0.425, -0.415, -0.4051, -0.3951, 0.0] {
            assert!(
                production_weapon_stock_upper_profile_cap_lip_trial_mutate(
                    &program,
                    cap_lip_longitudinal_m
                )
                .is_err(),
                "cap-lip variant must fail closed: {cap_lip_longitudinal_m:?}"
            );
        }
        for cap_lip_longitudinal_m in STOCK_UPPER_PROFILE_CAP_LIP_VARIANTS_M {
            assert!(production_weapon_stock_upper_profile_cap_lip_trial_mutate(
                &program,
                cap_lip_longitudinal_m
            )
            .is_ok());
        }
    }

    #[test]
    fn stock_upper_profile_shoulder_trial_changes_only_two_outer_points_per_station() {
        let program = mixed_d1_fixture();
        let baseline_profile =
            production_weapon_stock_upper_profile_trial_mutate(&program, 0.85).unwrap();
        let trial =
            production_weapon_stock_upper_profile_shoulder_trial_mutate(&program, -0.085).unwrap();
        let baseline_upper = node(&baseline_profile, STOCK_UPPER_NODE_ID);
        let trial_upper = node(&trial, STOCK_UPPER_NODE_ID);
        let baseline_profiles = baseline_upper["parameters"]["profiles"].as_array().unwrap();
        let trial_profiles = trial_upper["parameters"]["profiles"].as_array().unwrap();
        for (baseline_station, trial_station) in baseline_profiles.iter().zip(trial_profiles) {
            assert_eq!(baseline_station["station_m"], trial_station["station_m"]);
            assert_eq!(
                baseline_station["corner_indices"],
                trial_station["corner_indices"]
            );
            let baseline_points = baseline_station["points"].as_array().unwrap();
            let trial_points = trial_station["points"].as_array().unwrap();
            for point_index in 0..8 {
                if matches!(point_index, 0 | 3) {
                    assert_eq!(trial_points[point_index][0], json!(-0.085));
                    assert_eq!(
                        trial_points[point_index][1],
                        baseline_points[point_index][1]
                    );
                } else {
                    assert_eq!(trial_points[point_index], baseline_points[point_index]);
                }
            }
            assert_eq!(
                trial_points[1][0],
                json!(-0.055),
                "04Q lip must remain fixed"
            );
            assert_eq!(trial_points[2][0], json!(-0.055));
        }
        let mut normalized_trial = trial.clone();
        let normalized_upper = node_mut(&mut normalized_trial, STOCK_UPPER_NODE_ID);
        normalized_upper["parameters"]["profiles"] =
            baseline_upper["parameters"]["profiles"].clone();
        normalized_trial
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        let mut normalized_baseline = baseline_profile.clone();
        normalized_baseline
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        assert_eq!(normalized_trial, normalized_baseline);
    }

    #[test]
    fn stock_upper_profile_shoulder_trial_admits_only_two_variants() {
        let program = mixed_d1_fixture();
        for shoulder_y_m in [f64::NAN, -0.11, -0.055, -0.045, 0.0] {
            assert!(
                production_weapon_stock_upper_profile_shoulder_trial_mutate(&program, shoulder_y_m)
                    .is_err(),
                "shoulder variant must fail closed: {shoulder_y_m:?}"
            );
        }
        for shoulder_y_m in STOCK_UPPER_PROFILE_SHOULDER_VARIANTS_M {
            assert!(production_weapon_stock_upper_profile_shoulder_trial_mutate(
                &program,
                shoulder_y_m
            )
            .is_ok());
        }
    }

    #[test]
    fn stock_split_output_diagnostic_rehashes_and_isolates_geometry() {
        let program = fixture();
        let trial = production_weapon_stock_split_output_diagnostic(&program)
            .expect("stock split diagnostic");

        assert_eq!(trial["nodes"], program["nodes"]);
        assert_eq!(
            node(&trial, "rear-cap"),
            node(&program, "rear-cap"),
            "rear-cap must remain unchanged"
        );

        let source_output = part_output_mut(&mut program.clone(), STOCK_PART_ID).clone();
        let original_outputs = program["part_outputs"].as_array().unwrap();
        let mut expected_outputs = Vec::new();
        for output in original_outputs {
            if output["part_id"] != STOCK_PART_ID {
                expected_outputs.push(output.clone());
                continue;
            }
            let mut upper = source_output.clone();
            upper["part_id"] = json!(STOCK_UPPER_DIAGNOSTIC_PART_ID);
            upper["input_node_ids"] = json!([STOCK_UPPER_NODE_ID]);
            let mut lower = source_output.clone();
            lower["part_id"] = json!(STOCK_LOWER_DIAGNOSTIC_PART_ID);
            lower["input_node_ids"] = json!([STOCK_LOWER_NODE_ID]);
            expected_outputs.extend([upper, lower]);
        }
        assert_eq!(trial["part_outputs"], Value::Array(expected_outputs));

        let upper = trial["part_outputs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|output| output["part_id"] == STOCK_UPPER_DIAGNOSTIC_PART_ID)
            .expect("upper diagnostic output");
        let lower = trial["part_outputs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|output| output["part_id"] == STOCK_LOWER_DIAGNOSTIC_PART_ID)
            .expect("lower diagnostic output");
        assert_eq!(upper["material_zone_id"], source_output["material_zone_id"]);
        assert_eq!(lower["material_zone_id"], source_output["material_zone_id"]);
        assert_eq!(upper["solid"], source_output["solid"]);
        assert_eq!(lower["solid"], source_output["solid"]);

        let mut without_hash = trial.clone();
        without_hash
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        assert_eq!(
            trial["canonical_sha256"],
            canonical_json_hash(&without_hash)
        );
        ProgramIndex::parse_with_expected_hash(&trial, None).expect("rehashed split validates");
    }

    #[test]
    fn stock_split_output_diagnostic_rejects_missing_input() {
        let mut program = fixture();
        part_output_mut(&mut program, STOCK_PART_ID)["input_node_ids"] =
            json!([STOCK_UPPER_NODE_ID]);
        rehash(&mut program);
        assert!(production_weapon_stock_split_output_diagnostic(&program).is_err());
    }

    #[test]
    fn stock_split_output_diagnostic_rejects_duplicate_input() {
        let mut program = fixture();
        part_output_mut(&mut program, STOCK_PART_ID)["input_node_ids"] = json!([
            STOCK_UPPER_NODE_ID,
            STOCK_UPPER_NODE_ID,
            STOCK_LOWER_NODE_ID
        ]);
        rehash(&mut program);
        assert!(production_weapon_stock_split_output_diagnostic(&program).is_err());
    }

    #[test]
    fn stock_split_output_diagnostic_rejects_wrong_input() {
        let mut program = fixture();
        part_output_mut(&mut program, STOCK_PART_ID)["input_node_ids"] =
            json!([STOCK_UPPER_NODE_ID, "locked-node"]);
        rehash(&mut program);
        assert!(production_weapon_stock_split_output_diagnostic(&program).is_err());
    }

    #[test]
    fn stock_split_output_diagnostic_rejects_duplicate_stock_part_output() {
        let mut program = fixture();
        let stock_output = part_output_mut(&mut program, STOCK_PART_ID).clone();
        program["part_outputs"]
            .as_array_mut()
            .unwrap()
            .push(stock_output);
        rehash(&mut program);
        assert!(production_weapon_stock_split_output_diagnostic(&program).is_err());
    }

    #[test]
    fn mixed_d1_fixture_exposes_seven_exact_sinks_and_mutates_closed_fields() {
        let program = mixed_d1_fixture();
        let expected_hash = program["canonical_sha256"].as_str().unwrap();
        let report = production_weapon_assembly_parameter_descriptors(&program, expected_hash)
            .expect("mixed D1 descriptors");
        assert_eq!(
            report
                .available
                .iter()
                .map(|descriptor| descriptor.parameter_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "receiver-envelope-width",
                "receiver-envelope-height",
                "muzzle-axis-shroud-envelope",
                "muzzle-axis-emitter-envelope",
                "muzzle-axis-core-aperture",
                "stock-open-frame-clearance",
                "stock-open-frame-angle",
            ]
        );
        assert_eq!(
            report.unavailable_parameter_ids,
            vec!["receiver-envelope-shoulder"]
        );

        let width =
            production_weapon_assembly_parameter_mutate(&program, "receiver-envelope-width", 1.1)
                .unwrap();
        assert!(
            (node(&width, "receiver-upper-node")["parameters"]["size_m"][2]
                .as_f64()
                .unwrap()
                - 1.012)
                .abs()
                <= EPSILON
        );
        let height =
            production_weapon_assembly_parameter_mutate(&program, "receiver-envelope-height", 1.1)
                .unwrap();
        assert!(
            (node(&height, "receiver-upper-node")["parameters"]["size_m"][1]
                .as_f64()
                .unwrap()
                - 0.22)
                .abs()
                <= EPSILON
        );
        let shroud = production_weapon_assembly_parameter_mutate(
            &program,
            "muzzle-axis-shroud-envelope",
            1.1,
        )
        .unwrap();
        assert!(
            (node(&shroud, "muzzle-shroud-node")["parameters"]["size_m"][0]
                .as_f64()
                .unwrap()
                - 0.792)
                .abs()
                <= EPSILON
        );
        let emitter = production_weapon_assembly_parameter_mutate(
            &program,
            "muzzle-axis-emitter-envelope",
            1.1,
        )
        .unwrap();
        assert!(
            (node(&emitter, "muzzle-emitter-node")["parameters"]["radius_m"]
                .as_f64()
                .unwrap()
                - 0.33)
                .abs()
                <= EPSILON
        );
        assert!(
            (node(&emitter, "muzzle-emitter-node")["parameters"]["height_m"]
                .as_f64()
                .unwrap()
                - 0.528)
                .abs()
                <= EPSILON
        );
        assert!(production_weapon_assembly_parameter_mutate(
            &program,
            "receiver-envelope-shoulder",
            0.1,
        )
        .is_err());
    }

    #[test]
    fn mixed_d1_muzzle_axis_and_clearance_fail_closed() {
        let program = mixed_d1_fixture();
        let mut wrong_axis = program.clone();
        node_mut(&mut wrong_axis, "muzzle-emitter-node")["parameters"]["rotation_rad"] =
            json!([0.0, 0.0, 1.4]);
        rehash(&mut wrong_axis);
        assert!(production_weapon_assembly_parameter_mutate(
            &wrong_axis,
            "muzzle-axis-emitter-envelope",
            1.1,
        )
        .is_err());

        let mut no_clearance = program.clone();
        node_mut(&mut no_clearance, "muzzle-core-node")["parameters"]["radius_m"] = json!(0.31);
        rehash(&mut no_clearance);
        assert!(production_weapon_assembly_parameter_mutate(
            &no_clearance,
            "muzzle-axis-core-aperture",
            1.0,
        )
        .is_err());
    }

    #[test]
    fn unsupported_and_unknown_parameters_fail_closed() {
        let program = fixture();
        for parameter_id in UNSUPPORTED_PARAMETER_IDS {
            assert!(
                production_weapon_assembly_parameter_mutate(&program, parameter_id, 1.0).is_err()
            );
        }
        assert!(production_weapon_assembly_parameter_mutate(
            &program,
            "receiver-envelope-width-x",
            1.0
        )
        .is_err());
        assert!(production_weapon_assembly_parameter_supported(
            "receiver-envelope-width"
        ));
        assert!(production_weapon_assembly_parameter_supported(
            "stock-open-frame-angle"
        ));
        assert!(production_weapon_assembly_parameter_unavailable(
            "rail-spine-offset"
        ));
    }

    #[test]
    fn persisted_draft_requires_and_accepts_external_program_hash() {
        let canonical = fixture();
        let expected = canonical["canonical_sha256"].as_str().unwrap().to_owned();
        let mut draft = canonical.clone();
        draft.as_object_mut().unwrap().remove("canonical_sha256");
        let result = production_weapon_assembly_parameter_mutate_bound(
            &draft,
            &expected,
            "receiver-envelope-width",
            1.1,
        )
        .expect("external hash binds draft");
        assert!(result.get("canonical_sha256").is_none());
        assert!(production_weapon_assembly_parameter_mutate_bound(
            &draft,
            &"f".repeat(64),
            "receiver-envelope-width",
            1.1,
        )
        .is_err());
        assert!(production_weapon_assembly_parameter_mutate(
            &draft,
            "receiver-envelope-width",
            1.1,
        )
        .is_err());
    }

    #[test]
    fn wrong_operator_duplicate_missing_and_locked_nodes_fail_without_mutation() {
        let program = fixture();
        let mut wrong_operator = program.clone();
        wrong_operator["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["node_id"] == "receiver-main-node")
            .unwrap()["operator_id"] = Value::String(PRIMITIVE_OPERATOR.to_owned());
        let wrong_operator_before = wrong_operator.clone();
        assert!(production_weapon_assembly_parameter_mutate(
            &wrong_operator,
            "receiver-envelope-width",
            1.1
        )
        .is_err());
        assert_eq!(wrong_operator, wrong_operator_before);

        let mut duplicate = program.clone();
        let first_output = duplicate["part_outputs"][0].clone();
        duplicate["part_outputs"]
            .as_array_mut()
            .unwrap()
            .push(first_output);
        assert!(production_weapon_assembly_parameter_mutate(
            &duplicate,
            "receiver-envelope-width",
            1.1
        )
        .is_err());

        let mut missing = program.clone();
        missing["part_outputs"]
            .as_array_mut()
            .unwrap()
            .retain(|output| output["part_id"] != "receiver-upper");
        assert!(production_weapon_assembly_parameter_mutate(
            &missing,
            "receiver-envelope-width",
            1.1
        )
        .is_err());

        let locked_before = node(&program, "locked-node").clone();
        let draft =
            production_weapon_assembly_parameter_mutate(&program, "muzzle-axis-core-aperture", 1.1)
                .unwrap();
        assert_eq!(node(&draft, "locked-node"), &locked_before);
    }

    #[test]
    fn out_of_range_station_profile_axis_clearance_and_bounds_fail_closed() {
        let program = fixture();
        for (parameter, value) in [
            ("receiver-envelope-width", 0.0),
            ("receiver-envelope-height", 2.1),
            ("receiver-envelope-shoulder", 1.1),
            ("muzzle-axis-core-aperture", 0.1),
            ("stock-open-frame-clearance", 0.09),
            ("stock-open-frame-angle", 0.41),
        ] {
            assert!(
                production_weapon_assembly_parameter_mutate(&program, parameter, value).is_err()
            );
        }

        let mut nonfinite = program.clone();
        nonfinite["nodes"][0]["parameters"]["sections"][1]["points"][0][1] = json!(11.0);
        assert!(production_weapon_assembly_parameter_mutate(
            &nonfinite,
            "receiver-envelope-width",
            1.1
        )
        .is_err());

        let mut noncoaxial = program.clone();
        noncoaxial["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["node_id"] == "receiver-upper-node")
            .unwrap()["parameters"]["position_m"][2] = json!(0.2);
        assert!(production_weapon_assembly_parameter_mutate(
            &noncoaxial,
            "receiver-envelope-width",
            1.1
        )
        .is_err());

        let mut no_clearance = program.clone();
        no_clearance["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["node_id"] == "muzzle-core-node")
            .unwrap()["parameters"]["radius_m"] = json!(1.0);
        assert!(production_weapon_assembly_parameter_mutate(
            &no_clearance,
            "muzzle-axis-core-aperture",
            1.1
        )
        .is_err());

        let mut rotated = program.clone();
        rotated["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["node_id"] == "muzzle-emitter-node")
            .unwrap()["parameters"]["rotation_rad"][0] = json!(0.1);
        rotated.as_object_mut().unwrap().remove("canonical_sha256");
        rotated["canonical_sha256"] = Value::String(canonical_json_hash(&rotated));
        assert!(production_weapon_assembly_parameter_mutate(
            &rotated,
            "muzzle-axis-emitter-envelope",
            1.1
        )
        .is_err());

        let mut non_axial_core = program.clone();
        non_axial_core["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["node_id"] == "muzzle-core-node")
            .unwrap()["parameters"]["shape"] = json!("sphere");
        non_axial_core["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["node_id"] == "muzzle-core-node")
            .unwrap()["parameters"]
            .as_object_mut()
            .unwrap()
            .remove("height_m");
        non_axial_core["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["node_id"] == "muzzle-core-node")
            .unwrap()["parameters"]
            .as_object_mut()
            .unwrap()
            .remove("radial_segments");
        let core_parameters = non_axial_core["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["node_id"] == "muzzle-core-node")
            .unwrap()["parameters"]
            .as_object_mut()
            .unwrap();
        core_parameters.insert("longitude_segments".to_owned(), json!(16));
        core_parameters.insert("latitude_segments".to_owned(), json!(8));
        non_axial_core
            .as_object_mut()
            .unwrap()
            .remove("canonical_sha256");
        non_axial_core["canonical_sha256"] = Value::String(canonical_json_hash(&non_axial_core));
        assert!(production_weapon_assembly_parameter_mutate(
            &non_axial_core,
            "muzzle-axis-core-aperture",
            1.1
        )
        .is_err());
    }
}
