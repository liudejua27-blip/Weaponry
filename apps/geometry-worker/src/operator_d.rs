//! MCP010D bounded hard-surface operators.
//!
//! The operators in this module are deliberately small, deterministic mesh
//! constructors. They accept closed typed parameters only; no operator can
//! read a file, execute code, call a network service, or allocate outside the
//! parent GeometryProgram budget. The resulting mesh is handed back to the
//! existing strict GLB/readback path in the parent module.

use super::{
    add3, box_mesh, compile_v2_primitive, cross3, dot3, finite3, length3, normalize,
    require_exact_keys, rotate_xyz, scale3, subtract3, v2_scalar, v2_vec3, GeometryError,
    PrimitiveNodeMesh, ValidatedV2Primitive, MAX_COORDINATE, MAX_DIMENSION,
};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

const MAX_PROFILE_POINTS: usize = 64;
const MAX_LOFT_PROFILES: usize = 16;
const MAX_SWEEP_POINTS: usize = 128;

#[derive(Debug, Clone)]
pub enum ValidatedOperator {
    Primitive(ValidatedV2Primitive),
    ProfileExtrude {
        profile: Vec<[f32; 2]>,
        depth_m: f32,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    ProfileLoft {
        profiles: Vec<(f32, Vec<[f32; 2]>)>,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    Revolve {
        profile: Vec<[f32; 2]>,
        radial_segments: usize,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    TubeSweep {
        path: Vec<[f32; 3]>,
        radius_m: f32,
        radial_segments: usize,
        cap_ends: bool,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    Transform {
        input: String,
        translation_m: [f32; 3],
        rotation_rad: [f32; 3],
        scale: [f32; 3],
    },
    Mirror {
        input: String,
        axis: MirrorAxis,
        offset_m: f32,
    },
    Array {
        input: String,
        count: usize,
        offset_m: [f32; 3],
    },
    Panel {
        size_m: [f32; 3],
        thickness_m: f32,
        bevel_m: f32,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    VentArray {
        width_m: f32,
        height_m: f32,
        depth_m: f32,
        slot_count: usize,
        slot_width_m: f32,
        slot_spacing_m: f32,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    JointStack {
        radius_m: f32,
        depth_m: f32,
        ring_count: usize,
        ring_spacing_m: f32,
        radial_segments: usize,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    PartOutput {
        inputs: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum MirrorAxis {
    X,
    Y,
    Z,
}

impl ValidatedOperator {
    pub fn triangle_count(
        &self,
        input_counts: &BTreeMap<String, u64>,
    ) -> Result<u64, GeometryError> {
        let count = match self {
            Self::Primitive(primitive) => primitive.triangle_count(),
            Self::ProfileExtrude { profile, .. } => 4 * profile.len() as u64 - 4,
            Self::ProfileLoft { profiles, .. } => {
                let points = profiles.first().map(|(_, p)| p.len()).unwrap_or(0) as u64;
                2 * points.saturating_sub(2) + 2 * points * profiles.len().saturating_sub(1) as u64
            }
            Self::Revolve {
                profile,
                radial_segments,
                ..
            } => 2 * *radial_segments as u64 * (profile.len().saturating_sub(1) as u64 + 1),
            Self::TubeSweep {
                path,
                radial_segments,
                cap_ends,
                ..
            } => {
                2 * *radial_segments as u64 * path.len().saturating_sub(1) as u64
                    + if *cap_ends {
                        2 * *radial_segments as u64
                    } else {
                        0
                    }
            }
            Self::Transform { input, .. } => *input_counts
                .get(input)
                .ok_or_else(|| GeometryError::Invalid("operator input is unknown".to_owned()))?,
            Self::Mirror { input, .. } => input_counts
                .get(input)
                .ok_or_else(|| GeometryError::Invalid("operator input is unknown".to_owned()))?
                .checked_mul(2)
                .ok_or_else(|| {
                    GeometryError::Invalid("mirror triangle count overflow".to_owned())
                })?,
            Self::Array { input, count, .. } => input_counts
                .get(input)
                .ok_or_else(|| GeometryError::Invalid("array input is unknown".to_owned()))?
                .checked_mul(*count as u64)
                .ok_or_else(|| {
                    GeometryError::Invalid("array triangle count overflow".to_owned())
                })?,
            Self::Panel { bevel_m, .. } => {
                // A beveled panel uses a fixed four-segment quarter arc at
                // each corner.  The zero-bevel branch remains a plain box.
                if *bevel_m > 1.0e-6 {
                    76
                } else {
                    12
                }
            }
            Self::VentArray { slot_count, .. } => 12 * (*slot_count as u64 + 2),
            Self::JointStack {
                ring_count,
                radial_segments,
                ..
            } => 4 * *ring_count as u64 * *radial_segments as u64,
            Self::PartOutput { inputs } => inputs.iter().try_fold(0u64, |sum, input| {
                sum.checked_add(*input_counts.get(input).ok_or_else(|| {
                    GeometryError::Invalid("part-output input is unknown".to_owned())
                })?)
                .ok_or_else(|| {
                    GeometryError::Invalid("part-output triangle count overflow".to_owned())
                })
            })?,
        };
        if count == 0 {
            return Err(GeometryError::Invalid(
                "operator would emit an empty mesh".to_owned(),
            ));
        }
        Ok(count)
    }
}

pub fn validate_operator(
    operator_id: &str,
    inputs: &[String],
    parameters: &Map<String, Value>,
    input_counts: &BTreeMap<String, u64>,
) -> Result<(ValidatedOperator, u64), GeometryError> {
    let operation = match operator_id {
        "forgecad.geometry.primitive@2" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "primitive@2 accepts exactly zero inputs".to_owned(),
                ));
            }
            ValidatedOperator::Primitive(super::validate_v2_primitive_parameters(parameters)?)
        }
        "forgecad.geometry.profile-extrude@1" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "profile-extrude accepts no inputs".to_owned(),
                ));
            }
            require_shape(parameters, "profile-extrude")?;
            let profile = parse_profile(parameters, "profile", 3, MAX_PROFILE_POINTS)?;
            require_nonzero_area(&profile, "profile-extrude profile")?;
            ValidatedOperator::ProfileExtrude {
                profile,
                depth_m: v2_scalar(parameters, "depth_m", MAX_DIMENSION, true)?,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            }
        }
        "forgecad.geometry.profile-loft@1" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "profile-loft accepts no inputs".to_owned(),
                ));
            }
            require_shape(parameters, "profile-loft")?;
            let profiles_value = parameters
                .get("profiles")
                .and_then(Value::as_array)
                .ok_or_else(|| GeometryError::Invalid("profiles must be an array".to_owned()))?;
            if !(2..=MAX_LOFT_PROFILES).contains(&profiles_value.len()) {
                return Err(GeometryError::Invalid(
                    "profiles count is outside bounds".to_owned(),
                ));
            }
            let mut profiles: Vec<(f32, Vec<[f32; 2]>)> = Vec::with_capacity(profiles_value.len());
            let mut previous_height = f32::NEG_INFINITY;
            for profile_value in profiles_value {
                let profile_object = profile_value.as_object().ok_or_else(|| {
                    GeometryError::Invalid("loft profile must be an object".to_owned())
                })?;
                require_exact_keys(profile_object, &["height_m", "points"], "loft profile")?;
                let height = number_field(profile_object, "height_m", MAX_COORDINATE)?;
                if height <= previous_height {
                    return Err(GeometryError::Invalid(
                        "loft profile heights must be strictly increasing".to_owned(),
                    ));
                }
                previous_height = height;
                let profile = parse_points(profile_object, "points", 3, MAX_PROFILE_POINTS)?;
                require_nonzero_area(&profile, "profile-loft profile")?;
                if let Some((_, first)) = profiles.first() {
                    if first.len() != profile.len() {
                        return Err(GeometryError::Invalid(
                            "all loft profiles must have the same point count".to_owned(),
                        ));
                    }
                }
                profiles.push((height, profile));
            }
            ValidatedOperator::ProfileLoft {
                profiles,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            }
        }
        "forgecad.geometry.revolve@1" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "revolve accepts no inputs".to_owned(),
                ));
            }
            require_shape(parameters, "revolve")?;
            let profile = parse_profile(parameters, "profile", 2, MAX_PROFILE_POINTS)?;
            for point in &profile {
                if point[0] < 0.0 {
                    return Err(GeometryError::Invalid(
                        "revolve radius must be non-negative".to_owned(),
                    ));
                }
            }
            ValidatedOperator::Revolve {
                profile,
                radial_segments: bounded_count(parameters, "radial_segments", 8, 64)?,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            }
        }
        "forgecad.geometry.tube-sweep@1" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "tube-sweep accepts no inputs".to_owned(),
                ));
            }
            require_shape(parameters, "tube-sweep")?;
            let path = parse_vec3_array(parameters, "path", 2, MAX_SWEEP_POINTS)?;
            for pair in path.windows(2) {
                if length3(subtract3(pair[1], pair[0])) <= 1.0e-5 {
                    return Err(GeometryError::Invalid(
                        "tube-sweep path contains coincident points".to_owned(),
                    ));
                }
            }
            ValidatedOperator::TubeSweep {
                path,
                radius_m: v2_scalar(parameters, "radius_m", 5.0, true)?,
                radial_segments: bounded_count(parameters, "radial_segments", 8, 64)?,
                cap_ends: bool_field(parameters, "cap_ends")?,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            }
        }
        "forgecad.geometry.transform@2" => {
            require_one_input(inputs, "transform")?;
            require_exact_keys(
                parameters,
                &["shape", "translation_m", "rotation_rad", "scale"],
                "transform",
            )?;
            require_shape(parameters, "transform")?;
            ValidatedOperator::Transform {
                input: inputs[0].clone(),
                translation_m: v2_vec3(parameters, "translation_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
                scale: v2_vec3(parameters, "scale", 10.0, true)?,
            }
        }
        "forgecad.geometry.mirror@1" => {
            require_one_input(inputs, "mirror")?;
            require_exact_keys(parameters, &["shape", "axis", "offset_m"], "mirror")?;
            require_shape(parameters, "mirror")?;
            let axis = match parameters.get("axis").and_then(Value::as_str) {
                Some("x") => MirrorAxis::X,
                Some("y") => MirrorAxis::Y,
                Some("z") => MirrorAxis::Z,
                _ => return Err(GeometryError::Invalid("mirror axis is invalid".to_owned())),
            };
            ValidatedOperator::Mirror {
                input: inputs[0].clone(),
                axis,
                offset_m: number_field(parameters, "offset_m", MAX_COORDINATE)?,
            }
        }
        "forgecad.geometry.array@1" => {
            require_one_input(inputs, "array")?;
            require_exact_keys(parameters, &["shape", "count", "offset_m"], "array")?;
            require_shape(parameters, "array")?;
            ValidatedOperator::Array {
                input: inputs[0].clone(),
                count: bounded_count(parameters, "count", 1, 32)?,
                offset_m: v2_vec3(parameters, "offset_m", MAX_COORDINATE, false)?,
            }
        }
        "forgecad.geometry.panel@1" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid("panel accepts no inputs".to_owned()));
            }
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "size_m",
                    "thickness_m",
                    "bevel_m",
                    "position_m",
                    "rotation_rad",
                ],
                "panel",
            )?;
            require_shape(parameters, "panel")?;
            let size_m = v2_vec3(parameters, "size_m", MAX_DIMENSION, true)?;
            let thickness_m = v2_scalar(parameters, "thickness_m", MAX_DIMENSION, true)?;
            let bevel_m = v2_scalar(parameters, "bevel_m", MAX_DIMENSION / 2.0, false)?;
            if thickness_m > size_m[2] || bevel_m * 2.0 >= size_m[0].min(size_m[1]) {
                return Err(GeometryError::Invalid(
                    "panel thickness/bevel exceeds size".to_owned(),
                ));
            }
            ValidatedOperator::Panel {
                size_m,
                thickness_m,
                bevel_m,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            }
        }
        "forgecad.geometry.vent-array@1" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "vent-array accepts no inputs".to_owned(),
                ));
            }
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "width_m",
                    "height_m",
                    "depth_m",
                    "slot_count",
                    "slot_width_m",
                    "slot_spacing_m",
                    "position_m",
                    "rotation_rad",
                ],
                "vent-array",
            )?;
            require_shape(parameters, "vent-array")?;
            let width_m = v2_scalar(parameters, "width_m", MAX_DIMENSION, true)?;
            let height_m = v2_scalar(parameters, "height_m", MAX_DIMENSION, true)?;
            let depth_m = v2_scalar(parameters, "depth_m", MAX_DIMENSION, true)?;
            let slot_count = bounded_count(parameters, "slot_count", 1, 32)?;
            let slot_width_m = v2_scalar(parameters, "slot_width_m", width_m, true)?;
            let slot_spacing_m = v2_scalar(parameters, "slot_spacing_m", width_m, true)?;
            let required_width = slot_count as f32 * slot_width_m
                + slot_count.saturating_sub(1) as f32 * slot_spacing_m;
            if required_width > width_m {
                return Err(GeometryError::Invalid(
                    "vent slots exceed panel width".to_owned(),
                ));
            }
            ValidatedOperator::VentArray {
                width_m,
                height_m,
                depth_m,
                slot_count,
                slot_width_m,
                slot_spacing_m,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            }
        }
        "forgecad.geometry.joint-stack@1" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "joint-stack accepts no inputs".to_owned(),
                ));
            }
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "radius_m",
                    "depth_m",
                    "ring_count",
                    "ring_spacing_m",
                    "radial_segments",
                    "position_m",
                    "rotation_rad",
                ],
                "joint-stack",
            )?;
            require_shape(parameters, "joint-stack")?;
            ValidatedOperator::JointStack {
                radius_m: v2_scalar(parameters, "radius_m", 5.0, true)?,
                depth_m: v2_scalar(parameters, "depth_m", MAX_DIMENSION, true)?,
                ring_count: bounded_count(parameters, "ring_count", 1, 16)?,
                ring_spacing_m: v2_scalar(parameters, "ring_spacing_m", MAX_DIMENSION, true)?,
                radial_segments: bounded_count(parameters, "radial_segments", 8, 64)?,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            }
        }
        "forgecad.geometry.part-output@1" => {
            if !(1..=64).contains(&inputs.len()) {
                return Err(GeometryError::Invalid(
                    "part-output requires one to 64 inputs".to_owned(),
                ));
            }
            require_exact_keys(parameters, &["shape"], "part-output")?;
            require_shape(parameters, "part-output")?;
            ValidatedOperator::PartOutput {
                inputs: inputs.to_vec(),
            }
        }
        "forgecad.geometry.boolean@1" => {
            return Err(GeometryError::Invalid(
                "boolean@1 is unavailable until the isolated Manifold adoption gate passes"
                    .to_owned(),
            ));
        }
        other => {
            return Err(GeometryError::Invalid(format!(
                "operator is not active in OperatorCatalog@1: {other}"
            )))
        }
    };
    let count = operation.triangle_count(input_counts)?;
    Ok((operation, count))
}

pub fn compile_operator(
    operation: &ValidatedOperator,
    meshes: &BTreeMap<String, PrimitiveNodeMesh>,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let mut mesh = match operation {
        ValidatedOperator::Primitive(primitive) => {
            let (positions, normals, indices) = compile_v2_primitive(primitive);
            PrimitiveNodeMesh {
                operator_id: "forgecad.geometry.primitive@2".to_owned(),
                positions,
                normals,
                indices,
            }
        }
        ValidatedOperator::ProfileExtrude {
            profile,
            depth_m,
            position_m,
            rotation_rad,
        } => transform_mesh(
            profile_extrude_mesh(profile, *depth_m)?,
            *position_m,
            *rotation_rad,
            [1.0; 3],
        ),
        ValidatedOperator::ProfileLoft {
            profiles,
            position_m,
            rotation_rad,
        } => transform_mesh(
            profile_loft_mesh(profiles)?,
            *position_m,
            *rotation_rad,
            [1.0; 3],
        ),
        ValidatedOperator::Revolve {
            profile,
            radial_segments,
            position_m,
            rotation_rad,
        } => transform_mesh(
            revolve_mesh(profile, *radial_segments)?,
            *position_m,
            *rotation_rad,
            [1.0; 3],
        ),
        ValidatedOperator::TubeSweep {
            path,
            radius_m,
            radial_segments,
            cap_ends,
            position_m,
            rotation_rad,
        } => transform_mesh(
            tube_sweep_mesh(path, *radius_m, *radial_segments, *cap_ends)?,
            *position_m,
            *rotation_rad,
            [1.0; 3],
        ),
        ValidatedOperator::Transform {
            input,
            translation_m,
            rotation_rad,
            scale,
        } => transform_mesh(
            input_mesh(meshes, input)?.clone(),
            *translation_m,
            *rotation_rad,
            *scale,
        ),
        ValidatedOperator::Mirror {
            input,
            axis,
            offset_m,
        } => mirror_mesh(input_mesh(meshes, input)?, *axis, *offset_m),
        ValidatedOperator::Array {
            input,
            count,
            offset_m,
        } => array_mesh(input_mesh(meshes, input)?, *count, *offset_m),
        ValidatedOperator::Panel {
            size_m,
            thickness_m,
            bevel_m,
            position_m,
            rotation_rad,
        } => {
            let half = [size_m[0] / 2.0, size_m[1] / 2.0];
            let b = (*bevel_m).min(half[0].min(half[1]) * 0.9);
            let profile = if b <= 1.0e-6 {
                vec![
                    [-half[0], -half[1]],
                    [half[0], -half[1]],
                    [half[0], half[1]],
                    [-half[0], half[1]],
                ]
            } else {
                rounded_panel_profile(half, b, 4)
            };
            transform_mesh(
                profile_extrude_mesh(&profile, *thickness_m)?,
                *position_m,
                *rotation_rad,
                [1.0; 3],
            )
        }
        ValidatedOperator::VentArray {
            width_m,
            height_m,
            depth_m,
            slot_count,
            slot_width_m,
            slot_spacing_m,
            position_m,
            rotation_rad,
        } => {
            let mut mesh = empty_mesh();
            let rail_height = (*height_m * 0.1).max(0.01);
            append_mesh(
                &mut mesh,
                &box_as_mesh(
                    [*width_m, rail_height, *depth_m],
                    [0.0, *height_m / 2.0 - rail_height / 2.0, 0.0],
                ),
            );
            append_mesh(
                &mut mesh,
                &box_as_mesh(
                    [*width_m, rail_height, *depth_m],
                    [0.0, -*height_m / 2.0 + rail_height / 2.0, 0.0],
                ),
            );
            let total = *slot_count as f32 * *slot_width_m
                + (*slot_count).saturating_sub(1) as f32 * *slot_spacing_m;
            let start = -total / 2.0 + *slot_width_m / 2.0;
            let slot_height = (*height_m - 2.0 * rail_height).max(0.01);
            for index in 0..*slot_count {
                let x = start + index as f32 * (*slot_width_m + *slot_spacing_m);
                append_mesh(
                    &mut mesh,
                    &box_as_mesh([*slot_width_m, slot_height, *depth_m], [x, 0.0, 0.0]),
                );
            }
            transform_mesh(mesh, *position_m, *rotation_rad, [1.0; 3])
        }
        ValidatedOperator::JointStack {
            radius_m,
            depth_m,
            ring_count,
            ring_spacing_m,
            radial_segments,
            position_m,
            rotation_rad,
        } => {
            let mut mesh = empty_mesh();
            let start = -(*ring_count as f32 - 1.0) * *ring_spacing_m / 2.0;
            for index in 0..*ring_count {
                let (positions, normals, indices) = super::cylinder_mesh(
                    [*radius_m * 2.0, *depth_m, *radius_m * 2.0],
                    *radial_segments,
                );
                let translated = PrimitiveNodeMesh {
                    operator_id: String::new(),
                    positions: positions
                        .into_iter()
                        .map(|mut point| {
                            point[1] += start + index as f32 * *ring_spacing_m;
                            point
                        })
                        .collect(),
                    normals,
                    indices,
                };
                append_mesh(&mut mesh, &translated);
            }
            transform_mesh(mesh, *position_m, *rotation_rad, [1.0; 3])
        }
        ValidatedOperator::PartOutput { inputs } => {
            let mut result = empty_mesh();
            for input in inputs {
                append_mesh(&mut result, input_mesh(meshes, input)?);
            }
            result
        }
    };
    // Curved hard-surface operators are emitted as deterministic triangle
    // fans with duplicated chart vertices.  Reconstructing only their
    // compatible neighbouring normals removes the low-poly lighting seams
    // without smoothing panel/chamfer edges or changing topology/lineage.
    if matches!(
        operation,
        ValidatedOperator::ProfileLoft { .. }
            | ValidatedOperator::Revolve { .. }
            | ValidatedOperator::TubeSweep { .. }
            | ValidatedOperator::JointStack { .. }
    ) {
        smooth_curved_normals(&mut mesh);
    }
    if mesh.positions.is_empty() || mesh.indices.is_empty() || mesh.indices.len() % 3 != 0 {
        return Err(GeometryError::Invalid(
            "operator emitted an empty or invalid mesh".to_owned(),
        ));
    }
    if mesh.positions.iter().any(|value| !finite3(*value))
        || mesh.normals.iter().any(|value| !finite3(*value))
    {
        return Err(GeometryError::Invalid(
            "operator emitted non-finite mesh data".to_owned(),
        ));
    }
    Ok(mesh)
}

/// Smooth only coincident vertices whose face normals are part of the same
/// curved surface.  A cosine threshold intentionally leaves 90-degree caps
/// and hard panel edges crisp while joining adjacent profile/ring facets.
fn smooth_curved_normals(mesh: &mut PrimitiveNodeMesh) {
    const COMPATIBLE_NORMAL_DOT: f32 = 0.55;
    if mesh.positions.len() != mesh.normals.len() || mesh.indices.len() % 3 != 0 {
        return;
    }
    let mut face_normals: Vec<Vec<[f32; 3]>> = vec![Vec::new(); mesh.positions.len()];
    for triangle in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [
            triangle[0] as usize,
            triangle[1] as usize,
            triangle[2] as usize,
        ];
        if a >= mesh.positions.len() || b >= mesh.positions.len() || c >= mesh.positions.len() {
            return;
        }
        let face = normalize(cross3(
            subtract3(mesh.positions[b], mesh.positions[a]),
            subtract3(mesh.positions[c], mesh.positions[a]),
        ));
        if !finite3(face) || length3(face) <= f32::EPSILON {
            return;
        }
        for index in [a, b, c] {
            face_normals[index].push(face);
        }
    }

    let mut groups: BTreeMap<(u32, u32, u32), Vec<usize>> = BTreeMap::new();
    for (index, position) in mesh.positions.iter().enumerate() {
        groups
            .entry((
                position[0].to_bits(),
                position[1].to_bits(),
                position[2].to_bits(),
            ))
            .or_default()
            .push(index);
    }
    for indices in groups.values() {
        if indices.len() < 2 {
            continue;
        }
        for &index in indices {
            let Some(reference) = face_normals[index].first().copied() else {
                continue;
            };
            let mut sum = [0.0; 3];
            for &other in indices {
                for normal in &face_normals[other] {
                    if dot3(reference, *normal) >= COMPATIBLE_NORMAL_DOT {
                        sum = add3(sum, *normal);
                    }
                }
            }
            let smoothed = normalize(sum);
            if finite3(smoothed) && length3(smoothed) > f32::EPSILON {
                mesh.normals[index] = smoothed;
            }
        }
    }
}

/// Build a deterministic rounded rectangle in counter-clockwise order.
/// Four segments per corner are enough to remove the visible octagonal
/// "blockout" edge while keeping the operator bounded and reproducible.
fn rounded_panel_profile(half: [f32; 2], radius: f32, segments: usize) -> Vec<[f32; 2]> {
    let [half_x, half_y] = half;
    let segments = segments.max(1);
    let quarter = std::f32::consts::FRAC_PI_2;
    let mut points = Vec::with_capacity(segments * 4 + 4);
    points.push([-half_x + radius, -half_y]);
    points.push([half_x - radius, -half_y]);

    let corners = [
        (half_x - radius, -half_y + radius, -quarter),
        (half_x - radius, half_y - radius, 0.0),
        (-half_x + radius, half_y - radius, quarter),
        (-half_x + radius, -half_y + radius, std::f32::consts::PI),
    ];
    for (corner_index, (center_x, center_y, start_angle)) in corners.iter().enumerate() {
        let end_angle = *start_angle + quarter;
        for step in 1..=segments {
            let t = step as f32 / segments as f32;
            let angle = *start_angle + (end_angle - *start_angle) * t;
            points.push([
                *center_x + radius * angle.cos(),
                *center_y + radius * angle.sin(),
            ]);
        }
        if corner_index == 0 {
            points.push([half_x, half_y - radius]);
        } else if corner_index == 1 {
            points.push([-half_x + radius, half_y]);
        } else if corner_index == 2 {
            points.push([-half_x, -half_y + radius]);
        }
    }
    // The final bottom-left arc ends at the initial point; omit that duplicate
    // so the strict readback sees no zero-area perimeter edge.
    points.pop();
    points
}

fn input_mesh<'a>(
    meshes: &'a BTreeMap<String, PrimitiveNodeMesh>,
    input: &str,
) -> Result<&'a PrimitiveNodeMesh, GeometryError> {
    meshes
        .get(input)
        .ok_or_else(|| GeometryError::Invalid("operator input is unknown".to_owned()))
}

fn require_shape(parameters: &Map<String, Value>, expected: &str) -> Result<(), GeometryError> {
    if parameters.get("shape").and_then(Value::as_str) != Some(expected) {
        return Err(GeometryError::Invalid(format!("shape must be {expected}")));
    }
    Ok(())
}

fn require_one_input(inputs: &[String], operator: &str) -> Result<(), GeometryError> {
    if inputs.len() != 1 {
        return Err(GeometryError::Invalid(format!(
            "{operator} requires exactly one input"
        )));
    }
    Ok(())
}

fn bool_field(parameters: &Map<String, Value>, key: &str) -> Result<bool, GeometryError> {
    parameters
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be a boolean")))
}

fn bounded_count(
    parameters: &Map<String, Value>,
    key: &str,
    min: usize,
    max: usize,
) -> Result<usize, GeometryError> {
    let value = parameters
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be an integer")))?
        as usize;
    if !(min..=max).contains(&value) {
        return Err(GeometryError::Invalid(format!("{key} is outside bounds")));
    }
    Ok(value)
}

fn number_field(
    parameters: &Map<String, Value>,
    key: &str,
    limit: f32,
) -> Result<f32, GeometryError> {
    let value = parameters
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be a number")))?
        as f32;
    if !value.is_finite() || value.abs() > limit {
        return Err(GeometryError::Invalid(format!("{key} is outside bounds")));
    }
    Ok(value)
}

fn parse_profile(
    parameters: &Map<String, Value>,
    key: &str,
    min: usize,
    max: usize,
) -> Result<Vec<[f32; 2]>, GeometryError> {
    let values = parameters
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be an array")))?;
    parse_points_from_array(values, key, min, max)
}

fn parse_points(
    object: &Map<String, Value>,
    key: &str,
    min: usize,
    max: usize,
) -> Result<Vec<[f32; 2]>, GeometryError> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be an array")))?;
    parse_points_from_array(values, key, min, max)
}

fn parse_points_from_array(
    values: &[Value],
    key: &str,
    min: usize,
    max: usize,
) -> Result<Vec<[f32; 2]>, GeometryError> {
    if !(min..=max).contains(&values.len()) {
        return Err(GeometryError::Invalid(format!(
            "{key} point count is outside bounds"
        )));
    }
    let mut points = Vec::with_capacity(values.len());
    for value in values {
        let point = value
            .as_array()
            .filter(|point| point.len() == 2)
            .ok_or_else(|| GeometryError::Invalid(format!("{key} points must be pairs")))?;
        let x = point[0]
            .as_f64()
            .ok_or_else(|| GeometryError::Invalid(format!("{key} contains a non-number")))?
            as f32;
        let y = point[1]
            .as_f64()
            .ok_or_else(|| GeometryError::Invalid(format!("{key} contains a non-number")))?
            as f32;
        if !x.is_finite() || !y.is_finite() || x.abs() > MAX_COORDINATE || y.abs() > MAX_COORDINATE
        {
            return Err(GeometryError::Invalid(format!(
                "{key} point is outside bounds"
            )));
        }
        points.push([x, y]);
    }
    Ok(points)
}

fn parse_vec3_array(
    parameters: &Map<String, Value>,
    key: &str,
    min: usize,
    max: usize,
) -> Result<Vec<[f32; 3]>, GeometryError> {
    let values = parameters
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be an array")))?;
    if !(min..=max).contains(&values.len()) {
        return Err(GeometryError::Invalid(format!(
            "{key} point count is outside bounds"
        )));
    }
    values
        .iter()
        .map(|value| {
            let point = value
                .as_array()
                .filter(|point| point.len() == 3)
                .ok_or_else(|| GeometryError::Invalid(format!("{key} points must be triples")))?;
            let mut result = [0.0; 3];
            for (index, component) in point.iter().enumerate() {
                result[index] = component
                    .as_f64()
                    .ok_or_else(|| GeometryError::Invalid(format!("{key} contains a non-number")))?
                    as f32;
                if !result[index].is_finite() || result[index].abs() > MAX_COORDINATE {
                    return Err(GeometryError::Invalid(format!(
                        "{key} point is outside bounds"
                    )));
                }
            }
            Ok(result)
        })
        .collect()
}

fn signed_area(profile: &[[f32; 2]]) -> f32 {
    profile
        .iter()
        .zip(profile.iter().cycle().skip(1))
        .take(profile.len())
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum::<f32>()
        * 0.5
}

fn require_nonzero_area(profile: &[[f32; 2]], label: &str) -> Result<(), GeometryError> {
    if signed_area(profile).abs() <= 1.0e-5 {
        return Err(GeometryError::Invalid(format!("{label} has zero area")));
    }
    Ok(())
}

fn oriented_profile(profile: &[[f32; 2]]) -> Vec<[f32; 2]> {
    if signed_area(profile) >= 0.0 {
        profile.to_vec()
    } else {
        profile.iter().rev().copied().collect()
    }
}

fn push_triangle(
    mesh: &mut PrimitiveNodeMesh,
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
) -> Result<(), GeometryError> {
    let cross = cross3(subtract3(b, a), subtract3(c, a));
    let normal = normalize(cross);
    if !finite3(normal) || length3(cross) <= 1.0e-8 {
        return Err(GeometryError::Invalid(
            "operator emitted a degenerate triangle".to_owned(),
        ));
    }
    let base = mesh.positions.len() as u32;
    mesh.positions.extend([a, b, c]);
    mesh.normals.extend([normal; 3]);
    mesh.indices.extend([base, base + 1, base + 2]);
    Ok(())
}

fn profile_extrude_mesh(
    profile: &[[f32; 2]],
    depth: f32,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let profile = oriented_profile(profile);
    let mut mesh = empty_mesh();
    let half = depth / 2.0;
    let front = |point: [f32; 2]| [point[0], point[1], half];
    let back = |point: [f32; 2]| [point[0], point[1], -half];
    for index in 1..profile.len() - 1 {
        push_triangle(
            &mut mesh,
            front(profile[0]),
            front(profile[index]),
            front(profile[index + 1]),
        )?;
        push_triangle(
            &mut mesh,
            back(profile[0]),
            back(profile[index + 1]),
            back(profile[index]),
        )?;
    }
    for index in 0..profile.len() {
        let next = (index + 1) % profile.len();
        push_triangle(
            &mut mesh,
            front(profile[index]),
            back(profile[index]),
            back(profile[next]),
        )?;
        push_triangle(
            &mut mesh,
            front(profile[index]),
            back(profile[next]),
            front(profile[next]),
        )?;
    }
    Ok(mesh)
}

fn profile_loft_mesh(
    profiles: &[(f32, Vec<[f32; 2]>)],
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let mut mesh = empty_mesh();
    let first = oriented_profile(&profiles[0].1);
    for index in 1..first.len() - 1 {
        push_triangle(
            &mut mesh,
            [first[0][0], first[0][1], profiles[0].0],
            [first[index + 1][0], first[index + 1][1], profiles[0].0],
            [first[index][0], first[index][1], profiles[0].0],
        )?;
    }
    for level in 0..profiles.len() - 1 {
        let z0 = profiles[level].0;
        let z1 = profiles[level + 1].0;
        let p0 = oriented_profile(&profiles[level].1);
        let p1 = oriented_profile(&profiles[level + 1].1);
        for index in 0..p0.len() {
            let next = (index + 1) % p0.len();
            let a = [p0[index][0], p0[index][1], z0];
            let b = [p0[next][0], p0[next][1], z0];
            let c = [p1[next][0], p1[next][1], z1];
            let d = [p1[index][0], p1[index][1], z1];
            push_triangle(&mut mesh, a, b, c)?;
            push_triangle(&mut mesh, a, c, d)?;
        }
    }
    let (last_z, last_profile) = profiles.last().expect("validated loft profiles");
    let last = oriented_profile(last_profile);
    for index in 1..last.len() - 1 {
        push_triangle(
            &mut mesh,
            [last[0][0], last[0][1], *last_z],
            [last[index][0], last[index][1], *last_z],
            [last[index + 1][0], last[index + 1][1], *last_z],
        )?;
    }
    Ok(mesh)
}

fn revolve_mesh(profile: &[[f32; 2]], segments: usize) -> Result<PrimitiveNodeMesh, GeometryError> {
    let mut mesh = empty_mesh();
    for level in 0..profile.len() - 1 {
        for segment in 0..segments {
            let next = (segment + 1) % segments;
            let angle0 = std::f32::consts::TAU * segment as f32 / segments as f32;
            let angle1 = std::f32::consts::TAU * next as f32 / segments as f32;
            let vertex = |point: [f32; 2], angle: f32| {
                [point[0] * angle.cos(), point[1], point[0] * angle.sin()]
            };
            let a = vertex(profile[level], angle0);
            let b = vertex(profile[level], angle1);
            let c = vertex(profile[level + 1], angle1);
            let d = vertex(profile[level + 1], angle0);
            if length3(cross3(subtract3(b, a), subtract3(c, a))) > 1.0e-8 {
                push_triangle(&mut mesh, a, b, c)?;
            }
            if length3(cross3(subtract3(c, a), subtract3(d, a))) > 1.0e-8 {
                push_triangle(&mut mesh, a, c, d)?;
            }
        }
    }
    for &(point, reverse) in &[(profile[0], true), (*profile.last().unwrap(), false)] {
        let center = [0.0, point[1], 0.0];
        for segment in 0..segments {
            let next = (segment + 1) % segments;
            let angle0 = std::f32::consts::TAU * segment as f32 / segments as f32;
            let angle1 = std::f32::consts::TAU * next as f32 / segments as f32;
            let a = [point[0] * angle0.cos(), point[1], point[0] * angle0.sin()];
            let b = [point[0] * angle1.cos(), point[1], point[0] * angle1.sin()];
            if point[0] > 0.0 {
                if reverse {
                    push_triangle(&mut mesh, center, b, a)?;
                } else {
                    push_triangle(&mut mesh, center, a, b)?;
                }
            }
        }
    }
    Ok(mesh)
}

fn tube_sweep_mesh(
    path: &[[f32; 3]],
    radius: f32,
    segments: usize,
    cap_ends: bool,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let mut rings = Vec::with_capacity(path.len());
    for index in 0..path.len() {
        let tangent = if index == 0 {
            normalize(subtract3(path[1], path[0]))
        } else if index + 1 == path.len() {
            normalize(subtract3(path[index], path[index - 1]))
        } else {
            normalize(subtract3(path[index + 1], path[index - 1]))
        };
        let reference = if tangent[1].abs() < 0.9 {
            [0.0, 1.0, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let normal = normalize(cross3(tangent, reference));
        let binormal = normalize(cross3(tangent, normal));
        let ring = (0..segments)
            .map(|segment| {
                let angle = std::f32::consts::TAU * segment as f32 / segments as f32;
                add3(
                    path[index],
                    scale3(
                        add3(scale3(normal, angle.cos()), scale3(binormal, angle.sin())),
                        radius,
                    ),
                )
            })
            .collect::<Vec<_>>();
        rings.push(ring);
    }
    let mut mesh = empty_mesh();
    for ring_index in 0..rings.len() - 1 {
        for segment in 0..segments {
            let next = (segment + 1) % segments;
            let a = rings[ring_index][segment];
            let b = rings[ring_index][next];
            let c = rings[ring_index + 1][next];
            let d = rings[ring_index + 1][segment];
            push_triangle(&mut mesh, a, b, c)?;
            push_triangle(&mut mesh, a, c, d)?;
        }
    }
    if cap_ends {
        for (ring, reverse, center) in [
            (&rings[0], true, path[0]),
            (
                rings.last().expect("tube rings"),
                false,
                *path.last().unwrap(),
            ),
        ] {
            for segment in 0..segments {
                let next = (segment + 1) % segments;
                if reverse {
                    push_triangle(&mut mesh, center, ring[next], ring[segment])?;
                } else {
                    push_triangle(&mut mesh, center, ring[segment], ring[next])?;
                }
            }
        }
    }
    Ok(mesh)
}

fn empty_mesh() -> PrimitiveNodeMesh {
    PrimitiveNodeMesh {
        operator_id: String::new(),
        positions: Vec::new(),
        normals: Vec::new(),
        indices: Vec::new(),
    }
}

fn box_as_mesh(size: [f32; 3], translation: [f32; 3]) -> PrimitiveNodeMesh {
    let (mut positions, normals, indices) = box_mesh(size);
    for position in &mut positions {
        *position = add3(*position, translation);
    }
    PrimitiveNodeMesh {
        operator_id: String::new(),
        positions,
        normals,
        indices,
    }
}

fn append_mesh(target: &mut PrimitiveNodeMesh, source: &PrimitiveNodeMesh) {
    let base = target.positions.len() as u32;
    target.positions.extend_from_slice(&source.positions);
    target.normals.extend_from_slice(&source.normals);
    target
        .indices
        .extend(source.indices.iter().map(|index| base + *index));
}

fn transform_mesh(
    mut mesh: PrimitiveNodeMesh,
    translation: [f32; 3],
    rotation: [f32; 3],
    scale: [f32; 3],
) -> PrimitiveNodeMesh {
    for position in &mut mesh.positions {
        let scaled = [
            position[0] * scale[0],
            position[1] * scale[1],
            position[2] * scale[2],
        ];
        *position = add3(rotate_xyz(scaled, rotation), translation);
    }
    for normal in &mut mesh.normals {
        let scaled = [
            normal[0] / scale[0],
            normal[1] / scale[1],
            normal[2] / scale[2],
        ];
        *normal = normalize(rotate_xyz(scaled, rotation));
    }
    mesh
}

fn mirror_mesh(input: &PrimitiveNodeMesh, axis: MirrorAxis, offset: f32) -> PrimitiveNodeMesh {
    // A mirror operator is a modeling pair, not a destructive transform: the
    // authored source remains on its original side and the reflected copy is
    // appended with reversed winding. This keeps bilateral robot parts
    // complete while preserving deterministic source lineage for the node.
    let mut mesh = input.clone();
    for position in &mut mesh.positions {
        let coordinate = match axis {
            MirrorAxis::X => &mut position[0],
            MirrorAxis::Y => &mut position[1],
            MirrorAxis::Z => &mut position[2],
        };
        *coordinate = 2.0 * offset - *coordinate;
    }
    for normal in &mut mesh.normals {
        match axis {
            MirrorAxis::X => normal[0] = -normal[0],
            MirrorAxis::Y => normal[1] = -normal[1],
            MirrorAxis::Z => normal[2] = -normal[2],
        }
    }
    for triangle in mesh.indices.chunks_exact_mut(3) {
        triangle.swap(1, 2);
    }
    let mut result = input.clone();
    append_mesh(&mut result, &mesh);
    result
}

fn array_mesh(input: &PrimitiveNodeMesh, count: usize, offset: [f32; 3]) -> PrimitiveNodeMesh {
    let mut result = empty_mesh();
    for index in 0..count {
        let translated = transform_mesh(
            input.clone(),
            [
                offset[0] * index as f32,
                offset[1] * index as f32,
                offset[2] * index as f32,
            ],
            [0.0; 3],
            [1.0; 3],
        );
        append_mesh(&mut result, &translated);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mirror_emits_original_and_reflected_mesh() {
        let source = box_as_mesh([0.4, 0.6, 0.8], [-0.8, 0.0, 0.0]);
        let mirrored = mirror_mesh(&source, MirrorAxis::X, 0.0);
        assert_eq!(mirrored.indices.len(), source.indices.len() * 2);
        assert_eq!(mirrored.positions.len(), source.positions.len() * 2);
        assert!(mirrored.positions.iter().any(|position| position[0] < -0.5));
        assert!(mirrored.positions.iter().any(|position| position[0] > 0.5));
    }

    #[test]
    fn panel_bevel_uses_bounded_rounded_profile_without_changing_plain_box() {
        let rounded_parameters = json!({
            "shape": "panel",
            "size_m": [1.6, 0.8, 0.2],
            "thickness_m": 0.2,
            "bevel_m": 0.08,
            "position_m": [0.0, 0.0, 0.0],
            "rotation_rad": [0.0, 0.0, 0.0]
        });
        let (rounded, rounded_count) = validate_operator(
            "forgecad.geometry.panel@1",
            &[],
            rounded_parameters.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .expect("rounded panel should validate");
        assert_eq!(rounded_count, 76);
        let rounded_mesh = compile_operator(&rounded, &BTreeMap::new()).expect("rounded mesh");
        assert_eq!(rounded_mesh.indices.len() / 3, 76);
        assert!(rounded_mesh.positions.len() > 8);

        let plain_parameters = json!({
            "shape": "panel",
            "size_m": [1.6, 0.8, 0.2],
            "thickness_m": 0.2,
            "bevel_m": 0.0,
            "position_m": [0.0, 0.0, 0.0],
            "rotation_rad": [0.0, 0.0, 0.0]
        });
        let (plain, plain_count) = validate_operator(
            "forgecad.geometry.panel@1",
            &[],
            plain_parameters.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .expect("plain panel should validate");
        assert_eq!(plain_count, 12);
        let plain_mesh = compile_operator(&plain, &BTreeMap::new()).expect("plain mesh");
        assert_eq!(plain_mesh.indices.len() / 3, 12);
    }

    #[test]
    fn curved_operator_normals_join_compatible_ring_facets_but_keep_caps_sharp() {
        let parameters = json!({
            "shape": "revolve",
            "profile": [[0.24, -0.20], [0.24, 0.20]],
            "radial_segments": 16,
            "position_m": [0.0, 0.0, 0.0],
            "rotation_rad": [0.0, 0.0, 0.0]
        });
        let (operation, triangle_count) = validate_operator(
            "forgecad.geometry.revolve@1",
            &[],
            parameters.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .expect("revolve should validate");
        assert_eq!(triangle_count, 64);
        let mesh = compile_operator(&operation, &BTreeMap::new()).expect("revolve mesh");
        let mut groups: BTreeMap<(u32, u32, u32), Vec<[f32; 3]>> = BTreeMap::new();
        for (position, normal) in mesh.positions.iter().zip(mesh.normals.iter()) {
            groups
                .entry((
                    position[0].to_bits(),
                    position[1].to_bits(),
                    position[2].to_bits(),
                ))
                .or_default()
                .push(*normal);
        }
        let joined = groups
            .values()
            .filter(|normals| normals.len() > 1)
            .any(|normals| {
                normals
                    .iter()
                    .all(|normal| dot3(*normal, normals[0]) > 0.98)
            });
        assert!(
            joined,
            "adjacent curved facets should share a smooth normal"
        );
        let cap_edge = groups
            .iter()
            .filter(|((x, y, z), normals)| {
                normals.len() > 1
                    && f32::from_bits(*y).abs() > 0.19
                    && f32::from_bits(*x).abs() > 0.20 - 1.0e-4
                    && f32::from_bits(*z).abs() < 1.0e-4
            })
            .any(|(_, normals)| {
                normals.iter().any(|normal| normal[1].abs() > 0.9)
                    && normals.iter().any(|normal| normal[1].abs() < 0.2)
            });
        assert!(
            cap_edge,
            "cap and side normals should not be blended together"
        );
    }
}
