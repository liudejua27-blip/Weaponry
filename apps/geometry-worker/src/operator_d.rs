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
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

const MAX_PROFILE_POINTS: usize = 64;
const MAX_LOFT_PROFILES: usize = 16;
const MAX_LOFT_V2_RESAMPLE_POINTS: usize = 64;
const MAX_LOFT_V2_INTERPOLATION_RINGS: usize = 16;
const MAX_MULTI_LOOP_COMPONENTS: usize = 4;
const MAX_MULTI_LOOP_HOLES: usize = 4;
const MAX_SWEEP_POINTS: usize = 128;
const SURFACE_PATCH_CONTROL_POINTS: usize = 16;
const MAX_SUBD_CONTROL_POINTS: usize = 256;
const MAX_SUBD_CREASE_EDGES: usize = 128;
// `authoring-mesh@1` is also the fixed lowering target for Runtime-owned
// production foundation meshes.  Keep the surface decisively bounded, but
// large enough for a real editable game-asset source instead of only the
// historical small topology probes.  The enclosing GeometryProgram still
// enforces the 250k triangle, 96 MiB transport and 512 MiB Worker ceilings.
const MAX_AUTHORING_ELEMENTS: usize = 65_536;
const MAX_AUTHORING_FACES: usize = 32_768;
const VENT_ARRAY_FRAME_SEAM_GAP_M: f32 = 1.0e-4;
const RECESSED_CHANNEL_MAX_STATIONS: usize = 32;
const RECESSED_CHANNEL_MIN_SEGMENT_M: f32 = 1.0e-5;
const RECESSED_CHANNEL_REVERSE_DOT: f32 = -0.95;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnergyCoreComponent {
    GuardRing,
    MechanicalRing,
    EmitterCore,
    MechanicalBackplate,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthoringVertex {
    element_id: String,
    position_m: [f32; 3],
}

#[derive(Debug, Clone)]
pub(crate) struct AuthoringEdge {
    element_id: String,
    vertex_ids: [String; 2],
}

#[derive(Debug, Clone)]
pub(crate) struct AuthoringLoop {
    element_id: String,
    face_id: String,
    ordinal: usize,
    vertex_id: String,
    edge_id: String,
    edge_forward: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthoringFace {
    element_id: String,
    loop_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SubdCreaseEdge {
    vertex_a: usize,
    vertex_b: usize,
    sharpness_levels: u8,
}

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
struct MultiLoopProfileLoftLoop {
    loop_id: String,
    points: Vec<[f32; 2]>,
    corner_flags: Vec<bool>,
}

#[derive(Debug, Clone)]
struct MultiLoopProfileLoftComponent {
    component_id: String,
    outer: MultiLoopProfileLoftLoop,
    holes: Vec<MultiLoopProfileLoftLoop>,
}

#[derive(Debug, Clone)]
pub struct MultiLoopProfileLoftRing {
    station_m: f32,
    components: Vec<MultiLoopProfileLoftComponent>,
}

#[derive(Debug, Clone)]
struct RawMultiLoopProfileLoftLoop {
    loop_id: String,
    points: Vec<[f32; 2]>,
    corner_indices: Vec<usize>,
}

#[derive(Debug, Clone)]
struct RawMultiLoopProfileLoftComponent {
    component_id: String,
    outer: RawMultiLoopProfileLoftLoop,
    holes: Vec<RawMultiLoopProfileLoftLoop>,
}

#[derive(Debug, Clone)]
struct RawMultiLoopProfileLoftStation {
    station_m: f32,
    components: Vec<RawMultiLoopProfileLoftComponent>,
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
    ProfileLoftV2 {
        rings: Vec<ProfileLoftV2Ring>,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    MultiLoopProfileLoft {
        rings: Vec<MultiLoopProfileLoftRing>,
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
        crease_edges: Vec<SubdCreaseEdge>,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    AuthoringMesh {
        vertices: Vec<AuthoringVertex>,
        edges: Vec<AuthoringEdge>,
        loops: Vec<AuthoringLoop>,
        faces: Vec<AuthoringFace>,
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
    Bevel {
        input: String,
        width_m: f32,
        segments: usize,
        profile: f32,
        clamp_overlap: bool,
    },
    BevelV2 {
        input: String,
        source_edge_id: String,
        width_m: f32,
        segments: usize,
        profile: f32,
        clamp_overlap: bool,
    },
    NormalPolicy {
        input: String,
        crease_angle_rad: f32,
    },
    Panel {
        size_m: [f32; 3],
        thickness_m: f32,
        bevel_m: f32,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    PanelV2 {
        size_m: [f32; 3],
        thickness_m: f32,
        inset_m: f32,
        recess_depth_m: f32,
        border_width_m: f32,
        bevel_m: f32,
        bevel_segments: usize,
        support_loop_count: usize,
        support_loop_width_m: f32,
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
    VentArrayV2 {
        width_m: f32,
        height_m: f32,
        depth_m: f32,
        face_thickness_m: f32,
        backing_depth_m: f32,
        backing_gap_m: f32,
        slot_count: usize,
        slot_width_m: f32,
        slot_spacing_m: f32,
        slot_margin_m: f32,
        slot_edge_bevel_m: f32,
        bevel_segments: usize,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    RecessedChannel {
        stations: Vec<RecessedChannelStation>,
        floor_width_ratio: f32,
        edge_bevel_m: f32,
        start_transition_m: f32,
        end_transition_m: f32,
        transition_segments: usize,
        position_m: [f32; 3],
        rotation_rad: [f32; 3],
    },
    EnergyCore {
        component: EnergyCoreComponent,
        outer_radius_m: f32,
        inner_radius_m: f32,
        depth_m: f32,
        radial_segments: usize,
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
pub(crate) struct RecessedChannelStation {
    point_m: [f32; 3],
    width_m: f32,
    depth_m: f32,
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
            Self::MultiLoopProfileLoft { rings, .. } => {
                let first = rings.first().ok_or_else(|| {
                    GeometryError::Invalid("multi-loop profile-loft has no rings".to_owned())
                })?;
                let ring_count = rings.len() as u64;
                let input_triangles =
                    first.components.iter().try_fold(0u64, |sum, component| {
                        let outer = 2u64
                            .checked_mul(component.outer.points.len().saturating_sub(2) as u64)
                            .and_then(|value| {
                                value.checked_add(
                                    2u64.checked_mul(component.outer.points.len() as u64)?
                                        .checked_mul(ring_count.saturating_sub(1))?,
                                )
                            })
                            .ok_or_else(|| {
                                GeometryError::Invalid(
                                    "multi-loop profile-loft triangle count overflow".to_owned(),
                                )
                            })?;
                        let holes = component.holes.iter().try_fold(0u64, |hole_sum, hole| {
                            let count = 2u64
                                .checked_mul(hole.points.len().saturating_sub(2) as u64)
                                .and_then(|value| {
                                    value.checked_add(
                                        2u64.checked_mul(hole.points.len() as u64)?
                                            .checked_mul(ring_count.saturating_sub(1))?,
                                    )
                                })
                                .ok_or_else(|| {
                                    GeometryError::Invalid(
                                        "multi-loop profile-loft triangle count overflow"
                                            .to_owned(),
                                    )
                                })?;
                            hole_sum.checked_add(count).ok_or_else(|| {
                                GeometryError::Invalid(
                                    "multi-loop profile-loft triangle count overflow".to_owned(),
                                )
                            })
                        })?;
                        sum.checked_add(outer)
                            .and_then(|value| value.checked_add(holes))
                            .ok_or_else(|| {
                                GeometryError::Invalid(
                                    "multi-loop profile-loft triangle count overflow".to_owned(),
                                )
                            })
                    })?;
                input_triangles.checked_mul(8).ok_or_else(|| {
                    GeometryError::Invalid(
                        "multi-loop profile-loft triangle count overflow".to_owned(),
                    )
                })?
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
            Self::AuthoringMesh { faces, .. } => faces.iter().try_fold(0u64, |sum, face| {
                sum.checked_add(face.loop_ids.len().saturating_sub(2) as u64)
                    .ok_or_else(|| {
                        GeometryError::Invalid("authoring-mesh triangle count overflow".to_owned())
                    })
            })?,
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
            Self::Bevel {
                input, segments, ..
            } => {
                if input_counts.get(input).copied() != Some(12) {
                    return Err(GeometryError::Invalid(
                        "bevel@1 requires one direct primitive box input".to_owned(),
                    ));
                }
                let side_quads = (2 * *segments + 1) as u64;
                12u64
                    .checked_mul(side_quads)
                    .and_then(|count| count.checked_mul(side_quads))
                    .ok_or_else(|| {
                        GeometryError::Invalid("bevel triangle count overflow".to_owned())
                    })?
            }
            Self::BevelV2 {
                input, segments, ..
            } => input_counts
                .get(input)
                .ok_or_else(|| GeometryError::Invalid("operator input is unknown".to_owned()))?
                .checked_add(4 * *segments as u64)
                .ok_or_else(|| {
                    GeometryError::Invalid("bevel@2 triangle count overflow".to_owned())
                })?,
            Self::NormalPolicy { input, .. } => {
                let count = *input_counts.get(input).ok_or_else(|| {
                    GeometryError::Invalid("operator input is unknown".to_owned())
                })?;
                if count > 50_000 {
                    return Err(GeometryError::Invalid(
                        "normal-policy input exceeds the 50000 triangle local bound".to_owned(),
                    ));
                }
                count
            }
            Self::Panel { bevel_m, .. } => {
                // A beveled panel uses a fixed four-segment quarter arc at
                // each corner.  The zero-bevel branch remains a plain box.
                if *bevel_m > 1.0e-6 {
                    76
                } else {
                    12
                }
            }
            Self::PanelV2 {
                size_m,
                thickness_m,
                inset_m,
                recess_depth_m,
                border_width_m,
                bevel_m,
                bevel_segments,
                support_loop_count,
                support_loop_width_m,
                ..
            } => u64::try_from(
                recessed_panel_v2_mesh(
                    *size_m,
                    *thickness_m,
                    *inset_m,
                    *recess_depth_m,
                    *border_width_m,
                    *bevel_m,
                    *bevel_segments,
                    *support_loop_count,
                    *support_loop_width_m,
                )?
                .indices
                .len()
                    / 3,
            )
            .map_err(|_| GeometryError::Invalid("panel@2 triangle count overflow".to_owned()))?,
            Self::VentArray { slot_count, .. } => 12 * (*slot_count as u64 + 2),
            Self::VentArrayV2 {
                slot_count,
                bevel_segments,
                ..
            } => {
                // One connected slotted shell is made from two planar layers,
                // four outer walls, and a front chamfer plus through-wall ring
                // for every slot. The backing remains one closed geometric
                // sub-solid in this same PartOutput.
                let per_slot = 16u64
                    .checked_mul(*bevel_segments as u64)
                    .and_then(|count| count.checked_add(36))
                    .ok_or_else(|| {
                        GeometryError::Invalid("vent-array@2 triangle count overflow".to_owned())
                    })?;
                per_slot
                    .checked_mul(*slot_count as u64)
                    .and_then(|count| count.checked_add(40))
                    .ok_or_else(|| {
                        GeometryError::Invalid("vent-array@2 triangle count overflow".to_owned())
                    })?
            }
            Self::RecessedChannel {
                stations,
                edge_bevel_m,
                start_transition_m,
                end_transition_m,
                transition_segments,
                ..
            } => {
                let loop_vertices = recessed_channel_loop_vertex_count(*edge_bevel_m);
                let extra_rings = usize::from(*start_transition_m > RECESSED_CHANNEL_MIN_SEGMENT_M)
                    .saturating_mul(1 + *transition_segments)
                    .saturating_sub(usize::from(
                        *start_transition_m > RECESSED_CHANNEL_MIN_SEGMENT_M,
                    ))
                    + usize::from(*end_transition_m > RECESSED_CHANNEL_MIN_SEGMENT_M)
                        .saturating_mul(*transition_segments);
                let ring_count = stations.len().checked_add(extra_rings).ok_or_else(|| {
                    GeometryError::Invalid("recessed-channel ring count overflow".to_owned())
                })?;
                let side = (ring_count.saturating_sub(1) as u64)
                    .checked_mul(loop_vertices as u64)
                    .and_then(|value| value.checked_mul(2))
                    .ok_or_else(|| {
                        GeometryError::Invalid(
                            "recessed-channel triangle count overflow".to_owned(),
                        )
                    })?;
                side.checked_add(2 * loop_vertices.saturating_sub(2) as u64)
                    .ok_or_else(|| {
                        GeometryError::Invalid(
                            "recessed-channel cap triangle count overflow".to_owned(),
                        )
                    })?
            }
            Self::EnergyCore {
                component,
                radial_segments,
                ..
            } => match component {
                EnergyCoreComponent::GuardRing | EnergyCoreComponent::MechanicalRing => {
                    8 * *radial_segments as u64
                }
                EnergyCoreComponent::EmitterCore | EnergyCoreComponent::MechanicalBackplate => {
                    4 * *radial_segments as u64
                }
            },
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
        "forgecad.geometry.multi-loop-profile-loft@1" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "multi-loop-profile-loft@1 accepts no inputs".to_owned(),
                ));
            }
            require_multi_loop_profile_keys(parameters)?;
            require_shape(parameters, "multi-loop-profile-loft")?;
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
                        "multi-loop-profile-loft@1 interpolation must be linear or catmull-rom"
                            .to_owned(),
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
            let stations_value = parameters
                .get("stations")
                .and_then(Value::as_array)
                .ok_or_else(|| GeometryError::Invalid("stations must be an array".to_owned()))?;
            if !(2..=MAX_LOFT_PROFILES).contains(&stations_value.len()) {
                return Err(GeometryError::Invalid(
                    "multi-loop-profile-loft@1 station count is outside bounds".to_owned(),
                ));
            }
            let raw_stations = parse_multi_loop_stations(stations_value)?;
            let rings = build_multi_loop_profile_loft_rings(
                &raw_stations,
                resample_points,
                interpolation,
                interpolation_rings,
                preserve_corners,
            )?;
            ValidatedOperator::MultiLoopProfileLoft {
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
                crease_edges: Vec::new(),
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            }
        }
        "forgecad.geometry.subd-cage@2" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "subd-cage@2 accepts no inputs".to_owned(),
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
                    "crease_method",
                    "crease_edges",
                    "position_m",
                    "rotation_rad",
                ],
                "subd-cage@2",
            )?;
            require_shape(parameters, "subd-cage")?;
            if parameters.get("crease_method").and_then(Value::as_str)
                != Some("uniform-integer-level-decay@1")
            {
                return Err(GeometryError::Invalid(
                    "subd-cage@2 crease_method is unsupported".to_owned(),
                ));
            }
            let u_points = bounded_count(parameters, "u_points", 3, 16)?;
            let v_points = bounded_count(parameters, "v_points", 3, 16)?;
            let subdivision_levels = bounded_count(parameters, "subdivision_levels", 1, 2)?;
            let control_points =
                parse_vec3_array(parameters, "control_points", 9, MAX_SUBD_CONTROL_POINTS)?;
            let expected_points = u_points.checked_mul(v_points).ok_or_else(|| {
                GeometryError::Invalid("subd-cage@2 control point count overflow".to_owned())
            })?;
            if control_points.len() != expected_points {
                return Err(GeometryError::Invalid(format!(
                    "subd-cage@2 requires exactly {expected_points} control points"
                )));
            }
            let crease_edges =
                parse_subd_crease_edges(parameters, u_points, v_points, subdivision_levels)?;
            ValidatedOperator::SubdCage {
                control_points,
                u_points,
                v_points,
                subdivision_levels,
                crease_edges,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            }
        }
        "forgecad.geometry.authoring-mesh@1" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "authoring-mesh accepts no inputs".to_owned(),
                ));
            }
            let (vertices, edges, loops, faces, position_m, rotation_rad) =
                parse_authoring_mesh(parameters)?;
            ValidatedOperator::AuthoringMesh {
                vertices,
                edges,
                loops,
                faces,
                position_m,
                rotation_rad,
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
        "forgecad.geometry.bevel@1" => {
            require_one_input(inputs, "bevel")?;
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "width_m",
                    "segments",
                    "profile",
                    "edge_scope",
                    "clamp_overlap",
                ],
                "bevel",
            )?;
            require_shape(parameters, "bevel")?;
            if parameters.get("edge_scope").and_then(Value::as_str) != Some("all-source-box-edges")
            {
                return Err(GeometryError::Invalid(
                    "bevel edge_scope must be all-source-box-edges".to_owned(),
                ));
            }
            let profile = number_field(parameters, "profile", 0.75)?;
            if !(0.25..=0.75).contains(&profile) {
                return Err(GeometryError::Invalid(
                    "bevel profile is outside bounds".to_owned(),
                ));
            }
            ValidatedOperator::Bevel {
                input: inputs[0].clone(),
                width_m: v2_scalar(parameters, "width_m", 5.0, true)?,
                segments: bounded_count(parameters, "segments", 1, 4)?,
                profile,
                clamp_overlap: bool_field(parameters, "clamp_overlap")?,
            }
        }
        "forgecad.geometry.bevel@2" => {
            require_one_input(inputs, "bevel@2")?;
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "source_edge_ids",
                    "width_m",
                    "segments",
                    "profile",
                    "clamp_overlap",
                ],
                "bevel@2",
            )?;
            require_shape(parameters, "bevel")?;
            let source_edge_ids = parameters
                .get("source_edge_ids")
                .and_then(Value::as_array)
                .filter(|values| values.len() == 1)
                .ok_or_else(|| {
                    GeometryError::Invalid(
                        "bevel@2 requires exactly one selected source edge".to_owned(),
                    )
                })?;
            let source_edge_id =
                authoring_identifier(source_edge_ids.first(), "bevel@2 source edge")?;
            let profile = number_field(parameters, "profile", 0.75)?;
            if !(0.25..=0.75).contains(&profile) {
                return Err(GeometryError::Invalid(
                    "bevel@2 profile is outside bounds".to_owned(),
                ));
            }
            ValidatedOperator::BevelV2 {
                input: inputs[0].clone(),
                source_edge_id,
                width_m: v2_scalar(parameters, "width_m", 5.0, true)?,
                segments: bounded_count(parameters, "segments", 1, 4)?,
                profile,
                clamp_overlap: bool_field(parameters, "clamp_overlap")?,
            }
        }
        "forgecad.geometry.normal-policy@1" => {
            require_one_input(inputs, "normal-policy")?;
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "weighting",
                    "crease_angle_rad",
                    "keep_sharp",
                    "output_domain",
                ],
                "normal-policy",
            )?;
            require_shape(parameters, "normal-policy")?;
            if parameters.get("weighting").and_then(Value::as_str)
                != Some("face-area-x-corner-angle")
                || parameters.get("output_domain").and_then(Value::as_str) != Some("corner")
                || parameters.get("keep_sharp").and_then(Value::as_bool) != Some(true)
            {
                return Err(GeometryError::Invalid(
                    "normal-policy constants are invalid".to_owned(),
                ));
            }
            let crease_angle_rad =
                number_field(parameters, "crease_angle_rad", std::f32::consts::PI)?;
            if !(0.0..=std::f32::consts::PI).contains(&crease_angle_rad) {
                return Err(GeometryError::Invalid(
                    "normal-policy crease angle is outside bounds".to_owned(),
                ));
            }
            ValidatedOperator::NormalPolicy {
                input: inputs[0].clone(),
                crease_angle_rad,
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
        "forgecad.geometry.panel@2" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "panel@2 accepts no inputs".to_owned(),
                ));
            }
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "size_m",
                    "thickness_m",
                    "inset_m",
                    "recess_depth_m",
                    "border_width_m",
                    "bevel_m",
                    "bevel_segments",
                    "support_loop_count",
                    "support_loop_width_m",
                    "position_m",
                    "rotation_rad",
                ],
                "panel@2",
            )?;
            require_shape(parameters, "panel")?;
            let size_m = v2_vec3(parameters, "size_m", MAX_DIMENSION, true)?;
            let thickness_m = v2_scalar(parameters, "thickness_m", MAX_DIMENSION, true)?;
            let inset_m = v2_scalar(parameters, "inset_m", size_m[0].min(size_m[1]), true)?;
            let recess_depth_m = v2_scalar(parameters, "recess_depth_m", thickness_m, true)?;
            let border_width_m =
                v2_scalar(parameters, "border_width_m", size_m[0].min(size_m[1]), true)?;
            let bevel_m = v2_scalar(parameters, "bevel_m", MAX_DIMENSION / 2.0, true)?;
            let bevel_segments = bounded_count(parameters, "bevel_segments", 1, 4)?;
            let support_loop_count = bounded_count(parameters, "support_loop_count", 1, 3)?;
            let support_loop_width_m = v2_scalar(
                parameters,
                "support_loop_width_m",
                size_m[0].min(size_m[1]),
                true,
            )?;
            let half_min = size_m[0].min(size_m[1]) / 2.0;
            let support_span = support_loop_count as f32 * support_loop_width_m;
            if thickness_m > size_m[2]
                || recess_depth_m >= thickness_m
                || bevel_m * 2.0 >= size_m[2]
                || bevel_m >= half_min
                || inset_m <= bevel_m + support_span
                || border_width_m <= support_span + bevel_m
                || inset_m + border_width_m + bevel_m >= half_min
            {
                return Err(GeometryError::Invalid(
                    "panel@2 inset/recess/border/bevel/support-loop relationship is invalid"
                        .to_owned(),
                ));
            }
            ValidatedOperator::PanelV2 {
                size_m,
                thickness_m,
                inset_m,
                recess_depth_m,
                border_width_m,
                bevel_m,
                bevel_segments,
                support_loop_count,
                support_loop_width_m,
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
        "forgecad.geometry.vent-array@2" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "vent-array@2 accepts no inputs".to_owned(),
                ));
            }
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "width_m",
                    "height_m",
                    "depth_m",
                    "face_thickness_m",
                    "backing_depth_m",
                    "backing_gap_m",
                    "slot_count",
                    "slot_width_m",
                    "slot_spacing_m",
                    "slot_margin_m",
                    "slot_edge_bevel_m",
                    "bevel_segments",
                    "position_m",
                    "rotation_rad",
                ],
                "vent-array@2",
            )?;
            require_shape(parameters, "vent-array")?;
            let width_m = v2_scalar(parameters, "width_m", MAX_DIMENSION, true)?;
            let height_m = v2_scalar(parameters, "height_m", MAX_DIMENSION, true)?;
            let depth_m = v2_scalar(parameters, "depth_m", MAX_DIMENSION, true)?;
            let face_thickness_m = v2_scalar(parameters, "face_thickness_m", depth_m, true)?;
            let backing_depth_m = v2_scalar(parameters, "backing_depth_m", depth_m, true)?;
            let backing_gap_m = v2_scalar(parameters, "backing_gap_m", depth_m, true)?;
            let slot_count = bounded_count(parameters, "slot_count", 1, 32)?;
            let slot_width_m = v2_scalar(parameters, "slot_width_m", width_m, true)?;
            let slot_spacing_m = v2_scalar(parameters, "slot_spacing_m", width_m, true)?;
            let slot_margin_m = v2_scalar(parameters, "slot_margin_m", height_m / 2.0, true)?;
            let slot_edge_bevel_m =
                v2_scalar(parameters, "slot_edge_bevel_m", MAX_DIMENSION / 2.0, true)?;
            let bevel_segments = bounded_count(parameters, "bevel_segments", 1, 4)?;
            let occupied_width = slot_count as f32 * slot_width_m
                + slot_count.saturating_sub(1) as f32 * slot_spacing_m;
            let side_margin = (width_m - occupied_width) / 2.0;
            let slot_height = height_m - 2.0 * slot_margin_m;
            let seam_gap = VENT_ARRAY_FRAME_SEAM_GAP_M;
            let minimum_bar_width = if slot_count > 1 {
                (side_margin - seam_gap).min(slot_spacing_m - seam_gap)
            } else {
                side_margin - seam_gap
            };
            if occupied_width > width_m
                || side_margin <= seam_gap
                || slot_height <= seam_gap
                || minimum_bar_width <= 2.0 * slot_edge_bevel_m
                || slot_height <= 2.0 * slot_edge_bevel_m
                || face_thickness_m <= 2.0 * slot_edge_bevel_m
                || slot_width_m <= seam_gap + 2.0 * slot_edge_bevel_m
                || (depth_m - face_thickness_m - backing_depth_m - backing_gap_m).abs() > 1.0e-5
                || backing_gap_m <= seam_gap
                || slot_spacing_m <= seam_gap
                || slot_margin_m <= seam_gap
                || slot_margin_m <= slot_edge_bevel_m + seam_gap
            {
                return Err(GeometryError::Invalid(
                    "vent-array@2 slot spacing/size/bevel/backing relationship is invalid"
                        .to_owned(),
                ));
            }
            ValidatedOperator::VentArrayV2 {
                width_m,
                height_m,
                depth_m,
                face_thickness_m,
                backing_depth_m,
                backing_gap_m,
                slot_count,
                slot_width_m,
                slot_spacing_m,
                slot_margin_m,
                slot_edge_bevel_m,
                bevel_segments,
                position_m: v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
                rotation_rad: v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
            }
        }
        "forgecad.geometry.recessed-channel@1" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "recessed-channel@1 accepts no inputs".to_owned(),
                ));
            }
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "stations",
                    "path_frame",
                    "floor_width_ratio",
                    "edge_bevel_m",
                    "start_transition_m",
                    "end_transition_m",
                    "transition_segments",
                    "position_m",
                    "rotation_rad",
                ],
                "recessed-channel@1",
            )?;
            require_shape(parameters, "recessed-channel")?;
            if parameters.get("path_frame").and_then(Value::as_str) != Some("planar-xy-z-up@1") {
                return Err(GeometryError::Invalid(
                    "recessed-channel@1 path_frame is unsupported".to_owned(),
                ));
            }
            let station_values = parameters
                .get("stations")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    GeometryError::Invalid("recessed-channel stations must be an array".to_owned())
                })?;
            if !(2..=RECESSED_CHANNEL_MAX_STATIONS).contains(&station_values.len()) {
                return Err(GeometryError::Invalid(
                    "recessed-channel station count is outside bounds".to_owned(),
                ));
            }
            let mut stations = Vec::with_capacity(station_values.len());
            for (index, value) in station_values.iter().enumerate() {
                let station = value.as_object().ok_or_else(|| {
                    GeometryError::Invalid(format!(
                        "recessed-channel station {index} must be an object"
                    ))
                })?;
                require_exact_keys(
                    station,
                    &["point_m", "width_m", "depth_m"],
                    "recessed-channel station",
                )?;
                let point = v2_vec3(station, "point_m", MAX_COORDINATE, false)?;
                if point[2].abs() > RECESSED_CHANNEL_MIN_SEGMENT_M {
                    return Err(GeometryError::Invalid(
                        "recessed-channel station point_m.z must be zero".to_owned(),
                    ));
                }
                let width_m = v2_scalar(station, "width_m", MAX_DIMENSION, true)?;
                let depth_m = v2_scalar(station, "depth_m", MAX_DIMENSION, true)?;
                if depth_m >= 0.75 * width_m {
                    return Err(GeometryError::Invalid(
                        "recessed-channel depth must be below 0.75 * width".to_owned(),
                    ));
                }
                stations.push(RecessedChannelStation {
                    point_m: [point[0], point[1], 0.0],
                    width_m,
                    depth_m,
                });
            }
            validate_recessed_channel_path(&stations)?;
            let floor_width_ratio = v2_scalar(parameters, "floor_width_ratio", 0.9, true)?;
            if !(0.1..=0.8).contains(&floor_width_ratio) {
                return Err(GeometryError::Invalid(
                    "recessed-channel floor_width_ratio is outside bounds".to_owned(),
                ));
            }
            let edge_bevel_m = v2_scalar(parameters, "edge_bevel_m", MAX_DIMENSION / 2.0, false)?;
            let start_transition_m =
                v2_scalar(parameters, "start_transition_m", MAX_DIMENSION / 2.0, false)?;
            let end_transition_m =
                v2_scalar(parameters, "end_transition_m", MAX_DIMENSION / 2.0, false)?;
            let transition_segments = bounded_count(parameters, "transition_segments", 1, 4)?;
            let minimum_side_wall = stations
                .iter()
                .map(|station| station.width_m * (1.0 - floor_width_ratio) / 2.0)
                .fold(f32::INFINITY, f32::min);
            let minimum_floor_width = stations
                .iter()
                .map(|station| station.width_m * floor_width_ratio)
                .fold(f32::INFINITY, f32::min);
            let minimum_depth = stations
                .iter()
                .map(|station| station.depth_m)
                .fold(f32::INFINITY, f32::min);
            let base_thickness = minimum_depth * 0.25;
            if edge_bevel_m * 2.0 >= minimum_side_wall
                || edge_bevel_m * 2.0 >= minimum_floor_width
                || edge_bevel_m * 2.0 >= minimum_depth
                || edge_bevel_m * 2.0 >= base_thickness
            {
                return Err(GeometryError::Invalid(
                    "recessed-channel edge bevel exceeds the thinnest wall/floor/base".to_owned(),
                ));
            }
            let first_segment = segment_length(stations[0].point_m, stations[1].point_m);
            let last = stations.len() - 1;
            let last_segment = segment_length(stations[last - 1].point_m, stations[last].point_m);
            if start_transition_m > first_segment * 0.45 || end_transition_m > last_segment * 0.45 {
                return Err(GeometryError::Invalid(
                    "recessed-channel end transition exceeds the adjacent path segment".to_owned(),
                ));
            }
            ValidatedOperator::RecessedChannel {
                stations,
                floor_width_ratio,
                edge_bevel_m,
                start_transition_m,
                end_transition_m,
                transition_segments,
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
        "forgecad.geometry.energy-core@1" => {
            if !inputs.is_empty() {
                return Err(GeometryError::Invalid(
                    "energy-core accepts no inputs".to_owned(),
                ));
            }
            require_exact_keys(
                parameters,
                &[
                    "shape",
                    "component",
                    "outer_radius_m",
                    "inner_radius_m",
                    "depth_m",
                    "radial_segments",
                    "position_m",
                    "rotation_rad",
                ],
                "energy-core",
            )?;
            require_shape(parameters, "energy-core")?;
            let component = match parameters.get("component").and_then(Value::as_str) {
                Some("guard-ring") => EnergyCoreComponent::GuardRing,
                Some("mechanical-ring") => EnergyCoreComponent::MechanicalRing,
                Some("emitter-core") => EnergyCoreComponent::EmitterCore,
                Some("mechanical-backplate") => EnergyCoreComponent::MechanicalBackplate,
                _ => {
                    return Err(GeometryError::Invalid(
                        "energy-core component is invalid".to_owned(),
                    ))
                }
            };
            let outer_radius_m = v2_scalar(parameters, "outer_radius_m", 5.0, true)?;
            let inner_radius_m = v2_scalar(parameters, "inner_radius_m", 5.0, false)?;
            if inner_radius_m < 0.0 {
                return Err(GeometryError::Invalid(
                    "energy-core inner radius must be non-negative".to_owned(),
                ));
            }
            if inner_radius_m >= outer_radius_m - 1.0e-5 {
                return Err(GeometryError::Invalid(
                    "energy-core inner radius must remain inside the outer radius".to_owned(),
                ));
            }
            match component {
                EnergyCoreComponent::GuardRing | EnergyCoreComponent::MechanicalRing
                    if inner_radius_m <= 1.0e-5 =>
                {
                    return Err(GeometryError::Invalid(
                        "energy-core ring components require a positive inner radius".to_owned(),
                    ));
                }
                EnergyCoreComponent::EmitterCore | EnergyCoreComponent::MechanicalBackplate
                    if inner_radius_m != 0.0 =>
                {
                    return Err(GeometryError::Invalid(
                        "energy-core solid components require inner_radius_m = 0".to_owned(),
                    ));
                }
                _ => {}
            }
            ValidatedOperator::EnergyCore {
                component,
                outer_radius_m,
                inner_radius_m,
                depth_m: v2_scalar(parameters, "depth_m", MAX_DIMENSION, true)?,
                radial_segments: bounded_count(parameters, "radial_segments", 12, 64)?,
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

#[derive(Debug, Clone)]
pub(crate) struct BooleanLineageRaw {
    pub(crate) left_node_id: String,
    pub(crate) right_node_id: String,
    pub(crate) operation: String,
    pub(crate) source_ids: Vec<u32>,
    pub(crate) evaluated_face_ids: Vec<u64>,
}

pub fn compile_operator(
    operation: &ValidatedOperator,
    meshes: &BTreeMap<String, PrimitiveNodeMesh>,
    source_operators: &BTreeMap<String, ValidatedOperator>,
    max_triangles: u64,
    max_runtime_ms: u64,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    compile_operator_internal(
        operation,
        meshes,
        source_operators,
        max_triangles,
        max_runtime_ms,
        None,
    )
}

pub(crate) fn compile_operator_with_boolean_lineage(
    operation: &ValidatedOperator,
    meshes: &BTreeMap<String, PrimitiveNodeMesh>,
    source_operators: &BTreeMap<String, ValidatedOperator>,
    max_triangles: u64,
    max_runtime_ms: u64,
) -> Result<(PrimitiveNodeMesh, Option<BooleanLineageRaw>), GeometryError> {
    let mut lineage = None;
    let mesh = compile_operator_internal(
        operation,
        meshes,
        source_operators,
        max_triangles,
        max_runtime_ms,
        Some(&mut lineage),
    )?;
    Ok((mesh, lineage))
}

fn compile_operator_internal(
    operation: &ValidatedOperator,
    meshes: &BTreeMap<String, PrimitiveNodeMesh>,
    source_operators: &BTreeMap<String, ValidatedOperator>,
    max_triangles: u64,
    max_runtime_ms: u64,
    mut boolean_lineage: Option<&mut Option<BooleanLineageRaw>>,
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
        ValidatedOperator::MultiLoopProfileLoft {
            rings,
            position_m,
            rotation_rad,
        } => transform_mesh(
            multi_loop_profile_loft_mesh(rings, max_triangles, max_runtime_ms)?,
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
            crease_edges,
            position_m,
            rotation_rad,
        } => transform_mesh(
            subd_cage_mesh(
                control_points,
                *u_points,
                *v_points,
                *subdivision_levels,
                crease_edges,
            )?,
            *position_m,
            *rotation_rad,
            [1.0; 3],
        ),
        ValidatedOperator::AuthoringMesh {
            vertices,
            edges,
            loops,
            faces,
            position_m,
            rotation_rad,
        } => transform_mesh(
            authoring_mesh(vertices, edges, loops, faces)?,
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
        ValidatedOperator::Bevel {
            input,
            width_m,
            segments,
            profile,
            clamp_overlap,
        } => {
            let input_mesh = input_mesh(meshes, input)?;
            let source = source_operators.get(input).ok_or_else(|| {
                GeometryError::Invalid("bevel source operator is unavailable".to_owned())
            })?;
            let ValidatedOperator::Primitive(ValidatedV2Primitive::Box {
                size_m,
                position_m,
                rotation_rad,
            }) = source
            else {
                return Err(GeometryError::Invalid(
                    "bevel@1 only supports a direct primitive@2 box source".to_owned(),
                ));
            };
            let mut mesh = rounded_box_mesh(
                *size_m,
                *position_m,
                *rotation_rad,
                *width_m,
                *segments,
                *profile,
                *clamp_overlap,
            )?;
            mesh.lineage_source_node_ids = input_mesh.lineage_source_node_ids.clone();
            mesh
        }
        ValidatedOperator::BevelV2 {
            input,
            source_edge_id,
            width_m,
            segments,
            profile,
            clamp_overlap,
        } => {
            let input_mesh = input_mesh(meshes, input)?;
            let source = source_operators.get(input).ok_or_else(|| {
                GeometryError::Invalid("bevel@2 source operator is unavailable".to_owned())
            })?;
            let ValidatedOperator::AuthoringMesh {
                vertices,
                edges,
                loops,
                faces,
                position_m,
                rotation_rad,
            } = source
            else {
                return Err(GeometryError::Invalid(
                    "bevel@2 only supports one direct authoring-mesh@1 source".to_owned(),
                ));
            };
            let mut mesh = transform_mesh(
                bevel_authoring_edge(
                    vertices,
                    edges,
                    loops,
                    faces,
                    source_edge_id,
                    *width_m,
                    *segments,
                    *profile,
                    *clamp_overlap,
                )?,
                *position_m,
                *rotation_rad,
                [1.0; 3],
            );
            mesh.lineage_source_node_ids = input_mesh.lineage_source_node_ids.clone();
            mesh
        }
        ValidatedOperator::NormalPolicy {
            input,
            crease_angle_rad,
        } => area_angle_corner_normals(input_mesh(meshes, input)?, *crease_angle_rad)?,
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
        ValidatedOperator::PanelV2 {
            size_m,
            thickness_m,
            inset_m,
            recess_depth_m,
            border_width_m,
            bevel_m,
            bevel_segments,
            support_loop_count,
            support_loop_width_m,
            position_m,
            rotation_rad,
        } => transform_mesh(
            recessed_panel_v2_mesh(
                *size_m,
                *thickness_m,
                *inset_m,
                *recess_depth_m,
                *border_width_m,
                *bevel_m,
                *bevel_segments,
                *support_loop_count,
                *support_loop_width_m,
            )?,
            *position_m,
            *rotation_rad,
            [1.0; 3],
        ),
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
        ValidatedOperator::VentArrayV2 {
            width_m,
            height_m,
            depth_m,
            face_thickness_m,
            backing_depth_m,
            backing_gap_m,
            slot_count,
            slot_width_m,
            slot_spacing_m,
            slot_margin_m,
            slot_edge_bevel_m,
            bevel_segments,
            position_m,
            rotation_rad,
        } => {
            let mesh = slotted_face_shell_mesh(
                *width_m,
                *height_m,
                *face_thickness_m,
                *slot_count,
                *slot_width_m,
                *slot_spacing_m,
                *slot_margin_m,
                *slot_edge_bevel_m,
                *bevel_segments,
            )?;
            let mut mesh = transform_mesh(
                mesh,
                [0.0, 0.0, *depth_m / 2.0 - *face_thickness_m / 2.0],
                [0.0; 3],
                [1.0; 3],
            );
            // The backing is a separate closed sub-solid behind the cut-style
            // openings. It is deliberately kept behind the slotted face so
            // the slot voids remain observable in the decoded geometry.
            append_mesh(
                &mut mesh,
                &box_as_mesh(
                    [*width_m, *height_m, *backing_depth_m],
                    [
                        0.0,
                        0.0,
                        *depth_m / 2.0
                            - *face_thickness_m
                            - *backing_gap_m
                            - *backing_depth_m / 2.0,
                    ],
                ),
            );
            transform_mesh(mesh, *position_m, *rotation_rad, [1.0; 3])
        }
        ValidatedOperator::RecessedChannel {
            stations,
            floor_width_ratio,
            edge_bevel_m,
            start_transition_m,
            end_transition_m,
            transition_segments,
            position_m,
            rotation_rad,
            ..
        } => transform_mesh(
            recessed_channel_mesh(
                stations,
                *floor_width_ratio,
                *edge_bevel_m,
                *start_transition_m,
                *end_transition_m,
                *transition_segments,
            )?,
            *position_m,
            *rotation_rad,
            [1.0; 3],
        ),
        ValidatedOperator::EnergyCore {
            component,
            outer_radius_m,
            inner_radius_m,
            depth_m,
            radial_segments,
            position_m,
            rotation_rad,
        } => {
            let source = match component {
                EnergyCoreComponent::GuardRing | EnergyCoreComponent::MechanicalRing => {
                    annular_cylinder_mesh(
                        *outer_radius_m,
                        *inner_radius_m,
                        *depth_m,
                        *radial_segments,
                    )?
                }
                EnergyCoreComponent::EmitterCore | EnergyCoreComponent::MechanicalBackplate => {
                    let (positions, normals, indices) = super::cylinder_mesh(
                        [*outer_radius_m * 2.0, *depth_m, *outer_radius_m * 2.0],
                        *radial_segments,
                    );
                    PrimitiveNodeMesh {
                        operator_id: String::new(),
                        lineage_source_node_ids: Vec::new(),
                        positions,
                        normals,
                        indices,
                    }
                }
            };
            transform_mesh(source, *position_m, *rotation_rad, [1.0; 3])
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
            if let Some(lineage) = boolean_lineage.as_deref_mut() {
                *lineage = Some(BooleanLineageRaw {
                    left_node_id: left.clone(),
                    right_node_id: right.clone(),
                    operation: operation_name.to_owned(),
                    source_ids: result.source_ids.clone(),
                    evaluated_face_ids: result.face_ids.clone(),
                });
            }
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
            | ValidatedOperator::ProfileLoftV2 { .. }
            | ValidatedOperator::MultiLoopProfileLoft { .. }
            | ValidatedOperator::LongitudinalSectionLoft { .. }
            | ValidatedOperator::SurfacePatch { .. }
            | ValidatedOperator::SurfaceShell { .. }
            | ValidatedOperator::SubdCage { .. }
            | ValidatedOperator::Revolve { .. }
            | ValidatedOperator::TubeSweep { .. }
            | ValidatedOperator::EnergyCore { .. }
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

/// Emit one planar rectangle with outward winding for the requested z side.
/// Rectangles are used as a fixed direct-topology tessellation of the slotted
/// face; adjacent patches weld by position in strict readback.
fn emit_slotted_rect(
    mesh: &mut PrimitiveNodeMesh,
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
    z: f32,
    front: bool,
) -> Result<(), GeometryError> {
    if !(x1 > x0 && y1 > y0) {
        return Err(GeometryError::Invalid(
            "vent-array@2 emitted a non-positive face patch".to_owned(),
        ));
    }
    let a = [x0, y0, z];
    let b = [x1, y0, z];
    let c = [x1, y1, z];
    let d = [x0, y1, z];
    if front {
        push_triangle(mesh, a, b, c)?;
        push_triangle(mesh, a, c, d)?;
    } else {
        push_triangle(mesh, a, d, c)?;
        push_triangle(mesh, a, c, b)?;
    }
    Ok(())
}

/// Emit an oriented wall for a boundary loop. `loop_points` is CCW for an
/// outer boundary and clockwise for a slot boundary. The same reverse
/// winding therefore points away from the solid in both cases.
fn emit_slotted_boundary_wall(
    mesh: &mut PrimitiveNodeMesh,
    loop_points: &[[f32; 2]],
    z_front: f32,
    z_back: f32,
    subdivisions: usize,
) -> Result<(), GeometryError> {
    for edge in 0..loop_points.len() {
        let p = loop_points[edge];
        let q = loop_points[(edge + 1) % loop_points.len()];
        for segment in 0..subdivisions {
            let t0 = segment as f32 / subdivisions as f32;
            let t1 = (segment + 1) as f32 / subdivisions as f32;
            let p0 = [p[0] + (q[0] - p[0]) * t0, p[1] + (q[1] - p[1]) * t0];
            let p1 = [p[0] + (q[0] - p[0]) * t1, p[1] + (q[1] - p[1]) * t1];
            let front_p0 = [p0[0], p0[1], z_front];
            let front_p1 = [p1[0], p1[1], z_front];
            let back_p0 = [p0[0], p0[1], z_back];
            let back_p1 = [p1[0], p1[1], z_back];
            push_triangle(mesh, front_p0, back_p1, front_p1)?;
            push_triangle(mesh, front_p0, back_p0, back_p1)?;
        }
    }
    Ok(())
}

/// Emit one four-corner cut-edge chamfer transition. The loops are deliberately
/// not subdivided along their straight edges: each additional bevel segment is
/// a complete depth ring, so every ring continues to match the four-corner
/// planar opening boundary without a T-junction.
fn emit_slotted_chamfer_transition(
    mesh: &mut PrimitiveNodeMesh,
    outer_loop: &[[f32; 2]],
    inner_loop: &[[f32; 2]],
    z_outer: f32,
    z_inner: f32,
) -> Result<(), GeometryError> {
    if outer_loop.len() != inner_loop.len() || outer_loop.len() < 3 {
        return Err(GeometryError::Invalid(
            "vent-array@2 slot chamfer loop mismatch".to_owned(),
        ));
    }
    for edge in 0..outer_loop.len() {
        let outer_p = outer_loop[edge];
        let outer_q = outer_loop[(edge + 1) % outer_loop.len()];
        let inner_p = inner_loop[edge];
        let inner_q = inner_loop[(edge + 1) % inner_loop.len()];
        let outer_p = [outer_p[0], outer_p[1], z_outer];
        let outer_q = [outer_q[0], outer_q[1], z_outer];
        let inner_p = [inner_p[0], inner_p[1], z_inner];
        let inner_q = [inner_q[0], inner_q[1], z_inner];
        push_triangle(mesh, outer_p, inner_q, outer_q)?;
        push_triangle(mesh, outer_p, inner_p, inner_q)?;
    }
    Ok(())
}

/// Add `bevel_segments` symmetric rectangular chamfer depth rings so the
/// parameter changes the actual entrance profile while every ring remains
/// four-cornered and matches the planar cut boundary. This is not a rounded
/// profile; the final nominal slot wall is one straight strip.
fn emit_slotted_chamfer(
    mesh: &mut PrimitiveNodeMesh,
    outer_loop: &[[f32; 2]],
    inner_loop: &[[f32; 2]],
    z_outer: f32,
    z_inner: f32,
    subdivisions: usize,
) -> Result<(), GeometryError> {
    if outer_loop.len() != inner_loop.len() || outer_loop.len() < 3 || subdivisions == 0 {
        return Err(GeometryError::Invalid(
            "vent-array@2 slot chamfer ring parameters are invalid".to_owned(),
        ));
    }
    let mut previous = outer_loop.to_vec();
    for ring in 1..=subdivisions {
        let t = ring as f32 / subdivisions as f32;
        let current = inner_loop
            .iter()
            .zip(outer_loop)
            .map(|(inner, outer)| {
                [
                    outer[0] + (inner[0] - outer[0]) * t,
                    outer[1] + (inner[1] - outer[1]) * t,
                ]
            })
            .collect::<Vec<_>>();
        let current_z = z_outer + (z_inner - z_outer) * t;
        emit_slotted_chamfer_transition(
            mesh,
            &previous,
            &current,
            z_outer + (z_inner - z_outer) * ((ring - 1) as f32 / subdivisions as f32),
            current_z,
        )?;
        previous = current;
    }
    Ok(())
}

/// Emit the front or back layer of a rectangular panel whose middle row has
/// a deterministic array of rectangular holes. This grid-like decomposition
/// keeps one connected shell without introducing internal Boolean faces.
fn emit_slotted_layer(
    mesh: &mut PrimitiveNodeMesh,
    width_m: f32,
    height_m: f32,
    z: f32,
    slot_count: usize,
    slot_width_m: f32,
    slot_spacing_m: f32,
    hole_half_height: f32,
    front: bool,
) -> Result<(), GeometryError> {
    let occupied_width =
        slot_count as f32 * slot_width_m + slot_count.saturating_sub(1) as f32 * slot_spacing_m;
    let first_center = -occupied_width / 2.0 + slot_width_m / 2.0;
    let hole_y0 = -hole_half_height;
    let hole_y1 = hole_half_height;
    let mut x_boundaries = Vec::with_capacity(slot_count * 2 + 2);
    x_boundaries.push(-width_m / 2.0);
    for index in 0..slot_count {
        let center = first_center + index as f32 * (slot_width_m + slot_spacing_m);
        x_boundaries.push(center - slot_width_m / 2.0);
        x_boundaries.push(center + slot_width_m / 2.0);
    }
    x_boundaries.push(width_m / 2.0);
    for pair in x_boundaries.windows(2) {
        emit_slotted_rect(mesh, pair[0], pair[1], hole_y1, height_m / 2.0, z, front)?;
        emit_slotted_rect(mesh, pair[0], pair[1], -height_m / 2.0, hole_y0, z, front)?;
    }
    let first_left = first_center - slot_width_m / 2.0;
    emit_slotted_rect(mesh, -width_m / 2.0, first_left, hole_y0, hole_y1, z, front)?;
    for index in 0..slot_count.saturating_sub(1) {
        let left_center = first_center + index as f32 * (slot_width_m + slot_spacing_m);
        let right_center = left_center + slot_width_m + slot_spacing_m;
        emit_slotted_rect(
            mesh,
            left_center + slot_width_m / 2.0,
            right_center - slot_width_m / 2.0,
            hole_y0,
            hole_y1,
            z,
            front,
        )?;
    }
    let last_center =
        first_center + slot_count.saturating_sub(1) as f32 * (slot_width_m + slot_spacing_m);
    emit_slotted_rect(
        mesh,
        last_center + slot_width_m / 2.0,
        width_m / 2.0,
        hole_y0,
        hole_y1,
        z,
        front,
    )?;
    Ok(())
}

/// Return the detailed outer perimeter that the slotted planar layers use.
/// Every boundary split is carried into the wall strip so the strict welded
/// topology has no long-edge/T-junction mismatch.
fn slotted_outer_perimeter(
    width_m: f32,
    height_m: f32,
    slot_count: usize,
    slot_width_m: f32,
    slot_spacing_m: f32,
    hole_half_height: f32,
) -> Vec<[f32; 2]> {
    let occupied_width =
        slot_count as f32 * slot_width_m + slot_count.saturating_sub(1) as f32 * slot_spacing_m;
    let first_center = -occupied_width / 2.0 + slot_width_m / 2.0;
    let mut x_boundaries = Vec::with_capacity(slot_count * 2 + 2);
    x_boundaries.push(-width_m / 2.0);
    for index in 0..slot_count {
        let center = first_center + index as f32 * (slot_width_m + slot_spacing_m);
        x_boundaries.push(center - slot_width_m / 2.0);
        x_boundaries.push(center + slot_width_m / 2.0);
    }
    x_boundaries.push(width_m / 2.0);

    let mut perimeter = Vec::with_capacity(x_boundaries.len() * 2 + 6);
    perimeter.push([-width_m / 2.0, -height_m / 2.0]);
    for x in x_boundaries.iter().skip(1) {
        perimeter.push([*x, -height_m / 2.0]);
    }
    for y in [-hole_half_height, hole_half_height, height_m / 2.0] {
        perimeter.push([width_m / 2.0, y]);
    }
    for x in x_boundaries.iter().rev().skip(1) {
        perimeter.push([*x, height_m / 2.0]);
    }
    for y in [hole_half_height, -hole_half_height] {
        perimeter.push([-width_m / 2.0, y]);
    }
    perimeter
}

/// Build a single connected, watertight face shell with real through-slots.
/// Both face entrances use the enlarged bevel profile and transition to a
/// nominal slot before the straight core wall; the backing is appended by the
/// caller as one closed geometric sub-solid in the same mesh.
fn slotted_face_shell_mesh(
    width_m: f32,
    height_m: f32,
    face_thickness_m: f32,
    slot_count: usize,
    slot_width_m: f32,
    slot_spacing_m: f32,
    slot_margin_m: f32,
    slot_edge_bevel_m: f32,
    bevel_segments: usize,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let mut mesh = empty_mesh();
    let z_front = face_thickness_m / 2.0;
    let z_back = -face_thickness_m / 2.0;
    let inner_half_height = height_m / 2.0 - slot_margin_m;
    let outer_half_height = inner_half_height + slot_edge_bevel_m;
    let inner_slot_width = slot_width_m;
    let outer_slot_width = slot_width_m + 2.0 * slot_edge_bevel_m;
    let outer_slot_spacing = slot_spacing_m - 2.0 * slot_edge_bevel_m;
    emit_slotted_layer(
        &mut mesh,
        width_m,
        height_m,
        z_front,
        slot_count,
        outer_slot_width,
        outer_slot_spacing,
        outer_half_height,
        true,
    )?;
    emit_slotted_layer(
        &mut mesh,
        width_m,
        height_m,
        z_back,
        slot_count,
        outer_slot_width,
        outer_slot_spacing,
        outer_half_height,
        false,
    )?;

    let outer_perimeter = slotted_outer_perimeter(
        width_m,
        height_m,
        slot_count,
        outer_slot_width,
        outer_slot_spacing,
        outer_half_height,
    );
    emit_slotted_boundary_wall(&mut mesh, &outer_perimeter, z_front, z_back, 1)?;

    let occupied_width =
        slot_count as f32 * slot_width_m + slot_count.saturating_sub(1) as f32 * slot_spacing_m;
    let first_center = -occupied_width / 2.0 + slot_width_m / 2.0;
    for index in 0..slot_count {
        let center = first_center + index as f32 * (slot_width_m + slot_spacing_m);
        let inner_loop = [
            [center + inner_slot_width / 2.0, -inner_half_height],
            [center - inner_slot_width / 2.0, -inner_half_height],
            [center - inner_slot_width / 2.0, inner_half_height],
            [center + inner_slot_width / 2.0, inner_half_height],
        ];
        let outer_loop = [
            [center + outer_slot_width / 2.0, -outer_half_height],
            [center - outer_slot_width / 2.0, -outer_half_height],
            [center - outer_slot_width / 2.0, outer_half_height],
            [center + outer_slot_width / 2.0, outer_half_height],
        ];
        let back_inner_loop = [inner_loop[3], inner_loop[2], inner_loop[1], inner_loop[0]];
        let back_outer_loop = [outer_loop[3], outer_loop[2], outer_loop[1], outer_loop[0]];
        emit_slotted_chamfer(
            &mut mesh,
            &outer_loop,
            &inner_loop,
            z_front,
            z_front - slot_edge_bevel_m,
            bevel_segments,
        )?;
        emit_slotted_boundary_wall(
            &mut mesh,
            &inner_loop,
            z_front - slot_edge_bevel_m,
            z_back + slot_edge_bevel_m,
            1,
        )?;
        emit_slotted_chamfer(
            &mut mesh,
            &back_outer_loop,
            &back_inner_loop,
            z_back,
            z_back + slot_edge_bevel_m,
            bevel_segments,
        )?;
    }
    Ok(mesh)
}

fn segment_length(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    (dx * dx + dy * dy).sqrt()
}

fn recessed_channel_loop_vertex_count(edge_bevel_m: f32) -> usize {
    if edge_bevel_m > RECESSED_CHANNEL_MIN_SEGMENT_M {
        16
    } else {
        8
    }
}

fn recessed_channel_cross2(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn recessed_channel_on_segment(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> bool {
    p[0] >= a[0].min(b[0]) - RECESSED_CHANNEL_MIN_SEGMENT_M
        && p[0] <= a[0].max(b[0]) + RECESSED_CHANNEL_MIN_SEGMENT_M
        && p[1] >= a[1].min(b[1]) - RECESSED_CHANNEL_MIN_SEGMENT_M
        && p[1] <= a[1].max(b[1]) + RECESSED_CHANNEL_MIN_SEGMENT_M
}

fn recessed_channel_segments_intersect(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2]) -> bool {
    let ab_c = recessed_channel_cross2(a, b, c);
    let ab_d = recessed_channel_cross2(a, b, d);
    let cd_a = recessed_channel_cross2(c, d, a);
    let cd_b = recessed_channel_cross2(c, d, b);
    let eps = RECESSED_CHANNEL_MIN_SEGMENT_M;
    if ab_c.abs() <= eps && recessed_channel_on_segment(a, b, c)
        || ab_d.abs() <= eps && recessed_channel_on_segment(a, b, d)
        || cd_a.abs() <= eps && recessed_channel_on_segment(c, d, a)
        || cd_b.abs() <= eps && recessed_channel_on_segment(c, d, b)
    {
        return true;
    }
    ((ab_c > eps && ab_d < -eps) || (ab_c < -eps && ab_d > eps))
        && ((cd_a > eps && cd_b < -eps) || (cd_a < -eps && cd_b > eps))
}

fn recessed_channel_point_segment_distance_squared(
    point: [f32; 2],
    start: [f32; 2],
    end: [f32; 2],
) -> f32 {
    let direction = [end[0] - start[0], end[1] - start[1]];
    let length_squared = direction[0] * direction[0] + direction[1] * direction[1];
    if length_squared <= RECESSED_CHANNEL_MIN_SEGMENT_M * RECESSED_CHANNEL_MIN_SEGMENT_M {
        let delta = [point[0] - start[0], point[1] - start[1]];
        return delta[0] * delta[0] + delta[1] * delta[1];
    }
    let offset = [point[0] - start[0], point[1] - start[1]];
    let t =
        ((offset[0] * direction[0] + offset[1] * direction[1]) / length_squared).clamp(0.0, 1.0);
    let closest = [start[0] + direction[0] * t, start[1] + direction[1] * t];
    let delta = [point[0] - closest[0], point[1] - closest[1]];
    delta[0] * delta[0] + delta[1] * delta[1]
}

fn recessed_channel_segment_distance(
    first_start: [f32; 2],
    first_end: [f32; 2],
    second_start: [f32; 2],
    second_end: [f32; 2],
) -> f32 {
    if recessed_channel_segments_intersect(first_start, first_end, second_start, second_end) {
        return 0.0;
    }
    recessed_channel_point_segment_distance_squared(first_start, second_start, second_end)
        .min(recessed_channel_point_segment_distance_squared(
            first_end,
            second_start,
            second_end,
        ))
        .min(recessed_channel_point_segment_distance_squared(
            second_start,
            first_start,
            first_end,
        ))
        .min(recessed_channel_point_segment_distance_squared(
            second_end,
            first_start,
            first_end,
        ))
        .sqrt()
}

fn validate_recessed_channel_swept_envelope(
    stations: &[RecessedChannelStation],
) -> Result<(), GeometryError> {
    let segment_count = stations.len() - 1;
    for first in 0..segment_count {
        let first_start = stations[first].point_m;
        let first_end = stations[first + 1].point_m;
        let first_half_width = stations[first].width_m.max(stations[first + 1].width_m) * 0.5;
        for second in (first + 2)..segment_count {
            let second_start = stations[second].point_m;
            let second_end = stations[second + 1].point_m;
            let second_half_width =
                stations[second].width_m.max(stations[second + 1].width_m) * 0.5;
            let minimum_distance = recessed_channel_segment_distance(
                [first_start[0], first_start[1]],
                [first_end[0], first_end[1]],
                [second_start[0], second_start[1]],
                [second_end[0], second_end[1]],
            );
            if minimum_distance
                <= first_half_width + second_half_width + RECESSED_CHANNEL_MIN_SEGMENT_M
            {
                return Err(GeometryError::Invalid(
                    "recessed-channel swept envelope overlaps between non-adjacent segments"
                        .to_owned(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_recessed_channel_path(
    stations: &[RecessedChannelStation],
) -> Result<(), GeometryError> {
    for pair in stations.windows(2) {
        if segment_length(pair[0].point_m, pair[1].point_m) <= RECESSED_CHANNEL_MIN_SEGMENT_M {
            return Err(GeometryError::Invalid(
                "recessed-channel path contains a zero-length segment".to_owned(),
            ));
        }
    }
    for window in stations.windows(3) {
        let incoming = [
            window[1].point_m[0] - window[0].point_m[0],
            window[1].point_m[1] - window[0].point_m[1],
        ];
        let outgoing = [
            window[2].point_m[0] - window[1].point_m[0],
            window[2].point_m[1] - window[1].point_m[1],
        ];
        let incoming_len = (incoming[0] * incoming[0] + incoming[1] * incoming[1]).sqrt();
        let outgoing_len = (outgoing[0] * outgoing[0] + outgoing[1] * outgoing[1]).sqrt();
        let dot =
            (incoming[0] * outgoing[0] + incoming[1] * outgoing[1]) / (incoming_len * outgoing_len);
        if dot <= RECESSED_CHANNEL_REVERSE_DOT {
            return Err(GeometryError::Invalid(
                "recessed-channel path contains a near-reverse turn".to_owned(),
            ));
        }
    }
    for first in 0..stations.len() - 1 {
        for second in first + 1..stations.len() - 1 {
            if second <= first + 1 {
                continue;
            }
            let a = stations[first].point_m;
            let b = stations[first + 1].point_m;
            let c = stations[second].point_m;
            let d = stations[second + 1].point_m;
            if recessed_channel_segments_intersect(
                [a[0], a[1]],
                [b[0], b[1]],
                [c[0], c[1]],
                [d[0], d[1]],
            ) {
                return Err(GeometryError::Invalid(
                    "recessed-channel path self-intersects".to_owned(),
                ));
            }
        }
    }
    validate_recessed_channel_swept_envelope(stations)?;
    Ok(())
}

fn recessed_channel_signed_area(points: &[[f32; 2]]) -> f32 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum::<f32>()
        * 0.5
}

fn recessed_channel_cross_section(
    width_m: f32,
    depth_m: f32,
    floor_width_ratio: f32,
    edge_bevel_m: f32,
) -> Vec<[f32; 2]> {
    let half_width = width_m / 2.0;
    let inner_half_width = width_m * floor_width_ratio / 2.0;
    let base_thickness = depth_m * 0.25;
    let bottom_z = -depth_m - base_thickness;
    let points = if edge_bevel_m > RECESSED_CHANNEL_MIN_SEGMENT_M {
        let b = edge_bevel_m;
        vec![
            [-half_width + b, bottom_z],
            [half_width - b, bottom_z],
            [half_width, bottom_z + b],
            [half_width, -b],
            [half_width - b, 0.0],
            [inner_half_width + b, 0.0],
            [inner_half_width, -b],
            [inner_half_width, -depth_m + b],
            [inner_half_width - b, -depth_m],
            [-inner_half_width + b, -depth_m],
            [-inner_half_width, -depth_m + b],
            [-inner_half_width, -b],
            [-inner_half_width - b, 0.0],
            [-half_width + b, 0.0],
            [-half_width, -b],
            [-half_width, bottom_z + b],
        ]
    } else {
        vec![
            [-half_width, bottom_z],
            [half_width, bottom_z],
            [half_width, 0.0],
            [inner_half_width, 0.0],
            [inner_half_width, -depth_m],
            [-inner_half_width, -depth_m],
            [-inner_half_width, 0.0],
            [-half_width, 0.0],
        ]
    };
    if recessed_channel_signed_area(&points) >= 0.0 {
        points
    } else {
        points.into_iter().rev().collect()
    }
}

fn recessed_channel_point_in_triangle(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let ab = recessed_channel_cross2(a, b, p);
    let bc = recessed_channel_cross2(b, c, p);
    let ca = recessed_channel_cross2(c, a, p);
    let eps = RECESSED_CHANNEL_MIN_SEGMENT_M;
    ab >= -eps && bc >= -eps && ca >= -eps
}

fn recessed_channel_cap_triangles(points: &[[f32; 2]]) -> Result<Vec<[usize; 3]>, GeometryError> {
    if points.len() < 3 || recessed_channel_signed_area(points) <= RECESSED_CHANNEL_MIN_SEGMENT_M {
        return Err(GeometryError::Invalid(
            "recessed-channel cross-section is not a valid CCW polygon".to_owned(),
        ));
    }
    let mut remaining: Vec<usize> = (0..points.len()).collect();
    let mut triangles = Vec::with_capacity(points.len() - 2);
    let mut guard = 0usize;
    while remaining.len() > 3 {
        let mut ear_found = false;
        for cursor in 0..remaining.len() {
            let previous = remaining[(cursor + remaining.len() - 1) % remaining.len()];
            let current = remaining[cursor];
            let next = remaining[(cursor + 1) % remaining.len()];
            if recessed_channel_cross2(points[previous], points[current], points[next])
                <= RECESSED_CHANNEL_MIN_SEGMENT_M
            {
                continue;
            }
            if remaining.iter().any(|candidate| {
                *candidate != previous
                    && *candidate != current
                    && *candidate != next
                    && recessed_channel_point_in_triangle(
                        points[*candidate],
                        points[previous],
                        points[current],
                        points[next],
                    )
            }) {
                continue;
            }
            triangles.push([previous, current, next]);
            remaining.remove(cursor);
            ear_found = true;
            break;
        }
        guard += 1;
        if !ear_found || guard > points.len() * points.len() {
            return Err(GeometryError::Invalid(
                "recessed-channel cross-section triangulation failed".to_owned(),
            ));
        }
    }
    triangles.push([remaining[0], remaining[1], remaining[2]]);
    Ok(triangles)
}

fn recessed_channel_tangent(stations: &[RecessedChannelStation], index: usize) -> [f32; 2] {
    let raw = if index == 0 {
        [
            stations[1].point_m[0] - stations[0].point_m[0],
            stations[1].point_m[1] - stations[0].point_m[1],
        ]
    } else if index + 1 == stations.len() {
        [
            stations[index].point_m[0] - stations[index - 1].point_m[0],
            stations[index].point_m[1] - stations[index - 1].point_m[1],
        ]
    } else {
        let previous = [
            stations[index].point_m[0] - stations[index - 1].point_m[0],
            stations[index].point_m[1] - stations[index - 1].point_m[1],
        ];
        let next = [
            stations[index + 1].point_m[0] - stations[index].point_m[0],
            stations[index + 1].point_m[1] - stations[index].point_m[1],
        ];
        let previous_len = (previous[0] * previous[0] + previous[1] * previous[1]).sqrt();
        let next_len = (next[0] * next[0] + next[1] * next[1]).sqrt();
        [
            previous[0] / previous_len + next[0] / next_len,
            previous[1] / previous_len + next[1] / next_len,
        ]
    };
    let length = (raw[0] * raw[0] + raw[1] * raw[1]).sqrt();
    [raw[0] / length, raw[1] / length]
}

fn recessed_channel_ring(
    center: [f32; 3],
    tangent: [f32; 2],
    width_m: f32,
    depth_m: f32,
    floor_width_ratio: f32,
    edge_bevel_m: f32,
    scale: f32,
) -> Vec<[f32; 3]> {
    let normal = [-tangent[1], tangent[0]];
    recessed_channel_cross_section(
        width_m * scale,
        depth_m * scale,
        floor_width_ratio,
        edge_bevel_m * scale,
    )
    .into_iter()
    .map(|point| {
        [
            center[0] + normal[0] * point[0],
            center[1] + normal[1] * point[0],
            point[1],
        ]
    })
    .collect()
}

fn recessed_channel_mesh(
    stations: &[RecessedChannelStation],
    floor_width_ratio: f32,
    edge_bevel_m: f32,
    start_transition_m: f32,
    end_transition_m: f32,
    transition_segments: usize,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let mut rings = Vec::<Vec<[f32; 3]>>::new();
    let start_tangent = recessed_channel_tangent(stations, 0);
    if start_transition_m > RECESSED_CHANNEL_MIN_SEGMENT_M {
        let endpoint = [
            stations[0].point_m[0] - start_tangent[0] * start_transition_m,
            stations[0].point_m[1] - start_tangent[1] * start_transition_m,
            0.0,
        ];
        rings.push(recessed_channel_ring(
            endpoint,
            start_tangent,
            stations[0].width_m,
            stations[0].depth_m,
            floor_width_ratio,
            edge_bevel_m,
            0.6,
        ));
        for step in 1..=transition_segments {
            let t = step as f32 / transition_segments as f32;
            let center = [
                stations[0].point_m[0] - start_tangent[0] * start_transition_m * (1.0 - t),
                stations[0].point_m[1] - start_tangent[1] * start_transition_m * (1.0 - t),
                0.0,
            ];
            rings.push(recessed_channel_ring(
                center,
                start_tangent,
                stations[0].width_m,
                stations[0].depth_m,
                floor_width_ratio,
                edge_bevel_m,
                0.6 + 0.4 * t,
            ));
        }
        for index in 1..stations.len() {
            rings.push(recessed_channel_ring(
                stations[index].point_m,
                recessed_channel_tangent(stations, index),
                stations[index].width_m,
                stations[index].depth_m,
                floor_width_ratio,
                edge_bevel_m,
                1.0,
            ));
        }
    } else {
        for index in 0..stations.len() {
            rings.push(recessed_channel_ring(
                stations[index].point_m,
                recessed_channel_tangent(stations, index),
                stations[index].width_m,
                stations[index].depth_m,
                floor_width_ratio,
                edge_bevel_m,
                1.0,
            ));
        }
    }
    if end_transition_m > RECESSED_CHANNEL_MIN_SEGMENT_M {
        let last = stations.len() - 1;
        let tangent = recessed_channel_tangent(stations, last);
        for step in 1..=transition_segments {
            let t = step as f32 / transition_segments as f32;
            let center = [
                stations[last].point_m[0] + tangent[0] * end_transition_m * t,
                stations[last].point_m[1] + tangent[1] * end_transition_m * t,
                0.0,
            ];
            rings.push(recessed_channel_ring(
                center,
                tangent,
                stations[last].width_m,
                stations[last].depth_m,
                floor_width_ratio,
                edge_bevel_m,
                1.0 - 0.4 * t,
            ));
        }
    }

    let cross_section = recessed_channel_cross_section(
        stations[0].width_m,
        stations[0].depth_m,
        floor_width_ratio,
        edge_bevel_m,
    );
    let cap_triangles = recessed_channel_cap_triangles(&cross_section)?;
    let loop_vertices = cross_section.len();
    if rings.iter().any(|ring| ring.len() != loop_vertices) {
        return Err(GeometryError::Invalid(
            "recessed-channel rings have inconsistent topology".to_owned(),
        ));
    }
    let mut mesh = empty_mesh();
    for pair in rings.windows(2) {
        for index in 0..loop_vertices {
            let next = (index + 1) % loop_vertices;
            push_triangle(&mut mesh, pair[0][index], pair[0][next], pair[1][next])?;
            push_triangle(&mut mesh, pair[0][index], pair[1][next], pair[1][index])?;
        }
    }
    for triangle in &cap_triangles {
        push_triangle(
            &mut mesh,
            rings[0][triangle[0]],
            rings[0][triangle[2]],
            rings[0][triangle[1]],
        )?;
        let last_ring = rings.last().expect("recessed-channel rings");
        push_triangle(
            &mut mesh,
            last_ring[triangle[0]],
            last_ring[triangle[1]],
            last_ring[triangle[2]],
        )?;
    }
    Ok(mesh)
}

/// Generate a deterministic rounded box from source-level primitive data.
/// The scope is intentionally narrower than Blender's arbitrary BMesh bevel:
/// all twelve edges of one direct primitive box are rounded together.
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

fn require_multi_loop_profile_keys(parameters: &Map<String, Value>) -> Result<(), GeometryError> {
    require_exact_keys(
        parameters,
        &[
            "shape",
            "stations",
            "resample_points",
            "interpolation",
            "interpolation_rings",
            "preserve_corners",
            "position_m",
            "rotation_rad",
        ],
        "multi-loop-profile-loft@1",
    )
}

fn require_multi_loop_keys(
    object: &Map<String, Value>,
    required: &[&str],
    optional: &[&str],
    label: &str,
) -> Result<(), GeometryError> {
    if object
        .keys()
        .any(|key| !required.contains(&key.as_str()) && !optional.contains(&key.as_str()))
        || required.iter().any(|key| !object.contains_key(*key))
    {
        return Err(GeometryError::Invalid(format!(
            "{label} must use exactly the closed parameter set"
        )));
    }
    Ok(())
}

fn stable_multi_loop_id(
    object: &Map<String, Value>,
    keys: &[&str],
    label: &str,
) -> Result<Option<String>, GeometryError> {
    let present = keys
        .iter()
        .filter(|key| object.contains_key(**key))
        .copied()
        .collect::<Vec<_>>();
    if present.len() > 1 {
        return Err(GeometryError::Invalid(format!(
            "{label} contains more than one stable id field"
        )));
    }
    let Some(key) = present.first().copied() else {
        return Ok(None);
    };
    let value = object.get(key).and_then(Value::as_str).ok_or_else(|| {
        GeometryError::Invalid(format!("{label}.{key} must be a stable identifier"))
    })?;
    if value.is_empty()
        || value.len() > 128
        || !value
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(GeometryError::Invalid(format!(
            "{label}.{key} is not a stable identifier"
        )));
    }
    Ok(Some(value.to_owned()))
}

fn parse_multi_loop_stations(
    values: &[Value],
) -> Result<Vec<RawMultiLoopProfileLoftStation>, GeometryError> {
    let mut stations = Vec::with_capacity(values.len());
    let mut station_ids = BTreeSet::new();
    let mut previous_station = f32::NEG_INFINITY;
    for station_value in values {
        let station = station_value.as_object().ok_or_else(|| {
            GeometryError::Invalid("multi-loop-profile-loft@1 station must be an object".to_owned())
        })?;
        require_multi_loop_keys(
            station,
            &["station_id", "station_m", "components"],
            &[],
            "multi-loop-profile-loft@1 station",
        )?;
        let station_m = number_field(station, "station_m", MAX_COORDINATE)?;
        if station_m <= previous_station {
            return Err(GeometryError::Invalid(
                "multi-loop-profile-loft@1 stations must be strictly increasing along +X"
                    .to_owned(),
            ));
        }
        previous_station = station_m;
        let station_id = stable_multi_loop_id(station, &["station_id"], "station")?
            .ok_or_else(|| GeometryError::Invalid("station.station_id is required".to_owned()))?;
        if !station_ids.insert(station_id) {
            return Err(GeometryError::Invalid(
                "multi-loop-profile-loft@1 station ids must be unique".to_owned(),
            ));
        }
        let components_value = station
            .get("components")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                GeometryError::Invalid(
                    "multi-loop-profile-loft@1 station components must be an array".to_owned(),
                )
            })?;
        if !(1..=MAX_MULTI_LOOP_COMPONENTS).contains(&components_value.len()) {
            return Err(GeometryError::Invalid(
                "multi-loop-profile-loft@1 component count is outside bounds".to_owned(),
            ));
        }
        let mut components = Vec::with_capacity(components_value.len());
        let mut component_ids = BTreeSet::new();
        let mut station_hole_ids = BTreeSet::new();
        for component_value in components_value {
            let component = component_value.as_object().ok_or_else(|| {
                GeometryError::Invalid(
                    "multi-loop-profile-loft@1 component must be an object".to_owned(),
                )
            })?;
            require_multi_loop_keys(
                component,
                &["component_id", "outer", "holes"],
                &[],
                "multi-loop-profile-loft@1 component",
            )?;
            let component_id = stable_multi_loop_id(component, &["component_id"], "component")?
                .ok_or_else(|| {
                    GeometryError::Invalid("component.component_id is required".to_owned())
                })?;
            if !component_ids.insert(component_id.clone()) {
                return Err(GeometryError::Invalid(
                    "multi-loop-profile-loft@1 component ids must be unique per station".to_owned(),
                ));
            }
            let outer_value = component.get("outer").expect("required outer key");
            let outer = parse_multi_loop_loop(
                outer_value,
                Some(format!("{component_id}.outer")),
                false,
                "outer",
            )?;
            let holes_value = component
                .get("holes")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    GeometryError::Invalid(
                        "multi-loop-profile-loft@1 component holes must be an array".to_owned(),
                    )
                })?;
            if holes_value.len() > MAX_MULTI_LOOP_HOLES {
                return Err(GeometryError::Invalid(
                    "multi-loop-profile-loft@1 hole count is outside bounds".to_owned(),
                ));
            }
            let mut holes = Vec::with_capacity(holes_value.len());
            let mut hole_ids = BTreeSet::new();
            for hole_value in holes_value {
                let hole = parse_multi_loop_loop(hole_value, None, true, "hole")?;
                if !station_hole_ids.insert(hole.loop_id.clone()) {
                    return Err(GeometryError::Invalid(
                        "multi-loop-profile-loft@1 hole ids must be globally unique per station"
                            .to_owned(),
                    ));
                }
                if !hole_ids.insert(hole.loop_id.clone()) {
                    return Err(GeometryError::Invalid(
                        "multi-loop-profile-loft@1 hole ids must be unique per component"
                            .to_owned(),
                    ));
                }
                holes.push(hole);
            }
            components.push(RawMultiLoopProfileLoftComponent {
                component_id,
                outer,
                holes,
            });
        }
        stations.push(RawMultiLoopProfileLoftStation {
            station_m,
            components,
        });
    }
    Ok(stations)
}

fn parse_multi_loop_loop(
    value: &Value,
    default_id: Option<String>,
    require_id: bool,
    label: &str,
) -> Result<RawMultiLoopProfileLoftLoop, GeometryError> {
    let object = value
        .as_object()
        .ok_or_else(|| GeometryError::Invalid(format!("{label} must be an object")))?;
    if require_id {
        require_multi_loop_keys(object, &["hole_id", "points"], &["corner_indices"], label)?;
    } else {
        require_multi_loop_keys(object, &["points"], &["corner_indices"], label)?;
    }
    let loop_id = if require_id {
        stable_multi_loop_id(object, &["hole_id"], label)?
            .ok_or_else(|| GeometryError::Invalid(format!("{label}.hole_id is required")))?
    } else {
        default_id
            .ok_or_else(|| GeometryError::Invalid(format!("{label} requires an internal id")))?
    };
    let points_value = object
        .get("points")
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid(format!("{label}.points must be an array")))?;
    let corner_indices = if object.contains_key("corner_indices") {
        parse_corner_indices(object, "corner_indices", points_value.len())?
    } else {
        Vec::new()
    };
    let points = parse_points_from_array(
        points_value,
        &format!("{label}.points"),
        3,
        MAX_PROFILE_POINTS,
    )?;
    if points
        .iter()
        .any(|point| !point[0].is_finite() || !point[1].is_finite())
    {
        return Err(GeometryError::Invalid(format!(
            "{label} contains a non-finite point"
        )));
    }
    Ok(RawMultiLoopProfileLoftLoop {
        loop_id,
        points,
        corner_indices,
    })
}

fn build_multi_loop_profile_loft_rings(
    raw_stations: &[RawMultiLoopProfileLoftStation],
    resample_points: usize,
    interpolation: ProfileLoftV2Interpolation,
    interpolation_rings: usize,
    preserve_corners: bool,
) -> Result<Vec<MultiLoopProfileLoftRing>, GeometryError> {
    if !(2..=MAX_LOFT_PROFILES).contains(&raw_stations.len()) {
        return Err(GeometryError::Invalid(
            "multi-loop-profile-loft@1 station count is outside bounds".to_owned(),
        ));
    }
    let mut authored = Vec::with_capacity(raw_stations.len());
    let mut expected_components: Option<BTreeSet<String>> = None;
    let mut expected_holes = BTreeMap::<String, BTreeSet<String>>::new();

    for station in raw_stations {
        let mut components_by_id = BTreeMap::<String, &RawMultiLoopProfileLoftComponent>::new();
        for component in &station.components {
            if components_by_id
                .insert(component.component_id.clone(), component)
                .is_some()
            {
                return Err(GeometryError::Invalid(
                    "multi-loop-profile-loft@1 component ids drift or repeat".to_owned(),
                ));
            }
        }
        let component_ids = components_by_id.keys().cloned().collect::<BTreeSet<_>>();
        if let Some(expected) = &expected_components {
            if expected != &component_ids {
                return Err(GeometryError::Invalid(
                    "multi-loop-profile-loft@1 component topology drifted between stations"
                        .to_owned(),
                ));
            }
        } else {
            expected_components = Some(component_ids);
        }

        let mut components = Vec::with_capacity(components_by_id.len());
        for (component_id, raw_component) in components_by_id {
            validate_multi_loop_raw_component(raw_component)?;
            let hole_ids = raw_component
                .holes
                .iter()
                .map(|hole| hole.loop_id.clone())
                .collect::<BTreeSet<_>>();
            if let Some(expected) = expected_holes.get(&component_id) {
                if expected != &hole_ids {
                    return Err(GeometryError::Invalid(
                        "multi-loop-profile-loft@1 hole topology drifted between stations"
                            .to_owned(),
                    ));
                }
            } else {
                expected_holes.insert(component_id.clone(), hole_ids);
            }

            let outer = resample_multi_loop_profile_loop(
                &raw_component.outer,
                resample_points,
                preserve_corners,
                true,
            )?;
            let mut holes = raw_component
                .holes
                .iter()
                .map(|hole| {
                    resample_multi_loop_profile_loop(hole, resample_points, preserve_corners, false)
                })
                .collect::<Result<Vec<_>, _>>()?;
            holes.sort_by(|left, right| left.loop_id.cmp(&right.loop_id));
            components.push(MultiLoopProfileLoftComponent {
                component_id,
                outer,
                holes,
            });
        }
        components.sort_by(|left, right| left.component_id.cmp(&right.component_id));
        let ring = MultiLoopProfileLoftRing {
            station_m: station.station_m,
            components,
        };
        validate_multi_loop_ring_topology(&ring.components, "authored station")?;
        authored.push(ring);
    }

    // Stable IDs, rather than input array position, define correspondence and
    // phase alignment.  This makes reordered component/hole arrays harmless
    // while still rejecting additions/removals as topology drift above.
    let first = authored[0].clone();
    for ring in authored.iter_mut().skip(1) {
        for component in &mut ring.components {
            let reference = first
                .components
                .iter()
                .find(|candidate| candidate.component_id == component.component_id)
                .ok_or_else(|| {
                    GeometryError::Invalid(
                        "multi-loop-profile-loft@1 component correspondence is invalid".to_owned(),
                    )
                })?;
            align_multi_loop_phase(
                &reference.outer.points,
                &mut component.outer.points,
                &mut component.outer.corner_flags,
            );
            for hole in &mut component.holes {
                let reference_hole = reference
                    .holes
                    .iter()
                    .find(|candidate| candidate.loop_id == hole.loop_id)
                    .ok_or_else(|| {
                        GeometryError::Invalid(
                            "multi-loop-profile-loft@1 hole correspondence is invalid".to_owned(),
                        )
                    })?;
                align_multi_loop_phase(
                    &reference_hole.points,
                    &mut hole.points,
                    &mut hole.corner_flags,
                );
            }
        }
        validate_multi_loop_ring_topology(&ring.components, "phase-aligned station")?;
    }

    let total_rings = authored
        .len()
        .checked_add(
            (authored.len() - 1)
                .checked_mul(interpolation_rings)
                .ok_or_else(|| {
                    GeometryError::Invalid(
                        "multi-loop-profile-loft@1 interpolation ring count overflow".to_owned(),
                    )
                })?,
        )
        .ok_or_else(|| {
            GeometryError::Invalid(
                "multi-loop-profile-loft@1 interpolation ring count overflow".to_owned(),
            )
        })?;
    if !(2..=257).contains(&total_rings) {
        return Err(GeometryError::Invalid(
            "multi-loop-profile-loft@1 total ring count is outside bounds".to_owned(),
        ));
    }

    let mut rings = Vec::with_capacity(total_rings);
    for interval in 0..(authored.len() - 1) {
        let left = &authored[interval];
        let right = &authored[interval + 1];
        rings.push(left.clone());
        for step in 1..=interpolation_rings {
            let t = step as f32 / (interpolation_rings + 1) as f32;
            let mut components = Vec::with_capacity(left.components.len());
            for left_component in &left.components {
                let right_component = right
                    .components
                    .iter()
                    .find(|component| component.component_id == left_component.component_id)
                    .ok_or_else(|| {
                        GeometryError::Invalid(
                            "multi-loop-profile-loft@1 interpolated component correspondence is invalid"
                                .to_owned(),
                        )
                    })?;
                let previous_component = interval.checked_sub(1).and_then(|index| {
                    authored[index]
                        .components
                        .iter()
                        .find(|component| component.component_id == left_component.component_id)
                });
                let next_component = (interval + 2 < authored.len())
                    .then(|| {
                        authored[interval + 2]
                            .components
                            .iter()
                            .find(|component| component.component_id == left_component.component_id)
                    })
                    .flatten();
                let outer = interpolate_multi_loop_profile_loop(
                    &left_component.outer,
                    &right_component.outer,
                    previous_component.map(|component| &component.outer),
                    next_component.map(|component| &component.outer),
                    t,
                    interpolation,
                    true,
                )?;
                let mut holes = Vec::with_capacity(left_component.holes.len());
                for left_hole in &left_component.holes {
                    let right_hole = right_component
                        .holes
                        .iter()
                        .find(|hole| hole.loop_id == left_hole.loop_id)
                        .ok_or_else(|| {
                            GeometryError::Invalid(
                                "multi-loop-profile-loft@1 interpolated hole correspondence is invalid"
                                    .to_owned(),
                            )
                        })?;
                    let previous_hole = previous_component.and_then(|component| {
                        component
                            .holes
                            .iter()
                            .find(|hole| hole.loop_id == left_hole.loop_id)
                    });
                    let next_hole = next_component.and_then(|component| {
                        component
                            .holes
                            .iter()
                            .find(|hole| hole.loop_id == left_hole.loop_id)
                    });
                    holes.push(interpolate_multi_loop_profile_loop(
                        left_hole,
                        right_hole,
                        previous_hole,
                        next_hole,
                        t,
                        interpolation,
                        false,
                    )?);
                }
                components.push(MultiLoopProfileLoftComponent {
                    component_id: left_component.component_id.clone(),
                    outer,
                    holes,
                });
            }
            components.sort_by(|left, right| left.component_id.cmp(&right.component_id));
            let interpolated = MultiLoopProfileLoftRing {
                station_m: left.station_m + (right.station_m - left.station_m) * t,
                components,
            };
            validate_multi_loop_ring_topology(&interpolated.components, "interpolated station")?;
            rings.push(interpolated);
        }
    }
    rings.push(authored.last().expect("at least two stations").clone());
    for pair in rings.windows(2) {
        if pair[1].station_m <= pair[0].station_m {
            return Err(GeometryError::Invalid(
                "multi-loop-profile-loft@1 ring stations must be strictly increasing".to_owned(),
            ));
        }
    }
    Ok(rings)
}

fn validate_multi_loop_raw_component(
    component: &RawMultiLoopProfileLoftComponent,
) -> Result<(), GeometryError> {
    validate_multi_loop_oriented_loop(&component.outer.points, true, "outer loop")?;
    for hole in &component.holes {
        validate_multi_loop_oriented_loop(&hole.points, false, "hole loop")?;
    }
    validate_multi_loop_component_geometry(
        &component.outer.points,
        &component
            .holes
            .iter()
            .map(|hole| hole.points.as_slice())
            .collect::<Vec<_>>(),
        "raw component",
    )
}

fn validate_multi_loop_oriented_loop(
    points: &[[f32; 2]],
    expect_ccw: bool,
    label: &str,
) -> Result<(), GeometryError> {
    validate_simple_profile(points, label)?;
    let area = signed_area(points);
    if !area.is_finite() || area.abs() <= 1.0e-5 {
        return Err(GeometryError::Invalid(format!(
            "multi-loop-profile-loft@1 {label} has zero or non-finite area"
        )));
    }
    if (expect_ccw && area <= 1.0e-5) || (!expect_ccw && area >= -1.0e-5) {
        return Err(GeometryError::Invalid(format!(
            "multi-loop-profile-loft@1 {label} winding is invalid"
        )));
    }
    Ok(())
}

fn resample_multi_loop_profile_loop(
    raw: &RawMultiLoopProfileLoftLoop,
    sample_count: usize,
    preserve_corners: bool,
    expect_ccw: bool,
) -> Result<MultiLoopProfileLoftLoop, GeometryError> {
    let corners = merge_corner_indices(&raw.points, &raw.corner_indices, preserve_corners);
    let sampled =
        resample_closed_profile_with_winding(&raw.points, &corners, sample_count, expect_ccw)?;
    Ok(MultiLoopProfileLoftLoop {
        loop_id: raw.loop_id.clone(),
        points: sampled.points,
        corner_flags: sampled.corner_flags,
    })
}

fn validate_multi_loop_ring_topology(
    components: &[MultiLoopProfileLoftComponent],
    label: &str,
) -> Result<(), GeometryError> {
    if !(1..=MAX_MULTI_LOOP_COMPONENTS).contains(&components.len()) {
        return Err(GeometryError::Invalid(format!(
            "multi-loop-profile-loft@1 {label} component count is outside bounds"
        )));
    }
    let mut component_ids = BTreeSet::new();
    for component in components {
        if !component_ids.insert(component.component_id.clone()) {
            return Err(GeometryError::Invalid(format!(
                "multi-loop-profile-loft@1 {label} component ids are not unique"
            )));
        }
        validate_multi_loop_oriented_loop(&component.outer.points, true, "outer loop")?;
        if component.holes.len() > MAX_MULTI_LOOP_HOLES {
            return Err(GeometryError::Invalid(format!(
                "multi-loop-profile-loft@1 {label} hole count is outside bounds"
            )));
        }
        let mut hole_ids = BTreeSet::new();
        for hole in &component.holes {
            if !hole_ids.insert(hole.loop_id.clone()) {
                return Err(GeometryError::Invalid(format!(
                    "multi-loop-profile-loft@1 {label} hole ids are not unique"
                )));
            }
            validate_multi_loop_oriented_loop(&hole.points, false, "hole loop")?;
            if hole.points.len() != component.outer.points.len() {
                return Err(GeometryError::Invalid(
                    "multi-loop-profile-loft@1 loop resampling correspondence is invalid"
                        .to_owned(),
                ));
            }
            if hole.points.len() != hole.corner_flags.len() {
                return Err(GeometryError::Invalid(
                    "multi-loop-profile-loft@1 hole resampling is invalid".to_owned(),
                ));
            }
        }
        if component.outer.points.len() != component.outer.corner_flags.len()
            || component.outer.points.len() < 4
        {
            return Err(GeometryError::Invalid(
                "multi-loop-profile-loft@1 outer resampling is invalid".to_owned(),
            ));
        }
        validate_multi_loop_component_geometry(
            &component.outer.points,
            &component
                .holes
                .iter()
                .map(|hole| hole.points.as_slice())
                .collect::<Vec<_>>(),
            label,
        )?;
    }
    // Components are independent material domains.  Their boundaries may not
    // touch or cross, but an island is valid when its complete outer loop is
    // strictly inside another component's hole.  Conversely, putting an
    // island inside another component's solid material would create an
    // overlapping Boolean operand and is rejected here before mesh emission.
    for left_index in 0..components.len() {
        for right_index in (left_index + 1)..components.len() {
            let left = &components[left_index];
            let right = &components[right_index];
            for left_loop in component_loops(left) {
                for right_loop in component_loops(right) {
                    if polygons_boundaries_intersect(left_loop, right_loop) {
                        return Err(GeometryError::Invalid(
                            "multi-loop-profile-loft@1 loops from different components touch or cross"
                                .to_owned(),
                        ));
                    }
                }
            }
            if component_material_domains_overlap(left, right) {
                return Err(GeometryError::Invalid(
                    "multi-loop-profile-loft@1 components overlap in material domain".to_owned(),
                ));
            }
        }
    }
    Ok(())
}

fn component_loops<'a>(component: &'a MultiLoopProfileLoftComponent) -> Vec<&'a [[f32; 2]]> {
    let mut loops = Vec::with_capacity(component.holes.len() + 1);
    loops.push(component.outer.points.as_slice());
    loops.extend(component.holes.iter().map(|hole| hole.points.as_slice()));
    loops
}

fn validate_multi_loop_component_geometry(
    outer: &[[f32; 2]],
    holes: &[&[[f32; 2]]],
    label: &str,
) -> Result<(), GeometryError> {
    for hole in holes {
        if polygons_boundaries_intersect(outer, hole)
            || !hole
                .iter()
                .all(|point| point_in_polygon_strict(*point, outer))
        {
            return Err(GeometryError::Invalid(format!(
                "multi-loop-profile-loft@1 {label} hole is outside or touches outer"
            )));
        }
    }
    for left_index in 0..holes.len() {
        for right_index in (left_index + 1)..holes.len() {
            if polygons_touch_or_overlap(holes[left_index], holes[right_index]) {
                return Err(GeometryError::Invalid(format!(
                    "multi-loop-profile-loft@1 {label} holes overlap, nest or touch"
                )));
            }
        }
    }
    Ok(())
}

fn polygons_boundaries_intersect(left: &[[f32; 2]], right: &[[f32; 2]]) -> bool {
    for left_index in 0..left.len() {
        let left_next = (left_index + 1) % left.len();
        for right_index in 0..right.len() {
            let right_next = (right_index + 1) % right.len();
            if segments_intersect(
                left[left_index],
                left[left_next],
                right[right_index],
                right[right_next],
            ) {
                return true;
            }
        }
    }
    false
}

fn component_material_domains_overlap(
    left: &MultiLoopProfileLoftComponent,
    right: &MultiLoopProfileLoftComponent,
) -> bool {
    left.outer
        .points
        .iter()
        .any(|point| point_in_component_material(*point, right))
        || right
            .outer
            .points
            .iter()
            .any(|point| point_in_component_material(*point, left))
}

fn point_in_component_material(point: [f32; 2], component: &MultiLoopProfileLoftComponent) -> bool {
    point_in_polygon_strict(point, &component.outer.points)
        && !component
            .holes
            .iter()
            .any(|hole| point_in_polygon_strict(point, &hole.points))
}

fn polygons_touch_or_overlap(left: &[[f32; 2]], right: &[[f32; 2]]) -> bool {
    for left_index in 0..left.len() {
        let left_next = (left_index + 1) % left.len();
        for right_index in 0..right.len() {
            let right_next = (right_index + 1) % right.len();
            if segments_intersect(
                left[left_index],
                left[left_next],
                right[right_index],
                right[right_next],
            ) {
                return true;
            }
        }
    }
    left.first()
        .is_some_and(|point| point_in_polygon_strict(*point, right))
        || right
            .first()
            .is_some_and(|point| point_in_polygon_strict(*point, left))
}

fn point_in_polygon_strict(point: [f32; 2], polygon: &[[f32; 2]]) -> bool {
    let mut inside = false;
    for index in 0..polygon.len() {
        let next = (index + 1) % polygon.len();
        let a = polygon[index];
        let b = polygon[next];
        if point_on_segment(point, a, b) {
            return false;
        }
        let crosses = (a[1] > point[1]) != (b[1] > point[1]);
        if crosses {
            let x = a[0] + (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]);
            if x > point[0] {
                inside = !inside;
            }
        }
    }
    inside
}

fn point_on_segment(point: [f32; 2], a: [f32; 2], b: [f32; 2]) -> bool {
    const EPSILON: f32 = 1.0e-6;
    let edge = subtract2(b, a);
    let offset = subtract2(point, a);
    cross2(edge, offset).abs() <= EPSILON
        && point[0] >= a[0].min(b[0]) - EPSILON
        && point[0] <= a[0].max(b[0]) + EPSILON
        && point[1] >= a[1].min(b[1]) - EPSILON
        && point[1] <= a[1].max(b[1]) + EPSILON
}

fn interpolate_multi_loop_profile_loop(
    left: &MultiLoopProfileLoftLoop,
    right: &MultiLoopProfileLoftLoop,
    previous: Option<&MultiLoopProfileLoftLoop>,
    next: Option<&MultiLoopProfileLoftLoop>,
    t: f32,
    interpolation: ProfileLoftV2Interpolation,
    expect_ccw: bool,
) -> Result<MultiLoopProfileLoftLoop, GeometryError> {
    if left.loop_id != right.loop_id
        || left.points.len() != right.points.len()
        || left.points.len() != left.corner_flags.len()
        || right.points.len() != right.corner_flags.len()
    {
        return Err(GeometryError::Invalid(
            "multi-loop-profile-loft@1 interpolation correspondence is invalid".to_owned(),
        ));
    }
    if previous.is_some_and(|ring| ring.points.len() != left.points.len())
        || next.is_some_and(|ring| ring.points.len() != left.points.len())
    {
        return Err(GeometryError::Invalid(
            "multi-loop-profile-loft@1 interpolation neighborhood is invalid".to_owned(),
        ));
    }
    let mut points = Vec::with_capacity(left.points.len());
    let mut corner_flags = Vec::with_capacity(left.points.len());
    for index in 0..left.points.len() {
        let point = match interpolation {
            ProfileLoftV2Interpolation::Linear => lerp2(left.points[index], right.points[index], t),
            ProfileLoftV2Interpolation::CatmullRom => catmull_rom2(
                previous
                    .map(|ring| ring.points[index])
                    .unwrap_or(left.points[index]),
                left.points[index],
                right.points[index],
                next.map(|ring| ring.points[index])
                    .unwrap_or(right.points[index]),
                t,
            ),
        };
        if !point[0].is_finite()
            || !point[1].is_finite()
            || point[0].abs() > MAX_COORDINATE
            || point[1].abs() > MAX_COORDINATE
        {
            return Err(GeometryError::Invalid(
                "multi-loop-profile-loft@1 interpolation emitted an invalid point".to_owned(),
            ));
        }
        points.push(point);
        corner_flags.push(left.corner_flags[index] || right.corner_flags[index]);
    }
    validate_multi_loop_oriented_loop(&points, expect_ccw, "interpolated loop")?;
    Ok(MultiLoopProfileLoftLoop {
        loop_id: left.loop_id.clone(),
        points,
        corner_flags,
    })
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

fn resample_closed_profile_with_winding(
    profile: &[[f32; 2]],
    corner_indices: &[usize],
    sample_count: usize,
    expect_ccw: bool,
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
    let area = signed_area(&points);
    if !area.is_finite() || (expect_ccw && area <= 1.0e-5) || (!expect_ccw && area >= -1.0e-5) {
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
    align_multi_loop_phase(
        reference,
        &mut candidate.points,
        &mut candidate.corner_flags,
    );
}

fn align_multi_loop_phase(
    reference: &[[f32; 2]],
    candidate_points: &mut Vec<[f32; 2]>,
    candidate_flags: &mut Vec<bool>,
) {
    if reference.len() != candidate_points.len()
        || candidate_points.len() != candidate_flags.len()
        || reference.is_empty()
    {
        return;
    }
    let (reference_normalized, _, _) = normalized_ring_points(reference);
    let (candidate_normalized, _, _) = normalized_ring_points(candidate_points);
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
    let points = candidate_points.clone();
    let flags = candidate_flags.clone();
    for index in 0..sample_count {
        candidate_points[index] = points[(index + best_shift) % sample_count];
        candidate_flags[index] = flags[(index + best_shift) % sample_count];
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

fn multi_loop_profile_loft_mesh(
    rings: &[MultiLoopProfileLoftRing],
    max_triangles: u64,
    max_runtime_ms: u64,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let first = rings.first().ok_or_else(|| {
        GeometryError::Invalid("multi-loop-profile-loft@1 requires at least two rings".to_owned())
    })?;
    if rings.len() < 2 || first.components.is_empty() {
        return Err(GeometryError::Invalid(
            "multi-loop-profile-loft@1 ring topology is invalid".to_owned(),
        ));
    }

    let mut result = empty_mesh();
    let total_hole_operations = first
        .components
        .iter()
        .map(|component| component.holes.len())
        .sum::<usize>();
    let mut remaining_hole_operations = total_hole_operations;
    let mut remaining_runtime_ms = max_runtime_ms;
    for component in &first.components {
        let mut solid = multi_loop_single_loop_mesh(
            rings,
            &component.component_id,
            &component.outer.loop_id,
            false,
        )?;
        for hole in &component.holes {
            if remaining_hole_operations == 0 {
                return Err(GeometryError::Invalid(
                    "multi-loop-profile-loft@1 runtime operation budget underflow".to_owned(),
                ));
            }
            // Every Boolean receives a deterministic slice of the one
            // operator-level budget. This prevents a four-hole profile from
            // resetting the full worker timeout for every difference.
            let difference_runtime_ms = remaining_runtime_ms / remaining_hole_operations as u64;
            if difference_runtime_ms == 0 {
                return Err(GeometryError::Invalid(
                    "multi-loop-profile-loft@1 runtime budget is outside bounds".to_owned(),
                ));
            }
            remaining_runtime_ms -= difference_runtime_ms;
            remaining_hole_operations -= 1;
            let cutter =
                multi_loop_single_loop_mesh(rings, &component.component_id, &hole.loop_id, true)?;
            let cut = manifold_bridge::execute_boolean(
                &solid,
                &cutter,
                "difference",
                max_triangles,
                difference_runtime_ms,
            )?;
            solid = PrimitiveNodeMesh {
                operator_id: "forgecad.geometry.multi-loop-profile-loft@1".to_owned(),
                lineage_source_node_ids: Vec::new(),
                positions: cut.positions,
                normals: cut.normals,
                indices: cut.indices,
            };
        }
        append_mesh(&mut result, &solid);
    }
    result.operator_id = "forgecad.geometry.multi-loop-profile-loft@1".to_owned();
    let triangle_budget = usize::try_from(max_triangles).map_err(|_| {
        GeometryError::Invalid(
            "multi-loop-profile-loft@1 triangle budget is not representable".to_owned(),
        )
    })?;
    if result.indices.len() / 3 > triangle_budget {
        return Err(GeometryError::Invalid(
            "multi-loop-profile-loft@1 output exceeds the triangle budget".to_owned(),
        ));
    }
    Ok(result)
}

fn multi_loop_single_loop_mesh(
    rings: &[MultiLoopProfileLoftRing],
    component_id: &str,
    loop_id: &str,
    extend_ends: bool,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let endpoint_extension = if extend_ends {
        let first_station = rings
            .first()
            .expect("multi-loop mesh requires at least two rings")
            .station_m;
        let last_station = rings
            .last()
            .expect("multi-loop mesh requires at least two rings")
            .station_m;
        let span = last_station - first_station;
        if !span.is_finite() || span <= 0.0 {
            return Err(GeometryError::Invalid(
                "multi-loop-profile-loft@1 cutter station span is invalid".to_owned(),
            ));
        }
        // This is derived cutter geometry, not authored station data.  It is
        // intentionally allowed to extend a tiny bounded distance beyond the
        // input envelope so stations at the contract coordinate boundary
        // still produce a true through-hole rather than a coplanar cut.
        let desired = (span * 0.01).clamp(1.0e-4, 0.05);
        if !desired.is_finite() || desired <= 1.0e-6 {
            return Err(GeometryError::Invalid(
                "multi-loop-profile-loft@1 cutter extension is invalid".to_owned(),
            ));
        }
        desired
    } else {
        0.0
    };
    let mut profile_rings = Vec::with_capacity(rings.len());
    for (ring_index, ring) in rings.iter().enumerate() {
        let component = ring
            .components
            .iter()
            .find(|component| component.component_id == component_id)
            .ok_or_else(|| {
                GeometryError::Invalid(
                    "multi-loop-profile-loft@1 mesh component correspondence is invalid".to_owned(),
                )
            })?;
        let loop_value = if component.outer.loop_id.as_str() == loop_id {
            &component.outer
        } else {
            component
                .holes
                .iter()
                .find(|hole| hole.loop_id.as_str() == loop_id)
                .ok_or_else(|| {
                    GeometryError::Invalid(
                        "multi-loop-profile-loft@1 mesh loop correspondence is invalid".to_owned(),
                    )
                })?
        };
        // ProfileLoftV2 is a positive-solid kernel.  A hole's authored CW
        // loop is reversed only for the temporary cutter solid; the Boolean
        // difference then restores the required interior-wall orientation.
        let (points, corner_flags) = if signed_area(&loop_value.points) > 0.0 {
            (loop_value.points.clone(), loop_value.corner_flags.clone())
        } else {
            (
                loop_value.points.iter().rev().copied().collect(),
                loop_value.corner_flags.iter().rev().copied().collect(),
            )
        };
        let station_m = if extend_ends && ring_index == 0 {
            ring.station_m - endpoint_extension
        } else if extend_ends && ring_index + 1 == rings.len() {
            ring.station_m + endpoint_extension
        } else {
            ring.station_m
        };
        profile_rings.push(ProfileLoftV2Ring {
            station_m,
            points,
            corner_flags,
        });
    }
    profile_loft_v2_mesh(&profile_rings)
}

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

fn add2(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

fn cross2(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[1] - a[1] * b[0]
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

fn dot2(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

fn length2(value: [f32; 2]) -> f32 {
    dot2(value, value).sqrt()
}

fn lerp2(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    add2(a, scale2(subtract2(b, a), t))
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

fn resample_closed_profile(
    profile: &[[f32; 2]],
    corner_indices: &[usize],
    sample_count: usize,
) -> Result<ProfileLoftV2Ring, GeometryError> {
    resample_closed_profile_with_winding(profile, corner_indices, sample_count, true)
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

fn scale2(value: [f32; 2], scalar: f32) -> [f32; 2] {
    [value[0] * scalar, value[1] * scalar]
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

fn subtract2(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

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

fn rounded_box_mesh(
    size: [f32; 3],
    position: [f32; 3],
    rotation: [f32; 3],
    requested_width: f32,
    segments: usize,
    profile: f32,
    clamp_overlap: bool,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let half = [size[0] / 2.0, size[1] / 2.0, size[2] / 2.0];
    let maximum = half[0].min(half[1]).min(half[2]) * 0.999;
    let width = if requested_width > maximum {
        if !clamp_overlap {
            return Err(GeometryError::Invalid(
                "bevel width would overlap; enable clamp_overlap or reduce width".to_owned(),
            ));
        }
        maximum
    } else {
        requested_width
    };
    if width < 1.0e-5 || maximum < 1.0e-5 {
        return Err(GeometryError::Invalid(
            "bevel width or source box is below the stable tolerance".to_owned(),
        ));
    }
    let inner = [half[0] - width, half[1] - width, half[2] - width];
    let exponent = 1.0 + 2.0 * profile;
    let coordinates = |extent: f32, inset: f32| {
        let mut values = Vec::with_capacity(2 * segments + 2);
        for step in 0..segments {
            values.push(-extent + width * step as f32 / segments as f32);
        }
        values.push(-inset);
        values.push(inset);
        for step in 1..segments {
            values.push(inset + width * step as f32 / segments as f32);
        }
        values.push(extent);
        values
    };
    let xs = coordinates(half[0], inner[0]);
    let ys = coordinates(half[1], inner[1]);
    let zs = coordinates(half[2], inner[2]);
    let round = |point: [f32; 3]| {
        let clamped = [
            point[0].clamp(-inner[0], inner[0]),
            point[1].clamp(-inner[1], inner[1]),
            point[2].clamp(-inner[2], inner[2]),
        ];
        let delta = subtract3(point, clamped);
        let norm = (delta[0].abs().powf(exponent)
            + delta[1].abs().powf(exponent)
            + delta[2].abs().powf(exponent))
        .powf(1.0 / exponent);
        add3(clamped, scale3(delta, width / norm))
    };
    let mut mesh = empty_mesh();
    let mut emit_face = |u: &[f32],
                         v: &[f32],
                         point: &dyn Fn(f32, f32) -> [f32; 3],
                         reverse: bool|
     -> Result<(), GeometryError> {
        for ui in 0..u.len() - 1 {
            for vi in 0..v.len() - 1 {
                let a = round(point(u[ui], v[vi]));
                let b = round(point(u[ui + 1], v[vi]));
                let c = round(point(u[ui + 1], v[vi + 1]));
                let d = round(point(u[ui], v[vi + 1]));
                if reverse {
                    push_triangle(&mut mesh, a, c, b)?;
                    push_triangle(&mut mesh, a, d, c)?;
                } else {
                    push_triangle(&mut mesh, a, b, c)?;
                    push_triangle(&mut mesh, a, c, d)?;
                }
            }
        }
        Ok(())
    };
    emit_face(&ys, &zs, &|u, v| [half[0], u, v], false)?;
    emit_face(&ys, &zs, &|u, v| [-half[0], u, v], true)?;
    emit_face(&zs, &xs, &|u, v| [v, half[1], u], false)?;
    emit_face(&zs, &xs, &|u, v| [v, -half[1], u], true)?;
    emit_face(&xs, &ys, &|u, v| [u, v, half[2]], false)?;
    emit_face(&xs, &ys, &|u, v| [u, v, -half[2]], true)?;
    Ok(transform_mesh(mesh, position, rotation, [1.0; 3]))
}

/// Rebuild normals in the corner domain. Each triangle corner receives an
/// face-area x clean-room face-angle weighted normal from coincident
/// corners whose face normal remains within the explicit crease threshold.
fn area_angle_corner_normals(
    input: &PrimitiveNodeMesh,
    crease_angle: f32,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    #[derive(Clone, Copy)]
    struct Corner {
        position: [f32; 3],
        face_normal: [f32; 3],
        weight: f32,
    }
    let mut corners = Vec::with_capacity(input.indices.len());
    for triangle in input.indices.chunks_exact(3) {
        let ids = [
            triangle[0] as usize,
            triangle[1] as usize,
            triangle[2] as usize,
        ];
        if ids.iter().any(|index| *index >= input.positions.len()) {
            return Err(GeometryError::Invalid(
                "normal-policy input index is outside the mesh".to_owned(),
            ));
        }
        let points = [
            input.positions[ids[0]],
            input.positions[ids[1]],
            input.positions[ids[2]],
        ];
        let cross = cross3(
            subtract3(points[1], points[0]),
            subtract3(points[2], points[0]),
        );
        let twice_area = length3(cross);
        if !twice_area.is_finite() || twice_area <= 1.0e-8 {
            return Err(GeometryError::Invalid(
                "normal-policy rejects degenerate triangles".to_owned(),
            ));
        }
        let face_normal = normalize(cross);
        for corner_index in 0..3 {
            let a = normalize(subtract3(
                points[(corner_index + 1) % 3],
                points[corner_index],
            ));
            let b = normalize(subtract3(
                points[(corner_index + 2) % 3],
                points[corner_index],
            ));
            let interior_angle = dot3(a, b).clamp(-1.0, 1.0).acos();
            let face_angle = std::f32::consts::PI - interior_angle;
            corners.push(Corner {
                position: points[corner_index],
                face_normal,
                weight: twice_area * face_angle,
            });
        }
    }
    let key = |point: [f32; 3]| {
        (
            (point[0] * 1_000_000.0).round() as i32,
            (point[1] * 1_000_000.0).round() as i32,
            (point[2] * 1_000_000.0).round() as i32,
        )
    };
    let mut groups: BTreeMap<(i32, i32, i32), Vec<usize>> = BTreeMap::new();
    for (index, corner) in corners.iter().enumerate() {
        groups.entry(key(corner.position)).or_default().push(index);
    }
    let cosine = crease_angle.cos();
    let mut normals = Vec::with_capacity(corners.len());
    for (index, corner) in corners.iter().enumerate() {
        let mut sum = [0.0; 3];
        for other_index in groups.get(&key(corner.position)).expect("corner group") {
            let other = corners[*other_index];
            if dot3(corner.face_normal, other.face_normal) >= cosine {
                sum = add3(sum, scale3(other.face_normal, other.weight));
            }
        }
        let normal = if length3(sum) <= 1.0e-8 {
            // Opposing coincident faces can cancel exactly at a permissive
            // crease angle. Preserve the reference corner instead of using
            // normalize()'s generic fallback axis.
            corner.face_normal
        } else {
            normalize(sum)
        };
        if !finite3(normal) || length3(normal) <= f32::EPSILON {
            return Err(GeometryError::Invalid(format!(
                "normal-policy emitted an invalid corner normal at {index}"
            )));
        }
        normals.push(normal);
    }
    Ok(PrimitiveNodeMesh {
        operator_id: "forgecad.geometry.normal-policy@1".to_owned(),
        lineage_source_node_ids: input.lineage_source_node_ids.clone(),
        positions: corners.iter().map(|corner| corner.position).collect(),
        normals,
        indices: (0..corners.len() as u32).collect(),
    })
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
    let cross_length = length3(cross);
    if !finite3(cross) || !cross_length.is_finite() || cross_length <= 1.0e-8 {
        return Err(GeometryError::Invalid(
            "operator emitted a degenerate triangle".to_owned(),
        ));
    }
    let normal = scale3(cross, 1.0 / cross_length);
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

/// Build a closed, rectangular panel with explicit source-level inset,
/// recessed floor, border bands, sloped bevels, and concentric support loops.
///
/// This is intentionally a bounded panel grammar rather than a general BMesh
/// operation: every ring is a four-corner rectangle, all surfaces are emitted
/// deterministically, and the resulting solid is still consumed by the same
/// strict GLB/readback path as every other GeometryProgram@2 operator.
fn recessed_panel_v2_mesh(
    size_m: [f32; 3],
    thickness_m: f32,
    inset_m: f32,
    recess_depth_m: f32,
    border_width_m: f32,
    bevel_m: f32,
    bevel_segments: usize,
    support_loop_count: usize,
    support_loop_width_m: f32,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let half_x = size_m[0] / 2.0;
    let half_y = size_m[1] / 2.0;
    let top_z = thickness_m / 2.0;
    let bottom_z = -thickness_m / 2.0;
    let outer_side = rect_ring(half_x, half_y, top_z - bevel_m);
    let outer_top = rect_ring(half_x - bevel_m, half_y - bevel_m, top_z);
    let border_outer = rect_ring(half_x - inset_m, half_y - inset_m, top_z);
    let recess_start = rect_ring(
        half_x - inset_m - border_width_m,
        half_y - inset_m - border_width_m,
        top_z,
    );
    let back = rect_ring(half_x, half_y, bottom_z);
    let support_span = support_loop_count as f32 * support_loop_width_m;
    let minimum_strip_span = (bevel_m / bevel_segments as f32)
        .min(support_loop_width_m)
        .min(inset_m - bevel_m - support_span)
        .min(border_width_m - support_span);
    let panel_edge_segments = panel_edge_segments(size_m, minimum_strip_span);

    let mut mesh = empty_mesh();
    // The outer wall connects the full back footprint to the upper bevel.
    push_topology_safe_ring_strip(&mut mesh, back, outer_side, panel_edge_segments)?;

    // Outer bevel: the source edge transitions through exactly the requested
    // segment count, so increasing bevel_segments produces real support
    // geometry rather than a metadata-only quality hint.
    let mut previous = outer_side;
    for segment in 1..=bevel_segments {
        let t = segment as f32 / bevel_segments as f32;
        let ring = rect_ring(
            half_x - bevel_m * t,
            half_y - bevel_m * t,
            top_z - bevel_m + bevel_m * t,
        );
        push_topology_safe_ring_strip(&mut mesh, previous, ring, panel_edge_segments)?;
        previous = ring;
    }

    // Flat outer shoulder with explicit concentric support loops.
    let mut previous = outer_top;
    for loop_index in 1..=support_loop_count {
        let inset = support_loop_width_m * loop_index as f32;
        let ring = rect_ring(half_x - bevel_m - inset, half_y - bevel_m - inset, top_z);
        push_topology_safe_ring_strip(&mut mesh, previous, ring, panel_edge_segments)?;
        previous = ring;
    }
    push_topology_safe_ring_strip(&mut mesh, previous, border_outer, panel_edge_segments)?;

    // Border band with a second set of support loops immediately outside the
    // recessed edge. The two loop families keep both the outer silhouette and
    // the inset transition locally controllable in a bounded topology.
    let mut previous = border_outer;
    for loop_index in 1..=support_loop_count {
        let inset = inset_m + support_loop_width_m * loop_index as f32;
        let ring = rect_ring(half_x - inset, half_y - inset, top_z);
        push_topology_safe_ring_strip(&mut mesh, previous, ring, panel_edge_segments)?;
        previous = ring;
    }
    push_topology_safe_ring_strip(&mut mesh, previous, recess_start, panel_edge_segments)?;

    // Inner bevel descends into the recessed floor.
    let mut previous = recess_start;
    for segment in 1..=bevel_segments {
        let t = segment as f32 / bevel_segments as f32;
        let ring = rect_ring(
            half_x - inset_m - border_width_m - bevel_m * t,
            half_y - inset_m - border_width_m - bevel_m * t,
            top_z - recess_depth_m * t,
        );
        push_topology_safe_ring_strip(&mut mesh, previous, ring, panel_edge_segments)?;
        previous = ring;
    }

    // Close the recessed floor and the back. Winding is explicit so the
    // strict readback can verify outward-facing, manifold topology.
    // Reuse the final bevel ring verbatim. Recomputing the same decimal
    // coordinates through a separate arithmetic path can create sub-micron
    // T-junctions after rotation and strict welded readback.
    push_subdivided_rect_face(&mut mesh, previous, panel_edge_segments, false)?;
    push_subdivided_rect_face(&mut mesh, back, panel_edge_segments, true)?;
    Ok(mesh)
}

fn rect_ring(half_x: f32, half_y: f32, z: f32) -> [[f32; 3]; 4] {
    [
        [-half_x, -half_y, z],
        [half_x, -half_y, z],
        [half_x, half_y, z],
        [-half_x, half_y, z],
    ]
}

// Keep automatic panel subdivision below both the per-Part 512-face ceiling
// and the 1 MiB explicit-topology response budget, while reducing the former
// 80+ long-strip aspect ratios to a bounded production-safe target.
const PANEL_MAX_TRIANGLE_ASPECT_TARGET: f32 = 16.0;
const PANEL_MAX_EDGE_SEGMENTS: usize = 32;

fn panel_edge_segments(size_m: [f32; 3], minimum_strip_span: f32) -> [usize; 4] {
    let span = minimum_strip_span.max(1.0e-5);
    let x_segments = ((size_m[0] / (span * PANEL_MAX_TRIANGLE_ASPECT_TARGET)).ceil() as usize)
        .clamp(1, PANEL_MAX_EDGE_SEGMENTS);
    let y_segments = ((size_m[1] / (span * PANEL_MAX_TRIANGLE_ASPECT_TARGET)).ceil() as usize)
        .clamp(1, PANEL_MAX_EDGE_SEGMENTS);
    [x_segments, y_segments, x_segments, y_segments]
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    if t <= 0.0 {
        return a;
    }
    if t >= 1.0 {
        return b;
    }
    let stable_component = |left: f32, right: f32| {
        let interpolated = left as f64 + (right as f64 - left as f64) * t as f64;
        ((interpolated * 10_000_000.0).round() / 10_000_000.0) as f32
    };
    [
        stable_component(a[0], b[0]),
        stable_component(a[1], b[1]),
        stable_component(a[2], b[2]),
    ]
}

fn lerp3_ratio(a: [f32; 3], b: [f32; 3], numerator: usize, denominator: usize) -> [f32; 3] {
    if numerator == 0 {
        return a;
    }
    if numerator == denominator {
        return b;
    }
    let t = numerator as f64 / denominator as f64;
    let stable_component = |left: f32, right: f32| {
        let interpolated = left as f64 + (right as f64 - left as f64) * t;
        ((interpolated * 10_000_000.0).round() / 10_000_000.0) as f32
    };
    [
        stable_component(a[0], b[0]),
        stable_component(a[1], b[1]),
        stable_component(a[2], b[2]),
    ]
}

fn push_topology_safe_ring_strip(
    mesh: &mut PrimitiveNodeMesh,
    outer: [[f32; 3]; 4],
    inner: [[f32; 3]; 4],
    segment_counts: [usize; 4],
) -> Result<(), GeometryError> {
    for index in 0..4 {
        let next = (index + 1) % 4;
        for segment in 0..segment_counts[index] {
            let a = lerp3_ratio(outer[index], outer[next], segment, segment_counts[index]);
            let b = lerp3_ratio(
                outer[index],
                outer[next],
                segment + 1,
                segment_counts[index],
            );
            let c = lerp3_ratio(
                inner[index],
                inner[next],
                segment + 1,
                segment_counts[index],
            );
            let d = lerp3_ratio(inner[index], inner[next], segment, segment_counts[index]);
            push_triangle(&mut *mesh, a, b, c)?;
            push_triangle(&mut *mesh, a, c, d)?;
        }
    }
    Ok(())
}

fn push_subdivided_rect_face(
    mesh: &mut PrimitiveNodeMesh,
    corners: [[f32; 3]; 4],
    edge_segments: [usize; 4],
    reverse: bool,
) -> Result<(), GeometryError> {
    let u_segments = edge_segments[0].max(edge_segments[2]);
    let v_segments = edge_segments[1].max(edge_segments[3]);
    let point = |u_index: usize, v_index: usize| {
        if v_index == 0 {
            return lerp3_ratio(corners[0], corners[1], u_index, u_segments);
        }
        if v_index == v_segments {
            return lerp3_ratio(corners[2], corners[3], u_segments - u_index, u_segments);
        }
        if u_index == 0 {
            return lerp3_ratio(corners[3], corners[0], v_segments - v_index, v_segments);
        }
        if u_index == u_segments {
            return lerp3_ratio(corners[1], corners[2], v_index, v_segments);
        }
        let u = u_index as f32 / u_segments as f32;
        let v = v_index as f32 / v_segments as f32;
        let row0 = lerp3(corners[0], corners[1], u);
        let row1 = lerp3(corners[3], corners[2], u);
        lerp3(row0, row1, v)
    };
    for v_index in 0..v_segments {
        for u_index in 0..u_segments {
            let a = point(u_index, v_index);
            let b = point(u_index + 1, v_index);
            let c = point(u_index + 1, v_index + 1);
            let d = point(u_index, v_index + 1);
            if reverse {
                push_triangle(&mut *mesh, a, c, b)?;
                push_triangle(&mut *mesh, a, d, c)?;
            } else {
                push_triangle(&mut *mesh, a, b, c)?;
                push_triangle(&mut *mesh, a, c, d)?;
            }
        }
    }
    Ok(())
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

fn authoring_identifier(value: Option<&Value>, label: &str) -> Result<String, GeometryError> {
    let value = value
        .and_then(Value::as_str)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
        })
        .ok_or_else(|| GeometryError::Invalid(format!("{label} must be an opaque identifier")))?;
    Ok(value.to_owned())
}

fn authoring_array<'a>(
    parameters: &'a Map<String, Value>,
    key: &str,
    min: usize,
    max: usize,
) -> Result<&'a [Value], GeometryError> {
    let values = parameters
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| GeometryError::Invalid(format!("authoring-mesh {key} must be an array")))?;
    if !(min..=max).contains(&values.len()) {
        return Err(GeometryError::Invalid(format!(
            "authoring-mesh {key} count is outside {min}..={max}"
        )));
    }
    Ok(values)
}

fn require_strict_element_order(ids: &[String], label: &str) -> Result<(), GeometryError> {
    if ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(GeometryError::Invalid(format!(
            "authoring-mesh {label} IDs must be unique and lexicographically sorted"
        )));
    }
    Ok(())
}

fn parse_authoring_mesh(
    parameters: &Map<String, Value>,
) -> Result<
    (
        Vec<AuthoringVertex>,
        Vec<AuthoringEdge>,
        Vec<AuthoringLoop>,
        Vec<AuthoringFace>,
        [f32; 3],
        [f32; 3],
    ),
    GeometryError,
> {
    require_exact_keys(
        parameters,
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
        "authoring-mesh",
    )?;
    require_shape(parameters, "authoring-mesh")?;
    if parameters.get("topology_policy").and_then(Value::as_str)
        != Some("triangle-quad-manifold-with-boundary@1")
    {
        return Err(GeometryError::Invalid(
            "authoring-mesh topology_policy is unsupported".to_owned(),
        ));
    }

    let mut vertices = Vec::new();
    for value in authoring_array(parameters, "vertices", 3, MAX_AUTHORING_ELEMENTS)? {
        let object = value.as_object().ok_or_else(|| {
            GeometryError::Invalid("authoring-mesh vertex must be an object".to_owned())
        })?;
        require_exact_keys(
            object,
            &["element_id", "position_m"],
            "authoring-mesh vertex",
        )?;
        vertices.push(AuthoringVertex {
            element_id: authoring_identifier(object.get("element_id"), "vertex element_id")?,
            position_m: v2_vec3(object, "position_m", MAX_COORDINATE, false)?,
        });
    }
    require_strict_element_order(
        &vertices
            .iter()
            .map(|item| item.element_id.clone())
            .collect::<Vec<_>>(),
        "vertex",
    )?;
    let vertex_ids = vertices
        .iter()
        .map(|item| item.element_id.clone())
        .collect::<BTreeSet<_>>();

    let mut edges = Vec::new();
    for value in authoring_array(parameters, "edges", 3, MAX_AUTHORING_ELEMENTS)? {
        let object = value.as_object().ok_or_else(|| {
            GeometryError::Invalid("authoring-mesh edge must be an object".to_owned())
        })?;
        require_exact_keys(object, &["element_id", "vertex_ids"], "authoring-mesh edge")?;
        let element_id = authoring_identifier(object.get("element_id"), "edge element_id")?;
        let endpoints = object
            .get("vertex_ids")
            .and_then(Value::as_array)
            .filter(|items| items.len() == 2)
            .ok_or_else(|| {
                GeometryError::Invalid("authoring-mesh edge needs two vertex_ids".to_owned())
            })?;
        let left = authoring_identifier(endpoints.first(), "edge vertex_id")?;
        let right = authoring_identifier(endpoints.get(1), "edge vertex_id")?;
        if left >= right || !vertex_ids.contains(&left) || !vertex_ids.contains(&right) {
            return Err(GeometryError::Invalid(
                "authoring-mesh edge endpoints must be distinct known IDs in lexical order"
                    .to_owned(),
            ));
        }
        edges.push(AuthoringEdge {
            element_id,
            vertex_ids: [left, right],
        });
    }
    require_strict_element_order(
        &edges
            .iter()
            .map(|item| item.element_id.clone())
            .collect::<Vec<_>>(),
        "edge",
    )?;
    let edge_by_id = edges
        .iter()
        .map(|item| (item.element_id.clone(), item))
        .collect::<BTreeMap<_, _>>();

    let mut loops = Vec::new();
    for value in authoring_array(parameters, "loops", 3, MAX_AUTHORING_ELEMENTS)? {
        let object = value.as_object().ok_or_else(|| {
            GeometryError::Invalid("authoring-mesh loop must be an object".to_owned())
        })?;
        require_exact_keys(
            object,
            &[
                "element_id",
                "face_id",
                "ordinal",
                "vertex_id",
                "edge_id",
                "edge_forward",
            ],
            "authoring-mesh loop",
        )?;
        let ordinal = object
            .get("ordinal")
            .and_then(Value::as_u64)
            .filter(|value| *value <= 3)
            .ok_or_else(|| {
                GeometryError::Invalid("authoring-mesh loop ordinal is invalid".to_owned())
            })? as usize;
        let vertex_id = authoring_identifier(object.get("vertex_id"), "loop vertex_id")?;
        let edge_id = authoring_identifier(object.get("edge_id"), "loop edge_id")?;
        if !vertex_ids.contains(&vertex_id) || !edge_by_id.contains_key(&edge_id) {
            return Err(GeometryError::Invalid(
                "authoring-mesh loop references an unknown vertex or edge".to_owned(),
            ));
        }
        loops.push(AuthoringLoop {
            element_id: authoring_identifier(object.get("element_id"), "loop element_id")?,
            face_id: authoring_identifier(object.get("face_id"), "loop face_id")?,
            ordinal,
            vertex_id,
            edge_id,
            edge_forward: object
                .get("edge_forward")
                .and_then(Value::as_bool)
                .ok_or_else(|| {
                    GeometryError::Invalid("loop edge_forward must be boolean".to_owned())
                })?,
        });
    }
    require_strict_element_order(
        &loops
            .iter()
            .map(|item| item.element_id.clone())
            .collect::<Vec<_>>(),
        "loop",
    )?;
    let loop_by_id = loops
        .iter()
        .map(|item| (item.element_id.clone(), item))
        .collect::<BTreeMap<_, _>>();

    let mut faces = Vec::new();
    let mut edge_incidence: BTreeMap<String, Vec<(String, bool)>> = BTreeMap::new();
    let mut canonical_face_sets = BTreeSet::new();
    let mut used_vertex_ids = BTreeSet::new();
    for value in authoring_array(parameters, "faces", 1, MAX_AUTHORING_FACES)? {
        let object = value.as_object().ok_or_else(|| {
            GeometryError::Invalid("authoring-mesh face must be an object".to_owned())
        })?;
        require_exact_keys(object, &["element_id", "loop_ids"], "authoring-mesh face")?;
        let element_id = authoring_identifier(object.get("element_id"), "face element_id")?;
        let values = object
            .get("loop_ids")
            .and_then(Value::as_array)
            .filter(|items| (3..=4).contains(&items.len()))
            .ok_or_else(|| {
                GeometryError::Invalid("authoring-mesh face must have 3 or 4 loops".to_owned())
            })?;
        let loop_ids = values
            .iter()
            .map(|item| authoring_identifier(Some(item), "face loop_id"))
            .collect::<Result<Vec<_>, _>>()?;
        if loop_ids.iter().collect::<BTreeSet<_>>().len() != loop_ids.len()
            || loop_ids.first() != loop_ids.iter().min()
        {
            return Err(GeometryError::Invalid(
                "authoring-mesh face loops must be unique and rotation-canonical".to_owned(),
            ));
        }
        let mut face_vertices = Vec::new();
        let mut face_edge_ids = BTreeSet::new();
        for (ordinal, loop_id) in loop_ids.iter().enumerate() {
            let current = loop_by_id.get(loop_id).ok_or_else(|| {
                GeometryError::Invalid("authoring-mesh face references an unknown loop".to_owned())
            })?;
            let next = loop_by_id
                .get(&loop_ids[(ordinal + 1) % loop_ids.len()])
                .expect("all loops checked");
            if current.face_id != element_id || current.ordinal != ordinal {
                return Err(GeometryError::Invalid(
                    "authoring-mesh loop face_id or ordinal differs from face winding".to_owned(),
                ));
            }
            let edge = edge_by_id.get(&current.edge_id).expect("loop edge checked");
            let expected = if current.edge_forward {
                [&edge.vertex_ids[0], &edge.vertex_ids[1]]
            } else {
                [&edge.vertex_ids[1], &edge.vertex_ids[0]]
            };
            if current.vertex_id != *expected[0] || next.vertex_id != *expected[1] {
                return Err(GeometryError::Invalid(
                    "authoring-mesh loop edge direction differs from face winding".to_owned(),
                ));
            }
            if !face_edge_ids.insert(current.edge_id.clone()) {
                return Err(GeometryError::Invalid(
                    "authoring-mesh face cannot reuse an edge".to_owned(),
                ));
            }
            edge_incidence
                .entry(current.edge_id.clone())
                .or_default()
                .push((element_id.clone(), current.edge_forward));
            face_vertices.push(current.vertex_id.clone());
        }
        if face_vertices.iter().collect::<BTreeSet<_>>().len() != face_vertices.len() {
            return Err(GeometryError::Invalid(
                "authoring-mesh face cannot reuse a vertex".to_owned(),
            ));
        }
        used_vertex_ids.extend(face_vertices.iter().cloned());
        let mut set_key = face_vertices.clone();
        set_key.sort();
        if !canonical_face_sets.insert(set_key) {
            return Err(GeometryError::Invalid(
                "authoring-mesh contains a duplicate face".to_owned(),
            ));
        }
        let positions = face_vertices
            .iter()
            .map(|vertex_id| {
                vertices
                    .iter()
                    .find(|item| &item.element_id == vertex_id)
                    .map(|item| item.position_m)
                    .expect("vertex IDs checked")
            })
            .collect::<Vec<_>>();
        if length3(cross3(
            subtract3(positions[1], positions[0]),
            subtract3(positions[2], positions[0]),
        )) <= 1.0e-8
        {
            return Err(GeometryError::Invalid(
                "authoring-mesh face area is below tolerance".to_owned(),
            ));
        }
        faces.push(AuthoringFace {
            element_id,
            loop_ids,
        });
    }
    require_strict_element_order(
        &faces
            .iter()
            .map(|item| item.element_id.clone())
            .collect::<Vec<_>>(),
        "face",
    )?;
    let face_ids = faces
        .iter()
        .map(|item| item.element_id.as_str())
        .collect::<BTreeSet<_>>();
    if loops
        .iter()
        .any(|item| !face_ids.contains(item.face_id.as_str()))
    {
        return Err(GeometryError::Invalid(
            "authoring-mesh contains an unowned loop".to_owned(),
        ));
    }
    if loops.len() != faces.iter().map(|face| face.loop_ids.len()).sum::<usize>() {
        return Err(GeometryError::Invalid(
            "authoring-mesh loops must be referenced exactly once".to_owned(),
        ));
    }
    if used_vertex_ids != vertex_ids {
        return Err(GeometryError::Invalid(
            "authoring-mesh vertices must be referenced by a face".to_owned(),
        ));
    }
    for edge in &edges {
        let incidence = edge_incidence.get(&edge.element_id).ok_or_else(|| {
            GeometryError::Invalid("authoring-mesh contains an unused edge".to_owned())
        })?;
        if incidence.len() > 2
            || (incidence.len() == 2
                && (incidence[0].0 == incidence[1].0 || incidence[0].1 == incidence[1].1))
        {
            return Err(GeometryError::Invalid(
                "authoring-mesh edge is non-manifold or has inconsistent winding".to_owned(),
            ));
        }
        let left = vertices
            .iter()
            .find(|item| item.element_id == edge.vertex_ids[0])
            .unwrap();
        let right = vertices
            .iter()
            .find(|item| item.element_id == edge.vertex_ids[1])
            .unwrap();
        if length3(subtract3(left.position_m, right.position_m)) <= 1.0e-6 {
            return Err(GeometryError::Invalid(
                "authoring-mesh edge length is below tolerance".to_owned(),
            ));
        }
    }

    Ok((
        vertices,
        edges,
        loops,
        faces,
        v2_vec3(parameters, "position_m", MAX_COORDINATE, false)?,
        v2_vec3(parameters, "rotation_rad", std::f32::consts::TAU, false)?,
    ))
}

fn authoring_mesh(
    vertices: &[AuthoringVertex],
    _edges: &[AuthoringEdge],
    loops: &[AuthoringLoop],
    faces: &[AuthoringFace],
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let vertex_by_id = vertices
        .iter()
        .map(|item| (item.element_id.as_str(), item.position_m))
        .collect::<BTreeMap<_, _>>();
    let loop_by_id = loops
        .iter()
        .map(|item| (item.element_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let mut mesh = empty_mesh();
    for face in faces {
        let points = face
            .loop_ids
            .iter()
            .map(|loop_id| {
                let loop_item = loop_by_id.get(loop_id.as_str()).expect("validated loop");
                *vertex_by_id
                    .get(loop_item.vertex_id.as_str())
                    .expect("validated vertex")
            })
            .collect::<Vec<_>>();
        for index in 1..points.len() - 1 {
            push_triangle(&mut mesh, points[0], points[index], points[index + 1])?;
        }
    }
    Ok(mesh)
}

fn triangulate_convex_polygon(
    mesh: &mut PrimitiveNodeMesh,
    points: &[[f32; 3]],
) -> Result<(), GeometryError> {
    for anchor in 0..points.len() {
        let rotated = (0..points.len())
            .map(|offset| points[(anchor + offset) % points.len()])
            .collect::<Vec<_>>();
        if (1..rotated.len() - 1).all(|index| {
            length3(cross3(
                subtract3(rotated[index], rotated[0]),
                subtract3(rotated[index + 1], rotated[0]),
            )) > 1.0e-8
        }) {
            for index in 1..rotated.len() - 1 {
                push_triangle(mesh, rotated[0], rotated[index], rotated[index + 1])?;
            }
            return Ok(());
        }
    }
    Err(GeometryError::Invalid(
        "bevel@2 could not triangulate a split convex face".to_owned(),
    ))
}

fn validate_closed_triangle_mesh(mesh: &PrimitiveNodeMesh) -> Result<(), GeometryError> {
    let mut vertices = BTreeMap::<[i64; 3], usize>::new();
    let mut edges = BTreeMap::<(usize, usize), Vec<bool>>::new();
    for triangle in mesh.indices.chunks_exact(3) {
        let mut welded = [0usize; 3];
        for (corner, index) in triangle.iter().enumerate() {
            let position = mesh.positions[*index as usize];
            let key = position.map(|component| (component as f64 * 1_000_000.0).round() as i64);
            let next = vertices.len();
            welded[corner] = *vertices.entry(key).or_insert(next);
        }
        for (from, to) in [
            (welded[0], welded[1]),
            (welded[1], welded[2]),
            (welded[2], welded[0]),
        ] {
            let key = if from < to { (from, to) } else { (to, from) };
            edges.entry(key).or_default().push(from < to);
        }
    }
    let boundary = edges
        .values()
        .filter(|directions| directions.len() == 1)
        .count();
    let non_manifold = edges
        .values()
        .filter(|directions| directions.len() > 2)
        .count();
    let winding = edges
        .values()
        .filter(|directions| directions.len() == 2 && directions[0] == directions[1])
        .count();
    if boundary != 0 || non_manifold != 0 || winding != 0 {
        let vertex_keys = vertices
            .iter()
            .map(|(key, index)| (*index, *key))
            .collect::<BTreeMap<_, _>>();
        let first_issue = edges
            .iter()
            .find(|(_, directions)| directions.len() != 2 || directions[0] == directions[1])
            .map(|((left, right), directions)| {
                format!(
                    " edge={:?}->{:?} directions={directions:?}",
                    vertex_keys.get(left),
                    vertex_keys.get(right)
                )
            })
            .unwrap_or_default();
        return Err(GeometryError::Invalid(format!(
            "bevel@2 topology failed: boundary={boundary} non_manifold={non_manifold} winding={winding}{first_issue}"
        )));
    }
    Ok(())
}

fn bevel_authoring_edge(
    vertices: &[AuthoringVertex],
    edges: &[AuthoringEdge],
    loops: &[AuthoringLoop],
    faces: &[AuthoringFace],
    source_edge_id: &str,
    requested_width_m: f32,
    segments: usize,
    profile: f32,
    clamp_overlap: bool,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    let vertex_by_id = vertices
        .iter()
        .map(|item| (item.element_id.as_str(), item.position_m))
        .collect::<BTreeMap<_, _>>();
    let loop_by_id = loops
        .iter()
        .map(|item| (item.element_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let selected_edge = edges
        .iter()
        .find(|edge| edge.element_id == source_edge_id)
        .ok_or_else(|| GeometryError::Invalid("bevel@2 selected edge is unknown".to_owned()))?;

    let mut face_vertex_ids = Vec::with_capacity(faces.len());
    let mut face_edge_ids = Vec::with_capacity(faces.len());
    let mut edge_incidence: BTreeMap<&str, Vec<(usize, usize)>> = BTreeMap::new();
    for (face_index, face) in faces.iter().enumerate() {
        let mut vertex_ids = Vec::with_capacity(face.loop_ids.len());
        let mut edge_ids = Vec::with_capacity(face.loop_ids.len());
        for (loop_index, loop_id) in face.loop_ids.iter().enumerate() {
            let loop_item = loop_by_id
                .get(loop_id.as_str())
                .expect("authoring mesh loops were validated");
            vertex_ids.push(loop_item.vertex_id.as_str());
            edge_ids.push(loop_item.edge_id.as_str());
            edge_incidence
                .entry(loop_item.edge_id.as_str())
                .or_default()
                .push((face_index, loop_index));
        }
        face_vertex_ids.push(vertex_ids);
        face_edge_ids.push(edge_ids);
    }
    if edges
        .iter()
        .any(|edge| edge_incidence.get(edge.element_id.as_str()).map(Vec::len) != Some(2))
    {
        return Err(GeometryError::Invalid(
            "bevel@2 requires a closed two-face-per-edge authoring mesh".to_owned(),
        ));
    }

    let mut visited = BTreeSet::new();
    let mut pending = vec![0usize];
    while let Some(face_index) = pending.pop() {
        if !visited.insert(face_index) {
            continue;
        }
        for edge_id in &face_edge_ids[face_index] {
            for (neighbor, _) in edge_incidence
                .get(edge_id)
                .expect("closed edge incidence checked")
            {
                if !visited.contains(neighbor) {
                    pending.push(*neighbor);
                }
            }
        }
    }
    if visited.len() != faces.len() {
        return Err(GeometryError::Invalid(
            "bevel@2 requires one connected closed authoring solid".to_owned(),
        ));
    }

    let solid_centroid = scale3(
        vertices
            .iter()
            .fold([0.0; 3], |sum, vertex| add3(sum, vertex.position_m)),
        1.0 / vertices.len() as f32,
    );
    let mut face_normals = Vec::with_capacity(faces.len());
    for vertex_ids in &face_vertex_ids {
        let points = vertex_ids
            .iter()
            .map(|id| *vertex_by_id.get(id).expect("validated authoring vertex"))
            .collect::<Vec<_>>();
        let normal = normalize(cross3(
            subtract3(points[1], points[0]),
            subtract3(points[2], points[0]),
        ));
        if points
            .iter()
            .any(|point| dot3(subtract3(*point, points[0]), normal).abs() > 1.0e-5)
        {
            return Err(GeometryError::Invalid(
                "bevel@2 requires planar source faces".to_owned(),
            ));
        }
        for index in 0..points.len() {
            let previous = points[(index + points.len() - 1) % points.len()];
            let current = points[index];
            let next = points[(index + 1) % points.len()];
            if dot3(
                cross3(subtract3(current, previous), subtract3(next, current)),
                normal,
            ) <= 1.0e-8
            {
                return Err(GeometryError::Invalid(
                    "bevel@2 requires strictly convex source faces".to_owned(),
                ));
            }
        }
        if dot3(subtract3(solid_centroid, points[0]), normal) >= -1.0e-6 {
            return Err(GeometryError::Invalid(
                "bevel@2 requires outward source face winding".to_owned(),
            ));
        }
        if vertices
            .iter()
            .any(|vertex| dot3(subtract3(vertex.position_m, points[0]), normal) > 1.0e-5)
        {
            return Err(GeometryError::Invalid(
                "bevel@2 P0 requires a globally convex source solid".to_owned(),
            ));
        }
        face_normals.push(normal);
    }

    let selected_incidence = edge_incidence
        .get(source_edge_id)
        .filter(|items| items.len() == 2)
        .ok_or_else(|| {
            GeometryError::Invalid("bevel@2 selected edge must have exactly two faces".to_owned())
        })?;
    let [edge_a_id, edge_b_id] = &selected_edge.vertex_ids;
    let edge_a = *vertex_by_id
        .get(edge_a_id.as_str())
        .expect("selected edge vertex checked");
    let edge_b = *vertex_by_id
        .get(edge_b_id.as_str())
        .expect("selected edge vertex checked");
    let edge_length = length3(subtract3(edge_b, edge_a));

    struct IncidentFace {
        face_index: usize,
        edge_loop_index: usize,
        inward: [f32; 3],
        offset_by_vertex: BTreeMap<String, [f32; 3]>,
    }

    let mut clearance = edge_length * 0.25;
    let mut incident_faces = Vec::with_capacity(2);
    for (face_index, edge_loop_index) in selected_incidence {
        let ids = &face_vertex_ids[*face_index];
        let count = ids.len();
        let start_id = ids[*edge_loop_index];
        let end_id = ids[(*edge_loop_index + 1) % count];
        let start = *vertex_by_id.get(start_id).expect("incident start checked");
        let end = *vertex_by_id.get(end_id).expect("incident end checked");
        let previous = *vertex_by_id
            .get(ids[(*edge_loop_index + count - 1) % count])
            .expect("incident previous checked");
        let next = *vertex_by_id
            .get(ids[(*edge_loop_index + 2) % count])
            .expect("incident next checked");
        clearance = clearance
            .min(length3(subtract3(start, previous)) * 0.25)
            .min(length3(subtract3(end, next)) * 0.25);
        let inward = normalize(cross3(
            face_normals[*face_index],
            normalize(subtract3(end, start)),
        ));
        incident_faces.push(IncidentFace {
            face_index: *face_index,
            edge_loop_index: *edge_loop_index,
            inward,
            offset_by_vertex: BTreeMap::new(),
        });
    }
    if face_vertex_ids[incident_faces[0].face_index][incident_faces[0].edge_loop_index] != edge_a_id
    {
        incident_faces.swap(0, 1);
    }
    if clearance <= 1.0e-5 {
        return Err(GeometryError::Invalid(
            "bevel@2 source edge clearance is below tolerance".to_owned(),
        ));
    }
    let width_m = if requested_width_m > clearance {
        if clamp_overlap {
            clearance
        } else {
            return Err(GeometryError::Invalid(
                "bevel@2 width exceeds bounded source-edge clearance".to_owned(),
            ));
        }
    } else {
        requested_width_m
    };
    if width_m <= 1.0e-5 {
        return Err(GeometryError::Invalid(
            "bevel@2 effective width is below tolerance".to_owned(),
        ));
    }
    let inward_a = incident_faces[0].inward;
    let inward_b = incident_faces[1].inward;
    if dot3(inward_a, face_normals[incident_faces[1].face_index]) >= -1.0e-5
        || dot3(inward_b, face_normals[incident_faces[0].face_index]) >= -1.0e-5
    {
        return Err(GeometryError::Invalid(
            "bevel@2 selected edge is coplanar or concave".to_owned(),
        ));
    }
    for incident in &mut incident_faces {
        incident.offset_by_vertex.insert(
            edge_a_id.clone(),
            add3(edge_a, scale3(incident.inward, width_m)),
        );
        incident.offset_by_vertex.insert(
            edge_b_id.clone(),
            add3(edge_b, scale3(incident.inward, width_m)),
        );
    }

    let bisector_sum = add3(inward_a, inward_b);
    if length3(bisector_sum) <= 1.0e-5 {
        return Err(GeometryError::Invalid(
            "bevel@2 selected edge has an unsupported flat dihedral".to_owned(),
        ));
    }
    let bisector = normalize(bisector_sum);
    let profile_bulge = 0.15 + 0.3 * profile;
    let mut profile_a = Vec::with_capacity(segments + 1);
    let mut profile_b = Vec::with_capacity(segments + 1);
    for step in 0..=segments {
        let offset = if step == 0 {
            scale3(inward_a, width_m)
        } else if step == segments {
            scale3(inward_b, width_m)
        } else {
            let t = step as f32 / segments as f32;
            let blend = add3(scale3(inward_a, 1.0 - t), scale3(inward_b, t));
            let bulge = profile_bulge * (std::f32::consts::PI * t).sin();
            scale3(subtract3(blend, scale3(bisector, bulge)), width_m)
        };
        profile_a.push(add3(edge_a, offset));
        profile_b.push(add3(edge_b, offset));
    }

    let mut endpoint_replacements = BTreeMap::<(usize, String), Vec<[f32; 3]>>::new();
    for (endpoint_id, endpoint_profile) in [
        (edge_a_id.as_str(), &profile_a),
        (edge_b_id.as_str(), &profile_b),
    ] {
        let mut adjacent = Vec::with_capacity(2);
        for incident in &incident_faces {
            let ids = &face_vertex_ids[incident.face_index];
            let count = ids.len();
            let selected_start = ids[incident.edge_loop_index];
            let selected_end = ids[(incident.edge_loop_index + 1) % count];
            let adjacent_loop_index = if endpoint_id == selected_start {
                (incident.edge_loop_index + count - 1) % count
            } else if endpoint_id == selected_end {
                (incident.edge_loop_index + 1) % count
            } else {
                return Err(GeometryError::Invalid(
                    "bevel@2 endpoint binding differs from selected edge".to_owned(),
                ));
            };
            let adjacent_edge_id = face_edge_ids[incident.face_index][adjacent_loop_index];
            let (neighbor_face, _) = edge_incidence
                .get(adjacent_edge_id)
                .expect("endpoint adjacent edge checked")
                .iter()
                .find(|(face_index, _)| *face_index != incident.face_index)
                .copied()
                .expect("endpoint adjacent face checked");
            adjacent.push((neighbor_face, adjacent_edge_id));
        }
        if adjacent[0].0 != adjacent[1].0 {
            return Err(GeometryError::Invalid(
                "bevel@2 P0 requires exactly three faces at each selected-edge endpoint".to_owned(),
            ));
        }
        let endpoint_face = adjacent[0].0;
        let endpoint_ids = &face_vertex_ids[endpoint_face];
        let endpoint_index = endpoint_ids
            .iter()
            .position(|id| *id == endpoint_id)
            .ok_or_else(|| {
                GeometryError::Invalid("bevel@2 endpoint face lost its source vertex".to_owned())
            })?;
        let previous_edge = face_edge_ids[endpoint_face]
            [(endpoint_index + endpoint_ids.len() - 1) % endpoint_ids.len()];
        let next_edge = face_edge_ids[endpoint_face][endpoint_index];
        let ordered_profile = if adjacent[0].1 == previous_edge && adjacent[1].1 == next_edge {
            endpoint_profile.clone()
        } else if adjacent[1].1 == previous_edge && adjacent[0].1 == next_edge {
            endpoint_profile.iter().rev().copied().collect()
        } else {
            return Err(GeometryError::Invalid(
                "bevel@2 endpoint face adjacency is unsupported".to_owned(),
            ));
        };
        endpoint_replacements.insert((endpoint_face, endpoint_id.to_owned()), ordered_profile);
    }

    let mut mesh = empty_mesh();
    for (face_index, ids) in face_vertex_ids.iter().enumerate() {
        let incident = incident_faces
            .iter()
            .find(|item| item.face_index == face_index);
        let mut points = Vec::with_capacity(ids.len() + segments);
        for id in ids {
            if let Some(replacement) = endpoint_replacements.get(&(face_index, (*id).to_owned())) {
                points.extend(replacement.iter().copied());
            } else {
                points.push(
                    incident
                        .and_then(|item| item.offset_by_vertex.get(*id).copied())
                        .unwrap_or_else(|| *vertex_by_id.get(*id).expect("face vertex checked")),
                );
            }
        }
        triangulate_convex_polygon(&mut mesh, &points)?;
    }

    for step in 0..segments {
        push_triangle(
            &mut mesh,
            profile_a[step],
            profile_a[step + 1],
            profile_b[step + 1],
        )?;
        push_triangle(
            &mut mesh,
            profile_a[step],
            profile_b[step + 1],
            profile_b[step],
        )?;
    }
    validate_closed_triangle_mesh(&mesh)?;
    Ok(mesh)
}

#[derive(Debug, Clone)]
struct SubdStepResult {
    positions: Vec<[f32; 3]>,
    faces: Vec<[usize; 4]>,
    sharpness: SubdSharpnessMap,
    input_edges: Vec<SubdEdge>,
    face_edges: Vec<[usize; 4]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubdRootElement {
    ControlVertex(usize),
    ControlEdge(usize),
    ControlQuad(usize),
}

impl SubdRootElement {
    fn wire(self) -> Value {
        match self {
            Self::ControlVertex(index) => json!(["control_vertex", index]),
            Self::ControlEdge(index) => json!(["control_edge", index]),
            Self::ControlQuad(index) => json!(["control_quad", index]),
        }
    }
}

type SubdSharpnessMap = BTreeMap<(usize, usize), u8>;

fn parse_subd_crease_edges(
    parameters: &Map<String, Value>,
    u_points: usize,
    v_points: usize,
    subdivision_levels: usize,
) -> Result<Vec<SubdCreaseEdge>, GeometryError> {
    let values = parameters
        .get("crease_edges")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            GeometryError::Invalid("subd-cage@2 crease_edges must be an array".to_owned())
        })?;
    if values.is_empty() || values.len() > MAX_SUBD_CREASE_EDGES {
        return Err(GeometryError::Invalid(format!(
            "subd-cage@2 crease_edges must contain 1..={MAX_SUBD_CREASE_EDGES} entries"
        )));
    }
    let vertex_count = u_points.checked_mul(v_points).ok_or_else(|| {
        GeometryError::Invalid("subd-cage@2 control vertex count overflow".to_owned())
    })?;
    let mut result = Vec::with_capacity(values.len());
    let mut previous: Option<(usize, usize)> = None;
    for value in values {
        let object = value.as_object().ok_or_else(|| {
            GeometryError::Invalid("subd-cage@2 crease edge must be an object".to_owned())
        })?;
        require_exact_keys(
            object,
            &["vertex_a", "vertex_b", "sharpness_levels"],
            "subd-cage@2 crease edge",
        )?;
        let vertex_a = bounded_count(object, "vertex_a", 0, vertex_count - 1)?;
        let vertex_b = bounded_count(object, "vertex_b", 0, vertex_count - 1)?;
        let sharpness_levels = bounded_count(object, "sharpness_levels", 1, 2)? as u8;
        if vertex_a >= vertex_b {
            return Err(GeometryError::Invalid(
                "subd-cage@2 crease endpoints must be strictly ascending".to_owned(),
            ));
        }
        let a_row = vertex_a / u_points;
        let a_column = vertex_a % u_points;
        let b_row = vertex_b / u_points;
        let b_column = vertex_b % u_points;
        let adjacent = (a_row == b_row && b_column == a_column + 1)
            || (a_column == b_column && b_row == a_row + 1);
        if !adjacent {
            return Err(GeometryError::Invalid(
                "subd-cage@2 crease endpoints must identify one control-grid edge".to_owned(),
            ));
        }
        let boundary = (a_row == b_row && (a_row == 0 || a_row + 1 == v_points))
            || (a_column == b_column && (a_column == 0 || a_column + 1 == u_points));
        if boundary {
            return Err(GeometryError::Invalid(
                "subd-cage@2 explicit boundary creases are redundant with the fixed sharp boundary-edge rule"
                    .to_owned(),
            ));
        }
        let key = (vertex_a, vertex_b);
        if previous.is_some_and(|prior| key <= prior) {
            return Err(GeometryError::Invalid(
                "subd-cage@2 crease edges must be unique and lexicographically sorted".to_owned(),
            ));
        }
        previous = Some(key);
        result.push(SubdCreaseEdge {
            vertex_a,
            vertex_b,
            sharpness_levels,
        });
    }
    if subdivision_levels == 0 {
        return Err(GeometryError::Invalid(
            "subd-cage@2 crease evaluation requires at least one subdivision level".to_owned(),
        ));
    }
    Ok(result)
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
    crease_edges: &[SubdCreaseEdge],
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

    let mut sharpness: SubdSharpnessMap = crease_edges
        .iter()
        .map(|edge| ((edge.vertex_a, edge.vertex_b), edge.sharpness_levels))
        .collect();
    for _ in 0..subdivision_levels {
        let step = subd_catmull_clark_step(&positions, &faces, &sharpness)?;
        positions = step.positions;
        faces = step.faces;
        sharpness = step.sharpness;
    }
    subd_mesh_from_quads(positions, &faces)
}

/// Return a compact, deterministic control-cage -> evaluated-quad-topology
/// lineage projection produced by the same fixed evaluator used to compile
/// `subd-cage@2`. Array position is the evaluated element ID; this avoids a
/// verbose object per element and keeps the 16x16/level-2 envelope below the
/// one-MiB Worker/MCP response ceiling.
pub(crate) fn subdivision_topology_lineage(
    operator: &ValidatedOperator,
) -> Result<Value, GeometryError> {
    let ValidatedOperator::SubdCage {
        control_points,
        u_points,
        v_points,
        subdivision_levels,
        crease_edges,
        ..
    } = operator
    else {
        return Err(GeometryError::Invalid(
            "subdivision lineage requires a subd-cage@2 operator".to_owned(),
        ));
    };
    if *subdivision_levels == 0 || crease_edges.is_empty() {
        return Err(GeometryError::Invalid(
            "subdivision lineage requires the crease-aware subd-cage@2 envelope".to_owned(),
        ));
    }

    let expected_points = u_points.checked_mul(*v_points).ok_or_else(|| {
        GeometryError::Invalid("subdivision lineage control count overflow".to_owned())
    })?;
    if control_points.len() != expected_points {
        return Err(GeometryError::Invalid(
            "subdivision lineage control cage is inconsistent".to_owned(),
        ));
    }

    let mut positions = control_points.clone();
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
    let control_quad_count = faces.len();
    let horizontal_edge_count = v_points * (u_points - 1);
    let control_edge_count = horizontal_edge_count + (v_points - 1) * u_points;
    let mut edge_roots = BTreeMap::<(usize, usize), SubdRootElement>::new();
    for v_index in 0..*v_points {
        for u_index in 0..u_points - 1 {
            let a = v_index * u_points + u_index;
            edge_roots.insert(
                (a, a + 1),
                SubdRootElement::ControlEdge(v_index * (u_points - 1) + u_index),
            );
        }
    }
    for v_index in 0..v_points - 1 {
        for u_index in 0..*u_points {
            let a = v_index * u_points + u_index;
            let b = a + u_points;
            edge_roots.insert(
                (a, b),
                SubdRootElement::ControlEdge(horizontal_edge_count + v_index * u_points + u_index),
            );
        }
    }
    if edge_roots.len() != control_edge_count {
        return Err(GeometryError::Invalid(
            "subdivision lineage control edge inventory differs".to_owned(),
        ));
    }
    let mut vertex_roots = (0..positions.len())
        .map(SubdRootElement::ControlVertex)
        .collect::<Vec<_>>();
    let mut face_roots = (0..faces.len()).collect::<Vec<_>>();
    let mut sharpness: SubdSharpnessMap = crease_edges
        .iter()
        .map(|edge| ((edge.vertex_a, edge.vertex_b), edge.sharpness_levels))
        .collect();

    for _ in 0..*subdivision_levels {
        let step = subd_catmull_clark_step(&positions, &faces, &sharpness)?;
        let vertex_count = positions.len();
        let edge_offset = vertex_count;
        let face_offset = edge_offset + step.input_edges.len();
        let mut next_vertex_roots = vertex_roots.clone();
        for edge in &step.input_edges {
            let key = (edge.a.min(edge.b), edge.a.max(edge.b));
            next_vertex_roots.push(*edge_roots.get(&key).ok_or_else(|| {
                GeometryError::Invalid("subdivision lineage input edge root is missing".to_owned())
            })?);
        }
        for root in &face_roots {
            next_vertex_roots.push(SubdRootElement::ControlQuad(*root));
        }
        if next_vertex_roots.len() != step.positions.len() {
            return Err(GeometryError::Invalid(
                "subdivision lineage evaluated vertex inventory differs".to_owned(),
            ));
        }

        let mut next_edge_roots = BTreeMap::<(usize, usize), SubdRootElement>::new();
        for (edge_index, edge) in step.input_edges.iter().enumerate() {
            let key = (edge.a.min(edge.b), edge.a.max(edge.b));
            let root = *edge_roots.get(&key).ok_or_else(|| {
                GeometryError::Invalid("subdivision lineage input edge root is missing".to_owned())
            })?;
            let edge_point = edge_offset + edge_index;
            for endpoint in [edge.a, edge.b] {
                let child = (endpoint.min(edge_point), endpoint.max(edge_point));
                if next_edge_roots.insert(child, root).is_some() {
                    return Err(GeometryError::Invalid(
                        "subdivision lineage child edge is duplicated".to_owned(),
                    ));
                }
            }
        }
        for (face_index, edge_ids) in step.face_edges.iter().enumerate() {
            let face_point = face_offset + face_index;
            let root = SubdRootElement::ControlQuad(face_roots[face_index]);
            for edge_index in edge_ids {
                let edge_point = edge_offset + *edge_index;
                let child = (edge_point.min(face_point), edge_point.max(face_point));
                match next_edge_roots.insert(child, root) {
                    None => {}
                    Some(existing) if existing == root => {}
                    Some(_) => {
                        return Err(GeometryError::Invalid(
                            "subdivision lineage internal edge root conflicts".to_owned(),
                        ));
                    }
                }
            }
        }
        let mut next_face_roots = Vec::with_capacity(face_roots.len() * 4);
        for root in &face_roots {
            next_face_roots.extend([*root; 4]);
        }
        if next_face_roots.len() != step.faces.len() {
            return Err(GeometryError::Invalid(
                "subdivision lineage evaluated quad inventory differs".to_owned(),
            ));
        }
        positions = step.positions;
        faces = step.faces;
        sharpness = step.sharpness;
        vertex_roots = next_vertex_roots;
        edge_roots = next_edge_roots;
        face_roots = next_face_roots;
    }

    // Run the exact mesh conversion as a fail-closed proof that the lineage
    // topology is the same non-degenerate topology accepted by compilation.
    let mesh = subd_mesh_from_quads(positions, &faces)?;
    let mut evaluated_edge_keys = BTreeSet::<(usize, usize)>::new();
    for face in &faces {
        for (a, b) in [
            (face[0], face[1]),
            (face[1], face[2]),
            (face[2], face[3]),
            (face[3], face[0]),
        ] {
            evaluated_edge_keys.insert((a.min(b), a.max(b)));
        }
    }
    if evaluated_edge_keys.len() != edge_roots.len() {
        return Err(GeometryError::Invalid(
            "subdivision lineage evaluated edge inventory differs".to_owned(),
        ));
    }
    let mut control_edge_descendants = vec![Vec::<usize>::new(); control_edge_count];
    let mut evaluated_edge_origins = Vec::with_capacity(evaluated_edge_keys.len());
    for (evaluated_edge_id, key) in evaluated_edge_keys.iter().enumerate() {
        let root = *edge_roots.get(key).ok_or_else(|| {
            GeometryError::Invalid("subdivision lineage final edge root is missing".to_owned())
        })?;
        if let SubdRootElement::ControlEdge(control_edge_id) = root {
            control_edge_descendants[control_edge_id].push(evaluated_edge_id);
        }
        evaluated_edge_origins.push(root.wire());
    }
    let expected_chain_length = 1usize << subdivision_levels;
    if control_edge_descendants
        .iter()
        .any(|chain| chain.len() != expected_chain_length)
    {
        return Err(GeometryError::Invalid(
            "subdivision lineage control-edge chain length differs".to_owned(),
        ));
    }

    let descendants_per_quad = 4usize.pow(*subdivision_levels as u32);
    let mut control_quad_ranges = Vec::with_capacity(control_quad_count);
    for control_quad_id in 0..control_quad_count {
        let start = control_quad_id * descendants_per_quad;
        if face_roots
            .get(start..start + descendants_per_quad)
            .is_none_or(|roots| roots.iter().any(|root| *root != control_quad_id))
        {
            return Err(GeometryError::Invalid(
                "subdivision lineage control-quad range is not contiguous".to_owned(),
            ));
        }
        control_quad_ranges.push(json!({
            "evaluated_quad_start":start,
            "evaluated_quad_count":descendants_per_quad,
            "evaluated_triangle_start":start * 2,
            "evaluated_triangle_count":descendants_per_quad * 2
        }));
    }
    let control_crease_edge_ids = crease_edges
        .iter()
        .map(|crease| {
            let key = (crease.vertex_a, crease.vertex_b);
            match edge_roots_for_control_key(*u_points, *v_points, key) {
                Some(id) => Ok(json!({
                    "control_edge_id":id,
                    "sharpness_levels":crease.sharpness_levels,
                    "evaluated_edge_ids":control_edge_descendants[id]
                })),
                None => Err(GeometryError::Invalid(
                    "subdivision lineage crease is not a control-grid edge".to_owned(),
                )),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(json!({
        "control_dimensions":{"u_points":u_points,"v_points":v_points},
        "subdivision_levels":subdivision_levels,
        "control_counts":{"vertex_count":control_points.len(),"edge_count":control_edge_count,"quad_count":control_quad_count},
        "evaluated_counts":{"vertex_count":vertex_roots.len(),"edge_count":evaluated_edge_keys.len(),"quad_count":faces.len(),"triangle_count":mesh.indices.len() / 3},
        "control_vertex_to_evaluated_vertex_ids":(0..control_points.len()).collect::<Vec<_>>(),
        "control_edge_to_evaluated_edge_ids":control_edge_descendants,
        "control_quad_descendant_ranges":control_quad_ranges,
        "control_crease_edge_chains":control_crease_edge_ids,
        "evaluated_vertex_root_origins":vertex_roots.into_iter().map(SubdRootElement::wire).collect::<Vec<_>>(),
        "evaluated_edge_root_origins":evaluated_edge_origins,
        "evaluated_quad_control_quad_ids":face_roots,
        "quad_triangulation":"0-1-2_0-2-3"
    }))
}

fn edge_roots_for_control_key(
    u_points: usize,
    v_points: usize,
    key: (usize, usize),
) -> Option<usize> {
    let (a, b) = key;
    let a_row = a / u_points;
    let a_column = a % u_points;
    let b_row = b / u_points;
    let b_column = b % u_points;
    if a_row == b_row && b_column == a_column + 1 && a_row < v_points {
        return Some(a_row * (u_points - 1) + a_column);
    }
    if a_column == b_column && b_row == a_row + 1 && b_row < v_points {
        return Some(v_points * (u_points - 1) + a_row * u_points + a_column);
    }
    None
}

fn subd_catmull_clark_step(
    positions: &[[f32; 3]],
    faces: &[[usize; 4]],
    sharpness: &SubdSharpnessMap,
) -> Result<SubdStepResult, GeometryError> {
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
            let key = (edge.a.min(edge.b), edge.a.max(edge.b));
            if edge.faces.len() == 1 || sharpness.get(&key).copied().unwrap_or(0) > 0 {
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
        if !boundary_edges.is_empty() && boundary_edges.len() != 2 {
            return Err(GeometryError::Invalid(
                "subd-cage boundary valence is not supported".to_owned(),
            ));
        }
        let sharp_neighbors: Vec<usize> = vertex_edges[vertex_index]
            .iter()
            .filter_map(|edge_index| {
                let edge = &edges[*edge_index];
                let key = (edge.a.min(edge.b), edge.a.max(edge.b));
                (edge.faces.len() == 1 || sharpness.get(&key).copied().unwrap_or(0) > 0).then_some(
                    if edge.a == vertex_index {
                        edge.b
                    } else {
                        edge.a
                    },
                )
            })
            .collect();
        let next = if sharp_neighbors.len() >= 3 {
            position
        } else if sharp_neighbors.len() == 2 {
            scale3(
                add3(
                    scale3(position, 6.0),
                    add3(positions[sharp_neighbors[0]], positions[sharp_neighbors[1]]),
                ),
                0.125,
            )
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
    let mut next_sharpness = SubdSharpnessMap::new();
    for (edge_index, edge) in edges.iter().enumerate() {
        let key = (edge.a.min(edge.b), edge.a.max(edge.b));
        let child_sharpness = sharpness.get(&key).copied().unwrap_or(0).saturating_sub(1);
        if child_sharpness == 0 {
            continue;
        }
        let edge_point = edge_offset + edge_index;
        for endpoint in [edge.a, edge.b] {
            let child_key = (endpoint.min(edge_point), endpoint.max(edge_point));
            if next_sharpness.insert(child_key, child_sharpness).is_some() {
                return Err(GeometryError::Invalid(
                    "subd-cage@2 crease propagation produced a duplicate child edge".to_owned(),
                ));
            }
        }
    }
    Ok(SubdStepResult {
        positions: next_positions,
        faces: next_faces,
        sharpness: next_sharpness,
        input_edges: edges,
        face_edges,
    })
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

/// Emit one closed annular cylinder around the local Y axis. The explicit
/// inner wall keeps every ring a real watertight solid.
fn annular_cylinder_mesh(
    outer_radius_m: f32,
    inner_radius_m: f32,
    depth_m: f32,
    radial_segments: usize,
) -> Result<PrimitiveNodeMesh, GeometryError> {
    if !(outer_radius_m > inner_radius_m
        && inner_radius_m > 1.0e-5
        && depth_m > 1.0e-5
        && (12..=64).contains(&radial_segments))
    {
        return Err(GeometryError::Invalid(
            "energy-core annular cylinder dimensions are invalid".to_owned(),
        ));
    }
    let half_depth = depth_m / 2.0;
    let point = |radius: f32, y: f32, segment: usize| {
        let angle = std::f32::consts::TAU * segment as f32 / radial_segments as f32;
        [radius * angle.cos(), y, radius * angle.sin()]
    };
    let mut mesh = empty_mesh();
    for segment in 0..radial_segments {
        let next = (segment + 1) % radial_segments;
        let outer_bottom = point(outer_radius_m, -half_depth, segment);
        let outer_top = point(outer_radius_m, half_depth, segment);
        let outer_bottom_next = point(outer_radius_m, -half_depth, next);
        let outer_top_next = point(outer_radius_m, half_depth, next);
        let inner_bottom = point(inner_radius_m, -half_depth, segment);
        let inner_top = point(inner_radius_m, half_depth, segment);
        let inner_bottom_next = point(inner_radius_m, -half_depth, next);
        let inner_top_next = point(inner_radius_m, half_depth, next);

        push_triangle(&mut mesh, outer_bottom, outer_top, outer_bottom_next)?;
        push_triangle(&mut mesh, outer_bottom_next, outer_top, outer_top_next)?;
        push_triangle(&mut mesh, inner_bottom, inner_bottom_next, inner_top)?;
        push_triangle(&mut mesh, inner_bottom_next, inner_top_next, inner_top)?;

        push_triangle(&mut mesh, outer_top, inner_top, inner_top_next)?;
        push_triangle(&mut mesh, outer_top, inner_top_next, outer_top_next)?;
        push_triangle(
            &mut mesh,
            outer_bottom,
            outer_bottom_next,
            inner_bottom_next,
        )?;
        push_triangle(&mut mesh, outer_bottom, inner_bottom_next, inner_bottom)?;
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
    use std::collections::{BTreeMap, BTreeSet};

    fn crease_fixture_points() -> Vec<[f32; 3]> {
        vec![
            [-1.0, -1.0, 0.0],
            [0.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [-1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ]
    }

    #[test]
    fn subd_integer_edge_creases_apply_dart_crease_corner_and_level_decay_masks() {
        let points = crease_fixture_points();
        let smooth = subd_cage_mesh(&points, 3, 3, 1, &[]).expect("smooth level one");
        assert!((smooth.positions[4][2] - 0.5625).abs() < 1.0e-6);

        let dart = subd_cage_mesh(
            &points,
            3,
            3,
            1,
            &[SubdCreaseEdge {
                vertex_a: 3,
                vertex_b: 4,
                sharpness_levels: 1,
            }],
        )
        .expect("dart level one");
        assert!((dart.positions[4][2] - 0.5625).abs() < 1.0e-6);

        let crease_edges = [
            SubdCreaseEdge {
                vertex_a: 3,
                vertex_b: 4,
                sharpness_levels: 1,
            },
            SubdCreaseEdge {
                vertex_a: 4,
                vertex_b: 5,
                sharpness_levels: 1,
            },
        ];
        let crease = subd_cage_mesh(&points, 3, 3, 1, &crease_edges).expect("crease level one");
        assert!((crease.positions[4][2] - 0.75).abs() < 1.0e-6);

        let corner = subd_cage_mesh(
            &points,
            3,
            3,
            1,
            &[
                SubdCreaseEdge {
                    vertex_a: 1,
                    vertex_b: 4,
                    sharpness_levels: 1,
                },
                SubdCreaseEdge {
                    vertex_a: 3,
                    vertex_b: 4,
                    sharpness_levels: 1,
                },
                SubdCreaseEdge {
                    vertex_a: 4,
                    vertex_b: 5,
                    sharpness_levels: 1,
                },
            ],
        )
        .expect("three-edge corner level one");
        assert!((corner.positions[4][2] - 1.0).abs() < 1.0e-6);

        let mut boundary_junction_points = crease_fixture_points();
        boundary_junction_points[1][2] = 1.0;
        boundary_junction_points[4][2] = 0.0;
        let boundary_junction = subd_cage_mesh(
            &boundary_junction_points,
            3,
            3,
            1,
            &[SubdCreaseEdge {
                vertex_a: 1,
                vertex_b: 4,
                sharpness_levels: 1,
            }],
        )
        .expect("boundary plus interior crease junction");
        assert!((boundary_junction.positions[1][2] - 1.0).abs() < 1.0e-6);

        let one_level = subd_cage_mesh(&points, 3, 3, 2, &crease_edges)
            .expect("one-level sharpness after two subdivisions");
        let two_level_edges = crease_edges.map(|mut edge| {
            edge.sharpness_levels = 2;
            edge
        });
        let two_level = subd_cage_mesh(&points, 3, 3, 2, &two_level_edges)
            .expect("two-level sharpness after two subdivisions");
        assert!((one_level.positions[4][2] - 0.601_562_5).abs() < 1.0e-6);
        assert!((two_level.positions[4][2] - 0.6875).abs() < 1.0e-6);
        assert_ne!(one_level.positions, two_level.positions);
    }

    #[test]
    fn subd_crease_operator_rejects_ambiguous_or_redundant_edges() {
        let base = json!({
            "shape":"subd-cage",
            "control_points":crease_fixture_points(),
            "u_points":3,
            "v_points":3,
            "subdivision_levels":2,
            "crease_method":"uniform-integer-level-decay@1",
            "crease_edges":[
                {"vertex_a":1,"vertex_b":4,"sharpness_levels":2},
                {"vertex_a":3,"vertex_b":4,"sharpness_levels":1}
            ],
            "position_m":[0.0,0.0,0.0],
            "rotation_rad":[0.0,0.0,0.0]
        });
        let (validated, triangle_count) = validate_operator(
            "forgecad.geometry.subd-cage@2",
            &[],
            base.as_object().expect("parameters"),
            &BTreeMap::new(),
        )
        .expect("valid crease operator");
        assert_eq!(triangle_count, 128);
        let mesh = compile_operator(
            &validated,
            &BTreeMap::new(),
            &BTreeMap::new(),
            250_000,
            10_000,
        )
        .expect("actual crease mesh");
        assert_eq!(mesh.indices.len() / 3, 128);

        for invalid_edges in [
            json!([{"vertex_a":0,"vertex_b":1,"sharpness_levels":1}]),
            json!([{"vertex_a":1,"vertex_b":7,"sharpness_levels":1}]),
            json!([
                {"vertex_a":3,"vertex_b":4,"sharpness_levels":1},
                {"vertex_a":3,"vertex_b":4,"sharpness_levels":2}
            ]),
            json!([
                {"vertex_a":3,"vertex_b":4,"sharpness_levels":1},
                {"vertex_a":1,"vertex_b":4,"sharpness_levels":2}
            ]),
            json!([{"vertex_a":3,"vertex_b":4,"sharpness_levels":0}]),
            json!([{"vertex_a":3,"vertex_b":4,"sharpness_levels":3}]),
            json!([{"vertex_a":4,"vertex_b":3,"sharpness_levels":1}]),
        ] {
            let mut invalid = base.clone();
            invalid["crease_edges"] = invalid_edges;
            assert!(validate_operator(
                "forgecad.geometry.subd-cage@2",
                &[],
                invalid.as_object().expect("parameters"),
                &BTreeMap::new(),
            )
            .is_err());
        }
        let mut too_small = base.clone();
        too_small["control_points"] = json!([
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [-1.0, 1.0, 0.0],
            [1.0, 1.0, 0.0]
        ]);
        too_small["u_points"] = json!(2);
        too_small["v_points"] = json!(2);
        assert!(validate_operator(
            "forgecad.geometry.subd-cage@2",
            &[],
            too_small.as_object().expect("parameters"),
            &BTreeMap::new(),
        )
        .is_err());
        let mut too_many = base.clone();
        too_many["crease_edges"] = Value::Array(
            (0..129)
                .map(|_| json!({"vertex_a":3,"vertex_b":4,"sharpness_levels":1}))
                .collect(),
        );
        assert!(validate_operator(
            "forgecad.geometry.subd-cage@2",
            &[],
            too_many.as_object().expect("parameters"),
            &BTreeMap::new(),
        )
        .is_err());
        let mut unknown = base;
        unknown["script"] = json!("bpy.ops.mesh.subdivide() ");
        assert!(validate_operator(
            "forgecad.geometry.subd-cage@2",
            &[],
            unknown.as_object().expect("parameters"),
            &BTreeMap::new(),
        )
        .is_err());
    }

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
    fn normal_policy_uses_face_area_times_blender_face_angle_weight() {
        let source = PrimitiveNodeMesh {
            operator_id: "fixture".to_owned(),
            lineage_source_node_ids: vec!["source".to_owned()],
            positions: vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.866_025_4, 0.0, 0.5],
            ],
            normals: vec![[0.0, 0.0, 1.0]; 6],
            indices: vec![0, 1, 2, 3, 4, 5],
        };
        let result = area_angle_corner_normals(&source, std::f32::consts::FRAC_PI_2)
            .expect("weighted corner normals");
        let shared_corner = result.normals[0];
        // First face: twice-area 2 * face-angle PI/2. Second face:
        // twice-area 0.5 * face-angle 5PI/6. Their expected -Y/Z ratio is 5/12.
        assert!((shared_corner[1] / shared_corner[2] + 5.0 / 12.0).abs() < 1.0e-4);
        assert_eq!(result.lineage_source_node_ids, vec!["source"]);
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
        let rounded_mesh = compile_operator(
            &rounded,
            &BTreeMap::new(),
            &BTreeMap::new(),
            250_000,
            10_000,
        )
        .expect("rounded mesh");
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
            compile_operator(&plain, &BTreeMap::new(), &BTreeMap::new(), 250_000, 10_000)
                .expect("plain mesh");
        assert_eq!(plain_mesh.indices.len() / 3, 12);
    }

    #[test]
    fn panel_v2_emits_real_recess_border_bevel_and_support_loops() {
        let parameters = json!({
            "shape": "panel",
            "size_m": [2.4, 1.6, 0.4],
            "thickness_m": 0.4,
            "inset_m": 0.25,
            "recess_depth_m": 0.12,
            "border_width_m": 0.18,
            "bevel_m": 0.08,
            "bevel_segments": 2,
            "support_loop_count": 2,
            "support_loop_width_m": 0.03,
            "position_m": [0.0, 0.0, 0.0],
            "rotation_rad": [0.0, 0.0, 0.0]
        });
        let (operation, triangle_count) = validate_operator(
            "forgecad.geometry.panel@2",
            &[],
            parameters.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .expect("panel@2 should validate");
        assert!(
            triangle_count > 92,
            "topology-safe edge subdivisions were not emitted"
        );
        let mesh = compile_operator(
            &operation,
            &BTreeMap::new(),
            &BTreeMap::new(),
            250_000,
            10_000,
        )
        .expect("panel@2 mesh");
        assert_eq!(mesh.indices.len() / 3, triangle_count as usize);
        assert!(mesh
            .positions
            .iter()
            .any(|point| (point[2] - 0.08).abs() < 1.0e-6));
        assert!(mesh
            .positions
            .iter()
            .any(|point| (point[2] - 0.20).abs() < 1.0e-6));
        assert!(mesh
            .positions
            .iter()
            .any(|point| (point[2] + 0.20).abs() < 1.0e-6));
        let distinct_x = mesh
            .positions
            .iter()
            .map(|point| point[0].abs().to_bits())
            .collect::<BTreeSet<_>>();
        assert!(distinct_x.len() >= 8, "nested panel rings were not emitted");
        assert!(mesh.normals.iter().all(|normal| finite3(*normal)));
    }

    #[test]
    fn panel_v2_rejects_unknown_fields_and_invalid_feature_relationships() {
        let base = json!({
            "shape": "panel",
            "size_m": [2.4, 1.6, 0.4],
            "thickness_m": 0.4,
            "inset_m": 0.25,
            "recess_depth_m": 0.12,
            "border_width_m": 0.18,
            "bevel_m": 0.08,
            "bevel_segments": 2,
            "support_loop_count": 2,
            "support_loop_width_m": 0.03,
            "position_m": [0.0, 0.0, 0.0],
            "rotation_rad": [0.0, 0.0, 0.0]
        });
        let mut unknown = base.clone();
        unknown["script"] = json!("bpy.ops.mesh.inset()");
        assert!(validate_operator(
            "forgecad.geometry.panel@2",
            &[],
            unknown.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());

        let mut invalid_recess = base.clone();
        invalid_recess["recess_depth_m"] = json!(0.4);
        assert!(validate_operator(
            "forgecad.geometry.panel@2",
            &[],
            invalid_recess.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());

        let mut invalid_support = base.clone();
        invalid_support["support_loop_width_m"] = json!(0.2);
        assert!(validate_operator(
            "forgecad.geometry.panel@2",
            &[],
            invalid_support.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());

        let mut invalid_segments = base;
        invalid_segments["bevel_segments"] = json!(5);
        assert!(validate_operator(
            "forgecad.geometry.panel@2",
            &[],
            invalid_segments.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());
    }

    #[test]
    fn panel_v2_long_narrow_production_profile_remains_closed() {
        let parameters = json!({
            "shape":"panel",
            "size_m":[1.3,0.25,0.1],
            "thickness_m":0.1,
            "inset_m":0.045,
            "recess_depth_m":0.02,
            "border_width_m":0.035,
            "bevel_m":0.015,
            "bevel_segments":1,
            "support_loop_count":1,
            "support_loop_width_m":0.015,
            "position_m":[0.62,1.78,0.47],
            "rotation_rad":[0.0,0.0,-0.04]
        });
        let (operation, triangle_count) = validate_operator(
            "forgecad.geometry.panel@2",
            &[],
            parameters.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .expect("long narrow panel should validate");
        let mesh = compile_operator(
            &operation,
            &BTreeMap::new(),
            &BTreeMap::new(),
            250_000,
            10_000,
        )
        .expect("long narrow panel mesh");
        assert_eq!(mesh.indices.len() / 3, triangle_count as usize);
        validate_closed_triangle_mesh(&mesh).expect("long narrow panel must remain closed");
    }

    #[test]
    fn vent_array_v1_remains_compatible_with_legacy_box_array_semantics() {
        let parameters = json!({
            "shape": "vent-array",
            "width_m": 1.2,
            "height_m": 0.6,
            "depth_m": 0.18,
            "slot_count": 4,
            "slot_width_m": 0.12,
            "slot_spacing_m": 0.12,
            "position_m": [0.0, 0.0, 0.0],
            "rotation_rad": [0.0, 0.0, 0.0]
        });
        let (operation, triangle_count) = validate_operator(
            "forgecad.geometry.vent-array@1",
            &[],
            parameters.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .expect("legacy vent-array@1 should validate");
        assert_eq!(triangle_count, 72);
        let mesh = compile_operator(
            &operation,
            &BTreeMap::new(),
            &BTreeMap::new(),
            250_000,
            10_000,
        )
        .expect("legacy vent-array@1 mesh");
        assert_eq!(mesh.indices.len() / 3, 72);
    }

    #[test]
    fn vent_array_v2_emits_cut_style_openings_and_backing_layer() {
        let parameters = json!({
            "shape": "vent-array",
            "width_m": 1.6,
            "height_m": 0.8,
            "depth_m": 0.26,
            "face_thickness_m": 0.08,
            "backing_depth_m": 0.08,
            "backing_gap_m": 0.10,
            "slot_count": 4,
            "slot_width_m": 0.16,
            "slot_spacing_m": 0.12,
            "slot_margin_m": 0.16,
            "slot_edge_bevel_m": 0.02,
            "bevel_segments": 2,
            "position_m": [0.0, 0.0, 0.0],
            "rotation_rad": [0.0, 0.0, 0.0]
        });
        let (operation, triangle_count) = validate_operator(
            "forgecad.geometry.vent-array@2",
            &[],
            parameters.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .expect("vent-array@2 should validate");
        assert_eq!(triangle_count, 312);
        let mesh = compile_operator(
            &operation,
            &BTreeMap::new(),
            &BTreeMap::new(),
            250_000,
            10_000,
        )
        .expect("vent-array@2 mesh");
        assert_eq!(mesh.indices.len() / 3, triangle_count as usize);
        type WeldKey = (i64, i64, i64);
        let weld = |point: [f32; 3]| -> WeldKey {
            (
                (point[0] as f64 * 1_000_000.0).round() as i64,
                (point[1] as f64 * 1_000_000.0).round() as i64,
                (point[2] as f64 * 1_000_000.0).round() as i64,
            )
        };
        let mut shell_adjacency = BTreeMap::<WeldKey, BTreeSet<WeldKey>>::new();
        for triangle in mesh.indices.chunks_exact(3) {
            let points = [
                mesh.positions[triangle[0] as usize],
                mesh.positions[triangle[1] as usize],
                mesh.positions[triangle[2] as usize],
            ];
            if points.iter().all(|point| point[2] > -0.045) {
                let keys = points.map(weld);
                for (left, right) in [(keys[0], keys[1]), (keys[1], keys[2]), (keys[2], keys[0])] {
                    shell_adjacency.entry(left).or_default().insert(right);
                    shell_adjacency.entry(right).or_default().insert(left);
                }
            }
        }
        let mut visited = BTreeSet::new();
        let mut component_count = 0;
        for start in shell_adjacency.keys().copied() {
            if !visited.insert(start) {
                continue;
            }
            component_count += 1;
            let mut stack = vec![start];
            while let Some(current) = stack.pop() {
                for neighbor in shell_adjacency
                    .get(&current)
                    .into_iter()
                    .flat_map(|neighbors| neighbors.iter())
                    .copied()
                {
                    if visited.insert(neighbor) {
                        stack.push(neighbor);
                    }
                }
            }
        }
        assert_eq!(
            component_count, 1,
            "slotted face must be one connected shell"
        );
        let mut all_adjacency = BTreeMap::<WeldKey, BTreeSet<WeldKey>>::new();
        for triangle in mesh.indices.chunks_exact(3) {
            let keys = [
                weld(mesh.positions[triangle[0] as usize]),
                weld(mesh.positions[triangle[1] as usize]),
                weld(mesh.positions[triangle[2] as usize]),
            ];
            for (left, right) in [(keys[0], keys[1]), (keys[1], keys[2]), (keys[2], keys[0])] {
                all_adjacency.entry(left).or_default().insert(right);
                all_adjacency.entry(right).or_default().insert(left);
            }
        }
        let mut all_visited = BTreeSet::new();
        let mut all_component_count = 0;
        for start in all_adjacency.keys().copied() {
            if !all_visited.insert(start) {
                continue;
            }
            all_component_count += 1;
            let mut stack = vec![start];
            while let Some(current) = stack.pop() {
                for neighbor in all_adjacency
                    .get(&current)
                    .into_iter()
                    .flat_map(|neighbors| neighbors.iter())
                    .copied()
                {
                    if all_visited.insert(neighbor) {
                        stack.push(neighbor);
                    }
                }
            }
        }
        assert_eq!(
            all_component_count, 2,
            "one connected face shell plus one closed backing sub-solid is expected"
        );
        let depth_m = 0.26_f32;
        let face_thickness_m = 0.08_f32;
        let backing_gap_m = 0.10_f32;
        let backing_front_z = depth_m / 2.0 - face_thickness_m - backing_gap_m;
        let face_front_z = depth_m / 2.0;
        assert!(mesh
            .positions
            .iter()
            .any(|point| point[2] > face_front_z - 0.005));
        assert!(mesh
            .positions
            .iter()
            .any(|point| point[2] < -depth_m / 2.0 + 0.005));

        let slot_count = 4_usize;
        let slot_width_m = 0.16_f32;
        let slot_spacing_m = 0.12_f32;
        let slot_margin_m = 0.16_f32;
        let occupied_width =
            slot_count as f32 * slot_width_m + (slot_count - 1) as f32 * slot_spacing_m;
        let first_center = -occupied_width / 2.0 + slot_width_m / 2.0;
        let slot_centers = (0..slot_count)
            .map(|index| first_center + index as f32 * (slot_width_m + slot_spacing_m))
            .collect::<Vec<_>>();
        let opening_half_height = 0.8_f32 / 2.0 - slot_margin_m;
        let bbox_overlaps = |triangle: &[u32], x_min: f32, x_max: f32| {
            let mut min_point = [f32::INFINITY; 3];
            let mut max_point = [f32::NEG_INFINITY; 3];
            for index in triangle {
                let point = mesh.positions[*index as usize];
                for axis in 0..3 {
                    min_point[axis] = min_point[axis].min(point[axis]);
                    max_point[axis] = max_point[axis].max(point[axis]);
                }
            }
            min_point[0] < x_max
                && max_point[0] > x_min
                && min_point[1] < opening_half_height - 0.005
                && max_point[1] > -opening_half_height + 0.005
        };
        let ray_z_intersections = |x: f32| {
            let mut hits = Vec::new();
            for triangle in mesh.indices.chunks_exact(3) {
                let points = [
                    mesh.positions[triangle[0] as usize],
                    mesh.positions[triangle[1] as usize],
                    mesh.positions[triangle[2] as usize],
                ];
                let denominator = (points[1][1] - points[2][1]) * (points[0][0] - points[2][0])
                    + (points[2][0] - points[1][0]) * (points[0][1] - points[2][1]);
                if denominator.abs() <= 1.0e-8 {
                    continue;
                }
                let u = ((points[1][1] - points[2][1]) * (x - points[2][0])
                    + (points[2][0] - points[1][0]) * (0.0 - points[2][1]))
                    / denominator;
                let v = ((points[2][1] - points[0][1]) * (x - points[2][0])
                    + (points[0][0] - points[2][0]) * (0.0 - points[2][1]))
                    / denominator;
                let w = 1.0 - u - v;
                if u >= -1.0e-5 && v >= -1.0e-5 && w >= -1.0e-5 {
                    hits.push(u * points[0][2] + v * points[1][2] + w * points[2][2]);
                }
            }
            hits.sort_by(|left, right| left.partial_cmp(right).expect("finite z hit"));
            hits
        };
        let face_back_z = depth_m / 2.0 - face_thickness_m;
        for center in slot_centers {
            let x_min = center - slot_width_m / 2.0 + 0.005;
            let x_max = center + slot_width_m / 2.0 - 0.005;
            let has_front_triangle_in_slot = mesh.indices.chunks_exact(3).any(|triangle| {
                bbox_overlaps(triangle, x_min, x_max)
                    && triangle
                        .iter()
                        .all(|index| mesh.positions[*index as usize][2] > face_front_z - 0.005)
            });
            assert!(
                !has_front_triangle_in_slot,
                "front frame must leave a real cut-style opening at slot center {center}"
            );
            let has_backing_triangle_in_slot = mesh.indices.chunks_exact(3).any(|triangle| {
                bbox_overlaps(triangle, x_min, x_max)
                    && triangle
                        .iter()
                        .all(|index| mesh.positions[*index as usize][2] < backing_front_z + 0.005)
            });
            assert!(
                has_backing_triangle_in_slot,
                "backing layer is missing at slot center {center}"
            );
            let ray_hits = ray_z_intersections(center);
            assert!(
                ray_hits.iter().any(|z| *z <= backing_front_z + 0.005),
                "front-to-back slot ray must hit the backing at slot center {center}"
            );
            assert!(
                ray_hits.iter().all(|z| {
                    *z <= backing_front_z + 0.005 || *z >= face_back_z - 0.005
                }),
                "front-to-back slot ray must remain clear between backing and face at slot center {center}"
            );
        }
    }

    #[test]
    fn vent_array_v2_rejects_unknown_fields_spacing_and_backing_drift() {
        let base = json!({
            "shape": "vent-array",
            "width_m": 1.6,
            "height_m": 0.8,
            "depth_m": 0.26,
            "face_thickness_m": 0.08,
            "backing_depth_m": 0.08,
            "backing_gap_m": 0.10,
            "slot_count": 4,
            "slot_width_m": 0.16,
            "slot_spacing_m": 0.12,
            "slot_margin_m": 0.16,
            "slot_edge_bevel_m": 0.02,
            "bevel_segments": 2,
            "position_m": [0.0, 0.0, 0.0],
            "rotation_rad": [0.0, 0.0, 0.0]
        });
        let mut unknown = base.clone();
        unknown["script"] = json!("bpy.ops.mesh.boolean()");
        assert!(validate_operator(
            "forgecad.geometry.vent-array@2",
            &[],
            unknown.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());

        let mut overfull = base.clone();
        overfull["slot_spacing_m"] = json!(0.30);
        assert!(validate_operator(
            "forgecad.geometry.vent-array@2",
            &[],
            overfull.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());

        let mut backing_drift = base.clone();
        backing_drift["backing_gap_m"] = json!(0.30);
        assert!(validate_operator(
            "forgecad.geometry.vent-array@2",
            &[],
            backing_drift.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());

        let mut bevel_drift = base.clone();
        bevel_drift["slot_edge_bevel_m"] = json!(0.05);
        assert!(validate_operator(
            "forgecad.geometry.vent-array@2",
            &[],
            bevel_drift.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());

        let mut narrow_slot = base.clone();
        narrow_slot["slot_width_m"] = json!(0.02);
        assert!(validate_operator(
            "forgecad.geometry.vent-array@2",
            &[],
            narrow_slot.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());

        let mut wrong_input = base;
        assert!(validate_operator(
            "forgecad.geometry.vent-array@2",
            &["source".to_owned()],
            wrong_input.as_object_mut().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());
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
        let mesh = compile_operator(
            &operation,
            &BTreeMap::new(),
            &BTreeMap::new(),
            250_000,
            10_000,
        )
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
        let mesh = compile_operator(
            &operation,
            &BTreeMap::new(),
            &BTreeMap::new(),
            250_000,
            10_000,
        )
        .expect("revolve mesh");
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

    fn recessed_channel_fixture() -> Value {
        json!({
            "shape":"recessed-channel",
            "stations":[
                {"point_m":[-0.8,0.0,0.0],"width_m":0.30,"depth_m":0.12},
                {"point_m":[0.0,0.08,0.0],"width_m":0.36,"depth_m":0.16},
                {"point_m":[0.82,0.0,0.0],"width_m":0.28,"depth_m":0.10}
            ],
            "path_frame":"planar-xy-z-up@1",
            "floor_width_ratio":0.42,
            "edge_bevel_m":0.01,
            "start_transition_m":0.08,
            "end_transition_m":0.10,
            "transition_segments":2,
            "position_m":[0.0,0.0,0.0],
            "rotation_rad":[0.0,0.0,0.0]
        })
    }

    #[test]
    fn recessed_channel_is_closed_variable_and_bevel_transition_aware() {
        let parameters = recessed_channel_fixture();
        let (operation, triangle_count) = validate_operator(
            "forgecad.geometry.recessed-channel@1",
            &[],
            parameters.as_object().expect("recessed-channel parameters"),
            &BTreeMap::new(),
        )
        .expect("recessed-channel should validate");
        assert_eq!(triangle_count, 220);
        let mesh = compile_operator(
            &operation,
            &BTreeMap::new(),
            &BTreeMap::new(),
            250_000,
            10_000,
        )
        .expect("recessed-channel mesh");
        assert_eq!(mesh.indices.len() / 3, 220);
        assert!(mesh.positions.iter().all(|point| finite3(*point)));
        let min_z = mesh
            .positions
            .iter()
            .map(|point| point[2])
            .fold(f32::INFINITY, f32::min);
        assert!(
            min_z < -0.15,
            "depth and floor must be represented in geometry"
        );
        assert!(
            mesh.positions.iter().any(|point| point[0].abs() > 0.17),
            "station width variation must affect the generated mesh"
        );
    }

    #[test]
    fn recessed_channel_rejects_bad_path_depth_bevel_and_unknown_fields() {
        let base = recessed_channel_fixture();
        let mut invalid = base.clone();
        invalid["stations"][1]["point_m"] = json!([-0.8, 0.0, 0.0]);
        assert!(validate_operator(
            "forgecad.geometry.recessed-channel@1",
            &[],
            invalid.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());

        let mut self_intersection = base.clone();
        self_intersection["stations"] = json!([
            {"point_m":[-0.8,-0.4,0.0],"width_m":0.3,"depth_m":0.1},
            {"point_m":[0.8,0.4,0.0],"width_m":0.3,"depth_m":0.1},
            {"point_m":[-0.8,0.4,0.0],"width_m":0.3,"depth_m":0.1},
            {"point_m":[0.8,-0.4,0.0],"width_m":0.3,"depth_m":0.1}
        ]);
        assert!(validate_operator(
            "forgecad.geometry.recessed-channel@1",
            &[],
            self_intersection.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());

        let mut bad_depth = base.clone();
        bad_depth["stations"][0]["depth_m"] = json!(0.23);
        assert!(validate_operator(
            "forgecad.geometry.recessed-channel@1",
            &[],
            bad_depth.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());

        let mut bad_bevel = base.clone();
        bad_bevel["edge_bevel_m"] = json!(0.08);
        assert!(validate_operator(
            "forgecad.geometry.recessed-channel@1",
            &[],
            bad_bevel.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());

        let mut near_reverse = base.clone();
        near_reverse["stations"] = json!([
            {"point_m":[-1.0,0.0,0.0],"width_m":0.30,"depth_m":0.10},
            {"point_m":[0.0,0.0,0.0],"width_m":0.30,"depth_m":0.10},
            {"point_m":[-0.99,0.01,0.0],"width_m":0.30,"depth_m":0.10}
        ]);
        let error = validate_operator(
            "forgecad.geometry.recessed-channel@1",
            &[],
            near_reverse.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .expect_err("near-reverse path must fail closed");
        assert!(error.to_string().contains("near-reverse"), "{error}");

        let mut collinear_overlap = base.clone();
        collinear_overlap["stations"] = json!([
            {"point_m":[-2.0,0.0,0.0],"width_m":0.30,"depth_m":0.10},
            {"point_m":[-1.0,0.0,0.0],"width_m":0.30,"depth_m":0.10},
            {"point_m":[-1.0,1.0,0.0],"width_m":0.30,"depth_m":0.10},
            {"point_m":[-1.5,1.0,0.0],"width_m":0.30,"depth_m":0.10},
            {"point_m":[-1.5,0.0,0.0],"width_m":0.30,"depth_m":0.10},
            {"point_m":[-0.5,0.0,0.0],"width_m":0.30,"depth_m":0.10}
        ]);
        let error = validate_operator(
            "forgecad.geometry.recessed-channel@1",
            &[],
            collinear_overlap.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .expect_err("collinear non-adjacent overlap must fail closed");
        assert!(error.to_string().contains("self-intersects"), "{error}");

        let mut swept_overlap = base.clone();
        swept_overlap["stations"] = json!([
            {"point_m":[0.0,0.0,0.0],"width_m":0.30,"depth_m":0.10},
            {"point_m":[1.0,0.0,0.0],"width_m":0.30,"depth_m":0.10},
            {"point_m":[1.0,1.0,0.0],"width_m":0.30,"depth_m":0.10},
            {"point_m":[-1.0,1.0,0.0],"width_m":0.30,"depth_m":0.10},
            {"point_m":[-1.0,0.2,0.0],"width_m":0.30,"depth_m":0.10},
            {"point_m":[0.0,0.2,0.0],"width_m":0.30,"depth_m":0.10}
        ]);
        let error = validate_operator(
            "forgecad.geometry.recessed-channel@1",
            &[],
            swept_overlap.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .expect_err("swept-envelope overlap must fail closed");
        assert!(error.to_string().contains("swept envelope"), "{error}");

        let mut unknown = base;
        unknown["script"] = json!("bpy.ops.mesh.inset()");
        assert!(validate_operator(
            "forgecad.geometry.recessed-channel@1",
            &[],
            unknown.as_object().expect("object parameters"),
            &BTreeMap::new(),
        )
        .is_err());
    }
}
