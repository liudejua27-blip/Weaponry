//! MCP010D bounded hard-surface operators.
//!
//! The operators in this module are deliberately small, deterministic mesh
//! constructors. They accept closed typed parameters only; no operator can
//! read a file, execute code, call a network service, or allocate outside the
//! parent GeometryProgram budget. The resulting mesh is handed back to the
//! existing strict GLB/readback path in the parent module.

use super::manifold_bridge;
use super::{
    add3, box_mesh, compile_v2_primitive, cross3, dot3, finite3, length3, normalize,
    require_exact_keys, rotate_xyz, scale3, subtract3, v2_scalar, v2_vec3, GeometryError,
    PrimitiveNodeMesh, ValidatedV2Primitive, MAX_COORDINATE, MAX_DIMENSION,
};
use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

const MAX_PROFILE_POINTS: usize = 64;
const MAX_LOFT_PROFILES: usize = 16;
const MAX_LOFT_V2_RESAMPLE_POINTS: usize = 64;
const MAX_LOFT_V2_INTERPOLATION_RINGS: usize = 16;
const MAX_SWEEP_POINTS: usize = 128;
const SURFACE_PATCH_CONTROL_POINTS: usize = 16;
const MAX_SUBD_CONTROL_POINTS: usize = 256;

#[derive(Debug, Clone, Copy)]
enum ProfileLoftV2Interpolation {
    Linear,
    CatmullRom,
}

#[derive(Debug, Clone)]
pub struct ProfileLoftV2Ring {
    station_m: f32,
    points: Vec<[f32; 2]>,
    corner_flags: Vec<bool>,
}

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
    /// Profile-loft@2 is intentionally kept as a Worker-side typed kernel
    /// until the Runtime contracts/catalog are promoted in a separate change.
    /// Validation materializes all resampled/interpolated rings before mesh
    /// allocation, so compile cannot silently accept a malformed intermediate.
    ProfileLoftV2 {
        rings: Vec<ProfileLoftV2Ring>,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    LongitudinalSectionLoft {
        sections: Vec<(f32, Vec<[f32; 2]>)>,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    SurfacePatch {
        control_points: [[f32; 3]; SURFACE_PATCH_CONTROL_POINTS],
        u_segments: usize,
        v_segments: usize,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    SurfaceShell {
        control_points: [[f32; 3]; SURFACE_PATCH_CONTROL_POINTS],
        u_segments: usize,
        v_segments: usize,
        thickness_m: f32,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    SubdCage {
        control_points: Vec<[f32; 3]>,
        u_points: usize,
        v_points: usize,
        subdivision_levels: usize,
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
    Boolean {
        left: String,
        right: String,
        operation: BooleanOperation,
    },
    PartOutput {
        inputs: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum BooleanOperation {
    Union,
    Difference,
    Intersection,
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
            Self::ProfileLoftV2 { rings, .. } => {
                let points = rings.first().map(|ring| ring.points.len()).unwrap_or(0) as u64;
                let ring_count = rings.len() as u64;
                2 * points.saturating_sub(2) + 2 * points * ring_count.saturating_sub(1)
            }
            Self::LongitudinalSectionLoft { sections, .. } => {
                let points = sections.first().map(|(_, p)| p.len()).unwrap_or(0) as u64;
                2 * points.saturating_sub(2) + 2 * points * sections.len().saturating_sub(1) as u64
            }
            Self::SurfacePatch {
                u_segments,
                v_segments,
                ..
            } => 2 * *u_segments as u64 * *v_segments as u64,
            Self::SurfaceShell {
                u_segments,
                v_segments,
                ..
            } => {
                4 * *u_segments as u64 * *v_segments as u64
                    + 4 * (*u_segments as u64 + *v_segments as u64)
            }
            Self::SubdCage {
                u_points,
                v_points,
                subdivision_levels,
                ..
            } => {
                let base_triangles = (*u_points as u64 - 1)
                    .checked_mul(*v_points as u64 - 1)
                    .and_then(|quads| quads.checked_mul(2))
                    .ok_or_else(|| {
                        GeometryError::Invalid("subd-cage triangle count overflow".to_owned())
                    })?;
                base_triangles
                    .checked_mul(4u64.pow(*subdivision_levels as u32))
                    .ok_or_else(|| {
                        GeometryError::Invalid("subd-cage triangle count overflow".to_owned())
                    })?
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
            Self::Boolean { left, right, .. } => input_counts
                .get(left)
                .ok_or_else(|| GeometryError::Invalid("boolean left input is unknown".to_owned()))?
                .checked_add(*input_counts.get(right).ok_or_else(|| {
                    GeometryError::Invalid("boolean right input is unknown".to_owned())
                })?)
                // Intersections split boundary faces.  Reserve a bounded
                // eight-fold topology allowance instead of treating the
                // input triangle sum as an exact or unsafe upper bound.
                .and_then(|sum| sum.checked_mul(8))
                .ok_or_else(|| {
                    GeometryError::Invalid("boolean triangle count overflow".to_owned())
                })?,
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
        "forgecad.geometry.profile-loft@2" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "profile-loft@2 accepts no inputs".to_owned(),
                ));
            }
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "profiles",
                    "resample_points",
                    "interpolation",
                    "interpolation_rings",
                    "preserve_corners",
                    "position_m",
                    "rotation_rad",
                ],
                "profile-loft@2",
            )?;
            require_shape(parameters, "profile-loft-v2")?;
            let resample_points = bounded_count(
                parameters,
                "resample_points",
                4,
                MAX_LOFT_V2_RESAMPLE_POINTS,
            )?;
            let interpolation = match parameters.get("interpolation").and_then(Value::as_str) {
                Some("linear") => ProfileLoftV2Interpolation::Linear,
                Some("catmull-rom") => ProfileLoftV2Interpolation::CatmullRom,
                _ => {
                    return Err(GeometryError::Invalid(
                        "profile-loft@2 interpolation must be linear or catmull-rom".to_owned(),
                    ))
                }
            };
            let interpolation_rings = bounded_count(
                parameters,
                "interpolation_rings",
                0,
                MAX_LOFT_V2_INTERPOLATION_RINGS,
            )?;
            let preserve_corners = bool_field(parameters, "preserve_corners")?;
            let profiles_value = parameters
                .get("profiles")
                .and_then(Value::as_array)
                .ok_or_else(|| GeometryError::Invalid("profiles must be an array".to_owned()))?;
            if !(2..=MAX_LOFT_PROFILES).contains(&profiles_value.len()) {
                return Err(GeometryError::Invalid(
                    "profile-loft@2 profiles count is outside bounds".to_owned(),
                ));
            }

            let mut profiles = Vec::with_capacity(profiles_value.len());
            let mut previous_station = f32::NEG_INFINITY;
            let mut winding_positive: Option<bool> = None;
            for profile_value in profiles_value {
                let profile_object = profile_value.as_object().ok_or_else(|| {
                    GeometryError::Invalid("profile-loft@2 profile must be an object".to_owned())
                })?;
                require_exact_keys(
                    profile_object,
                    &["station_m", "points", "corner_indices"],
                    "profile-loft@2 profile",
                )?;
                let station_m = number_field(profile_object, "station_m", MAX_COORDINATE)?;
                if station_m <= previous_station {
                    return Err(GeometryError::Invalid(
                        "profile-loft@2 stations must be strictly increasing".to_owned(),
                    ));
                }
                previous_station = station_m;
                let points = parse_points(profile_object, "points", 3, MAX_PROFILE_POINTS)?;
                validate_simple_profile(&points, "profile-loft@2 profile")?;
                let area = signed_area(&points);
                if !area.is_finite() || area.abs() <= 1.0e-5 {
                    return Err(GeometryError::Invalid(
                        "profile-loft@2 profile has zero or non-finite area".to_owned(),
                    ));
                }
                let positive = area > 0.0;
                if let Some(expected) = winding_positive {
                    if expected != positive {
                        return Err(GeometryError::Invalid(
                            "profile-loft@2 profile winding must be consistent".to_owned(),
                        ));
                    }
                } else {
                    winding_positive = Some(positive);
                }
                let corner_indices =
                    parse_corner_indices(profile_object, "corner_indices", points.len())?;
                profiles.push((station_m, points, corner_indices));
            }

            let rings = build_profile_loft_v2_rings(
                &profiles,
                resample_points,
                interpolation,
                interpolation_rings,
                preserve_corners,
            )?;
            ValidatedOperator::ProfileLoftV2 {
                rings,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            }
        }
        "forgecad.geometry.longitudinal-section-loft@1" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "longitudinal-section-loft accepts no inputs".to_owned(),
                ));
            }
            require_exact_keys(
                parameters,
                &["shape", "sections", "position_m", "rotation_rad"],
                "longitudinal-section-loft",
            )?;
            require_shape(parameters, "longitudinal-section-loft")?;
            let sections_value = parameters
                .get("sections")
                .and_then(Value::as_array)
                .ok_or_else(|| GeometryError::Invalid("sections must be an array".to_owned()))?;
            if !(2..=MAX_LOFT_PROFILES).contains(&sections_value.len()) {
                return Err(GeometryError::Invalid(
                    "longitudinal section count is outside bounds".to_owned(),
                ));
            }
            let mut sections: Vec<(f32, Vec<[f32; 2]>)> = Vec::with_capacity(sections_value.len());
            let mut previous_station = f32::NEG_INFINITY;
            for section_value in sections_value {
                let section_object = section_value.as_object().ok_or_else(|| {
                    GeometryError::Invalid("longitudinal section must be an object".to_owned())
                })?;
                require_exact_keys(
                    section_object,
                    &["station_m", "points"],
                    "longitudinal section",
                )?;
                let station = number_field(section_object, "station_m", MAX_COORDINATE)?;
                if station <= previous_station {
                    return Err(GeometryError::Invalid(
                        "longitudinal section stations must be strictly increasing".to_owned(),
                    ));
                }
                previous_station = station;
                let section = parse_points(section_object, "points", 3, MAX_PROFILE_POINTS)?;
                require_nonzero_area(&section, "longitudinal-section-loft section")?;
                if let Some((_, first)) = sections.first() {
                    if first.len() != section.len() {
                        return Err(GeometryError::Invalid(
                            "all longitudinal sections must have the same point count".to_owned(),
                        ));
                    }
                }
                sections.push((station, section));
            }
            ValidatedOperator::LongitudinalSectionLoft {
                sections,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            }
        }
        "forgecad.geometry.surface-patch@1" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "surface-patch accepts no inputs".to_owned(),
                ));
            }
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "control_points",
                    "u_segments",
                    "v_segments",
                    "position_m",
                    "rotation_rad",
                ],
                "surface-patch",
            )?;
            require_shape(parameters, "surface-patch")?;
            let points = parse_vec3_array(
                parameters,
                "control_points",
                SURFACE_PATCH_CONTROL_POINTS,
                SURFACE_PATCH_CONTROL_POINTS,
            )?;
            let control_points: [[f32; 3]; SURFACE_PATCH_CONTROL_POINTS] =
                points.try_into().map_err(|_| {
                    GeometryError::Invalid(
                        "surface-patch control point count is invalid".to_owned(),
                    )
                })?;
            ValidatedOperator::SurfacePatch {
                control_points,
                u_segments: bounded_count(parameters, "u_segments", 4, 32)?,
                v_segments: bounded_count(parameters, "v_segments", 4, 32)?,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            }
        }
        "forgecad.geometry.surface-shell@1" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "surface-shell accepts no inputs".to_owned(),
                ));
            }
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "control_points",
                    "u_segments",
                    "v_segments",
                    "thickness_m",
                    "position_m",
                    "rotation_rad",
                ],
                "surface-shell",
            )?;
            require_shape(parameters, "surface-shell")?;
            let points = parse_vec3_array(
                parameters,
                "control_points",
                SURFACE_PATCH_CONTROL_POINTS,
                SURFACE_PATCH_CONTROL_POINTS,
            )?;
            let control_points: [[f32; 3]; SURFACE_PATCH_CONTROL_POINTS] =
                points.try_into().map_err(|_| {
                    GeometryError::Invalid(
                        "surface-shell control point count is invalid".to_owned(),
                    )
                })?;
            let thickness_m = v2_scalar(parameters, "thickness_m", MAX_DIMENSION, true)?;
            if thickness_m < 1.0e-4 {
                return Err(GeometryError::Invalid(
                    "surface-shell thickness is below the stable mesh tolerance".to_owned(),
                ));
            }
            ValidatedOperator::SurfaceShell {
                control_points,
                u_segments: bounded_count(parameters, "u_segments", 4, 32)?,
                v_segments: bounded_count(parameters, "v_segments", 4, 32)?,
                thickness_m,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            }
        }
        "forgecad.geometry.subd-cage@1" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "subd-cage accepts no inputs".to_owned(),
                ));
            }
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "control_points",
                    "u_points",
                    "v_points",
                    "subdivision_levels",
                    "position_m",
                    "rotation_rad",
                ],
                "subd-cage",
            )?;
            require_shape(parameters, "subd-cage")?;
            let u_points = bounded_count(parameters, "u_points", 2, 16)?;
            let v_points = bounded_count(parameters, "v_points", 2, 16)?;
            let subdivision_levels = bounded_count(parameters, "subdivision_levels", 0, 2)?;
            let control_points =
                parse_vec3_array(parameters, "control_points", 4, MAX_SUBD_CONTROL_POINTS)?;
            let expected_points = u_points.checked_mul(v_points).ok_or_else(|| {
                GeometryError::Invalid("subd-cage control point count overflow".to_owned())
            })?;
            if control_points.len() != expected_points {
                return Err(GeometryError::Invalid(format!(
                    "subd-cage requires exactly {expected_points} control points"
                )));
            }
            ValidatedOperator::SubdCage {
                control_points,
                u_points,
                v_points,
                subdivision_levels,
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
            if inputs.len() != 2 {
                return Err(GeometryError::Invalid(
                    "boolean requires exactly two inputs".to_owned(),
                ));
            }
            require_exact_keys(parameters, &["shape"], "boolean")?;
            let operation = match parameters.get("shape").and_then(Value::as_str) {
                Some("union") => BooleanOperation::Union,
                Some("difference") => BooleanOperation::Difference,
                Some("intersection") => BooleanOperation::Intersection,
                _ => {
                    return Err(GeometryError::Invalid(
                        "boolean shape must be union, difference or intersection".to_owned(),
                    ))
                }
            };
            ValidatedOperator::Boolean {
                left: inputs[0].clone(),
                right: inputs[1].clone(),
                operation,
            }
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
    max_triangles: u64,
    max_runtime_ms: u64,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let mut mesh = match operation {
        ValidatedOperator::Primitive(primitive) => {
            let (positions, normals, indices) = compile_v2_primitive(primitive);
            PrimitiveNodeMesh {
                operator_id: "forgecad.geometry.primitive@2".to_owned(),
                lineage_source_node_ids: Vec::new(),
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
        ValidatedOperator::ProfileLoftV2 {
            rings,
            position_m,
            rotation_rad,
        } => transform_mesh(
            profile_loft_v2_mesh(rings)?,
            *position_m,
            *rotation_rad,
            [1.0; 3],
        ),
        ValidatedOperator::LongitudinalSectionLoft {
            sections,
            position_m,
            rotation_rad,
        } => transform_mesh(
            longitudinal_section_loft_mesh(sections)?,
            *position_m,
            *rotation_rad,
            [1.0; 3],
        ),
        ValidatedOperator::SurfacePatch {
            control_points,
            u_segments,
            v_segments,
            position_m,
            rotation_rad,
        } => transform_mesh(
            surface_patch_mesh(control_points, *u_segments, *v_segments)?,
            *position_m,
            *rotation_rad,
            [1.0; 3],
        ),
        ValidatedOperator::SurfaceShell {
            control_points,
            u_segments,
            v_segments,
            thickness_m,
            position_m,
            rotation_rad,
        } => transform_mesh(
            surface_shell_mesh(control_points, *u_segments, *v_segments, *thickness_m)?,
            *position_m,
            *rotation_rad,
            [1.0; 3],
        ),
        ValidatedOperator::SubdCage {
            control_points,
            u_points,
            v_points,
            subdivision_levels,
            position_m,
            rotation_rad,
        } => transform_mesh(
            subd_cage_mesh(control_points, *u_points, *v_points, *subdivision_levels)?,
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
                    lineage_source_node_ids: Vec::new(),
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
        ValidatedOperator::Boolean {
            left,
            right,
            operation,
        } => {
            let left_mesh = input_mesh(meshes, left)?;
            let right_mesh = input_mesh(meshes, right)?;
            let operation_name = match operation {
                BooleanOperation::Union => "union",
                BooleanOperation::Difference => "difference",
                BooleanOperation::Intersection => "intersection",
            };
            let result = manifold_bridge::execute_boolean(
                left_mesh,
                right_mesh,
                operation_name,
                max_triangles,
                max_runtime_ms,
            )?;
            // The C bridge has already strict-read the output topology,
            // source-run IDs, and face IDs.  The existing GLB path consumes
            // the typed mesh and regenerates UV/tangent data at the semantic
            // Part boundary, while the bridge result remains the source of
            // truth for the Boolean topology and lineage check.
            let _lineage_probe = (result.source_ids.len(), result.face_ids.len());
            let _topology_metrics = (result.volume, result.surface_area, result.genus);
            let mut lineage_source_node_ids = left_mesh.lineage_source_node_ids.clone();
            for source_node_id in &right_mesh.lineage_source_node_ids {
                if !lineage_source_node_ids
                    .iter()
                    .any(|existing| existing == source_node_id)
                {
                    lineage_source_node_ids.push(source_node_id.clone());
                }
            }
            PrimitiveNodeMesh {
                operator_id: "forgecad.geometry.boolean@1".to_owned(),
                lineage_source_node_ids,
                positions: result.positions,
                normals: result.normals,
                indices: result.indices,
            }
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
            | ValidatedOperator::LongitudinalSectionLoft { .. }
            | ValidatedOperator::SurfacePatch { .. }
            | ValidatedOperator::SurfaceShell { .. }
            | ValidatedOperator::SubdCage { .. }
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
    smooth_curved_normals_with_hard_points(mesh, &BTreeSet::new());
}

fn smooth_curved_normals_with_hard_points(
    mesh: &mut PrimitiveNodeMesh,
    hard_positions: &BTreeSet<(u32, u32, u32)>,
) {
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
        if indices.iter().any(|index| {
            let position = mesh.positions[*index];
            hard_positions.contains(&(
                position[0].to_bits(),
                position[1].to_bits(),
                position[2].to_bits(),
            ))
        }) {
            // Explicit/detected profile corners are crease anchors.  Keeping
            // the duplicated chart normals untouched preserves the authored
            // hard break even when neighbouring facets are otherwise smooth.
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

fn parse_corner_indices(
    object: &Map<String, Value>,
    key: &str,
    point_count: usize,
) -> Result<Vec<usize>, GeometryError> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} must be an array")))?;
    if values.len() > point_count || values.len() > MAX_PROFILE_POINTS {
        return Err(GeometryError::Invalid(format!(
            "{key} count is outside bounds"
        )));
    }
    let mut indices = Vec::with_capacity(values.len());
    for value in values {
        let index = value.as_u64().ok_or_else(|| {
            GeometryError::Invalid(format!("{key} must contain non-negative integers"))
        })? as usize;
        if index >= point_count || indices.contains(&index) {
            return Err(GeometryError::Invalid(format!(
                "{key} contains a duplicate or out-of-range index"
            )));
        }
        indices.push(index);
    }
    indices.sort_unstable();
    Ok(indices)
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

fn subtract2(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn add2(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

fn scale2(value: [f32; 2], scalar: f32) -> [f32; 2] {
    [value[0] * scalar, value[1] * scalar]
}

fn dot2(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

fn cross2(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[1] - a[1] * b[0]
}

fn length2(value: [f32; 2]) -> f32 {
    dot2(value, value).sqrt()
}

fn lerp2(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    add2(a, scale2(subtract2(b, a), t))
}

/// Reject a profile before any resampling.  A profile is deliberately a
/// simple polygon rather than a general winding/path expression: accepting
/// self-intersections here would make the cap triangulation and station
/// correspondence ambiguous.
fn validate_simple_profile(profile: &[[f32; 2]], label: &str) -> Result<(), GeometryError> {
    const EDGE_EPSILON_SQUARED: f32 = 1.0e-10;
    if profile.len() < 3 {
        return Err(GeometryError::Invalid(format!(
            "{label} requires at least three points"
        )));
    }
    for index in 0..profile.len() {
        let next = (index + 1) % profile.len();
        let edge = subtract2(profile[next], profile[index]);
        if !edge[0].is_finite() || !edge[1].is_finite() || dot2(edge, edge) <= EDGE_EPSILON_SQUARED
        {
            return Err(GeometryError::Invalid(format!(
                "{label} contains a zero-length edge"
            )));
        }
    }
    for first in 0..profile.len() {
        let first_next = (first + 1) % profile.len();
        for second in (first + 1)..profile.len() {
            let second_next = (second + 1) % profile.len();
            // Adjacent edges are allowed to meet at their shared vertex.  The
            // edge pair (0,last) is adjacent as well.
            if first == second
                || first_next == second
                || second_next == first
                || (first == 0 && second_next == 0)
            {
                continue;
            }
            if segments_intersect(
                profile[first],
                profile[first_next],
                profile[second],
                profile[second_next],
            ) {
                return Err(GeometryError::Invalid(format!(
                    "{label} contains a self-intersection"
                )));
            }
        }
    }
    Ok(())
}

fn segments_intersect(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2]) -> bool {
    const EPSILON: f32 = 1.0e-6;
    let ab = subtract2(b, a);
    let ac = subtract2(c, a);
    let ad = subtract2(d, a);
    let cd = subtract2(d, c);
    let ca = subtract2(a, c);
    let cb = subtract2(b, c);
    let o1 = cross2(ab, ac);
    let o2 = cross2(ab, ad);
    let o3 = cross2(cd, ca);
    let o4 = cross2(cd, cb);
    let opposite = |left: f32, right: f32| {
        (left > EPSILON && right < -EPSILON) || (left < -EPSILON && right > EPSILON)
    };
    if opposite(o1, o2) && opposite(o3, o4) {
        return true;
    }
    let on_segment = |p: [f32; 2], start: [f32; 2], end: [f32; 2]| {
        p[0] >= start[0].min(end[0]) - EPSILON
            && p[0] <= start[0].max(end[0]) + EPSILON
            && p[1] >= start[1].min(end[1]) - EPSILON
            && p[1] <= start[1].max(end[1]) + EPSILON
    };
    (o1.abs() <= EPSILON && on_segment(c, a, b))
        || (o2.abs() <= EPSILON && on_segment(d, a, b))
        || (o3.abs() <= EPSILON && on_segment(a, c, d))
        || (o4.abs() <= EPSILON && on_segment(b, c, d))
}

fn oriented_profile(profile: &[[f32; 2]]) -> Vec<[f32; 2]> {
    if signed_area(profile) >= 0.0 {
        profile.to_vec()
    } else {
        profile.iter().rev().copied().collect()
    }
}

fn oriented_profile_with_corners(
    profile: &[[f32; 2]],
    corner_indices: &[usize],
) -> (Vec<[f32; 2]>, Vec<usize>) {
    if signed_area(profile) >= 0.0 {
        (profile.to_vec(), corner_indices.to_vec())
    } else {
        (
            profile.iter().rev().copied().collect(),
            corner_indices
                .iter()
                .map(|index| profile.len() - 1 - *index)
                .collect(),
        )
    }
}

fn detected_corner_indices(profile: &[[f32; 2]]) -> Vec<usize> {
    const CORNER_COSINE_LIMIT: f32 = 0.82;
    let mut indices = Vec::new();
    for index in 0..profile.len() {
        let previous = profile[(index + profile.len() - 1) % profile.len()];
        let current = profile[index];
        let next = profile[(index + 1) % profile.len()];
        let incoming = subtract2(previous, current);
        let outgoing = subtract2(next, current);
        let denominator = length2(incoming) * length2(outgoing);
        if denominator <= f32::EPSILON {
            continue;
        }
        let cosine = dot2(incoming, outgoing) / denominator;
        if cosine.is_finite() && cosine <= CORNER_COSINE_LIMIT {
            indices.push(index);
        }
    }
    indices
}

fn merge_corner_indices(
    profile: &[[f32; 2]],
    explicit: &[usize],
    preserve_corners: bool,
) -> Vec<usize> {
    let mut indices = BTreeSet::new();
    if preserve_corners {
        indices.extend(explicit.iter().copied());
        indices.extend(detected_corner_indices(profile));
    }
    indices.into_iter().collect()
}

/// Resample a simple closed contour by perimeter distance.  Explicit and
/// detected corners become fixed anchors; the remaining samples are allocated
/// by interval length with a stable largest-remainder rule.  This avoids the
/// common failure where uniform sampling silently moves a weapon's sharp
/// silhouette corners off their intended positions.
fn resample_closed_profile(
    profile: &[[f32; 2]],
    corner_indices: &[usize],
    sample_count: usize,
) -> Result<ProfileLoftV2Ring, GeometryError> {
    if !(3..=MAX_PROFILE_POINTS).contains(&profile.len()) {
        return Err(GeometryError::Invalid(
            "profile-loft@2 profile point count is outside bounds".to_owned(),
        ));
    }
    if !(4..=MAX_LOFT_V2_RESAMPLE_POINTS).contains(&sample_count) {
        return Err(GeometryError::Invalid(
            "profile-loft@2 resample_points is outside bounds".to_owned(),
        ));
    }
    let mut cumulative = vec![0.0f32; profile.len() + 1];
    for index in 0..profile.len() {
        cumulative[index + 1] = cumulative[index]
            + length2(subtract2(
                profile[(index + 1) % profile.len()],
                profile[index],
            ));
    }
    let perimeter = *cumulative.last().expect("closed profile cumulative length");
    if !perimeter.is_finite() || perimeter <= 1.0e-5 {
        return Err(GeometryError::Invalid(
            "profile-loft@2 profile perimeter is invalid".to_owned(),
        ));
    }

    let mut anchors = BTreeSet::new();
    // Keep the authored start as a deterministic phase anchor even when it is
    // not a crease.  Other profiles are phase-aligned after resampling.
    anchors.insert(0usize);
    anchors.extend(corner_indices.iter().copied());
    if anchors.len() > sample_count {
        return Err(GeometryError::Invalid(
            "profile-loft@2 resample_points cannot preserve all corners".to_owned(),
        ));
    }
    let anchors = anchors.into_iter().collect::<Vec<_>>();
    let interval_count = anchors.len();
    let mut lengths = Vec::with_capacity(interval_count);
    for interval in 0..interval_count {
        let start_index = anchors[interval];
        let end_index = anchors[(interval + 1) % interval_count];
        let start = cumulative[start_index];
        let mut end = cumulative[end_index];
        if interval + 1 == interval_count {
            end += perimeter;
        }
        let length = end - start;
        if !length.is_finite() || length <= 1.0e-7 {
            return Err(GeometryError::Invalid(
                "profile-loft@2 corner anchors are coincident".to_owned(),
            ));
        }
        lengths.push(length);
    }
    let extra = sample_count - interval_count;
    let mut allocations = vec![1usize; interval_count];
    let mut remainders = Vec::with_capacity(interval_count);
    let extra_f = extra as f32;
    let mut assigned_extra = 0usize;
    for (index, length) in lengths.iter().copied().enumerate() {
        let ideal = if extra == 0 {
            0.0
        } else {
            length / perimeter * extra_f
        };
        let floor = ideal.floor() as usize;
        allocations[index] += floor;
        assigned_extra += floor;
        remainders.push((ideal - floor as f32, index));
    }
    remainders.sort_by(
        |(left_remainder, left_index), (right_remainder, right_index)| {
            right_remainder
                .partial_cmp(left_remainder)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left_index.cmp(right_index))
        },
    );
    for (_, index) in remainders
        .into_iter()
        .take(extra.saturating_sub(assigned_extra))
    {
        allocations[index] += 1;
    }

    let is_corner = |index: usize| corner_indices.binary_search(&index).is_ok();
    let mut points = Vec::with_capacity(sample_count);
    let mut corner_flags = Vec::with_capacity(sample_count);
    for interval in 0..interval_count {
        let start_index = anchors[interval];
        let start_distance = cumulative[start_index];
        let end_distance = start_distance + lengths[interval];
        points.push(profile[start_index]);
        corner_flags.push(is_corner(start_index));
        for step in 1..allocations[interval] {
            let distance =
                start_distance + lengths[interval] * (step as f32 / allocations[interval] as f32);
            points.push(sample_profile_distance(
                profile,
                &cumulative,
                perimeter,
                distance,
            ));
            corner_flags.push(false);
        }
        debug_assert!(end_distance.is_finite());
    }
    if points.len() != sample_count || corner_flags.len() != sample_count {
        return Err(GeometryError::Invalid(
            "profile-loft@2 resampling produced an invalid sample count".to_owned(),
        ));
    }
    validate_simple_profile(&points, "profile-loft@2 resampled profile")?;
    if signed_area(&points) <= 1.0e-5 {
        return Err(GeometryError::Invalid(
            "profile-loft@2 resampled profile winding is invalid".to_owned(),
        ));
    }
    Ok(ProfileLoftV2Ring {
        station_m: 0.0,
        points,
        corner_flags,
    })
}

fn sample_profile_distance(
    profile: &[[f32; 2]],
    cumulative: &[f32],
    perimeter: f32,
    mut distance: f32,
) -> [f32; 2] {
    distance = distance.rem_euclid(perimeter);
    let mut edge = 0usize;
    while edge + 1 < cumulative.len() && cumulative[edge + 1] < distance {
        edge += 1;
    }
    if edge >= profile.len() {
        return profile[0];
    }
    let edge_length = cumulative[edge + 1] - cumulative[edge];
    if edge_length <= f32::EPSILON {
        return profile[edge];
    }
    let t = ((distance - cumulative[edge]) / edge_length).clamp(0.0, 1.0);
    lerp2(profile[edge], profile[(edge + 1) % profile.len()], t)
}

fn normalized_ring_points(points: &[[f32; 2]]) -> (Vec<[f32; 2]>, [f32; 2], f32) {
    let mut center = [0.0f32; 2];
    for point in points {
        center = add2(center, *point);
    }
    let inverse_count = 1.0 / points.len() as f32;
    center = scale2(center, inverse_count);
    let mut radius_squared = 0.0f32;
    for point in points {
        let delta = subtract2(*point, center);
        radius_squared += dot2(delta, delta);
    }
    let radius = (radius_squared * inverse_count).sqrt().max(1.0e-5);
    (
        points
            .iter()
            .map(|point| scale2(subtract2(*point, center), 1.0 / radius))
            .collect(),
        center,
        radius,
    )
}

fn align_ring_phase(reference: &[[f32; 2]], candidate: &mut ProfileLoftV2Ring) {
    if reference.len() != candidate.points.len() || reference.is_empty() {
        return;
    }
    let (reference_normalized, _, _) = normalized_ring_points(reference);
    let (candidate_normalized, _, _) = normalized_ring_points(&candidate.points);
    let sample_count = reference.len();
    let mut best_shift = 0usize;
    let mut best_cost = f32::INFINITY;
    for shift in 0..sample_count {
        let mut cost = 0.0f32;
        for index in 0..sample_count {
            let delta = subtract2(
                reference_normalized[index],
                candidate_normalized[(index + shift) % sample_count],
            );
            cost += dot2(delta, delta);
        }
        if cost < best_cost - 1.0e-7 {
            best_cost = cost;
            best_shift = shift;
        }
    }
    if best_shift == 0 {
        return;
    }
    let points = candidate.points.clone();
    let flags = candidate.corner_flags.clone();
    for index in 0..sample_count {
        candidate.points[index] = points[(index + best_shift) % sample_count];
        candidate.corner_flags[index] = flags[(index + best_shift) % sample_count];
    }
}

fn catmull_rom2(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2], t: f32) -> [f32; 2] {
    let t2 = t * t;
    let t3 = t2 * t;
    scale2(
        add2(
            add2(scale2(p1, 2.0), scale2(subtract2(p2, p0), t)),
            add2(
                scale2(
                    add2(
                        add2(scale2(p0, 2.0), scale2(p1, -5.0)),
                        add2(scale2(p2, 4.0), scale2(p3, -1.0)),
                    ),
                    t2,
                ),
                scale2(
                    add2(
                        add2(scale2(p0, -1.0), scale2(p1, 3.0)),
                        add2(scale2(p2, -3.0), p3),
                    ),
                    t3,
                ),
            ),
        ),
        0.5,
    )
}

fn build_profile_loft_v2_rings(
    profiles: &[(f32, Vec<[f32; 2]>, Vec<usize>)],
    resample_points: usize,
    interpolation: ProfileLoftV2Interpolation,
    interpolation_rings: usize,
    preserve_corners: bool,
) -> Result<Vec<ProfileLoftV2Ring>, GeometryError> {
    let mut sampled = Vec::with_capacity(profiles.len());
    for (station_m, profile, explicit_corners) in profiles {
        let (oriented, oriented_explicit_corners) =
            oriented_profile_with_corners(profile, explicit_corners);
        let corners = merge_corner_indices(&oriented, &oriented_explicit_corners, preserve_corners);
        let mut ring = resample_closed_profile(&oriented, &corners, resample_points)?;
        ring.station_m = *station_m;
        sampled.push(ring);
    }
    let reference = sampled[0].points.clone();
    for ring in sampled.iter_mut().skip(1) {
        align_ring_phase(&reference, ring);
    }

    let total_rings = sampled
        .len()
        .checked_add(
            (sampled.len() - 1)
                .checked_mul(interpolation_rings)
                .ok_or_else(|| {
                    GeometryError::Invalid(
                        "profile-loft@2 interpolation ring count overflow".to_owned(),
                    )
                })?,
        )
        .ok_or_else(|| {
            GeometryError::Invalid("profile-loft@2 interpolation ring count overflow".to_owned())
        })?;
    if total_rings < 2 || total_rings > 257 {
        return Err(GeometryError::Invalid(
            "profile-loft@2 total ring count is outside bounds".to_owned(),
        ));
    }

    let mut rings = Vec::with_capacity(total_rings);
    for interval in 0..(sampled.len() - 1) {
        let left = &sampled[interval];
        let right = &sampled[interval + 1];
        rings.push(left.clone());
        for step in 1..=interpolation_rings {
            let t = step as f32 / (interpolation_rings + 1) as f32;
            let mut points = Vec::with_capacity(resample_points);
            let mut corner_flags = Vec::with_capacity(resample_points);
            for point_index in 0..resample_points {
                let point = match interpolation {
                    ProfileLoftV2Interpolation::Linear => {
                        lerp2(left.points[point_index], right.points[point_index], t)
                    }
                    ProfileLoftV2Interpolation::CatmullRom => {
                        let p0 = if interval == 0 {
                            left.points[point_index]
                        } else {
                            sampled[interval - 1].points[point_index]
                        };
                        let p3 = if interval + 2 >= sampled.len() {
                            right.points[point_index]
                        } else {
                            sampled[interval + 2].points[point_index]
                        };
                        catmull_rom2(
                            p0,
                            left.points[point_index],
                            right.points[point_index],
                            p3,
                            t,
                        )
                    }
                };
                if !point[0].is_finite() || !point[1].is_finite() {
                    return Err(GeometryError::Invalid(
                        "profile-loft@2 interpolation emitted non-finite point".to_owned(),
                    ));
                }
                points.push(point);
                corner_flags
                    .push(left.corner_flags[point_index] || right.corner_flags[point_index]);
            }
            validate_simple_profile(&points, "profile-loft@2 interpolated ring")?;
            if signed_area(&points) <= 1.0e-5 {
                return Err(GeometryError::Invalid(
                    "profile-loft@2 interpolation changed profile winding".to_owned(),
                ));
            }
            rings.push(ProfileLoftV2Ring {
                station_m: left.station_m + (right.station_m - left.station_m) * t,
                points,
                corner_flags,
            });
        }
    }
    rings.push(sampled.last().expect("at least two profiles").clone());
    for pair in rings.windows(2) {
        if pair[1].station_m <= pair[0].station_m {
            return Err(GeometryError::Invalid(
                "profile-loft@2 ring stations must be strictly increasing".to_owned(),
            ));
        }
    }
    Ok(rings)
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

fn profile_loft_v2_mesh(rings: &[ProfileLoftV2Ring]) -> Result<PrimitiveNodeMesh, GeometryError> {
    let first = rings.first().ok_or_else(|| {
        GeometryError::Invalid("profile-loft@2 requires at least two rings".to_owned())
    })?;
    if rings.len() < 2 || first.points.len() < 3 {
        return Err(GeometryError::Invalid(
            "profile-loft@2 ring topology is invalid".to_owned(),
        ));
    }
    let point_count = first.points.len();
    if rings
        .iter()
        .any(|ring| ring.points.len() != point_count || ring.corner_flags.len() != point_count)
    {
        return Err(GeometryError::Invalid(
            "profile-loft@2 ring correspondence is invalid".to_owned(),
        ));
    }
    let cap_triangles = triangulate_simple_polygon(&first.points)?;
    let mut mesh = empty_mesh();
    for [a, b, c] in &cap_triangles {
        push_triangle(
            &mut mesh,
            [first.station_m, first.points[*a][0], first.points[*a][1]],
            [first.station_m, first.points[*c][0], first.points[*c][1]],
            [first.station_m, first.points[*b][0], first.points[*b][1]],
        )?;
    }
    for pair in rings.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        for index in 0..point_count {
            let next = (index + 1) % point_count;
            let a = [left.station_m, left.points[index][0], left.points[index][1]];
            let b = [left.station_m, left.points[next][0], left.points[next][1]];
            let c = [
                right.station_m,
                right.points[next][0],
                right.points[next][1],
            ];
            let d = [
                right.station_m,
                right.points[index][0],
                right.points[index][1],
            ];
            push_triangle(&mut mesh, a, b, c)?;
            push_triangle(&mut mesh, a, c, d)?;
        }
    }
    let last = rings.last().expect("at least two rings");
    let cap_triangles = triangulate_simple_polygon(&last.points)?;
    for [a, b, c] in &cap_triangles {
        push_triangle(
            &mut mesh,
            [last.station_m, last.points[*a][0], last.points[*a][1]],
            [last.station_m, last.points[*b][0], last.points[*b][1]],
            [last.station_m, last.points[*c][0], last.points[*c][1]],
        )?;
    }
    let hard_positions = rings
        .iter()
        .flat_map(|ring| {
            ring.points
                .iter()
                .zip(ring.corner_flags.iter())
                .filter_map(|(point, is_corner)| {
                    is_corner.then_some((
                        ring.station_m.to_bits(),
                        point[0].to_bits(),
                        point[1].to_bits(),
                    ))
                })
        })
        .collect::<BTreeSet<_>>();
    smooth_curved_normals_with_hard_points(&mut mesh, &hard_positions);
    Ok(mesh)
}

/// Deterministically triangulate one validated, counter-clockwise simple
/// polygon. A fan is not valid for arbitrary concave hard-surface profiles;
/// stable ear clipping keeps endpoint caps inside the authored silhouette.
fn triangulate_simple_polygon(profile: &[[f32; 2]]) -> Result<Vec<[usize; 3]>, GeometryError> {
    if profile.len() < 3 || signed_area(profile) <= 1.0e-5 {
        return Err(GeometryError::Invalid(
            "profile-loft@2 cap profile must be counter-clockwise and non-zero".to_owned(),
        ));
    }
    let mut remaining = (0..profile.len()).collect::<Vec<_>>();
    let mut triangles = Vec::with_capacity(profile.len() - 2);
    while remaining.len() > 3 {
        let mut clipped = false;
        for cursor in 0..remaining.len() {
            let previous = remaining[(cursor + remaining.len() - 1) % remaining.len()];
            let current = remaining[cursor];
            let next = remaining[(cursor + 1) % remaining.len()];
            let a = profile[previous];
            let b = profile[current];
            let c = profile[next];
            if cross2(subtract2(b, a), subtract2(c, b)) <= 1.0e-7 {
                continue;
            }
            if remaining.iter().copied().any(|candidate| {
                candidate != previous
                    && candidate != current
                    && candidate != next
                    && point_in_or_on_triangle(profile[candidate], a, b, c)
            }) {
                continue;
            }
            triangles.push([previous, current, next]);
            remaining.remove(cursor);
            clipped = true;
            break;
        }
        if !clipped {
            return Err(GeometryError::Invalid(
                "profile-loft@2 cap triangulation failed".to_owned(),
            ));
        }
    }
    triangles.push([remaining[0], remaining[1], remaining[2]]);
    Ok(triangles)
}

fn point_in_or_on_triangle(point: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    const EPSILON: f32 = 1.0e-7;
    let ab = cross2(subtract2(b, a), subtract2(point, a));
    let bc = cross2(subtract2(c, b), subtract2(point, b));
    let ca = cross2(subtract2(a, c), subtract2(point, c));
    ab >= -EPSILON && bc >= -EPSILON && ca >= -EPSILON
}

/// Loft bounded Y/Z cross-sections along the subject's longitudinal +X axis.
///
/// `profile-loft@1` intentionally keeps its historical Z-station semantics.
/// This separate operator prevents a side-view contour from becoming a thin
/// slab while giving reference-driven products explicit width/depth stations.
fn longitudinal_section_loft_mesh(
    sections: &[(f32, Vec<[f32; 2]>)],
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let mut mesh = empty_mesh();
    let first = oriented_profile(&sections[0].1);
    for index in 1..first.len() - 1 {
        push_triangle(
            &mut mesh,
            [sections[0].0, first[0][0], first[0][1]],
            [sections[0].0, first[index][0], first[index][1]],
            [sections[0].0, first[index + 1][0], first[index + 1][1]],
        )?;
    }
    for level in 0..sections.len() - 1 {
        let x0 = sections[level].0;
        let x1 = sections[level + 1].0;
        let p0 = oriented_profile(&sections[level].1);
        let p1 = oriented_profile(&sections[level + 1].1);
        for index in 0..p0.len() {
            let next = (index + 1) % p0.len();
            let a = [x0, p0[index][0], p0[index][1]];
            let b = [x0, p0[next][0], p0[next][1]];
            let c = [x1, p1[next][0], p1[next][1]];
            let d = [x1, p1[index][0], p1[index][1]];
            push_triangle(&mut mesh, a, c, b)?;
            push_triangle(&mut mesh, a, d, c)?;
        }
    }
    let (last_x, last_section) = sections.last().expect("validated longitudinal sections");
    let last = oriented_profile(last_section);
    for index in 1..last.len() - 1 {
        push_triangle(
            &mut mesh,
            [*last_x, last[0][0], last[0][1]],
            [*last_x, last[index + 1][0], last[index + 1][1]],
            [*last_x, last[index][0], last[index][1]],
        )?;
    }
    Ok(mesh)
}

/// Evaluate one bounded bicubic Bezier patch.  This is the first Visual
/// Surface representation: it gives Codex a smooth, editable primary shell
/// envelope without admitting arbitrary subdivision scripts or a hidden DCC.
/// The patch is intentionally an open surface; the caller must mark its
/// semantic Part `solid=false` until a future typed shell-thickness operator
/// closes it.
fn surface_patch_mesh(
    control_points: &[[f32; 3]; SURFACE_PATCH_CONTROL_POINTS],
    u_segments: usize,
    v_segments: usize,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let mut mesh = empty_mesh();
    let stride = u_segments + 1;
    let mut positions = Vec::with_capacity((u_segments + 1) * (v_segments + 1));
    let mut normals = Vec::with_capacity((u_segments + 1) * (v_segments + 1));
    for v_index in 0..=v_segments {
        let v = v_index as f32 / v_segments as f32;
        for u_index in 0..=u_segments {
            let u = u_index as f32 / u_segments as f32;
            let position = bezier_patch_point(control_points, u, v, false);
            let du = bezier_patch_point(control_points, u, v, true);
            let dv = bezier_patch_point_v_derivative(control_points, u, v);
            let cross = cross3(du, dv);
            let normal = normalize(cross);
            if !finite3(position) || !finite3(normal) || !finite3(cross) || length3(cross) <= 1.0e-6
            {
                return Err(GeometryError::Invalid(
                    "surface-patch contains a degenerate or non-finite sample".to_owned(),
                ));
            }
            positions.push(position);
            normals.push(normal);
        }
    }
    let mut indices = Vec::with_capacity(u_segments * v_segments * 6);
    for v_index in 0..v_segments {
        for u_index in 0..u_segments {
            let a = (v_index * stride + u_index) as u32;
            let b = a + 1;
            let c = a + stride as u32;
            let d = c + 1;
            indices.extend([a, b, c, b, d, c]);
        }
    }
    mesh.positions = positions;
    mesh.normals = normals;
    mesh.indices = indices;
    Ok(mesh)
}

/// Close a bounded Bézier patch with a uniform, symmetric shell. The shell
/// duplicates boundary vertices for hard side normals, but the strict GLB
/// readback welds those positions and verifies that the resulting semantic
/// Part is watertight. This is intentionally a constant-thickness mesh
/// envelope, not a general offset/shell solver: self-intersection, variable
/// thickness, trim loops, and feature-line semantics remain outside this
/// operator's contract.
fn surface_shell_mesh(
    control_points: &[[f32; 3]; SURFACE_PATCH_CONTROL_POINTS],
    u_segments: usize,
    v_segments: usize,
    thickness_m: f32,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let mut mesh = empty_mesh();
    let stride = u_segments + 1;
    let half_thickness = thickness_m * 0.5;

    for v_index in 0..=v_segments {
        let v = v_index as f32 / v_segments as f32;
        for u_index in 0..=u_segments {
            let u = u_index as f32 / u_segments as f32;
            let position = bezier_patch_point(control_points, u, v, false);
            let du = bezier_patch_point(control_points, u, v, true);
            let dv = bezier_patch_point_v_derivative(control_points, u, v);
            let cross = cross3(du, dv);
            let normal = normalize(cross);
            if !finite3(position) || !finite3(normal) || !finite3(cross) || length3(cross) <= 1.0e-6
            {
                return Err(GeometryError::Invalid(
                    "surface-shell contains a degenerate or non-finite sample".to_owned(),
                ));
            }
            mesh.positions
                .push(add3(position, scale3(normal, half_thickness)));
            mesh.normals.push(normal);
            mesh.positions
                .push(subtract3(position, scale3(normal, half_thickness)));
            mesh.normals.push(scale3(normal, -1.0));
        }
    }

    let mut indices =
        Vec::with_capacity(6 * u_segments * v_segments + 12 * (u_segments + v_segments));
    for v_index in 0..v_segments {
        for u_index in 0..u_segments {
            let surface_index = v_index * stride + u_index;
            let a = (surface_index * 2) as u32;
            let b = ((surface_index + 1) * 2) as u32;
            let c = ((surface_index + stride) * 2) as u32;
            let d = ((surface_index + stride + 1) * 2) as u32;
            indices.extend([a, b, c, b, d, c]);
            let a = a + 1;
            let b = b + 1;
            let c = c + 1;
            let d = d + 1;
            indices.extend([a, c, b, b, c, d]);
        }
    }

    let add_boundary_quad =
        |mesh: &mut PrimitiveNodeMesh, corners: [[f32; 3]; 4]| -> Result<(), GeometryError> {
            let normal = normalize(cross3(
                subtract3(corners[1], corners[0]),
                subtract3(corners[2], corners[0]),
            ));
            if !finite3(normal) || length3(normal) <= 1.0e-6 {
                return Err(GeometryError::Invalid(
                    "surface-shell boundary contains a degenerate side".to_owned(),
                ));
            }
            let base = mesh.positions.len() as u32;
            mesh.positions.extend(corners);
            mesh.normals.extend([normal; 4]);
            mesh.indices
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            Ok(())
        };

    let top_index = |surface_index: usize| (surface_index * 2) as usize;
    let bottom_index = |surface_index: usize| (surface_index * 2 + 1) as usize;
    for u_index in 0..u_segments {
        let first = u_index;
        let next = u_index + 1;
        let corners = [
            mesh.positions[top_index(first)],
            mesh.positions[bottom_index(first)],
            mesh.positions[bottom_index(next)],
            mesh.positions[top_index(next)],
        ];
        add_boundary_quad(&mut mesh, corners)?;

        let first = v_segments * stride + u_index;
        let next = first + 1;
        let corners = [
            mesh.positions[top_index(first)],
            mesh.positions[top_index(next)],
            mesh.positions[bottom_index(next)],
            mesh.positions[bottom_index(first)],
        ];
        add_boundary_quad(&mut mesh, corners)?;
    }
    for v_index in 0..v_segments {
        let first = v_index * stride;
        let next = (v_index + 1) * stride;
        let corners = [
            mesh.positions[top_index(first)],
            mesh.positions[top_index(next)],
            mesh.positions[bottom_index(next)],
            mesh.positions[bottom_index(first)],
        ];
        add_boundary_quad(&mut mesh, corners)?;

        let first = v_index * stride + u_segments;
        let next = (v_index + 1) * stride + u_segments;
        let corners = [
            mesh.positions[top_index(first)],
            mesh.positions[bottom_index(first)],
            mesh.positions[bottom_index(next)],
            mesh.positions[top_index(next)],
        ];
        add_boundary_quad(&mut mesh, corners)?;
    }
    mesh.indices.splice(0..0, indices);
    Ok(mesh)
}

#[derive(Debug, Clone)]
struct SubdEdge {
    a: usize,
    b: usize,
    faces: Vec<usize>,
}

/// Build a bounded regular quad Catmull-Clark surface from an editable cage.
///
/// This is deliberately not an arbitrary-topology SubD kernel: the input is a
/// rectangular quad grid, the level count is capped by validation, and the
/// output remains an open triangle mesh.  It is nevertheless a real editable
/// control-cage path: changing one cage point changes the deterministic
/// subdivision result without asking a DCC, executing a script, or hiding a
/// mesh delta behind the Runtime.
fn subd_cage_mesh(
    control_points: &[[f32; 3]],
    u_points: usize,
    v_points: usize,
    subdivision_levels: usize,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let expected_points = u_points.checked_mul(v_points).ok_or_else(|| {
        GeometryError::Invalid("subd-cage control point count overflow".to_owned())
    })?;
    if control_points.len() != expected_points || u_points < 2 || v_points < 2 {
        return Err(GeometryError::Invalid(
            "subd-cage rectangular control grid is invalid".to_owned(),
        ));
    }
    let mut positions = control_points.to_vec();
    let mut faces = Vec::with_capacity((u_points - 1) * (v_points - 1));
    for v_index in 0..v_points - 1 {
        for u_index in 0..u_points - 1 {
            let a = v_index * u_points + u_index;
            let b = a + 1;
            let d = a + u_points;
            let c = d + 1;
            faces.push([a, b, c, d]);
        }
    }

    for _ in 0..subdivision_levels {
        let (next_positions, next_faces) = subd_catmull_clark_step(&positions, &faces)?;
        positions = next_positions;
        faces = next_faces;
    }
    subd_mesh_from_quads(positions, &faces)
}

fn subd_catmull_clark_step(
    positions: &[[f32; 3]],
    faces: &[[usize; 4]],
) -> Result<(Vec<[f32; 3]>, Vec<[usize; 4]>), GeometryError> {
    if positions.is_empty() || faces.is_empty() {
        return Err(GeometryError::Invalid(
            "subd-cage cannot subdivide an empty mesh".to_owned(),
        ));
    }
    let mut edge_lookup: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut edges: Vec<SubdEdge> = Vec::new();
    let mut vertex_edges: Vec<Vec<usize>> = vec![Vec::new(); positions.len()];
    let mut vertex_faces: Vec<Vec<usize>> = vec![Vec::new(); positions.len()];
    let mut face_edges: Vec<[usize; 4]> = Vec::with_capacity(faces.len());

    for (face_index, face) in faces.iter().enumerate() {
        if face.iter().any(|index| *index >= positions.len()) {
            return Err(GeometryError::Invalid(
                "subd-cage face index is outside the control mesh".to_owned(),
            ));
        }
        if face[0] == face[1] || face[1] == face[2] || face[2] == face[3] || face[3] == face[0] {
            return Err(GeometryError::Invalid(
                "subd-cage face contains a repeated edge vertex".to_owned(),
            ));
        }
        for vertex in face {
            vertex_faces[*vertex].push(face_index);
        }
        let pairs = [
            (face[0], face[1]),
            (face[1], face[2]),
            (face[2], face[3]),
            (face[3], face[0]),
        ];
        let mut indices = [0usize; 4];
        for (slot, (left, right)) in pairs.into_iter().enumerate() {
            let key = (left.min(right), left.max(right));
            let edge_index = if let Some(existing) = edge_lookup.get(&key).copied() {
                let edge = edges
                    .get_mut(existing)
                    .expect("edge lookup is synchronized");
                if edge.faces.contains(&face_index) || edge.faces.len() >= 2 {
                    return Err(GeometryError::Invalid(
                        "subd-cage topology is non-manifold".to_owned(),
                    ));
                }
                edge.faces.push(face_index);
                existing
            } else {
                let edge_index = edges.len();
                edge_lookup.insert(key, edge_index);
                edges.push(SubdEdge {
                    a: key.0,
                    b: key.1,
                    faces: vec![face_index],
                });
                edge_index
            };
            indices[slot] = edge_index;
            if !vertex_edges[left].contains(&edge_index) {
                vertex_edges[left].push(edge_index);
            }
            if !vertex_edges[right].contains(&edge_index) {
                vertex_edges[right].push(edge_index);
            }
        }
        face_edges.push(indices);
    }

    let face_points: Vec<[f32; 3]> = faces
        .iter()
        .map(|face| {
            scale3(
                add3(
                    add3(positions[face[0]], positions[face[1]]),
                    add3(positions[face[2]], positions[face[3]]),
                ),
                0.25,
            )
        })
        .collect();
    let edge_points: Vec<[f32; 3]> = edges
        .iter()
        .map(|edge| {
            let midpoint = scale3(add3(positions[edge.a], positions[edge.b]), 0.5);
            if edge.faces.len() == 1 {
                midpoint
            } else {
                scale3(
                    add3(
                        add3(positions[edge.a], positions[edge.b]),
                        add3(face_points[edge.faces[0]], face_points[edge.faces[1]]),
                    ),
                    0.25,
                )
            }
        })
        .collect();

    let mut new_vertex_points = Vec::with_capacity(positions.len());
    for (vertex_index, position) in positions.iter().copied().enumerate() {
        let boundary_edges: Vec<usize> = vertex_edges[vertex_index]
            .iter()
            .copied()
            .filter(|edge_index| edges[*edge_index].faces.len() == 1)
            .collect();
        let next = if !boundary_edges.is_empty() {
            if boundary_edges.len() != 2 {
                return Err(GeometryError::Invalid(
                    "subd-cage boundary valence is not supported".to_owned(),
                ));
            }
            let mut boundary_sum = scale3(position, 6.0);
            for edge_index in boundary_edges {
                let edge = &edges[edge_index];
                let neighbor = if edge.a == vertex_index {
                    edge.b
                } else {
                    edge.a
                };
                boundary_sum = add3(boundary_sum, positions[neighbor]);
            }
            scale3(boundary_sum, 0.125)
        } else {
            let valence = vertex_edges[vertex_index].len();
            if valence == 0 || vertex_faces[vertex_index].len() != valence {
                return Err(GeometryError::Invalid(
                    "subd-cage vertex valence is not regular".to_owned(),
                ));
            }
            let face_sum = vertex_faces[vertex_index]
                .iter()
                .fold([0.0; 3], |sum, face_index| {
                    add3(sum, face_points[*face_index])
                });
            let face_average = scale3(face_sum, 1.0 / valence as f32);
            let edge_midpoint_sum =
                vertex_edges[vertex_index]
                    .iter()
                    .fold([0.0; 3], |sum, edge_index| {
                        let edge = &edges[*edge_index];
                        add3(sum, scale3(add3(positions[edge.a], positions[edge.b]), 0.5))
                    });
            let edge_midpoint_average = scale3(edge_midpoint_sum, 1.0 / valence as f32);
            scale3(
                add3(
                    add3(face_average, scale3(edge_midpoint_average, 2.0)),
                    scale3(position, valence as f32 - 3.0),
                ),
                1.0 / valence as f32,
            )
        };
        if !finite3(next) {
            return Err(GeometryError::Invalid(
                "subd-cage subdivision emitted a non-finite control point".to_owned(),
            ));
        }
        new_vertex_points.push(next);
    }

    let vertex_count = positions.len();
    let edge_offset = vertex_count;
    let face_offset = edge_offset + edge_points.len();
    let mut next_positions = new_vertex_points;
    next_positions.extend(edge_points);
    next_positions.extend(face_points);
    let mut next_faces = Vec::with_capacity(faces.len() * 4);
    for (face_index, face) in faces.iter().enumerate() {
        let [edge_ab, edge_bc, edge_cd, edge_da] = face_edges[face_index];
        let a = face[0];
        let b = face[1];
        let c = face[2];
        let d = face[3];
        let face_point = face_offset + face_index;
        next_faces.extend([
            [a, edge_offset + edge_ab, face_point, edge_offset + edge_da],
            [b, edge_offset + edge_bc, face_point, edge_offset + edge_ab],
            [c, edge_offset + edge_cd, face_point, edge_offset + edge_bc],
            [d, edge_offset + edge_da, face_point, edge_offset + edge_cd],
        ]);
    }
    Ok((next_positions, next_faces))
}

fn subd_mesh_from_quads(
    positions: Vec<[f32; 3]>,
    faces: &[[usize; 4]],
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let mut normals = vec![[0.0; 3]; positions.len()];
    let mut indices = Vec::with_capacity(faces.len() * 6);
    for face in faces {
        let triangles = [[face[0], face[1], face[2]], [face[0], face[2], face[3]]];
        for [a, b, c] in triangles {
            if a >= positions.len() || b >= positions.len() || c >= positions.len() {
                return Err(GeometryError::Invalid(
                    "subd-cage output index is outside the mesh".to_owned(),
                ));
            }
            let cross = cross3(
                subtract3(positions[b], positions[a]),
                subtract3(positions[c], positions[a]),
            );
            if !finite3(cross) || length3(cross) <= 1.0e-8 {
                return Err(GeometryError::Invalid(
                    "subd-cage output contains a degenerate triangle".to_owned(),
                ));
            }
            normals[a] = add3(normals[a], cross);
            normals[b] = add3(normals[b], cross);
            normals[c] = add3(normals[c], cross);
            indices.extend([a as u32, b as u32, c as u32]);
        }
    }
    for normal in &mut normals {
        *normal = normalize(*normal);
        if !finite3(*normal) {
            return Err(GeometryError::Invalid(
                "subd-cage output contains a non-finite normal".to_owned(),
            ));
        }
    }
    Ok(PrimitiveNodeMesh {
        operator_id: String::new(),
        lineage_source_node_ids: Vec::new(),
        positions,
        normals,
        indices,
    })
}

fn cubic_basis(t: f32) -> [f32; 4] {
    let one = 1.0 - t;
    [
        one * one * one,
        3.0 * one * one * t,
        3.0 * one * t * t,
        t * t * t,
    ]
}

fn cubic_basis_derivative(t: f32) -> [f32; 4] {
    let one = 1.0 - t;
    [
        -3.0 * one * one,
        3.0 * one * one - 6.0 * one * t,
        6.0 * one * t - 3.0 * t * t,
        3.0 * t * t,
    ]
}

fn bezier_patch_point(
    control_points: &[[f32; 3]; SURFACE_PATCH_CONTROL_POINTS],
    u: f32,
    v: f32,
    derivative_u: bool,
) -> [f32; 3] {
    let u_basis = if derivative_u {
        cubic_basis_derivative(u)
    } else {
        cubic_basis(u)
    };
    let v_basis = cubic_basis(v);
    let mut result = [0.0; 3];
    for row in 0..4 {
        for column in 0..4 {
            let weight = u_basis[column] * v_basis[row];
            let point = control_points[row * 4 + column];
            result[0] += weight * point[0];
            result[1] += weight * point[1];
            result[2] += weight * point[2];
        }
    }
    result
}

fn bezier_patch_point_v_derivative(
    control_points: &[[f32; 3]; SURFACE_PATCH_CONTROL_POINTS],
    u: f32,
    v: f32,
) -> [f32; 3] {
    let u_basis = cubic_basis(u);
    let v_basis = cubic_basis_derivative(v);
    let mut result = [0.0; 3];
    for row in 0..4 {
        for column in 0..4 {
            let weight = u_basis[column] * v_basis[row];
            let point = control_points[row * 4 + column];
            result[0] += weight * point[0];
            result[1] += weight * point[1];
            result[2] += weight * point[2];
        }
    }
    result
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
        lineage_source_node_ids: Vec::new(),
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
        lineage_source_node_ids: Vec::new(),
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
    for source_node_id in &source.lineage_source_node_ids {
        if !target
            .lineage_source_node_ids
            .iter()
            .any(|existing| existing == source_node_id)
        {
            target.lineage_source_node_ids.push(source_node_id.clone());
        }
    }
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
        let rounded_mesh =
            compile_operator(&rounded, &BTreeMap::new(), 250_000, 10_000).expect("rounded mesh");
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
        let plain_mesh =
            compile_operator(&plain, &BTreeMap::new(), 250_000, 10_000).expect("plain mesh");
        assert_eq!(plain_mesh.indices.len() / 3, 12);
    }

    #[test]
    fn longitudinal_section_loft_builds_x_station_volume() {
        let parameters = json!({
            "shape": "longitudinal-section-loft",
            "sections": [
                {"station_m": -1.0, "points": [[-0.18, -0.12], [0.18, -0.12], [0.18, 0.12], [-0.18, 0.12]]},
                {"station_m": 0.0, "points": [[-0.42, -0.30], [0.42, -0.30], [0.42, 0.30], [-0.42, 0.30]]},
                {"station_m": 1.4, "points": [[-0.12, -0.08], [0.12, -0.08], [0.12, 0.08], [-0.12, 0.08]]}
            ],
            "position_m": [0.0, 1.0, 0.0],
            "rotation_rad": [0.0, 0.0, 0.0]
        });
        let (operation, triangle_count) = validate_operator(
            "forgecad.geometry.longitudinal-section-loft@1",
            &[],
            parameters.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .expect("longitudinal loft should validate");
        assert_eq!(triangle_count, 20);
        let mesh = compile_operator(&operation, &BTreeMap::new(), 250_000, 10_000)
            .expect("longitudinal loft mesh");
        assert_eq!(mesh.indices.len() / 3, 20);
        let min_x = mesh
            .positions
            .iter()
            .map(|position| position[0])
            .fold(f32::INFINITY, f32::min);
        let max_x = mesh
            .positions
            .iter()
            .map(|position| position[0])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!((min_x + 1.0).abs() < 1.0e-6);
        assert!((max_x - 1.4).abs() < 1.0e-6);
        assert!(mesh.positions.iter().all(|position| position[1] > 0.5));
    }

    #[test]
    fn longitudinal_section_loft_rejects_station_and_correspondence_drift() {
        let non_increasing = json!({
            "shape": "longitudinal-section-loft",
            "sections": [
                {"station_m": 0.0, "points": [[-0.2, -0.2], [0.2, -0.2], [0.2, 0.2], [-0.2, 0.2]]},
                {"station_m": 0.0, "points": [[-0.1, -0.1], [0.1, -0.1], [0.1, 0.1], [-0.1, 0.1]]}
            ],
            "position_m": [0.0, 0.0, 0.0],
            "rotation_rad": [0.0, 0.0, 0.0]
        });
        assert!(validate_operator(
            "forgecad.geometry.longitudinal-section-loft@1",
            &[],
            non_increasing.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());

        let mismatched_points = json!({
            "shape": "longitudinal-section-loft",
            "sections": [
                {"station_m": -0.5, "points": [[-0.2, -0.2], [0.2, -0.2], [0.2, 0.2], [-0.2, 0.2]]},
                {"station_m": 0.5, "points": [[-0.1, -0.1], [0.1, -0.1], [0.0, 0.1]]}
            ],
            "position_m": [0.0, 0.0, 0.0],
            "rotation_rad": [0.0, 0.0, 0.0]
        });
        assert!(validate_operator(
            "forgecad.geometry.longitudinal-section-loft@1",
            &[],
            mismatched_points.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());
    }

    fn profile_loft_v2_parameters() -> Value {
        json!({
            "shape": "profile-loft-v2",
            "profiles": [
                {
                    "station_m": -1.0,
                    "points": [[-0.30,-0.18],[0.30,-0.18],[0.42,0.0],[0.20,0.22],[-0.20,0.22],[-0.42,0.0]],
                    "corner_indices": [0,1,2,3,4,5]
                },
                {
                    "station_m": 0.4,
                    "points": [[0.18,-0.12],[0.28,0.0],[0.12,0.18],[-0.12,0.18],[-0.28,0.0],[-0.18,-0.12]],
                    "corner_indices": [0,1,2,3,4,5]
                },
                {
                    "station_m": 1.6,
                    "points": [[-0.14,-0.10],[0.14,-0.10],[0.20,0.0],[0.0,0.17],[-0.20,0.0]],
                    "corner_indices": [0,1,2,3,4]
                }
            ],
            "resample_points": 16,
            "interpolation": "linear",
            "interpolation_rings": 2,
            "preserve_corners": true,
            "position_m": [0.0,0.0,0.0],
            "rotation_rad": [0.0,0.0,0.0]
        })
    }

    #[test]
    fn profile_loft_v2_resamples_aligns_and_builds_longitudinal_x_volume() {
        let parameters = profile_loft_v2_parameters();
        let (operation, triangle_count) = validate_operator(
            "forgecad.geometry.profile-loft@2",
            &[],
            parameters.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .expect("profile loft v2 should validate");
        // Three authored rings plus two interpolated rings in each interval.
        assert_eq!(triangle_count, 220);
        let first = compile_operator(&operation, &BTreeMap::new(), 250_000, 10_000)
            .expect("profile loft v2 mesh");
        let second = compile_operator(&operation, &BTreeMap::new(), 250_000, 10_000)
            .expect("profile loft v2 deterministic mesh");
        assert_eq!(first.positions, second.positions);
        assert_eq!(first.indices, second.indices);
        assert_eq!(first.indices.len() / 3, triangle_count as usize);
        let min_x = first
            .positions
            .iter()
            .map(|position| position[0])
            .fold(f32::INFINITY, f32::min);
        let max_x = first
            .positions
            .iter()
            .map(|position| position[0])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!((min_x + 1.0).abs() < 1.0e-6);
        assert!((max_x - 1.6).abs() < 1.0e-6);
    }

    #[test]
    fn profile_loft_v2_rejects_self_intersection_and_duplicate_closure() {
        let mut bow_tie = profile_loft_v2_parameters();
        bow_tie["profiles"][0]["points"] =
            json!([[-0.2, -0.2], [0.2, 0.2], [-0.2, 0.2], [0.2, -0.2]]);
        bow_tie["profiles"][0]["corner_indices"] = json!([]);
        assert!(validate_operator(
            "forgecad.geometry.profile-loft@2",
            &[],
            bow_tie.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());

        let mut duplicate_closure = profile_loft_v2_parameters();
        duplicate_closure["profiles"][0]["points"] = json!([
            [-0.2, -0.2],
            [0.2, -0.2],
            [0.2, 0.2],
            [-0.2, 0.2],
            [-0.2, -0.2]
        ]);
        duplicate_closure["profiles"][0]["corner_indices"] = json!([]);
        assert!(validate_operator(
            "forgecad.geometry.profile-loft@2",
            &[],
            duplicate_closure.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());
    }

    #[test]
    fn profile_loft_v2_ear_clips_concave_caps() {
        let profile = [
            [-0.4, -0.3],
            [0.4, -0.3],
            [0.4, 0.3],
            [0.0, 0.05],
            [-0.4, 0.3],
        ];
        validate_simple_profile(&profile, "concave fixture").expect("simple concave profile");
        let triangles = triangulate_simple_polygon(&profile).expect("ear-clipped cap");
        assert_eq!(triangles.len(), profile.len() - 2);
        let triangle_area = triangles
            .iter()
            .map(|[a, b, c]| {
                cross2(
                    subtract2(profile[*b], profile[*a]),
                    subtract2(profile[*c], profile[*a]),
                ) * 0.5
            })
            .sum::<f32>();
        assert!((triangle_area - signed_area(&profile)).abs() < 1.0e-5);
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
        let mesh =
            compile_operator(&operation, &BTreeMap::new(), 250_000, 10_000).expect("revolve mesh");
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
