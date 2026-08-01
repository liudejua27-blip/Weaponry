//! FGC-VP203 typed high-level geometry for the Visual Program v2 route.
//!
//! The source is bounded data. Rust validates profiles, feature topology and
//! static budgets before lowering only to operations already declared by the
//! ShapeProgram runtime manifest. Geometry remains owned by the restricted
//! worker; this module owns source truth, deterministic IDs and lineage.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    compiled_visual_base_material_id, normalize_persisted_shape_program, semantic_sha256,
    CoreError, CoreResult, ForgeVisualMaterialV2, ForgeVisualUnitSystemV2,
};

pub const FORGE_VISUAL_GEOMETRY_PROGRAM_SCHEMA_VERSION: &str = "ForgeVisualGeometryProgram@2";
pub const EXPANDED_VISUAL_GEOMETRY_DAG_SCHEMA_VERSION: &str = "ExpandedVisualGeometryDAG@1";
pub const FORGE_VISUAL_GEOMETRY_LOWERING_SCHEMA_VERSION: &str = "ForgeVisualGeometryLowering@1";
pub const VP203_COMPILER_VERSION: &str = "forgecad-core-vp203.1";
pub const VP203_ID_ALGORITHM_VERSION: &str = "geometry-source-path-v1";
const MAX_LATTICE_CORNER_OFFSET_RATIO: f64 = 0.25;

fn invalid(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message.into())
}

fn valid_id(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix)
        && value.len() > prefix.len()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
}

fn require_id(value: &str, prefix: &str) -> CoreResult<()> {
    if valid_id(value, prefix) {
        Ok(())
    } else {
        Err(invalid(
            "FORGE_VISUAL_VP203_ID_INVALID",
            format!("ID must match the bounded lowercase {prefix} set"),
        ))
    }
}

fn require_role(value: &str) -> CoreResult<()> {
    if (2..=64).contains(&value.len())
        && value.starts_with(|character: char| character.is_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
    {
        Ok(())
    } else {
        Err(invalid(
            "FORGE_VISUAL_VP203_ROLE_INVALID",
            "part role is outside the ShapeProgram role set",
        ))
    }
}

fn finite(values: impl IntoIterator<Item = f64>) -> bool {
    values.into_iter().all(f64::is_finite)
}

fn zero_rotation() -> [f64; 3] {
    [0.0, 0.0, 0.0]
}

fn is_zero_rotation(value: &[f64; 3]) -> bool {
    value.iter().all(|item| item.abs() <= 1e-12)
}

fn validate_rotation(value: &[f64; 3]) -> CoreResult<()> {
    if !finite(value.iter().copied())
        || value
            .iter()
            .any(|item| item.abs() > std::f64::consts::PI + 1e-9)
    {
        return Err(invalid(
            "FORGE_VISUAL_VP203_ROTATION_INVALID",
            "static Euler rotation must be finite and remain within one bounded turn",
        ));
    }
    Ok(())
}

fn insert_rotation(operation: &mut Value, rotation: [f64; 3]) {
    if is_zero_rotation(&rotation) {
        return;
    }
    if let Some(args) = operation.get_mut("args").and_then(Value::as_object_mut) {
        args.insert("rotation".into(), json!(rotation));
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GeometryAxisV2 {
    X,
    Y,
    Z,
}

impl GeometryAxisV2 {
    fn vector(self) -> [f64; 3] {
        match self {
            Self::X => [1.0, 0.0, 0.0],
            Self::Y => [0.0, 1.0, 0.0],
            Self::Z => [0.0, 0.0, 1.0],
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GeometryCapPolicyV2 {
    None,
    Start,
    End,
}

/// `surface_panel` is intentionally more constrained than a general transform.
/// The restricted worker supports only one of the six axis-aligned local faces;
/// source programs cannot claim arbitrary face projection or a free transform
/// that the executor cannot reproduce.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SurfacePanelAxisV2 {
    PositiveX,
    NegativeX,
    PositiveY,
    NegativeY,
    PositiveZ,
    NegativeZ,
}

impl SurfacePanelAxisV2 {
    fn vector(self) -> [f64; 3] {
        match self {
            Self::PositiveX => [1.0, 0.0, 0.0],
            Self::NegativeX => [-1.0, 0.0, 0.0],
            Self::PositiveY => [0.0, 1.0, 0.0],
            Self::NegativeY => [0.0, -1.0, 0.0],
            Self::PositiveZ => [0.0, 0.0, 1.0],
            Self::NegativeZ => [0.0, 0.0, -1.0],
        }
    }

    fn normal_index(self) -> usize {
        match self {
            Self::PositiveX | Self::NegativeX => 0,
            Self::PositiveY | Self::NegativeY => 1,
            Self::PositiveZ | Self::NegativeZ => 2,
        }
    }

    fn face_indices(self) -> [usize; 2] {
        match self {
            Self::PositiveX | Self::NegativeX => [1, 2],
            Self::PositiveY | Self::NegativeY => [0, 2],
            Self::PositiveZ | Self::NegativeZ => [0, 1],
        }
    }
}

impl GeometryCapPolicyV2 {
    fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Start => "start",
            Self::End => "end",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GeometryProfileV2 {
    pub profile_id: String,
    pub points: Vec<[f64; 2]>,
    pub resample_count: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GeometrySectionV2 {
    pub section_id: String,
    pub position: f64,
    pub profile_id: String,
    pub scale: f64,
    pub twist_degrees: f64,
    pub cap_policy: GeometryCapPolicyV2,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GeometrySectionSetV2 {
    pub section_set_id: String,
    pub main_axis: GeometryAxisV2,
    pub sections: Vec<GeometrySectionV2>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HighLevelGeometryNodeV2 {
    Box {
        node_id: String,
        size: [f64; 3],
        position: [f64; 3],
        #[serde(default = "zero_rotation", skip_serializing_if = "is_zero_rotation")]
        rotation: [f64; 3],
    },
    Cylinder {
        node_id: String,
        radius: f64,
        height: f64,
        axis: GeometryAxisV2,
        position: [f64; 3],
        #[serde(default = "zero_rotation", skip_serializing_if = "is_zero_rotation")]
        rotation: [f64; 3],
    },
    Capsule {
        node_id: String,
        radius: f64,
        height: f64,
        axis: GeometryAxisV2,
        position: [f64; 3],
        #[serde(default = "zero_rotation", skip_serializing_if = "is_zero_rotation")]
        rotation: [f64; 3],
    },
    Wedge {
        node_id: String,
        size: [f64; 3],
        position: [f64; 3],
        #[serde(default = "zero_rotation", skip_serializing_if = "is_zero_rotation")]
        rotation: [f64; 3],
    },
    Extrude {
        node_id: String,
        profile_id: String,
        profile_scale: [f64; 2],
        height: f64,
        position: [f64; 3],
        cap_start: bool,
        cap_end: bool,
        #[serde(default = "zero_rotation", skip_serializing_if = "is_zero_rotation")]
        rotation: [f64; 3],
    },
    Revolve {
        node_id: String,
        profile_id: String,
        profile_scale: [f64; 2],
        angle: f64,
        radial_segments: u16,
        position: [f64; 3],
        #[serde(default = "zero_rotation", skip_serializing_if = "is_zero_rotation")]
        rotation: [f64; 3],
    },
    Loft {
        node_id: String,
        section_set_id: String,
        cross_section_scale: [f64; 2],
        axis_length: f64,
        position: [f64; 3],
        #[serde(default = "zero_rotation", skip_serializing_if = "is_zero_rotation")]
        rotation: [f64; 3],
    },
    Sweep {
        node_id: String,
        profile_id: String,
        profile_scale: [f64; 2],
        path_points: Vec<[f64; 3]>,
        path_closed: bool,
        path_twist_degrees: f64,
        cap_start: bool,
        cap_end: bool,
        position: [f64; 3],
        #[serde(default = "zero_rotation", skip_serializing_if = "is_zero_rotation")]
        rotation: [f64; 3],
    },
    Mirror {
        node_id: String,
        input_node_id: String,
        axis: GeometryAxisV2,
    },
    Array {
        node_id: String,
        input_node_id: String,
        axis: GeometryAxisV2,
        count: u16,
        spacing: f64,
    },
    RadialArray {
        node_id: String,
        input_node_id: String,
        axis: GeometryAxisV2,
        count: u16,
        radius: f64,
        angle: f64,
    },
    BevelApprox {
        node_id: String,
        input_node_id: String,
        radius: f64,
        segments: u8,
    },
    SurfacePanel {
        node_id: String,
        input_node_id: String,
        size: [f64; 3],
        position: [f64; 3],
        axis: SurfacePanelAxisV2,
    },
    /// A bounded shallow recess on one axis-aligned source face.  Lowering
    /// expands this ergonomic node to one sealed cutter box plus one
    /// `subtract`; it is not an arbitrary boolean or provider-authored mesh.
    Groove {
        node_id: String,
        input_node_id: String,
        face_size: [f64; 2],
        position: [f64; 3],
        axis: SurfacePanelAxisV2,
        depth: f64,
    },
    /// A bounded closed shell produced from one box or bevelled box by a local
    /// CSG subtraction. It is intentionally narrower than a general CAD
    /// shell/offset feature.
    Shell {
        node_id: String,
        input_node_id: String,
        thickness: f64,
    },
    /// A fixed 2x2x2 trilinear cage.  Offsets are relative to the source
    /// AABB, bounded to retain a local, topology-preserving deformation.
    LatticeDeform {
        node_id: String,
        input_node_id: String,
        corner_offsets: [[f64; 3]; 8],
    },
    /// A bounded normalized local patch over an earlier mesh. The worker
    /// preserves topology and provenance while applying a smooth local
    /// displacement; no imported mesh bytes are accepted here.
    LocalMeshPatch {
        node_id: String,
        input_node_id: String,
        patch_center: [f64; 3],
        patch_radius: f64,
        patch_offset: [f64; 3],
    },
    Union {
        node_id: String,
        input_node_ids: Vec<String>,
    },
    Subtract {
        node_id: String,
        input_node_ids: Vec<String>,
    },
    Part {
        node_id: String,
        input_node_id: String,
        part_id: String,
        role: String,
    },
    MaterialZone {
        node_id: String,
        input_node_id: String,
        zone_id: String,
        material_id: String,
    },
}

impl HighLevelGeometryNodeV2 {
    fn node_id(&self) -> &str {
        match self {
            Self::Box { node_id, .. }
            | Self::Cylinder { node_id, .. }
            | Self::Capsule { node_id, .. }
            | Self::Wedge { node_id, .. }
            | Self::Extrude { node_id, .. }
            | Self::Revolve { node_id, .. }
            | Self::Loft { node_id, .. }
            | Self::Sweep { node_id, .. }
            | Self::Mirror { node_id, .. }
            | Self::Array { node_id, .. }
            | Self::RadialArray { node_id, .. }
            | Self::BevelApprox { node_id, .. }
            | Self::SurfacePanel { node_id, .. }
            | Self::Groove { node_id, .. }
            | Self::Shell { node_id, .. }
            | Self::LatticeDeform { node_id, .. }
            | Self::LocalMeshPatch { node_id, .. }
            | Self::Union { node_id, .. }
            | Self::Subtract { node_id, .. }
            | Self::Part { node_id, .. }
            | Self::MaterialZone { node_id, .. } => node_id,
        }
    }

    fn inputs(&self) -> Vec<&str> {
        match self {
            Self::Mirror { input_node_id, .. }
            | Self::Array { input_node_id, .. }
            | Self::RadialArray { input_node_id, .. }
            | Self::BevelApprox { input_node_id, .. }
            | Self::SurfacePanel { input_node_id, .. }
            | Self::Groove { input_node_id, .. }
            | Self::Shell { input_node_id, .. }
            | Self::LatticeDeform { input_node_id, .. }
            | Self::LocalMeshPatch { input_node_id, .. }
            | Self::Part { input_node_id, .. }
            | Self::MaterialZone { input_node_id, .. } => vec![input_node_id],
            Self::Union { input_node_ids, .. } | Self::Subtract { input_node_ids, .. } => {
                input_node_ids.iter().map(String::as_str).collect()
            }
            _ => Vec::new(),
        }
    }

    fn is_geometry(&self) -> bool {
        !matches!(self, Self::Part { .. } | Self::MaterialZone { .. })
    }

    fn rotation(&self) -> Option<[f64; 3]> {
        match self {
            Self::Box { rotation, .. }
            | Self::Cylinder { rotation, .. }
            | Self::Capsule { rotation, .. }
            | Self::Wedge { rotation, .. }
            | Self::Extrude { rotation, .. }
            | Self::Revolve { rotation, .. }
            | Self::Loft { rotation, .. }
            | Self::Sweep { rotation, .. } => Some(*rotation),
            _ => None,
        }
    }
}

fn detail_source_size(
    node_id: &str,
    nodes: &BTreeMap<&str, &HighLevelGeometryNodeV2>,
) -> Option<[f64; 3]> {
    match nodes.get(node_id)? {
        HighLevelGeometryNodeV2::Box { size, .. } => Some(*size),
        HighLevelGeometryNodeV2::BevelApprox { input_node_id, .. } => {
            detail_source_size(input_node_id, nodes)
        }
        HighLevelGeometryNodeV2::Shell { input_node_id, .. } => {
            detail_source_size(input_node_id, nodes)
        }
        HighLevelGeometryNodeV2::Groove { input_node_id, .. } => {
            detail_source_size(input_node_id, nodes)
        }
        _ => None,
    }
}

fn detail_source_position(
    node_id: &str,
    nodes: &BTreeMap<&str, &HighLevelGeometryNodeV2>,
) -> Option<[f64; 3]> {
    match nodes.get(node_id)? {
        HighLevelGeometryNodeV2::Box { position, .. } => Some(*position),
        HighLevelGeometryNodeV2::BevelApprox { input_node_id, .. }
        | HighLevelGeometryNodeV2::Groove { input_node_id, .. } => {
            detail_source_position(input_node_id, nodes)
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HighLevelGeometryOutputV2 {
    pub output_id: String,
    pub node_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HighLevelGeometryBudgetV2 {
    pub schema_version: String,
    pub max_profiles: u16,
    pub max_section_sets: u16,
    pub max_nodes: u16,
    pub max_parts: u16,
    pub max_materials: u16,
    pub max_outputs: u16,
    pub max_operations: u16,
    pub triangle_budget: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualGeometryProgramV2 {
    pub schema_version: String,
    pub program_id: String,
    pub domain: String,
    pub units: ForgeVisualUnitSystemV2,
    pub seed: u32,
    pub materials: Vec<ForgeVisualMaterialV2>,
    pub profiles: Vec<GeometryProfileV2>,
    #[serde(default)]
    pub section_sets: Vec<GeometrySectionSetV2>,
    pub nodes: Vec<HighLevelGeometryNodeV2>,
    pub outputs: Vec<HighLevelGeometryOutputV2>,
    pub budgets: HighLevelGeometryBudgetV2,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualGeometryBudgetEvidenceV2 {
    pub profile_count: u16,
    pub section_set_count: u16,
    pub node_count: u16,
    pub part_count: u16,
    pub output_count: u16,
    pub operation_count: u16,
    pub estimated_triangle_upper_bound: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExpandedVisualGeometryNodeLineageV2 {
    pub source_node_id: String,
    pub expanded_node_id: String,
    pub source_macro_path: Vec<String>,
    pub instance_indices: Vec<u16>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExpandedVisualGeometryDagV2 {
    pub schema_version: String,
    pub compiler_version: String,
    pub id_algorithm_version: String,
    pub source_program_sha256: String,
    pub expanded_program_sha256: String,
    pub lineage_sha256: String,
    pub expanded_dag_sha256: String,
    pub budget_evidence: VisualGeometryBudgetEvidenceV2,
    pub lineage: Vec<ExpandedVisualGeometryNodeLineageV2>,
    pub expanded_program: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualGeometrySourceMapEntryV2 {
    pub output_id: String,
    pub source_node_ids: Vec<String>,
    pub expanded_node_ids: Vec<String>,
    pub shape_operation_ids: Vec<String>,
    pub terminal_operation_id: String,
    pub part_id: String,
    pub material_zone_id: String,
    pub authored_material_id: String,
    pub compiled_material_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualGeometryLoweringV2 {
    pub schema_version: String,
    pub compiler_version: String,
    pub source_program_sha256: String,
    pub expanded_dag: ExpandedVisualGeometryDagV2,
    pub source_map_sha256: String,
    pub source_map: Vec<VisualGeometrySourceMapEntryV2>,
    pub shape_program_sha256: String,
    pub shape_program: Value,
}

fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn segments_intersect(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let ab_c = cross(a, b, c);
    let ab_d = cross(a, b, d);
    let cd_a = cross(c, d, a);
    let cd_b = cross(c, d, b);
    ab_c * ab_d < -1e-12 && cd_a * cd_b < -1e-12
}

fn path_self_intersects(points: &[[f64; 3]]) -> bool {
    for first in 0..points.len().saturating_sub(1) {
        for second in first + 2..points.len().saturating_sub(1) {
            if second == first + 1 {
                continue;
            }
            for (u, v, dropped) in [(0, 1, 2), (0, 2, 1), (1, 2, 0)] {
                let plane = [
                    points[first][dropped],
                    points[first + 1][dropped],
                    points[second][dropped],
                    points[second + 1][dropped],
                ];
                if plane.iter().all(|value| (*value - plane[0]).abs() <= 1e-9)
                    && segments_intersect(
                        [points[first][u], points[first][v]],
                        [points[first + 1][u], points[first + 1][v]],
                        [points[second][u], points[second][v]],
                        [points[second + 1][u], points[second + 1][v]],
                    )
                {
                    return true;
                }
            }
        }
    }
    false
}

fn validate_profile(profile: &GeometryProfileV2) -> CoreResult<()> {
    require_id(&profile.profile_id, "profile_")?;
    if !(3..=32).contains(&profile.points.len()) || !(8..=256).contains(&profile.resample_count) {
        return Err(invalid(
            "FORGE_VISUAL_VP203_PROFILE_BOUNDS",
            "profile point and resample counts must remain bounded",
        ));
    }
    if profile
        .points
        .iter()
        .flatten()
        .any(|value| !value.is_finite() || !(-1.0..=1.0).contains(value))
    {
        return Err(invalid(
            "FORGE_VISUAL_VP203_PROFILE_BOUNDS",
            "normalized profile points must remain within -1..=1",
        ));
    }
    let count = profile.points.len();
    for first in 0..count {
        let first_next = (first + 1) % count;
        for second in first + 1..count {
            let second_next = (second + 1) % count;
            if first == second || first_next == second || second_next == first {
                continue;
            }
            if segments_intersect(
                profile.points[first],
                profile.points[first_next],
                profile.points[second],
                profile.points[second_next],
            ) {
                return Err(invalid(
                    "FORGE_VISUAL_VP203_PROFILE_SELF_INTERSECTION",
                    "profile contour must not self-intersect",
                ));
            }
        }
    }
    let area = profile
        .points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let next = profile.points[(index + 1) % profile.points.len()];
            point[0] * next[1] - next[0] * point[1]
        })
        .sum::<f64>()
        * 0.5;
    if area <= 1e-9 {
        return Err(invalid(
            "FORGE_VISUAL_VP203_PROFILE_WINDING_OR_DEGENERATE",
            "profile must be closed implicitly, non-degenerate and counter-clockwise",
        ));
    }
    Ok(())
}

fn profile_payload(profile: &GeometryProfileV2) -> Value {
    let min_x = profile
        .points
        .iter()
        .map(|point| point[0])
        .fold(f64::INFINITY, f64::min);
    let min_y = profile
        .points
        .iter()
        .map(|point| point[1])
        .fold(f64::INFINITY, f64::min);
    let max_x = profile
        .points
        .iter()
        .map(|point| point[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = profile
        .points
        .iter()
        .map(|point| point[1])
        .fold(f64::NEG_INFINITY, f64::max);
    let start = profile.points[0];
    let segments = profile
        .points
        .iter()
        .skip(1)
        .chain(std::iter::once(&profile.points[0]))
        .map(|point| json!({"kind": "line", "to": point}))
        .collect::<Vec<_>>();
    json!({
        "schema_version": "ProfileSketch@1",
        "sketch_id": format!("sketch_{}", profile.profile_id.strip_prefix("profile_").unwrap()),
        "version": 1,
        "plane": "cross_section",
        "closed": true,
        "winding": "counter_clockwise",
        "start": start,
        "segments": segments,
        "holes": [],
        "normalized_bounds": {"min": [min_x, min_y], "max": [max_x, max_y]},
        "symmetry": "none",
        "continuity_hint": "linear",
        "resample_count": profile.resample_count,
        "provenance": {"source": "agent", "source_ref": "vp203_typed_geometry"}
    })
}

fn checked_add(left: u32, right: u32) -> CoreResult<u32> {
    left.checked_add(right).ok_or_else(|| {
        invalid(
            "FORGE_VISUAL_VP203_BUDGET_EXCEEDED",
            "triangle estimate overflowed",
        )
    })
}

impl ForgeVisualGeometryProgramV2 {
    pub fn parse_and_validate(value: &Value) -> CoreResult<(Self, VisualGeometryBudgetEvidenceV2)> {
        let program: Self = serde_json::from_value(value.clone()).map_err(|error| {
            invalid(
                "FORGE_VISUAL_VP203_PARSE_FAILED",
                format!("high-level geometry source failed closed: {error}"),
            )
        })?;
        let evidence = program.validate()?;
        Ok((program, evidence))
    }

    pub fn validate(&self) -> CoreResult<VisualGeometryBudgetEvidenceV2> {
        if self.schema_version != FORGE_VISUAL_GEOMETRY_PROGRAM_SCHEMA_VERSION {
            return Err(invalid(
                "FORGE_VISUAL_VP203_SCHEMA_VERSION",
                "schema_version must be ForgeVisualGeometryProgram@2",
            ));
        }
        require_id(&self.program_id, "visual_")?;
        if self.domain.is_empty() || self.domain.len() > 96 || self.seed > i32::MAX as u32 {
            return Err(invalid(
                "FORGE_VISUAL_VP203_HEADER_INVALID",
                "domain or seed is outside bounds",
            ));
        }
        if self.budgets.schema_version != "GeometryProgramBudget@1"
            || self.budgets.max_profiles == 0
            || self.budgets.max_profiles > 32
            || self.budgets.max_section_sets > 16
            || self.budgets.max_nodes == 0
            || self.budgets.max_nodes > 256
            || self.budgets.max_parts == 0
            || self.budgets.max_parts > 128
            || self.budgets.max_materials == 0
            || self.budgets.max_materials > 64
            || self.budgets.max_outputs == 0
            || self.budgets.max_outputs > 128
            || self.budgets.max_operations == 0
            || self.budgets.max_operations > 256
            || !(100..=100_000).contains(&self.budgets.triangle_budget)
        {
            return Err(invalid(
                "FORGE_VISUAL_VP203_BUDGET_INVALID",
                "declared geometry budgets exceed reviewed ceilings",
            ));
        }
        if self.profiles.len() > self.budgets.max_profiles as usize
            || self.section_sets.len() > self.budgets.max_section_sets as usize
            || self.nodes.is_empty()
            || self.nodes.len() > self.budgets.max_nodes as usize
            || self.materials.is_empty()
            || self.materials.len() > self.budgets.max_materials as usize
            || self.outputs.is_empty()
            || self.outputs.len() > self.budgets.max_outputs as usize
        {
            return Err(invalid(
                "FORGE_VISUAL_VP203_BUDGET_EXCEEDED",
                "source cardinality exceeds declared budgets",
            ));
        }
        let mut profile_ids = BTreeSet::new();
        let mut profiles = BTreeMap::new();
        for profile in &self.profiles {
            validate_profile(profile)?;
            if !profile_ids.insert(profile.profile_id.as_str()) {
                return Err(invalid(
                    "FORGE_VISUAL_VP203_DUPLICATE_ID",
                    "profile IDs must be unique",
                ));
            }
            profiles.insert(profile.profile_id.as_str(), profile);
        }
        let mut section_set_ids = BTreeSet::new();
        let mut section_sets = BTreeMap::new();
        for set in &self.section_sets {
            require_id(&set.section_set_id, "sectionset_")?;
            if !section_set_ids.insert(set.section_set_id.as_str())
                || !(2..=12).contains(&set.sections.len())
            {
                return Err(invalid(
                    "FORGE_VISUAL_VP203_SECTION_SET_INVALID",
                    "section set IDs and counts must be bounded",
                ));
            }
            let mut previous = f64::NEG_INFINITY;
            let mut section_ids = BTreeSet::new();
            let mut sample_count = None;
            for section in &set.sections {
                require_id(&section.section_id, "section_")?;
                require_id(&section.profile_id, "profile_")?;
                let profile = profiles.get(section.profile_id.as_str()).ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_VP203_REFERENCE_MISSING",
                        "section references an unknown profile",
                    )
                })?;
                if !section_ids.insert(section.section_id.as_str())
                    || !section.position.is_finite()
                    || !(-1.0..=1.0).contains(&section.position)
                    || section.position <= previous
                    || !section.scale.is_finite()
                    || !(0.25..=4.0).contains(&section.scale)
                    || !section.twist_degrees.is_finite()
                    || !(-45.0..=45.0).contains(&section.twist_degrees)
                {
                    return Err(invalid(
                        "FORGE_VISUAL_VP203_SECTION_SET_INVALID",
                        "sections must be unique, ordered and bounded",
                    ));
                }
                if sample_count
                    .replace(profile.resample_count)
                    .is_some_and(|count| count != profile.resample_count)
                {
                    return Err(invalid(
                        "FORGE_VISUAL_VP203_SECTION_RESAMPLE_MISMATCH",
                        "loft sections must share one resample count",
                    ));
                }
                previous = section.position;
            }
            if set
                .sections
                .first()
                .is_some_and(|item| item.cap_policy != GeometryCapPolicyV2::Start)
                || set
                    .sections
                    .last()
                    .is_some_and(|item| item.cap_policy != GeometryCapPolicyV2::End)
                || set
                    .sections
                    .iter()
                    .skip(1)
                    .take(set.sections.len().saturating_sub(2))
                    .any(|item| item.cap_policy != GeometryCapPolicyV2::None)
            {
                return Err(invalid(
                    "FORGE_VISUAL_VP203_SECTION_CAP_INVALID",
                    "loft caps must be start/none/end",
                ));
            }
            section_sets.insert(set.section_set_id.as_str(), set);
        }
        let mut material_ids = BTreeSet::new();
        for material in &self.materials {
            require_id(&material.material_id, "mat_")?;
            require_id(&material.base_material_id, "mat_")?;
            if compiled_visual_base_material_id(&material.base_material_id).is_none() {
                return Err(invalid(
                    "FORGE_VISUAL_VP203_CAPABILITY_DENIED",
                    "material is outside reviewed PBR capability",
                ));
            }
            if !material_ids.insert(material.material_id.as_str()) {
                return Err(invalid(
                    "FORGE_VISUAL_VP203_DUPLICATE_ID",
                    "material IDs must be unique",
                ));
            }
        }

        let mut node_ids = BTreeSet::new();
        let mut nodes = BTreeMap::<&str, &HighLevelGeometryNodeV2>::new();
        let mut triangles = BTreeMap::<&str, u32>::new();
        let mut csg_depths = BTreeMap::<&str, u8>::new();
        let mut operation_count = 0_u32;
        let mut part_ids = BTreeSet::new();
        let mut zone_ids = BTreeSet::new();
        for node in &self.nodes {
            require_id(node.node_id(), "node_")?;
            if !node_ids.insert(node.node_id()) {
                return Err(invalid(
                    "FORGE_VISUAL_VP203_DUPLICATE_ID",
                    "node IDs must be unique",
                ));
            }
            for input in node.inputs() {
                if !nodes.contains_key(input) {
                    return Err(invalid(
                        "FORGE_VISUAL_VP203_FORWARD_OR_MISSING_REFERENCE",
                        "feature inputs must reference earlier nodes",
                    ));
                }
            }
            if let Some(rotation) = node.rotation() {
                validate_rotation(&rotation)?;
            }
            match node {
                HighLevelGeometryNodeV2::Part { input_node_id, .. }
                    if !nodes[input_node_id.as_str()].is_geometry() =>
                {
                    return Err(invalid(
                        "FORGE_VISUAL_VP203_GEOMETRY_INPUT_INVALID",
                        "Part must wrap geometry",
                    ));
                }
                HighLevelGeometryNodeV2::MaterialZone { input_node_id, .. }
                    if !matches!(
                        nodes[input_node_id.as_str()],
                        HighLevelGeometryNodeV2::Part { .. }
                    ) =>
                {
                    return Err(invalid(
                        "FORGE_VISUAL_VP203_GEOMETRY_INPUT_INVALID",
                        "MaterialZone must wrap Part",
                    ));
                }
                HighLevelGeometryNodeV2::Mirror { input_node_id, .. }
                | HighLevelGeometryNodeV2::Array { input_node_id, .. }
                | HighLevelGeometryNodeV2::RadialArray { input_node_id, .. }
                | HighLevelGeometryNodeV2::BevelApprox { input_node_id, .. }
                | HighLevelGeometryNodeV2::SurfacePanel { input_node_id, .. }
                | HighLevelGeometryNodeV2::Groove { input_node_id, .. }
                | HighLevelGeometryNodeV2::Shell { input_node_id, .. }
                | HighLevelGeometryNodeV2::LatticeDeform { input_node_id, .. }
                    if !nodes[input_node_id.as_str()].is_geometry() =>
                {
                    return Err(invalid(
                        "FORGE_VISUAL_VP203_GEOMETRY_INPUT_INVALID",
                        "transform feature must reference geometry",
                    ));
                }
                HighLevelGeometryNodeV2::Union { input_node_ids, .. }
                | HighLevelGeometryNodeV2::Subtract { input_node_ids, .. }
                    if input_node_ids
                        .iter()
                        .any(|input| !nodes[input.as_str()].is_geometry()) =>
                {
                    return Err(invalid(
                        "FORGE_VISUAL_VP203_GEOMETRY_INPUT_INVALID",
                        "boolean operands must be geometry",
                    ));
                }
                _ => {}
            }
            let estimate = match node {
                HighLevelGeometryNodeV2::Box { size, position, .. } => {
                    if !finite(size.iter().copied().chain(position.iter().copied()))
                        || size.iter().any(|value| *value <= 0.0 || *value > 100_000.0)
                        || position.iter().any(|value| value.abs() > 100_000.0)
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_DIMENSION_INVALID",
                            "box dimensions are outside bounds",
                        ));
                    }
                    operation_count += 1;
                    12
                }
                HighLevelGeometryNodeV2::Cylinder {
                    radius,
                    height,
                    position,
                    ..
                }
                | HighLevelGeometryNodeV2::Capsule {
                    radius,
                    height,
                    position,
                    ..
                } => {
                    if !finite(
                        std::iter::once(*radius)
                            .chain(std::iter::once(*height))
                            .chain(position.iter().copied()),
                    ) || !(*radius > 0.0 && *radius <= 50_000.0)
                        || !(*height > 0.0 && *height <= 100_000.0)
                        || position.iter().any(|value| value.abs() > 100_000.0)
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_DIMENSION_INVALID",
                            "cylinder/capsule dimensions are outside bounds",
                        ));
                    }
                    operation_count += 1;
                    if matches!(node, HighLevelGeometryNodeV2::Cylinder { .. }) {
                        256
                    } else {
                        432
                    }
                }
                HighLevelGeometryNodeV2::Wedge { size, position, .. } => {
                    if !finite(size.iter().copied().chain(position.iter().copied()))
                        || size.iter().any(|value| *value <= 0.0 || *value > 100_000.0)
                        || position.iter().any(|value| value.abs() > 100_000.0)
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_DIMENSION_INVALID",
                            "wedge dimensions are outside bounds",
                        ));
                    }
                    operation_count += 1;
                    12
                }
                HighLevelGeometryNodeV2::Extrude {
                    profile_id,
                    profile_scale,
                    height,
                    position,
                    ..
                } => {
                    let profile = profiles.get(profile_id.as_str()).ok_or_else(|| {
                        invalid(
                            "FORGE_VISUAL_VP203_REFERENCE_MISSING",
                            "extrude profile is missing",
                        )
                    })?;
                    if !finite(
                        profile_scale
                            .iter()
                            .copied()
                            .chain(std::iter::once(*height))
                            .chain(position.iter().copied()),
                    ) || profile_scale
                        .iter()
                        .any(|value| *value <= 0.0 || *value > 100_000.0)
                        || *height <= 0.0
                        || *height > 100_000.0
                        || position.iter().any(|value| value.abs() > 100_000.0)
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_DIMENSION_INVALID",
                            "extrude dimensions are outside bounds",
                        ));
                    }
                    operation_count += 2;
                    u32::from(profile.resample_count) * 4
                }
                HighLevelGeometryNodeV2::Revolve {
                    profile_id,
                    profile_scale,
                    angle,
                    radial_segments,
                    position,
                    ..
                } => {
                    let profile = profiles.get(profile_id.as_str()).ok_or_else(|| {
                        invalid(
                            "FORGE_VISUAL_VP203_REFERENCE_MISSING",
                            "revolve profile is missing",
                        )
                    })?;
                    if profile.points.iter().any(|point| point[0] < 0.0)
                        || !finite(
                            profile_scale
                                .iter()
                                .copied()
                                .chain(std::iter::once(*angle))
                                .chain(position.iter().copied()),
                        )
                        || profile_scale
                            .iter()
                            .any(|value| *value <= 0.0 || *value > 100_000.0)
                        || *angle <= 0.0
                        || *angle > std::f64::consts::TAU
                        || !(8..=64).contains(radial_segments)
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_REVOLVE_INVALID",
                            "revolve radius, scale, angle or segments are invalid",
                        ));
                    }
                    operation_count += 2;
                    u32::from(profile.resample_count) * u32::from(*radial_segments) * 2
                }
                HighLevelGeometryNodeV2::Loft {
                    section_set_id,
                    cross_section_scale,
                    axis_length,
                    position,
                    ..
                } => {
                    let set = section_sets.get(section_set_id.as_str()).ok_or_else(|| {
                        invalid(
                            "FORGE_VISUAL_VP203_REFERENCE_MISSING",
                            "loft section set is missing",
                        )
                    })?;
                    if !finite(
                        cross_section_scale
                            .iter()
                            .copied()
                            .chain(std::iter::once(*axis_length))
                            .chain(position.iter().copied()),
                    ) || cross_section_scale
                        .iter()
                        .any(|value| *value <= 0.0 || *value > 100_000.0)
                        || *axis_length <= 0.0
                        || *axis_length > 100_000.0
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_LOFT_INVALID",
                            "loft dimensions are outside bounds",
                        ));
                    }
                    operation_count += 1;
                    let samples =
                        u32::from(profiles[set.sections[0].profile_id.as_str()].resample_count);
                    samples * (set.sections.len() as u32 - 1) * 2 + samples * 2
                }
                HighLevelGeometryNodeV2::Sweep {
                    profile_id,
                    profile_scale,
                    path_points,
                    path_closed,
                    path_twist_degrees,
                    cap_start,
                    cap_end,
                    position,
                    ..
                } => {
                    let profile = profiles.get(profile_id.as_str()).ok_or_else(|| {
                        invalid(
                            "FORGE_VISUAL_VP203_REFERENCE_MISSING",
                            "sweep profile is missing",
                        )
                    })?;
                    if !(2..=32).contains(&path_points.len())
                        || !finite(
                            profile_scale
                                .iter()
                                .copied()
                                .chain(path_points.iter().flatten().copied())
                                .chain(std::iter::once(*path_twist_degrees))
                                .chain(position.iter().copied()),
                        )
                        || profile_scale
                            .iter()
                            .any(|value| *value <= 0.0 || *value > 100_000.0)
                        || path_points
                            .iter()
                            .flatten()
                            .any(|value| value.abs() > 100_000.0)
                        || !(-90.0..=90.0).contains(path_twist_degrees)
                        || (*path_closed && (*cap_start || *cap_end))
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_SWEEP_INVALID",
                            "sweep path, scale, twist or cap policy is invalid",
                        ));
                    }
                    if path_points.windows(2).any(|pair| pair[0] == pair[1]) {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_SWEEP_ZERO_LENGTH",
                            "sweep path contains a zero-length segment",
                        ));
                    }
                    if path_self_intersects(path_points) {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_SWEEP_SELF_INTERSECTION",
                            "sweep path must not self-intersect",
                        ));
                    }
                    operation_count += 1;
                    let path_segments = path_points.len() as u32 - u32::from(!*path_closed);
                    u32::from(profile.resample_count) * path_segments * 2
                        + u32::from(*cap_start || *cap_end) * u32::from(profile.resample_count) * 2
                }
                HighLevelGeometryNodeV2::Mirror { input_node_id, .. } => {
                    operation_count += 1;
                    triangles[input_node_id.as_str()]
                        .checked_mul(2)
                        .ok_or_else(|| {
                            invalid(
                                "FORGE_VISUAL_VP203_BUDGET_EXCEEDED",
                                "mirror estimate overflowed",
                            )
                        })?
                }
                HighLevelGeometryNodeV2::Array {
                    input_node_id,
                    count,
                    spacing,
                    ..
                } => {
                    if !(2..=64).contains(count)
                        || !spacing.is_finite()
                        || *spacing <= 0.0
                        || *spacing > 100_000.0
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_ARRAY_INVALID",
                            "array count and spacing must be bounded",
                        ));
                    }
                    operation_count += 1;
                    triangles[input_node_id.as_str()]
                        .checked_mul(u32::from(*count))
                        .ok_or_else(|| {
                            invalid(
                                "FORGE_VISUAL_VP203_BUDGET_EXCEEDED",
                                "array estimate overflowed",
                            )
                        })?
                }
                HighLevelGeometryNodeV2::RadialArray {
                    input_node_id,
                    count,
                    radius,
                    angle,
                    ..
                } => {
                    if !(2..=64).contains(count)
                        || !radius.is_finite()
                        || !(*radius > 0.0 && *radius <= 100_000.0)
                        || !angle.is_finite()
                        || !(*angle > 0.0 && *angle <= std::f64::consts::TAU)
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_RADIAL_ARRAY_INVALID",
                            "radial array count, radius and angle must be bounded",
                        ));
                    }
                    operation_count += 1;
                    triangles[input_node_id.as_str()]
                        .checked_mul(u32::from(*count))
                        .ok_or_else(|| {
                            invalid(
                                "FORGE_VISUAL_VP203_BUDGET_EXCEEDED",
                                "radial array estimate overflowed",
                            )
                        })?
                }
                HighLevelGeometryNodeV2::BevelApprox {
                    input_node_id,
                    radius,
                    segments,
                    ..
                } => {
                    let source_size =
                        detail_source_size(input_node_id, &nodes).ok_or_else(|| {
                            invalid(
                                "FORGE_VISUAL_VP203_DETAIL_SOURCE_INVALID",
                                "bevel_approx requires an earlier box or bevel_approx source",
                            )
                        })?;
                    if !radius.is_finite()
                        || !(*radius > 0.0
                            && *radius
                                <= source_size[0].min(source_size[2]) * 0.25)
                        || !(1..=3).contains(segments)
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_BEVEL_INVALID",
                            "bevel radius and segment count exceed the reviewed source bounds",
                        ));
                    }
                    operation_count += 1;
                    triangles[input_node_id.as_str()]
                        .checked_mul(u32::from(*segments) + 1)
                        .ok_or_else(|| {
                            invalid(
                                "FORGE_VISUAL_VP203_BUDGET_EXCEEDED",
                                "bevel estimate overflowed",
                            )
                        })?
                }
                HighLevelGeometryNodeV2::SurfacePanel {
                    input_node_id,
                    size,
                    position,
                    axis,
                    ..
                } => {
                    let source_size =
                        detail_source_size(input_node_id, &nodes).ok_or_else(|| {
                            invalid(
                                "FORGE_VISUAL_VP203_DETAIL_SOURCE_INVALID",
                                "surface_panel requires an earlier box or bevel_approx source",
                            )
                        })?;
                    let normal_index = axis.normal_index();
                    let face_indices = axis.face_indices();
                    if !finite(size.iter().copied().chain(position.iter().copied()))
                        || size.iter().any(|value| *value <= 0.0 || *value > 100_000.0)
                        || position[normal_index].abs() > 1e-9
                        || face_indices.iter().any(|index| {
                            position[*index].abs() + size[*index] / 2.0
                                > source_size[*index] / 2.0
                        })
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_SURFACE_PANEL_INVALID",
                            "surface panel must remain within its selected source face",
                        ));
                    }
                    operation_count += 1;
                    checked_add(triangles[input_node_id.as_str()], 12)?
                }
                HighLevelGeometryNodeV2::Groove {
                    input_node_id,
                    face_size,
                    position,
                    axis,
                    depth,
                    ..
                } => {
                    if !matches!(
                        nodes[input_node_id.as_str()],
                        HighLevelGeometryNodeV2::Box { .. }
                            | HighLevelGeometryNodeV2::BevelApprox { .. }
                    ) {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_GROOVE_SOURCE_INVALID",
                            "groove requires a direct box or bevel_approx source",
                        ));
                    }
                    let source_size = detail_source_size(input_node_id, &nodes).ok_or_else(|| {
                        invalid(
                            "FORGE_VISUAL_VP203_DETAIL_SOURCE_INVALID",
                            "groove requires an earlier box or bevel_approx source",
                        )
                    })?;
                    let source_position = detail_source_position(input_node_id, &nodes).ok_or_else(|| {
                        invalid(
                            "FORGE_VISUAL_VP203_DETAIL_SOURCE_INVALID",
                            "groove source position could not be resolved",
                        )
                    })?;
                    let normal_index = axis.normal_index();
                    let face_indices = axis.face_indices();
                    if !finite(
                        face_size
                            .iter()
                            .copied()
                            .chain(position.iter().copied())
                            .chain(std::iter::once(*depth)),
                    )
                        || face_size.iter().any(|value| *value <= 0.0 || *value > 100_000.0)
                        || !(*depth > 0.0 && *depth <= source_size[normal_index] * 0.25)
                        || position[normal_index].abs() > 1e-9
                        || face_indices.iter().enumerate().any(|(index, source_index)| {
                            position[*source_index].abs() + face_size[index] / 2.0
                                > source_size[*source_index] / 2.0
                        })
                        || !finite(source_position)
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_GROOVE_INVALID",
                            "groove must remain within one axis-aligned source face with bounded depth",
                        ));
                    }
                    operation_count = operation_count.checked_add(2).ok_or_else(|| {
                        invalid(
                            "FORGE_VISUAL_VP203_BUDGET_EXCEEDED",
                            "groove operation estimate overflowed",
                        )
                    })?;
                    checked_add(triangles[input_node_id.as_str()], 12)?
                }
                HighLevelGeometryNodeV2::Shell {
                    input_node_id,
                    thickness,
                    ..
                } => {
                    if !matches!(
                        nodes[input_node_id.as_str()],
                        HighLevelGeometryNodeV2::Box { .. }
                            | HighLevelGeometryNodeV2::BevelApprox { .. }
                    ) {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_SHELL_SOURCE_INVALID",
                            "shell requires a direct box or bevel_approx source",
                        ));
                    }
                    let source_size = detail_source_size(input_node_id, &nodes).ok_or_else(|| {
                        invalid(
                            "FORGE_VISUAL_VP203_DETAIL_SOURCE_INVALID",
                            "shell requires an earlier box source",
                        )
                    })?;
                    let min_extent = source_size.iter().copied().fold(f64::INFINITY, f64::min);
                    if !thickness.is_finite()
                        || !(*thickness > 0.0 && *thickness <= min_extent * 0.25)
                        || *thickness * 2.0 >= min_extent
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_SHELL_INVALID",
                            "shell thickness must be positive, bounded and leave an inner volume",
                        ));
                    }
                    if let HighLevelGeometryNodeV2::BevelApprox { radius, .. } =
                        nodes[input_node_id.as_str()]
                    {
                        let inner_min_extent = source_size
                            .iter()
                            .map(|value| *value - thickness * 2.0)
                            .fold(f64::INFINITY, f64::min);
                        if *radius * 2.0 >= inner_min_extent {
                            return Err(invalid(
                                "FORGE_VISUAL_VP203_SHELL_BEVEL_INVALID",
                                "shell thickness leaves insufficient inner room for the source bevel",
                            ));
                        }
                    }
                    operation_count += 1;
                    triangles[input_node_id.as_str()]
                        .checked_mul(4)
                        .ok_or_else(|| {
                            invalid(
                                "FORGE_VISUAL_VP203_BUDGET_EXCEEDED",
                                "shell estimate overflowed",
                            )
                        })?
                }
                HighLevelGeometryNodeV2::LatticeDeform {
                    input_node_id,
                    corner_offsets,
                    ..
                } => {
                    if !finite(corner_offsets.iter().flatten().copied())
                        || corner_offsets
                            .iter()
                            .flatten()
                            .any(|value| value.abs() > MAX_LATTICE_CORNER_OFFSET_RATIO)
                        || !corner_offsets
                            .iter()
                            .flatten()
                            .any(|value| value.abs() > 1e-9)
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_LATTICE_INVALID",
                            "lattice corner offsets must be finite, bounded and non-zero",
                        ));
                    }
                    operation_count += 1;
                    triangles[input_node_id.as_str()]
                }
                HighLevelGeometryNodeV2::LocalMeshPatch {
                    input_node_id,
                    patch_center,
                    patch_radius,
                    patch_offset,
                    ..
                } => {
                    if !finite(
                        patch_center
                            .iter()
                            .copied()
                            .chain(std::iter::once(*patch_radius))
                            .chain(patch_offset.iter().copied()),
                    )
                        || patch_center.iter().any(|value| !(0.0..=1.0).contains(value))
                        || !(*patch_radius >= 0.05 && *patch_radius <= 0.4)
                        || patch_offset.iter().any(|value| value.abs() > 0.2)
                        || !patch_offset.iter().any(|value| value.abs() > 1e-9)
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_LOCAL_MESH_PATCH_INVALID",
                            "local mesh patch center, radius and offset must remain within the normalized local bounds",
                        ));
                    }
                    operation_count += 1;
                    triangles[input_node_id.as_str()]
                }
                HighLevelGeometryNodeV2::Union { input_node_ids, .. }
                | HighLevelGeometryNodeV2::Subtract { input_node_ids, .. } => {
                    if !(2..=8).contains(&input_node_ids.len())
                        || input_node_ids.iter().collect::<BTreeSet<_>>().len()
                            != input_node_ids.len()
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_BOOLEAN_INVALID",
                            "boolean requires 2..=8 unique earlier operands",
                        ));
                    }
                    operation_count += 1;
                    input_node_ids
                        .iter()
                        .try_fold(0_u32, |total, input| {
                            checked_add(total, triangles[input.as_str()])
                        })?
                        .checked_mul(4)
                        .ok_or_else(|| {
                            invalid(
                                "FORGE_VISUAL_VP203_BUDGET_EXCEEDED",
                                "boolean estimate overflowed",
                            )
                        })?
                }
                HighLevelGeometryNodeV2::Part { part_id, role, .. } => {
                    require_id(part_id, "part_")?;
                    require_role(role)?;
                    if !part_ids.insert(part_id.as_str()) {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_DUPLICATE_ID",
                            "Part IDs must be unique",
                        ));
                    }
                    triangles[node.inputs()[0]]
                }
                HighLevelGeometryNodeV2::MaterialZone {
                    zone_id,
                    material_id,
                    ..
                } => {
                    require_id(zone_id, "zone_")?;
                    require_id(material_id, "mat_")?;
                    if !zone_ids.insert(zone_id.as_str())
                        || !material_ids.contains(material_id.as_str())
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP203_MATERIAL_ZONE_INVALID",
                            "zone must be unique and reference a declared material",
                        ));
                    }
                    triangles[node.inputs()[0]]
                }
            };
            let input_csg_depth = node
                .inputs()
                .iter()
                .filter_map(|input| csg_depths.get(input).copied())
                .max()
                .unwrap_or(0);
            let csg_depth = if matches!(
                node,
                HighLevelGeometryNodeV2::Union { .. }
                    | HighLevelGeometryNodeV2::Subtract { .. }
                    | HighLevelGeometryNodeV2::Groove { .. }
            ) {
                input_csg_depth.saturating_add(1)
            } else {
                input_csg_depth
            };
            if csg_depth > 8 {
                return Err(invalid(
                    "FORGE_VISUAL_VP203_CSG_DEPTH_EXCEEDED",
                    "boolean feature depth exceeds 8",
                ));
            }
            triangles.insert(node.node_id(), estimate);
            csg_depths.insert(node.node_id(), csg_depth);
            nodes.insert(node.node_id(), node);
        }
        if part_ids.len() > self.budgets.max_parts as usize
            || operation_count > u32::from(self.budgets.max_operations)
        {
            return Err(invalid(
                "FORGE_VISUAL_VP203_BUDGET_EXCEEDED",
                "Part or operation count exceeds declared budget",
            ));
        }
        let mut outputs = BTreeSet::new();
        let mut used_nodes = BTreeSet::new();
        let mut total_triangles = 0_u32;
        for output in &self.outputs {
            require_id(&output.output_id, "output_")?;
            if !outputs.insert(output.output_id.as_str()) {
                return Err(invalid(
                    "FORGE_VISUAL_VP203_DUPLICATE_ID",
                    "output IDs must be unique",
                ));
            }
            let zone = nodes.get(output.node_id.as_str()).ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP203_REFERENCE_MISSING",
                    "output node is missing",
                )
            })?;
            let HighLevelGeometryNodeV2::MaterialZone {
                input_node_id: part_id,
                ..
            } = zone
            else {
                return Err(invalid(
                    "FORGE_VISUAL_VP203_GRAPH_ORDER_INVALID",
                    "output must reference MaterialZone",
                ));
            };
            if !matches!(
                nodes.get(part_id.as_str()),
                Some(HighLevelGeometryNodeV2::Part { .. })
            ) {
                return Err(invalid(
                    "FORGE_VISUAL_VP203_GRAPH_ORDER_INVALID",
                    "MaterialZone must wrap Part",
                ));
            }
            let mut stack = vec![output.node_id.as_str()];
            while let Some(node_id) = stack.pop() {
                if !used_nodes.insert(node_id) {
                    return Err(invalid(
                        "FORGE_VISUAL_VP203_GRAPH_FANOUT_UNSUPPORTED",
                        "one source node cannot feed multiple outputs",
                    ));
                }
                stack.extend(nodes[node_id].inputs());
            }
            total_triangles = checked_add(total_triangles, triangles[output.node_id.as_str()])?;
        }
        if used_nodes.len() != self.nodes.len() {
            return Err(invalid(
                "FORGE_VISUAL_VP203_GRAPH_ORPHANED",
                "every node must belong to exactly one output graph",
            ));
        }
        if total_triangles > self.budgets.triangle_budget {
            return Err(invalid(
                "FORGE_VISUAL_VP203_BUDGET_EXCEEDED",
                "static triangle upper bound exceeds triangle_budget",
            ));
        }
        Ok(VisualGeometryBudgetEvidenceV2 {
            profile_count: self.profiles.len() as u16,
            section_set_count: self.section_sets.len() as u16,
            node_count: self.nodes.len() as u16,
            part_count: part_ids.len() as u16,
            output_count: self.outputs.len() as u16,
            operation_count: operation_count as u16,
            estimated_triangle_upper_bound: total_triangles,
        })
    }
}

fn operation_id(node_id: &str) -> String {
    format!("op_{}", node_id.strip_prefix("node_").unwrap())
}

fn profile_input_id(profile_id: &str) -> String {
    format!(
        "profileinput_{}",
        profile_id.strip_prefix("profile_").unwrap()
    )
}

fn collect_graph<'a>(
    terminal: &'a str,
    nodes: &BTreeMap<&'a str, &'a HighLevelGeometryNodeV2>,
    result: &mut BTreeSet<&'a str>,
) {
    if result.insert(terminal) {
        for input in nodes[terminal].inputs() {
            collect_graph(input, nodes, result);
        }
    }
}

pub fn lower_forge_visual_geometry_program_v2(
    value: &Value,
) -> CoreResult<ForgeVisualGeometryLoweringV2> {
    let (program, budget_evidence) = ForgeVisualGeometryProgramV2::parse_and_validate(value)?;
    let source_program_sha256 = semantic_sha256(&program)?;
    let expanded_program = serde_json::to_value(&program)
        .map_err(|error| invalid("JSON_SERIALIZATION_FAILED", error.to_string()))?;
    let expanded_program_sha256 = semantic_sha256(&expanded_program)?;
    let lineage = program
        .nodes
        .iter()
        .map(|node| ExpandedVisualGeometryNodeLineageV2 {
            source_node_id: node.node_id().to_string(),
            expanded_node_id: node.node_id().to_string(),
            source_macro_path: Vec::new(),
            instance_indices: Vec::new(),
        })
        .collect::<Vec<_>>();
    let lineage_sha256 = semantic_sha256(&lineage)?;
    let expanded_dag_sha256 = semantic_sha256(&json!({
        "compiler_version": VP203_COMPILER_VERSION,
        "id_algorithm_version": VP203_ID_ALGORITHM_VERSION,
        "source_program_sha256": source_program_sha256,
        "expanded_program_sha256": expanded_program_sha256,
        "lineage_sha256": lineage_sha256,
        "budget_evidence": budget_evidence,
    }))?;
    let expanded_dag = ExpandedVisualGeometryDagV2 {
        schema_version: EXPANDED_VISUAL_GEOMETRY_DAG_SCHEMA_VERSION.into(),
        compiler_version: VP203_COMPILER_VERSION.into(),
        id_algorithm_version: VP203_ID_ALGORITHM_VERSION.into(),
        source_program_sha256: source_program_sha256.clone(),
        expanded_program_sha256,
        lineage_sha256,
        expanded_dag_sha256,
        budget_evidence,
        lineage,
        expanded_program,
    };

    let profiles = program
        .profiles
        .iter()
        .map(|profile| (profile.profile_id.as_str(), profile))
        .collect::<BTreeMap<_, _>>();
    let materials = program
        .materials
        .iter()
        .map(|material| (material.material_id.as_str(), material))
        .collect::<BTreeMap<_, _>>();
    let nodes = program
        .nodes
        .iter()
        .map(|node| (node.node_id(), node))
        .collect::<BTreeMap<_, _>>();
    let mut profile_inputs = Vec::new();
    for profile in &program.profiles {
        let payload = profile_payload(profile);
        profile_inputs.push(json!({
            "input_id": profile_input_id(&profile.profile_id),
            "input_kind": "profile_sketch",
            "contract_version": "ProfileSketch@1",
            "input_sha256": semantic_sha256(&payload)?,
            "canonical_payload": payload,
        }));
    }
    for set in &program.section_sets {
        let mut referenced = set
            .sections
            .iter()
            .map(|section| section.profile_id.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|id| profile_payload(profiles[id]))
            .collect::<Vec<_>>();
        referenced
            .sort_by(|left, right| left["sketch_id"].as_str().cmp(&right["sketch_id"].as_str()));
        let payload = json!({
            "schema_version": "ProfileSectionSet@1",
            "section_set_id": set.section_set_id,
            "version": 1,
            "main_axis": set.main_axis.name(),
            "profiles": referenced,
            "sections": set.sections.iter().map(|section| json!({
                "section_id": section.section_id,
                "position": section.position,
                "profile_sketch_id": format!("sketch_{}", section.profile_id.strip_prefix("profile_").unwrap()),
                "scale": section.scale,
                "twist_degrees": section.twist_degrees,
                "cap_policy": section.cap_policy.name(),
            })).collect::<Vec<_>>(),
            "resample_policy": {"mode": "uniform_count", "count": profiles[set.sections[0].profile_id.as_str()].resample_count},
            "symmetry": "none",
            "provenance": {"source": "agent", "source_ref": "vp203_typed_geometry"},
        });
        profile_inputs.push(json!({
            "input_id": format!("profileinput_{}", set.section_set_id.strip_prefix("sectionset_").unwrap()),
            "input_kind": "profile_section_set",
            "contract_version": "ProfileSectionSet@1",
            "input_sha256": semantic_sha256(&payload)?,
            "canonical_payload": payload,
        }));
    }

    let mut operations = Vec::<Value>::new();
    let mut operation_ids_by_node = BTreeMap::<&str, Vec<String>>::new();
    for node in &program.nodes {
        let node_id = node.node_id();
        let terminal = operation_id(node_id);
        let (operation, ids) = match node {
            HighLevelGeometryNodeV2::Box {
                size,
                position,
                rotation,
                ..
            } => {
                let mut operation = json!({"operation_id": terminal, "op": "box", "inputs": [], "args": {"size": size, "position": position}});
                insert_rotation(&mut operation, *rotation);
                (operation, vec![terminal.clone()])
            }
            HighLevelGeometryNodeV2::Cylinder {
                radius,
                height,
                axis,
                position,
                rotation,
                ..
            } => {
                let mut operation = json!({"operation_id": terminal, "op": "cylinder", "inputs": [], "args": {"radius": radius, "height": height, "axis": axis.vector(), "position": position}});
                insert_rotation(&mut operation, *rotation);
                (operation, vec![terminal.clone()])
            }
            HighLevelGeometryNodeV2::Capsule {
                radius,
                height,
                axis,
                position,
                rotation,
                ..
            } => {
                let mut operation = json!({"operation_id": terminal, "op": "capsule", "inputs": [], "args": {"radius": radius, "height": height, "axis": axis.vector(), "position": position}});
                insert_rotation(&mut operation, *rotation);
                (operation, vec![terminal.clone()])
            }
            HighLevelGeometryNodeV2::Wedge {
                size,
                position,
                rotation,
                ..
            } => {
                let mut operation = json!({"operation_id": terminal, "op": "wedge", "inputs": [], "args": {"size": size, "position": position}});
                insert_rotation(&mut operation, *rotation);
                (operation, vec![terminal.clone()])
            }
            HighLevelGeometryNodeV2::Extrude {
                profile_id,
                profile_scale,
                height,
                position,
                cap_start,
                cap_end,
                rotation,
                ..
            } => {
                let profile_op = format!("{terminal}_profile");
                operations.push(json!({"operation_id": profile_op, "op": "profile", "inputs": [], "args": {"profile_input_id": profile_input_id(profile_id), "profile_scale": profile_scale}}));
                let mut operation = json!({"operation_id": terminal, "op": "extrude", "inputs": [profile_op], "args": {"height": height, "position": position, "cap_start": cap_start, "cap_end": cap_end}});
                insert_rotation(&mut operation, *rotation);
                (operation, vec![profile_op, terminal.clone()])
            }
            HighLevelGeometryNodeV2::Revolve {
                profile_id,
                profile_scale,
                angle,
                radial_segments,
                position,
                rotation,
                ..
            } => {
                let profile_op = format!("{terminal}_profile");
                operations.push(json!({"operation_id": profile_op, "op": "profile", "inputs": [], "args": {"profile_input_id": profile_input_id(profile_id), "profile_scale": profile_scale}}));
                let mut operation = json!({"operation_id": terminal, "op": "revolve", "inputs": [profile_op], "args": {"angle": angle, "radial_segments": radial_segments, "position": position}});
                insert_rotation(&mut operation, *rotation);
                (operation, vec![profile_op, terminal.clone()])
            }
            HighLevelGeometryNodeV2::Loft {
                section_set_id,
                cross_section_scale,
                axis_length,
                position,
                rotation,
                ..
            } => {
                let mut operation = json!({"operation_id": terminal, "op": "loft", "inputs": [], "args": {"section_set_input_id": format!("profileinput_{}", section_set_id.strip_prefix("sectionset_").unwrap()), "cross_section_scale": cross_section_scale, "axis_length": axis_length, "continuity": "linear", "position": position}});
                insert_rotation(&mut operation, *rotation);
                (operation, vec![terminal.clone()])
            }
            HighLevelGeometryNodeV2::Sweep {
                profile_id,
                profile_scale,
                path_points,
                path_closed,
                path_twist_degrees,
                cap_start,
                cap_end,
                position,
                rotation,
                ..
            } => {
                let mut operation = json!({"operation_id": terminal, "op": "sweep", "inputs": [], "args": {"profile_input_id": profile_input_id(profile_id), "profile_scale": profile_scale, "path_points": path_points, "path_closed": path_closed, "path_twist_degrees": path_twist_degrees, "cap_start": cap_start, "cap_end": cap_end, "position": position}});
                insert_rotation(&mut operation, *rotation);
                (operation, vec![terminal.clone()])
            }
            HighLevelGeometryNodeV2::Mirror {
                input_node_id,
                axis,
                ..
            } => (
                json!({"operation_id": terminal, "op": "mirror", "inputs": [operation_id(input_node_id)], "args": {"axis": axis.vector()}}),
                vec![terminal.clone()],
            ),
            HighLevelGeometryNodeV2::Array {
                input_node_id,
                axis,
                count,
                spacing,
                ..
            } => (
                json!({"operation_id": terminal, "op": "array", "inputs": [operation_id(input_node_id)], "args": {"axis": axis.vector(), "count": count, "spacing": spacing}}),
                vec![terminal.clone()],
            ),
            HighLevelGeometryNodeV2::RadialArray {
                input_node_id,
                axis,
                count,
                radius,
                angle,
                ..
            } => (
                json!({"operation_id": terminal, "op": "radial_array", "inputs": [operation_id(input_node_id)], "args": {"axis": axis.vector(), "count": count, "radius": radius, "angle": angle}}),
                vec![terminal.clone()],
            ),
            HighLevelGeometryNodeV2::BevelApprox {
                input_node_id,
                radius,
                segments,
                ..
            } => (
                json!({"operation_id": terminal, "op": "bevel_approx", "inputs": [operation_id(input_node_id)], "args": {"radius": radius, "segments": segments}}),
                vec![terminal.clone()],
            ),
            HighLevelGeometryNodeV2::SurfacePanel {
                input_node_id,
                size,
                position,
                axis,
                ..
            } => (
                json!({"operation_id": terminal, "op": "surface_panel", "inputs": [operation_id(input_node_id)], "args": {"size": size, "position": position, "axis": axis.vector()}}),
                vec![terminal.clone()],
            ),
            HighLevelGeometryNodeV2::Groove {
                input_node_id,
                face_size,
                position,
                axis,
                depth,
                ..
            } => {
                let source_id = operation_id(input_node_id);
                let cutter_id = format!("{terminal}_cutter");
                let source_size = detail_source_size(input_node_id, &nodes).ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_VP203_DETAIL_SOURCE_INVALID",
                        "groove source size could not be resolved during lowering",
                    )
                })?;
                let source_position = detail_source_position(input_node_id, &nodes).ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_VP203_DETAIL_SOURCE_INVALID",
                        "groove source position could not be resolved during lowering",
                    )
                })?;
                let normal_index = axis.normal_index();
                let face_indices = axis.face_indices();
                let sign = axis.vector()[normal_index];
                let mut cutter_size = [0.0_f64; 3];
                cutter_size[normal_index] = *depth + 0.2;
                cutter_size[face_indices[0]] = face_size[0];
                cutter_size[face_indices[1]] = face_size[1];
                let mut cutter_position = source_position;
                cutter_position[face_indices[0]] += position[face_indices[0]];
                cutter_position[face_indices[1]] += position[face_indices[1]];
                cutter_position[normal_index] +=
                    sign * (source_size[normal_index] / 2.0 - depth / 2.0 + 0.1);
                operations.push(json!({
                    "operation_id": cutter_id,
                    "op": "box",
                    "inputs": [],
                    "args": {"size": cutter_size, "position": cutter_position}
                }));
                (
                    json!({"operation_id": terminal, "op": "subtract", "inputs": [source_id, cutter_id], "args": {}}),
                    vec![cutter_id, terminal.clone()],
                )
            }
            HighLevelGeometryNodeV2::Shell {
                input_node_id,
                thickness,
                ..
            } => (
                json!({"operation_id": terminal, "op": "shell", "inputs": [operation_id(input_node_id)], "args": {"thickness": thickness}}),
                vec![terminal.clone()],
            ),
            HighLevelGeometryNodeV2::LatticeDeform {
                input_node_id,
                corner_offsets,
                ..
            } => (
                json!({"operation_id": terminal, "op": "lattice_deform", "inputs": [operation_id(input_node_id)], "args": {"corner_offsets": corner_offsets}}),
                vec![terminal.clone()],
            ),
            HighLevelGeometryNodeV2::LocalMeshPatch {
                input_node_id,
                patch_center,
                patch_radius,
                patch_offset,
                ..
            } => (
                json!({"operation_id": terminal, "op": "local_mesh_patch", "inputs": [operation_id(input_node_id)], "args": {"patch_center": patch_center, "patch_radius": patch_radius, "patch_offset": patch_offset}}),
                vec![terminal.clone()],
            ),
            HighLevelGeometryNodeV2::Union { input_node_ids, .. } => (
                json!({"operation_id": terminal, "op": "union", "inputs": input_node_ids.iter().map(|id| operation_id(id)).collect::<Vec<_>>(), "args": {}}),
                vec![terminal.clone()],
            ),
            HighLevelGeometryNodeV2::Subtract { input_node_ids, .. } => (
                json!({"operation_id": terminal, "op": "subtract", "inputs": input_node_ids.iter().map(|id| operation_id(id)).collect::<Vec<_>>(), "args": {}}),
                vec![terminal.clone()],
            ),
            HighLevelGeometryNodeV2::Part { .. } | HighLevelGeometryNodeV2::MaterialZone { .. } => {
                operation_ids_by_node.insert(node_id, Vec::new());
                continue;
            }
        };
        operations.push(operation);
        operation_ids_by_node.insert(node_id, ids);
    }

    let mut outputs = Vec::new();
    let mut source_map = Vec::new();
    for output in &program.outputs {
        let HighLevelGeometryNodeV2::MaterialZone {
            input_node_id: part_node_id,
            zone_id,
            material_id,
            ..
        } = nodes[output.node_id.as_str()]
        else {
            unreachable!()
        };
        let HighLevelGeometryNodeV2::Part {
            input_node_id: geometry_node_id,
            part_id,
            role,
            ..
        } = nodes[part_node_id.as_str()]
        else {
            unreachable!()
        };
        let terminal_operation_id = operation_id(geometry_node_id);
        let operation = operations
            .iter_mut()
            .find(|item| item["operation_id"] == terminal_operation_id)
            .ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP203_LOWERING_FAILED",
                    "terminal geometry operation is missing",
                )
            })?;
        let compiled_material_id =
            compiled_visual_base_material_id(&materials[material_id.as_str()].base_material_id)
                .unwrap()
                .to_string();
        let args = operation["args"].as_object_mut().ok_or_else(|| {
            invalid(
                "FORGE_VISUAL_VP203_LOWERING_FAILED",
                "operation args are invalid",
            )
        })?;
        args.insert("part_role".into(), json!(role));
        args.insert("zone_id".into(), json!(zone_id));
        args.insert("material_id".into(), json!(compiled_material_id));
        outputs.push(json!({"output_id": output.output_id, "operation_id": terminal_operation_id, "kind": "mesh", "part_role": role}));
        let mut graph = BTreeSet::new();
        collect_graph(output.node_id.as_str(), &nodes, &mut graph);
        let source_node_ids = program
            .nodes
            .iter()
            .filter(|node| graph.contains(node.node_id()))
            .map(|node| node.node_id().to_string())
            .collect::<Vec<_>>();
        let shape_operation_ids = source_node_ids
            .iter()
            .flat_map(|id| {
                operation_ids_by_node
                    .get(id.as_str())
                    .cloned()
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        // CSG and transform readback intentionally preserves the leaf faces'
        // source operation IDs. Bind the owning Part/Zone/material to every
        // mesh-producing operation in this output graph so those retained
        // faces still join back to the single semantic output.
        for shape_operation_id in &shape_operation_ids {
            let Some(source_operation) = operations
                .iter_mut()
                .find(|item| item["operation_id"] == *shape_operation_id)
            else {
                continue;
            };
            if source_operation["op"] == "profile" {
                continue;
            }
            let source_args = source_operation["args"].as_object_mut().ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP203_LOWERING_FAILED",
                    "operation args are invalid",
                )
            })?;
            source_args.insert("part_role".into(), json!(role));
            source_args.insert("zone_id".into(), json!(zone_id));
            source_args.insert("material_id".into(), json!(compiled_material_id));
        }
        source_map.push(VisualGeometrySourceMapEntryV2 {
            output_id: output.output_id.clone(),
            expanded_node_ids: source_node_ids.clone(),
            source_node_ids,
            shape_operation_ids,
            terminal_operation_id,
            part_id: part_id.clone(),
            material_zone_id: zone_id.clone(),
            authored_material_id: material_id.clone(),
            compiled_material_id,
        });
    }
    source_map.sort_by(|left, right| left.output_id.cmp(&right.output_id));
    let source_map_sha256 = semantic_sha256(&source_map)?;
    let shape_program = normalize_persisted_shape_program(&json!({
        "schema_version": "ShapeProgram@1",
        "program_id": format!("shape_{}", program.program_id.strip_prefix("visual_").unwrap()),
        "units": "millimeter",
        "seed": program.seed,
        "triangle_budget": program.budgets.triangle_budget,
        "parameters": [],
        "profile_inputs": profile_inputs,
        "operations": operations,
        "outputs": outputs,
        "non_functional_only": true,
    }))?;
    let shape_program_sha256 = semantic_sha256(&shape_program)?;
    Ok(ForgeVisualGeometryLoweringV2 {
        schema_version: FORGE_VISUAL_GEOMETRY_LOWERING_SCHEMA_VERSION.into(),
        compiler_version: VP203_COMPILER_VERSION.into(),
        source_program_sha256,
        expanded_dag,
        source_map_sha256,
        source_map,
        shape_program_sha256,
        shape_program,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Value {
        let raw = match name {
            "bracket" => include_str!("../../../../../../packages/concept-spec/fixtures/forge-visual-geometry-v2-bracket.json"),
            "rotor" => include_str!("../../../../../../packages/concept-spec/fixtures/forge-visual-geometry-v2-rotor.json"),
            "duct" => include_str!("../../../../../../packages/concept-spec/fixtures/forge-visual-geometry-v2-duct.json"),
            _ => unreachable!(),
        };
        serde_json::from_str(raw).unwrap()
    }

    #[test]
    fn vp203_three_unseen_fixtures_lower_with_distinct_feature_fingerprints() {
        let lowerings = ["bracket", "rotor", "duct"]
            .map(|name| lower_forge_visual_geometry_program_v2(&fixture(name)).unwrap());
        let operation_fingerprints = lowerings
            .iter()
            .map(|item| {
                item.shape_program["operations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|op| op["op"].as_str().unwrap())
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(operation_fingerprints.len(), 3);
        assert!(operation_fingerprints
            .iter()
            .any(|items| items.contains(&"subtract") && items.contains(&"mirror")));
        assert!(operation_fingerprints
            .iter()
            .any(|items| items.contains(&"revolve") && items.contains(&"array")));
        assert!(operation_fingerprints
            .iter()
            .any(|items| items.contains(&"loft") && items.contains(&"sweep")));
    }

    #[test]
    fn vp203_exposes_bounded_static_rotation_without_changing_zero_rotation_identity() {
        let mut source = json!({
            "schema_version": "ForgeVisualGeometryProgram@2",
            "program_id": "visual_rotation_contract",
            "domain": "generic_hard_surface",
            "units": "millimeter",
            "seed": 23,
            "materials": [{"material_id":"mat_shell","base_material_id":"mat_aluminum"}],
            "profiles": [],
            "section_sets": [],
            "nodes": [
                {"kind":"box","node_id":"node_box","size":[120.0,60.0,80.0],"position":[0.0,0.0,0.0]},
                {"kind":"part","node_id":"node_part","input_node_id":"node_box","part_id":"part_shell","role":"armor_shell"},
                {"kind":"material_zone","node_id":"node_zone","input_node_id":"node_part","zone_id":"zone_shell","material_id":"mat_shell"}
            ],
            "outputs": [{"output_id":"output_shell","node_id":"node_zone"}],
            "budgets": {"schema_version":"GeometryProgramBudget@1","max_profiles":1,"max_section_sets":1,"max_nodes":8,"max_parts":2,"max_materials":2,"max_outputs":2,"max_operations":8,"triangle_budget":1000}
        });

        let zero_rotation = lower_forge_visual_geometry_program_v2(&source).unwrap();
        let zero_args = &zero_rotation.shape_program["operations"][0]["args"];
        assert!(zero_args.get("rotation").is_none());

        source["nodes"][0]["rotation"] = json!([0.25, -0.5, 0.75]);
        let rotated = lower_forge_visual_geometry_program_v2(&source).unwrap();
        assert_eq!(
            rotated.shape_program["operations"][0]["args"]["rotation"],
            json!([0.25, -0.5, 0.75])
        );

        source["nodes"][0]["rotation"] = json!([std::f64::consts::PI + 0.01, 0.0, 0.0]);
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&source)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_ROTATION_INVALID"
        );
    }

    #[test]
    fn vp203_lowers_the_reviewed_worker_primitives_and_detail_operations() {
        let source = json!({
            "schema_version": "ForgeVisualGeometryProgram@2",
            "program_id": "visual_worker_surface",
            "domain": "generic_hard_surface",
            "units": "millimeter",
            "seed": 7,
            "materials": [{
                "material_id": "mat_shell",
                "base_material_id": "mat_aluminum"
            }],
            "profiles": [],
            "section_sets": [],
            "nodes": [
                {"kind":"box","node_id":"node_panel_base","size":[240.0,80.0,160.0],"position":[0.0,0.0,0.0]},
                {"kind":"bevel_approx","node_id":"node_panel_bevel","input_node_id":"node_panel_base","radius":16.0,"segments":2},
                {"kind":"surface_panel","node_id":"node_panel_detail","input_node_id":"node_panel_bevel","size":[180.0,8.0,100.0],"position":[0.0,0.0,0.0],"axis":"positive_y"},
                {"kind":"part","node_id":"node_panel_part","input_node_id":"node_panel_detail","part_id":"part_panel","role":"armor_panel"},
                {"kind":"material_zone","node_id":"node_panel_zone","input_node_id":"node_panel_part","zone_id":"zone_panel","material_id":"mat_shell"},
                {"kind":"cylinder","node_id":"node_rotor","radius":24.0,"height":36.0,"axis":"z","position":[0.0,0.0,0.0]},
                {"kind":"radial_array","node_id":"node_rotor_array","input_node_id":"node_rotor","axis":"z","count":6,"radius":120.0,"angle":6.283185307179586},
                {"kind":"part","node_id":"node_rotor_part","input_node_id":"node_rotor_array","part_id":"part_rotor","role":"rotor_detail"},
                {"kind":"material_zone","node_id":"node_rotor_zone","input_node_id":"node_rotor_part","zone_id":"zone_rotor","material_id":"mat_shell"},
                {"kind":"capsule","node_id":"node_cable","radius":12.0,"height":100.0,"axis":"y","position":[0.0,0.0,0.0]},
                {"kind":"mirror","node_id":"node_cable_mirror","input_node_id":"node_cable","axis":"x"},
                {"kind":"part","node_id":"node_cable_part","input_node_id":"node_cable_mirror","part_id":"part_cable","role":"cable_detail"},
                {"kind":"material_zone","node_id":"node_cable_zone","input_node_id":"node_cable_part","zone_id":"zone_cable","material_id":"mat_shell"},
                {"kind":"wedge","node_id":"node_fin","size":[90.0,40.0,180.0],"position":[0.0,0.0,0.0]},
                {"kind":"array","node_id":"node_fin_array","input_node_id":"node_fin","axis":"x","count":3,"spacing":110.0},
                {"kind":"part","node_id":"node_fin_part","input_node_id":"node_fin_array","part_id":"part_fin","role":"fin_detail"},
                {"kind":"material_zone","node_id":"node_fin_zone","input_node_id":"node_fin_part","zone_id":"zone_fin","material_id":"mat_shell"}
            ],
            "outputs": [
                {"output_id":"output_panel","node_id":"node_panel_zone"},
                {"output_id":"output_rotor","node_id":"node_rotor_zone"},
                {"output_id":"output_cable","node_id":"node_cable_zone"},
                {"output_id":"output_fin","node_id":"node_fin_zone"}
            ],
            "budgets": {"schema_version":"GeometryProgramBudget@1","max_profiles":4,"max_section_sets":2,"max_nodes":32,"max_parts":8,"max_materials":4,"max_outputs":8,"max_operations":32,"triangle_budget":10000}
        });
        let lowering = lower_forge_visual_geometry_program_v2(&source).unwrap();
        let operations = lowering.shape_program["operations"].as_array().unwrap();
        for operation in [
            "box",
            "cylinder",
            "capsule",
            "wedge",
            "radial_array",
            "bevel_approx",
            "surface_panel",
        ] {
            assert!(operations.iter().any(|item| item["op"] == operation));
        }
        assert!(
            lowering
                .expanded_dag
                .budget_evidence
                .estimated_triangle_upper_bound
                > 1_000
        );
    }

    #[test]
    fn vp203_lowers_a_bounded_face_groove_to_a_sealed_cutter_and_subtract() {
        let source = json!({
            "schema_version": "ForgeVisualGeometryProgram@2",
            "program_id": "visual_face_groove",
            "domain": "generic_hard_surface",
            "units": "millimeter",
            "seed": 19,
            "materials": [{"material_id":"mat_shell","base_material_id":"mat_aluminum"}],
            "profiles": [],
            "section_sets": [],
            "nodes": [
                {"kind":"box","node_id":"node_shell","size":[200.0,100.0,80.0],"position":[10.0,20.0,30.0]},
                {"kind":"groove","node_id":"node_groove","input_node_id":"node_shell","face_size":[120.0,30.0],"position":[8.0,0.0,-6.0],"axis":"positive_y","depth":8.0},
                {"kind":"part","node_id":"node_part","input_node_id":"node_groove","part_id":"part_shell","role":"armor_shell"},
                {"kind":"material_zone","node_id":"node_zone","input_node_id":"node_part","zone_id":"zone_shell","material_id":"mat_shell"}
            ],
            "outputs": [{"output_id":"output_shell","node_id":"node_zone"}],
            "budgets": {"schema_version":"GeometryProgramBudget@1","max_profiles":1,"max_section_sets":0,"max_nodes":8,"max_parts":2,"max_materials":2,"max_outputs":2,"max_operations":8,"triangle_budget":2000}
        });
        let lowering = lower_forge_visual_geometry_program_v2(&source).unwrap();
        let operations = lowering.shape_program["operations"].as_array().unwrap();
        let cutter = operations
            .iter()
            .find(|operation| operation["operation_id"] == "op_groove_cutter")
            .expect("groove must lower a deterministic cutter");
        assert_eq!(cutter["op"], "box");
        assert_eq!(cutter["args"]["size"], json!([120.0, 8.2, 30.0]));
        assert_eq!(cutter["args"]["position"], json!([18.0, 66.1, 24.0]));
        assert_eq!(
            operations
                .iter()
                .find(|operation| operation["operation_id"] == "op_groove")
                .unwrap()["op"],
            "subtract"
        );

        let mut invalid_source = source.clone();
        invalid_source["nodes"][1]["position"] = json!([8.0, 1.0, -6.0]);
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&invalid_source)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_GROOVE_INVALID"
        );

        let mut invalid_depth = source.clone();
        invalid_depth["nodes"][1]["depth"] = json!(26.0);
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&invalid_depth)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_GROOVE_INVALID"
        );

        let mut invalid_source_kind = source;
        invalid_source_kind["nodes"][0]["kind"] = json!("cylinder");
        invalid_source_kind["nodes"][0].as_object_mut().unwrap().remove("size");
        invalid_source_kind["nodes"][0]["radius"] = json!(50.0);
        invalid_source_kind["nodes"][0]["height"] = json!(80.0);
        invalid_source_kind["nodes"][0]["axis"] = json!("y");
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&invalid_source_kind)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_GROOVE_SOURCE_INVALID"
        );
    }

    #[test]
    fn vp203_lowers_bounded_lattice_deform_without_changing_triangle_budget() {
        let source = json!({
            "schema_version": "ForgeVisualGeometryProgram@2",
            "program_id": "visual_lattice_shell",
            "domain": "generic_hard_surface",
            "units": "millimeter",
            "seed": 41,
            "materials": [{"material_id": "mat_shell", "base_material_id": "mat_aluminum"}],
            "profiles": [],
            "section_sets": [],
            "nodes": [
                {"kind":"box","node_id":"node_shell","size":[240.0,120.0,80.0],"position":[0.0,0.0,0.0]},
                {"kind":"lattice_deform","node_id":"node_shell_deformed","input_node_id":"node_shell","corner_offsets":[[0.0,0.0,0.0],[0.08,0.0,0.0],[0.0,0.04,0.0],[0.08,0.04,0.0],[0.0,0.0,-0.10],[0.08,0.0,-0.10],[0.0,0.04,-0.10],[0.08,0.04,-0.10]]},
                {"kind":"part","node_id":"node_shell_part","input_node_id":"node_shell_deformed","part_id":"part_shell","role":"armor_shell"},
                {"kind":"material_zone","node_id":"node_shell_zone","input_node_id":"node_shell_part","zone_id":"zone_shell","material_id":"mat_shell"}
            ],
            "outputs": [{"output_id":"output_shell","node_id":"node_shell_zone"}],
            "budgets": {"schema_version":"GeometryProgramBudget@1","max_profiles":1,"max_section_sets":1,"max_nodes":8,"max_parts":2,"max_materials":2,"max_outputs":2,"max_operations":8,"triangle_budget":1000}
        });
        let lowering = lower_forge_visual_geometry_program_v2(&source).unwrap();
        assert!(lowering.shape_program["operations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|operation| operation["op"] == "lattice_deform"));
        assert_eq!(
            lowering
                .expanded_dag
                .budget_evidence
                .estimated_triangle_upper_bound,
            12
        );
    }

    #[test]
    fn vp203_lowers_bounded_local_mesh_patch_without_changing_triangle_budget() {
        let source = json!({
            "schema_version": "ForgeVisualGeometryProgram@2",
            "program_id": "visual_local_mesh_patch",
            "domain": "generic_hard_surface",
            "units": "millimeter",
            "seed": 42,
            "materials": [{"material_id": "mat_shell", "base_material_id": "mat_aluminum"}],
            "profiles": [],
            "section_sets": [],
            "nodes": [
                {"kind":"box","node_id":"node_shell","size":[240.0,120.0,80.0],"position":[0.0,0.0,0.0]},
                {"kind":"local_mesh_patch","node_id":"node_shell_patch","input_node_id":"node_shell","patch_center":[0.0,0.0,0.0],"patch_radius":0.2,"patch_offset":[0.1,0.0,0.0]},
                {"kind":"part","node_id":"node_shell_part","input_node_id":"node_shell_patch","part_id":"part_shell","role":"armor_shell"},
                {"kind":"material_zone","node_id":"node_shell_zone","input_node_id":"node_shell_part","zone_id":"zone_shell","material_id":"mat_shell"}
            ],
            "outputs": [{"output_id":"output_shell","node_id":"node_shell_zone"}],
            "budgets": {"schema_version":"GeometryProgramBudget@1","max_profiles":1,"max_section_sets":1,"max_nodes":8,"max_parts":2,"max_materials":2,"max_outputs":2,"max_operations":8,"triangle_budget":1000}
        });
        let lowering = lower_forge_visual_geometry_program_v2(&source).unwrap();
        let operations = lowering.shape_program["operations"].as_array().unwrap();
        let patch = operations
            .iter()
            .find(|operation| operation["op"] == "local_mesh_patch")
            .expect("local mesh patch must lower to the restricted worker operation");
        assert_eq!(patch["args"]["patch_radius"], json!(0.2));
        assert_eq!(
            lowering
                .expanded_dag
                .budget_evidence
                .estimated_triangle_upper_bound,
            12
        );
    }

    #[test]
    fn vp203_lowers_bounded_closed_shell_without_aliasing_boolean_or_template_geometry() {
        let source = json!({
            "schema_version": "ForgeVisualGeometryProgram@2",
            "program_id": "visual_closed_shell",
            "domain": "generic_hard_surface",
            "units": "millimeter",
            "seed": 43,
            "materials": [{"material_id": "mat_shell", "base_material_id": "mat_aluminum"}],
            "profiles": [],
            "section_sets": [],
            "nodes": [
                {"kind":"box","node_id":"node_shell_base","size":[240.0,120.0,80.0],"position":[0.0,0.0,0.0]},
                {"kind":"shell","node_id":"node_shell","input_node_id":"node_shell_base","thickness":20.0},
                {"kind":"part","node_id":"node_shell_part","input_node_id":"node_shell","part_id":"part_shell","role":"armor_shell"},
                {"kind":"material_zone","node_id":"node_shell_zone","input_node_id":"node_shell_part","zone_id":"zone_shell","material_id":"mat_shell"}
            ],
            "outputs": [{"output_id":"output_shell","node_id":"node_shell_zone"}],
            "budgets": {"schema_version":"GeometryProgramBudget@1","max_profiles":1,"max_section_sets":1,"max_nodes":8,"max_parts":2,"max_materials":2,"max_outputs":2,"max_operations":8,"triangle_budget":1000}
        });
        let lowering = lower_forge_visual_geometry_program_v2(&source).unwrap();
        let operations = lowering.shape_program["operations"].as_array().unwrap();
        assert!(operations.iter().any(|operation| operation["op"] == "shell"));
        assert_eq!(
            lowering
                .expanded_dag
                .budget_evidence
                .estimated_triangle_upper_bound,
            48
        );
    }

    #[test]
    fn vp203_lowers_a_bounded_shell_from_a_beveled_box() {
        let source = json!({
            "schema_version": "ForgeVisualGeometryProgram@2",
            "program_id": "visual_beveled_shell",
            "domain": "generic_hard_surface",
            "units": "millimeter",
            "seed": 44,
            "materials": [{"material_id": "mat_shell", "base_material_id": "mat_aluminum"}],
            "profiles": [],
            "section_sets": [],
            "nodes": [
                {"kind":"box","node_id":"node_shell_base","size":[240.0,120.0,80.0],"position":[0.0,0.0,0.0]},
                {"kind":"bevel_approx","node_id":"node_shell_bevel","input_node_id":"node_shell_base","radius":8.0,"segments":2},
                {"kind":"shell","node_id":"node_shell","input_node_id":"node_shell_bevel","thickness":12.0},
                {"kind":"part","node_id":"node_shell_part","input_node_id":"node_shell","part_id":"part_shell","role":"armor_shell"},
                {"kind":"material_zone","node_id":"node_shell_zone","input_node_id":"node_shell_part","zone_id":"zone_shell","material_id":"mat_shell"}
            ],
            "outputs": [{"output_id":"output_shell","node_id":"node_shell_zone"}],
            "budgets": {"schema_version":"GeometryProgramBudget@1","max_profiles":1,"max_section_sets":1,"max_nodes":8,"max_parts":2,"max_materials":2,"max_outputs":2,"max_operations":8,"triangle_budget":2000}
        });
        let lowering = lower_forge_visual_geometry_program_v2(&source).unwrap();
        let operations = lowering.shape_program["operations"].as_array().unwrap();
        assert!(operations.iter().any(|operation| operation["op"] == "bevel_approx"));
        assert!(operations.iter().any(|operation| operation["op"] == "shell"));
    }

    #[test]
    fn vp203_rejects_detail_nodes_that_the_restricted_worker_cannot_execute() {
        let mut source = json!({
            "schema_version": "ForgeVisualGeometryProgram@2",
            "program_id": "visual_invalid_detail",
            "domain": "generic_hard_surface",
            "units": "millimeter",
            "seed": 7,
            "materials": [{"material_id":"mat_shell","base_material_id":"mat_aluminum"}],
            "profiles": [], "section_sets": [],
            "nodes": [
                {"kind":"box","node_id":"node_base","size":[100.0,40.0,80.0],"position":[0.0,0.0,0.0]},
                {"kind":"surface_panel","node_id":"node_panel","input_node_id":"node_base","size":[120.0,8.0,40.0],"position":[0.0,0.0,0.0],"axis":"positive_y"},
                {"kind":"part","node_id":"node_part","input_node_id":"node_panel","part_id":"part_panel","role":"armor_panel"},
                {"kind":"material_zone","node_id":"node_zone","input_node_id":"node_part","zone_id":"zone_panel","material_id":"mat_shell"}
            ],
            "outputs": [{"output_id":"output_panel","node_id":"node_zone"}],
            "budgets": {"schema_version":"GeometryProgramBudget@1","max_profiles":4,"max_section_sets":2,"max_nodes":8,"max_parts":2,"max_materials":2,"max_outputs":2,"max_operations":8,"triangle_budget":10000}
        });
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&source)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_SURFACE_PANEL_INVALID"
        );
        source["nodes"][1]["kind"] = json!("bevel_approx");
        source["nodes"][1].as_object_mut().unwrap().remove("size");
        source["nodes"][1]
            .as_object_mut()
            .unwrap()
            .remove("position");
        source["nodes"][1].as_object_mut().unwrap().remove("axis");
        source["nodes"][1]["radius"] = json!(60.0);
        source["nodes"][1]["segments"] = json!(2);
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&source)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_BEVEL_INVALID"
        );
    }

    #[test]
    fn vp203_hash_is_key_order_stable_and_semantic_sensitive() {
        let source = fixture("rotor");
        let mut reordered = serde_json::Map::new();
        for key in [
            "budgets",
            "outputs",
            "nodes",
            "section_sets",
            "profiles",
            "materials",
            "seed",
            "units",
            "domain",
            "program_id",
            "schema_version",
        ] {
            reordered.insert(key.into(), source[key].clone());
        }
        let left = lower_forge_visual_geometry_program_v2(&source).unwrap();
        let right = lower_forge_visual_geometry_program_v2(&Value::Object(reordered)).unwrap();
        assert_eq!(left.source_program_sha256, right.source_program_sha256);
        let mut changed = source;
        changed["nodes"][0]["angle"] = json!(3.0);
        assert_ne!(
            left.source_program_sha256,
            lower_forge_visual_geometry_program_v2(&changed)
                .unwrap()
                .source_program_sha256
        );
    }

    #[test]
    fn vp203_rejects_self_intersection_before_lowering() {
        let mut source = fixture("bracket");
        source["profiles"][0]["points"] =
            json!([[-0.8, -0.8], [0.8, 0.8], [-0.8, 0.8], [0.8, -0.8]]);
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&source)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_PROFILE_SELF_INTERSECTION"
        );
    }

    #[test]
    fn vp203_rejects_forward_refs_boolean_and_array_bounds() {
        let mut forward = fixture("bracket");
        forward["nodes"][2]["input_node_ids"] = json!(["node_missing", "node_plate"]);
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&forward)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_FORWARD_OR_MISSING_REFERENCE"
        );
        let mut array = fixture("rotor");
        array["nodes"][1]["count"] = json!(65);
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&array)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_ARRAY_INVALID"
        );
    }

    #[test]
    fn vp203_rejects_static_budget_and_unknown_operation() {
        let mut budget = fixture("rotor");
        budget["budgets"]["triangle_budget"] = json!(100);
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&budget)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_BUDGET_EXCEEDED"
        );
        let mut unknown = fixture("rotor");
        unknown["nodes"][0]["kind"] = json!("lathe_script");
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&unknown)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_PARSE_FAILED"
        );
    }

    #[test]
    fn vp203_rejects_section_order_and_resample_mismatch() {
        let mut unordered = fixture("duct");
        unordered["section_sets"][0]["sections"][1]["position"] = json!(-0.95);
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&unordered)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_SECTION_SET_INVALID"
        );
        let mut mismatch = fixture("duct");
        mismatch["profiles"][1]["resample_count"] = json!(16);
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&mismatch)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_SECTION_RESAMPLE_MISMATCH"
        );
    }

    #[test]
    fn vp203_rejects_sweep_path_and_cap_failures() {
        let mut zero = fixture("duct");
        zero["nodes"][3]["path_points"][1] = zero["nodes"][3]["path_points"][0].clone();
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&zero)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_SWEEP_ZERO_LENGTH"
        );
        let mut crossing = fixture("duct");
        crossing["nodes"][3]["path_points"] = json!([
            [-400.0, -300.0, 0.0],
            [400.0, 300.0, 0.0],
            [-400.0, 300.0, 0.0],
            [400.0, -300.0, 0.0]
        ]);
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&crossing)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_SWEEP_SELF_INTERSECTION"
        );
        let mut closed_cap = fixture("duct");
        closed_cap["nodes"][3]["path_closed"] = json!(true);
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&closed_cap)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_SWEEP_INVALID"
        );
    }

    #[test]
    fn vp203_rejects_boolean_operand_count_and_unknown_mirror_plane() {
        let mut boolean = fixture("bracket");
        boolean["nodes"][2]["input_node_ids"] = json!(["node_plate"]);
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&boolean)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_BOOLEAN_INVALID"
        );
        let mut mirror = fixture("bracket");
        mirror["nodes"][5]["axis"] = json!("diagonal");
        assert_eq!(
            lower_forge_visual_geometry_program_v2(&mirror)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP203_PARSE_FAILED"
        );
    }
}
