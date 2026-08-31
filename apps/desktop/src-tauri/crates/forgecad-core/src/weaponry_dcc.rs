//! Pure Rust primitives for the Weaponry authoring/evaluation slice.
//!
//! This module intentionally stops at typed design data and deterministic
//! validation. It does not know about Runtime, SQLite, CAS, MCP, workers, or
//! a file system. No public type accepts an arbitrary JSON executor, path,
//! URL, script, or mesh buffer.

use crate::canonical_json_hash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

/// V2 knife blade language.  This remains a pure-core module: it owns no
/// Runtime, Store, CAS, Worker or MCP state.  The V1 four-sided sweep below
/// is intentionally preserved for compatibility; callers that need a blade
/// with sectioned asymmetric form should use `knife_blade_language`.
#[path = "knife_blade_language.rs"]
pub mod knife_blade_language;
pub use knife_blade_language::*;

const MAX_ID_BYTES: usize = 128;
const MAX_GRAPH_NODES: usize = 64;
const MAX_GRAPH_EDGES: usize = 128;
const MAX_SELECTION_REFS: usize = 4096;
const MAX_SELECTION_ADJACENCY_DEPTH: u8 = 16;
const MAX_ARRAY_COUNT: u32 = 32;
const MAX_MODIFIER_SEGMENTS: u8 = 4;
const MAX_COORDINATE_M: f64 = 10.0;
const MAX_BEVEL_WIDTH_M: f64 = 5.0;

/// Bounds for the deliberately small, knife-oriented curve kernel.  These
/// limits are part of the type boundary rather than tuning knobs supplied by
/// callers, so a curve cannot turn into an unbounded geometry workload.
pub const MAX_KNIFE_CURVE_CONTROL_POINTS: usize = 64;
pub const MAX_KNIFE_CURVE_KNOTS: usize = 256;
pub const MAX_KNIFE_CURVE_SAMPLES: usize = 2048;
pub const MAX_KNIFE_CURVE_DEGREE: u8 = 5;
/// Maximum topology emitted by the closed knife curve evaluator.  The
/// evaluator uses four vertices per station and a fixed four-sided sweep, so
/// these bounds are deliberately derived from the curve sample budget rather
/// than being caller-controlled mesh budgets.
pub const MAX_KNIFE_EVALUATED_MESH_VERTICES: usize = MAX_KNIFE_CURVE_SAMPLES * 4;
pub const MAX_KNIFE_EVALUATED_MESH_TRIANGLES: usize =
    MAX_KNIFE_CURVE_SAMPLES.saturating_mul(8).saturating_sub(4);
const MIN_KNIFE_CURVE_WEIGHT: f64 = 1.0e-3;
const MAX_KNIFE_CURVE_WEIGHT: f64 = 1.0e3;
const MAX_KNIFE_CURVE_TOLERANCE_M: f64 = 0.5;
const MIN_KNIFE_THICKNESS_M: f64 = 1.0e-5;
const MAX_KNIFE_THICKNESS_M: f64 = 2.0;
const MIN_KNIFE_FRAME_LENGTH_M: f64 = 1.0e-9;
const MIN_KNIFE_TRIANGLE_AREA_SQUARED_M2: f64 = 1.0e-24;
const MAX_EVALUATED_MESH_COORDINATE_M: f64 = MAX_COORDINATE_M * 2.0;
// Generated geometry crosses JSON/CAS boundaries. Quantising to a fixed
// nanometre grid makes the typed value survive canonical JSON round-trips
// without allowing platform/parser float spelling to change its semantic hash.
const EVALUATED_MESH_COORDINATE_QUANTIZATION_PER_M: f64 = 1.0e9;

/// The implicit source node used by a dependency graph.
pub const SOURCE_REVISION_NODE_ID: &str = "__source_revision__";

/// A validated opaque identifier. Its grammar is deliberately narrower than
/// a path or URI so identity cannot smuggle a resource locator into the
/// authoring kernel.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StableId(String);

impl StableId {
    pub fn new(value: impl AsRef<str>) -> Result<Self, WeaponryDccError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(WeaponryDccError::InvalidStableId {
                value: value.to_owned(),
                reason: "must not be empty".to_owned(),
            });
        }
        if value.len() > MAX_ID_BYTES {
            return Err(WeaponryDccError::InvalidStableId {
                value: value.to_owned(),
                reason: format!("must be at most {MAX_ID_BYTES} bytes"),
            });
        }
        if let Some(character) = value.chars().find(|character| {
            !character.is_ascii_alphanumeric() && !matches!(character, '.' | '_' | '-')
        }) {
            return Err(WeaponryDccError::InvalidStableId {
                value: value.to_owned(),
                reason: format!("contains unsupported character {character:?}"),
            });
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for StableId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for StableId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A validated lowercase SHA-256 digest used for cross-boundary references.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Sha256Hash(String);

impl Sha256Hash {
    pub fn new(value: impl AsRef<str>) -> Result<Self, WeaponryDccError> {
        let value = value.as_ref();
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(WeaponryDccError::InvalidSha256 {
                value: value.to_owned(),
            });
        }
        if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
            return Err(WeaponryDccError::InvalidSha256 {
                value: value.to_owned(),
            });
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Sha256Hash {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for Sha256Hash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// All malformed input is rejected before a value is returned.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WeaponryDccError {
    #[error("invalid stable id {value:?}: {reason}")]
    InvalidStableId { value: String, reason: String },
    #[error("invalid sha256 {value:?}")]
    InvalidSha256 { value: String },
    #[error("invalid numeric value for {field}: {reason}")]
    InvalidNumber { field: String, reason: String },
    #[error("invalid knife curve {curve_id}: {reason}")]
    InvalidKnifeCurve { curve_id: String, reason: String },
    #[error("curve tessellation plan is bound to a different curve")]
    CurvePlanBindingMismatch,
    #[error("invalid curve tessellation plan: {reason}")]
    CurvePlanInvalid { reason: String },
    #[error("curve sample budget exceeded: {count} > {maximum}")]
    CurveSampleBudgetExceeded { count: usize, maximum: usize },
    #[error("knife curve role mismatch: expected {expected:?}, got {actual:?}")]
    KnifeCurveRoleMismatch {
        expected: KnifeCurveRole,
        actual: KnifeCurveRole,
    },
    #[error("knife blade sweep plan is invalid: {reason}")]
    KnifeBladeSweepPlanInvalid { reason: String },
    #[error("knife blade sweep inputs are invalid: {reason}")]
    KnifeBladeInputInvalid { reason: String },
    #[error("knife blade sweep station counts differ: spine={spine}, edge={edge}")]
    KnifeBladeStationCountMismatch { spine: usize, edge: usize },
    #[error("knife evaluated mesh vertex budget exceeded: {count} > {maximum}")]
    KnifeEvaluatedMeshVertexBudgetExceeded { count: usize, maximum: usize },
    #[error("knife evaluated mesh triangle budget exceeded: {count} > {maximum}")]
    KnifeEvaluatedMeshTriangleBudgetExceeded { count: usize, maximum: usize },
    #[error("knife evaluated mesh contains a degenerate triangle at index {triangle_index}")]
    KnifeEvaluatedMeshDegenerateTriangle { triangle_index: usize },
    #[error("knife evaluated mesh triangle {triangle_index} has invalid vertex index {index} (vertex count {vertex_count})")]
    KnifeEvaluatedMeshInvalidIndex {
        triangle_index: usize,
        index: u32,
        vertex_count: usize,
    },
    #[error("knife evaluated mesh edge {edge:?} has {incidence} incident triangles")]
    KnifeEvaluatedMeshNonManifoldEdge { edge: [u32; 2], incidence: usize },
    #[error("knife evaluated mesh repeats stable {kind} id {id}")]
    KnifeEvaluatedMeshDuplicateId { kind: &'static str, id: StableId },
    #[error("knife evaluated mesh {kind} {index} has a derived lineage/id mismatch")]
    KnifeEvaluatedMeshLineageMismatch { kind: &'static str, index: usize },
    #[error("knife evaluated mesh semantic hash does not match its contents")]
    KnifeEvaluatedMeshSemanticHashMismatch,
    #[error("knife thickness frame is degenerate at station {station_index}")]
    KnifeThicknessFrameDegenerate { station_index: usize },
    #[error("serialization failed: {0}")]
    Serialization(String),
    #[error("selection query contains duplicate element reference {element:?}")]
    DuplicateSelectionElement { element: ElementRef },
    #[error("selection query contains an element of kind {actual:?}, expected {expected:?}")]
    SelectionKindMismatch {
        expected: ElementKind,
        actual: ElementKind,
    },
    #[error("selection resolution is bound to a different source revision")]
    SelectionRevisionMismatch,
    #[error("selection resolution has too many elements: {count} > {maximum}")]
    SelectionBudgetExceeded { count: usize, maximum: usize },
    #[error("graph node budget exceeded: {count} > {maximum}")]
    GraphNodeBudgetExceeded { count: usize, maximum: usize },
    #[error("graph edge budget exceeded: {count} > {maximum}")]
    GraphEdgeBudgetExceeded { count: usize, maximum: usize },
    #[error("duplicate modifier node id {node_id}")]
    DuplicateNodeId { node_id: StableId },
    #[error("modifier node {node_id} repeats input {input_id}")]
    DuplicateInput {
        node_id: StableId,
        input_id: StableId,
    },
    #[error("modifier node {node_id} references missing input {input_id}")]
    MissingInput {
        node_id: StableId,
        input_id: StableId,
    },
    #[error("modifier graph output {node_id} does not exist")]
    MissingOutput { node_id: StableId },
    #[error("modifier graph repeats output {node_id}")]
    DuplicateOutput { node_id: StableId },
    #[error("modifier graph uses reserved node id {node_id}")]
    ReservedNodeId { node_id: StableId },
    #[error("dependency graph contains a cycle: {node_ids:?}")]
    Cycle { node_ids: Vec<StableId> },
    #[error("dirty seed {node_id} is not present in the dependency graph")]
    UnknownDirtySeed { node_id: StableId },
    #[error("evaluated mesh identity contains duplicate input hash {hash}")]
    DuplicateEvaluationInput { hash: Sha256Hash },
}

fn canonical_hash<T: Serialize>(value: &T) -> Result<Sha256Hash, WeaponryDccError> {
    let value = serde_json::to_value(value)
        .map_err(|error| WeaponryDccError::Serialization(error.to_string()))?;
    Sha256Hash::new(canonical_json_hash(&value))
}

fn validate_finite(field: &str, value: f64) -> Result<(), WeaponryDccError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(WeaponryDccError::InvalidNumber {
            field: field.to_owned(),
            reason: "must be finite".to_owned(),
        })
    }
}

fn validate_bounded(
    field: &str,
    value: f64,
    minimum: f64,
    maximum: f64,
) -> Result<(), WeaponryDccError> {
    validate_finite(field, value)?;
    if !(minimum..=maximum).contains(&value) {
        return Err(WeaponryDccError::InvalidNumber {
            field: field.to_owned(),
            reason: format!("must be in [{minimum}, {maximum}]"),
        });
    }
    Ok(())
}

fn validate_vec3(
    field: &str,
    value: &[f64; 3],
    minimum: f64,
    maximum: f64,
) -> Result<(), WeaponryDccError> {
    for (index, component) in value.iter().enumerate() {
        validate_bounded(&format!("{field}[{index}]"), *component, minimum, maximum)?;
    }
    Ok(())
}

/// The semantic role of a knife curve.  Roles are intentionally closed so a
/// free-form string cannot silently become a new geometry input category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnifeCurveRole {
    BladeSpine,
    BladeEdge,
    Profile,
}

/// Bounded curve bases supported by the pure Rust kernel.  `NurbsLike` means a
/// clamped, rational B-spline evaluation with explicit normalized knots; it
/// does not expose Blender or any external scripting/runtime API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnifeCurveBasis {
    Bezier,
    NurbsLike,
}

/// A closed, typed curve source for the knife authoring slice.
///
/// The value contains only bounded control data.  It deliberately has no
/// path, URL, script, arbitrary JSON, or raw mesh field.  Bezier curves are a
/// single bounded segment; NURBS-like curves use clamped normalized knots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnifeCurve {
    pub curve_id: StableId,
    pub role: KnifeCurveRole,
    pub basis: KnifeCurveBasis,
    pub degree: u8,
    pub control_points_m: Vec<[f64; 3]>,
    pub weights: Vec<f64>,
    pub knots: Vec<f64>,
    pub closed: bool,
}

impl KnifeCurve {
    pub fn new(
        curve_id: impl AsRef<str>,
        role: KnifeCurveRole,
        basis: KnifeCurveBasis,
        degree: u8,
        control_points_m: Vec<[f64; 3]>,
        weights: Vec<f64>,
        knots: Vec<f64>,
        closed: bool,
    ) -> Result<Self, WeaponryDccError> {
        let curve_id = StableId::new(curve_id)?;
        let invalid = |reason: String| WeaponryDccError::InvalidKnifeCurve {
            curve_id: curve_id.as_str().to_owned(),
            reason,
        };

        if control_points_m.is_empty() {
            return Err(invalid(
                "must contain at least one control point".to_owned(),
            ));
        }
        if control_points_m.len() > MAX_KNIFE_CURVE_CONTROL_POINTS {
            return Err(invalid(format!(
                "control point count must be at most {MAX_KNIFE_CURVE_CONTROL_POINTS}"
            )));
        }
        if !(1..=MAX_KNIFE_CURVE_DEGREE).contains(&degree) {
            return Err(invalid(format!(
                "degree must be in [1, {MAX_KNIFE_CURVE_DEGREE}]"
            )));
        }
        for (index, point) in control_points_m.iter().enumerate() {
            validate_vec3(
                &format!("curve.control_points_m[{index}]"),
                point,
                -MAX_COORDINATE_M,
                MAX_COORDINATE_M,
            )
            .map_err(|error| invalid(error.to_string()))?;
        }

        match basis {
            KnifeCurveBasis::Bezier => {
                if control_points_m.len() != degree as usize + 1 {
                    return Err(invalid(format!(
                        "Bezier requires exactly degree + 1 control points, got {}",
                        control_points_m.len()
                    )));
                }
                if !weights.is_empty() {
                    return Err(invalid("Bezier weights must be omitted".to_owned()));
                }
                if !knots.is_empty() {
                    return Err(invalid("Bezier knots must be omitted".to_owned()));
                }
            }
            KnifeCurveBasis::NurbsLike => {
                if weights.len() != control_points_m.len() {
                    return Err(invalid(format!(
                        "NURBS-like weights must match control point count, got {} vs {}",
                        weights.len(),
                        control_points_m.len()
                    )));
                }
                for (index, weight) in weights.iter().enumerate() {
                    validate_bounded(
                        &format!("curve.weights[{index}]"),
                        *weight,
                        MIN_KNIFE_CURVE_WEIGHT,
                        MAX_KNIFE_CURVE_WEIGHT,
                    )
                    .map_err(|error| invalid(error.to_string()))?;
                }
                let expected_knot_count = control_points_m.len() + degree as usize + 1;
                if knots.len() != expected_knot_count {
                    return Err(invalid(format!(
                        "NURBS-like knot count must be control points + degree + 1 ({expected_knot_count})"
                    )));
                }
                if knots.len() > MAX_KNIFE_CURVE_KNOTS {
                    return Err(invalid(format!(
                        "knot count must be at most {MAX_KNIFE_CURVE_KNOTS}"
                    )));
                }
                for (index, knot) in knots.iter().enumerate() {
                    validate_bounded(&format!("curve.knots[{index}]"), *knot, 0.0, 1.0)
                        .map_err(|error| invalid(error.to_string()))?;
                    if let Some(previous) = index.checked_sub(1).and_then(|i| knots.get(i)) {
                        if *knot < *previous {
                            return Err(invalid("knots must be non-decreasing".to_owned()));
                        }
                    }
                }
                let degree_index = degree as usize;
                if knots[..=degree_index].iter().any(|knot| *knot != 0.0) {
                    return Err(invalid(
                        "NURBS-like knots must be clamped at zero".to_owned(),
                    ));
                }
                let last_start = knots.len() - (degree_index + 1);
                if knots[last_start..].iter().any(|knot| *knot != 1.0) {
                    return Err(invalid(
                        "NURBS-like knots must be clamped at one".to_owned(),
                    ));
                }
                if knots[control_points_m.len()] <= knots[degree_index] {
                    return Err(invalid(
                        "NURBS-like parameter domain must be non-empty".to_owned(),
                    ));
                }
            }
        }

        Ok(Self {
            curve_id,
            role,
            basis,
            degree,
            control_points_m,
            weights,
            knots,
            closed,
        })
    }

    pub fn validate(&self) -> Result<(), WeaponryDccError> {
        Self::new(
            self.curve_id.as_str(),
            self.role,
            self.basis,
            self.degree,
            self.control_points_m.clone(),
            self.weights.clone(),
            self.knots.clone(),
            self.closed,
        )
        .map(|_| ())
    }

    pub fn canonical_sha256(&self) -> Result<Sha256Hash, WeaponryDccError> {
        self.validate()?;
        canonical_hash(self)
    }

    fn parameter_domain(&self) -> Result<(f64, f64), WeaponryDccError> {
        self.validate()?;
        match self.basis {
            KnifeCurveBasis::Bezier => Ok((0.0, 1.0)),
            KnifeCurveBasis::NurbsLike => Ok((
                self.knots[self.degree as usize],
                self.knots[self.control_points_m.len()],
            )),
        }
    }

    /// Build a deterministic sampling/tessellation plan bound to this exact
    /// curve hash.  The plan is identity-bearing but has no execution side
    /// effects.
    pub fn tessellation_plan(
        &self,
        sample_count: u32,
        tolerance_m: f64,
        max_segment_length_m: f64,
    ) -> Result<KnifeCurveTessellationPlan, WeaponryDccError> {
        let (parameter_start, parameter_end) = self.parameter_domain()?;
        if sample_count < 2 || sample_count as usize > MAX_KNIFE_CURVE_SAMPLES {
            return Err(WeaponryDccError::CurveSampleBudgetExceeded {
                count: sample_count as usize,
                maximum: MAX_KNIFE_CURVE_SAMPLES,
            });
        }
        validate_bounded(
            "curve.tessellation.tolerance_m",
            tolerance_m,
            1.0e-8,
            MAX_KNIFE_CURVE_TOLERANCE_M,
        )?;
        validate_bounded(
            "curve.tessellation.max_segment_length_m",
            max_segment_length_m,
            1.0e-8,
            MAX_COORDINATE_M * 2.0,
        )?;
        let plan = KnifeCurveTessellationPlan {
            curve_sha256: self.canonical_sha256()?,
            sample_count,
            parameter_start,
            parameter_end,
            tolerance_m,
            max_segment_length_m,
        };
        plan.validate()?;
        Ok(plan)
    }

    /// Sample the curve with a stable, inclusive linear parameter schedule.
    /// The output is point data for a later typed compiler; this method does
    /// not create a mesh or touch persistence.
    pub fn sample(
        &self,
        plan: &KnifeCurveTessellationPlan,
    ) -> Result<KnifeCurveSampleSet, WeaponryDccError> {
        self.validate()?;
        plan.validate()?;
        if plan.curve_sha256 != self.canonical_sha256()? {
            return Err(WeaponryDccError::CurvePlanBindingMismatch);
        }
        let denominator = (plan.sample_count - 1) as f64;
        let span = plan.parameter_end - plan.parameter_start;
        let mut points_m = Vec::with_capacity(plan.sample_count as usize);
        for index in 0..plan.sample_count {
            let parameter = plan.parameter_start + span * (index as f64 / denominator);
            let point = self.evaluate_parameter(parameter)?;
            validate_vec3(
                &format!("curve.samples[{index}]"),
                &point,
                -MAX_COORDINATE_M,
                MAX_COORDINATE_M,
            )?;
            points_m.push(quantize_dcc_position(point));
        }
        let samples = KnifeCurveSampleSet {
            curve_sha256: plan.curve_sha256.clone(),
            plan_sha256: plan.canonical_sha256()?,
            points_m,
        };
        samples.validate()?;
        Ok(samples)
    }

    fn evaluate_parameter(&self, parameter: f64) -> Result<[f64; 3], WeaponryDccError> {
        validate_bounded("curve.parameter", parameter, 0.0, 1.0)?;
        match self.basis {
            KnifeCurveBasis::Bezier => {
                let mut points = self.control_points_m.clone();
                for level in 1..points.len() {
                    for index in 0..(points.len() - level) {
                        for component in 0..3 {
                            points[index][component] = points[index][component]
                                + (points[index + 1][component] - points[index][component])
                                    * parameter;
                        }
                    }
                }
                Ok(points[0])
            }
            KnifeCurveBasis::NurbsLike => {
                if parameter == 1.0 {
                    return Ok(*self
                        .control_points_m
                        .last()
                        .expect("validated curve points"));
                }
                let mut numerator = [0.0; 3];
                let mut denominator = 0.0;
                for (index, point) in self.control_points_m.iter().enumerate() {
                    let basis = bspline_basis(
                        index,
                        self.degree as usize,
                        parameter,
                        &self.knots,
                        self.control_points_m.len(),
                    );
                    let weighted_basis = basis * self.weights[index];
                    denominator += weighted_basis;
                    for component in 0..3 {
                        numerator[component] += point[component] * weighted_basis;
                    }
                }
                if !denominator.is_finite() || denominator <= f64::EPSILON {
                    return Err(WeaponryDccError::InvalidKnifeCurve {
                        curve_id: self.curve_id.as_str().to_owned(),
                        reason: "NURBS-like basis denominator is not positive and finite"
                            .to_owned(),
                    });
                }
                Ok([
                    numerator[0] / denominator,
                    numerator[1] / denominator,
                    numerator[2] / denominator,
                ])
            }
        }
    }
}

/// A deterministic, curve-bound sampling plan.  The alias keeps the API
/// readable for callers that refer to this as a sampling plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnifeCurveTessellationPlan {
    pub curve_sha256: Sha256Hash,
    pub sample_count: u32,
    pub parameter_start: f64,
    pub parameter_end: f64,
    pub tolerance_m: f64,
    pub max_segment_length_m: f64,
}

pub type KnifeCurveSamplingPlan = KnifeCurveTessellationPlan;

impl KnifeCurveTessellationPlan {
    pub fn validate(&self) -> Result<(), WeaponryDccError> {
        Sha256Hash::new(self.curve_sha256.as_str())?;
        if self.sample_count < 2 || self.sample_count as usize > MAX_KNIFE_CURVE_SAMPLES {
            return Err(WeaponryDccError::CurveSampleBudgetExceeded {
                count: self.sample_count as usize,
                maximum: MAX_KNIFE_CURVE_SAMPLES,
            });
        }
        validate_bounded("curve.plan.parameter_start", self.parameter_start, 0.0, 1.0).map_err(
            |error| WeaponryDccError::CurvePlanInvalid {
                reason: error.to_string(),
            },
        )?;
        validate_bounded("curve.plan.parameter_end", self.parameter_end, 0.0, 1.0).map_err(
            |error| WeaponryDccError::CurvePlanInvalid {
                reason: error.to_string(),
            },
        )?;
        if self.parameter_start >= self.parameter_end {
            return Err(WeaponryDccError::CurvePlanInvalid {
                reason: "parameter domain must be increasing".to_owned(),
            });
        }
        validate_bounded(
            "curve.plan.tolerance_m",
            self.tolerance_m,
            1.0e-8,
            MAX_KNIFE_CURVE_TOLERANCE_M,
        )
        .map_err(|error| WeaponryDccError::CurvePlanInvalid {
            reason: error.to_string(),
        })?;
        validate_bounded(
            "curve.plan.max_segment_length_m",
            self.max_segment_length_m,
            1.0e-8,
            MAX_COORDINATE_M * 2.0,
        )
        .map_err(|error| WeaponryDccError::CurvePlanInvalid {
            reason: error.to_string(),
        })?;
        Ok(())
    }

    pub fn canonical_sha256(&self) -> Result<Sha256Hash, WeaponryDccError> {
        self.validate()?;
        canonical_hash(self)
    }
}

/// Deterministic point samples tied to both a curve identity and plan
/// identity.  This is not a mesh and carries no external resource locator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnifeCurveSampleSet {
    pub curve_sha256: Sha256Hash,
    pub plan_sha256: Sha256Hash,
    pub points_m: Vec<[f64; 3]>,
}

impl KnifeCurveSampleSet {
    pub fn validate(&self) -> Result<(), WeaponryDccError> {
        Sha256Hash::new(self.curve_sha256.as_str())?;
        Sha256Hash::new(self.plan_sha256.as_str())?;
        if self.points_m.len() < 2 || self.points_m.len() > MAX_KNIFE_CURVE_SAMPLES {
            return Err(WeaponryDccError::CurveSampleBudgetExceeded {
                count: self.points_m.len(),
                maximum: MAX_KNIFE_CURVE_SAMPLES,
            });
        }
        for (index, point) in self.points_m.iter().enumerate() {
            validate_vec3(
                &format!("curve.samples[{index}]"),
                point,
                -MAX_COORDINATE_M,
                MAX_COORDINATE_M,
            )?;
            if *point != quantize_dcc_position(*point) {
                return Err(WeaponryDccError::CurvePlanInvalid {
                    reason: format!("curve.samples[{index}] is outside the fixed nanometre grid"),
                });
            }
        }
        Ok(())
    }

    pub fn canonical_sha256(&self) -> Result<Sha256Hash, WeaponryDccError> {
        self.validate()?;
        canonical_hash(self)
    }
}

fn bspline_basis(
    index: usize,
    degree: usize,
    parameter: f64,
    knots: &[f64],
    control_point_count: usize,
) -> f64 {
    if degree == 0 {
        let in_span = knots[index] <= parameter && parameter < knots[index + 1];
        let at_terminal = parameter == 1.0 && index + 1 == control_point_count;
        return if in_span || at_terminal { 1.0 } else { 0.0 };
    }
    let left_denominator = knots[index + degree] - knots[index];
    let right_denominator = knots[index + degree + 1] - knots[index + 1];
    let left = if left_denominator > 0.0 {
        (parameter - knots[index]) / left_denominator
            * bspline_basis(index, degree - 1, parameter, knots, control_point_count)
    } else {
        0.0
    };
    let right = if right_denominator > 0.0 {
        (knots[index + degree + 1] - parameter) / right_denominator
            * bspline_basis(index + 1, degree - 1, parameter, knots, control_point_count)
    } else {
        0.0
    };
    left + right
}

/// Mesh element domains used by deterministic selections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementKind {
    Vertex,
    Edge,
    HalfEdge,
    Corner,
    Face,
    Loop,
    Ring,
}

/// A stable source element reference.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ElementRef {
    pub kind: ElementKind,
    pub id: StableId,
}

impl ElementRef {
    pub fn new(kind: ElementKind, id: impl AsRef<str>) -> Result<Self, WeaponryDccError> {
        Ok(Self {
            kind,
            id: StableId::new(id)?,
        })
    }
}

/// Semantic scope for a selection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "id")]
pub enum SelectionScope {
    Any,
    Part(StableId),
    MaterialZone(StableId),
}

/// Predicate vocabulary for the first agent-native selection slice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SelectionPredicate {
    Boundary { is_boundary: bool },
    Adjacency { depth: u8 },
    Normal { direction: [f64; 3], min_dot: f64 },
    Angle { min_degrees: f64, max_degrees: f64 },
}

impl SelectionPredicate {
    fn validate(&self) -> Result<(), WeaponryDccError> {
        match self {
            Self::Boundary { .. } => Ok(()),
            Self::Adjacency { depth } => {
                if *depth <= MAX_SELECTION_ADJACENCY_DEPTH {
                    Ok(())
                } else {
                    Err(WeaponryDccError::InvalidNumber {
                        field: "selection.adjacency.depth".to_owned(),
                        reason: format!("must be <= {MAX_SELECTION_ADJACENCY_DEPTH}"),
                    })
                }
            }
            Self::Normal { direction, min_dot } => {
                validate_vec3("selection.normal.direction", direction, -1.0, 1.0)?;
                if direction
                    .iter()
                    .all(|component| component.abs() <= f64::EPSILON)
                {
                    return Err(WeaponryDccError::InvalidNumber {
                        field: "selection.normal.direction".to_owned(),
                        reason: "must not be zero".to_owned(),
                    });
                }
                validate_bounded("selection.normal.min_dot", *min_dot, -1.0, 1.0)
            }
            Self::Angle {
                min_degrees,
                max_degrees,
            } => {
                validate_bounded("selection.angle.min_degrees", *min_degrees, 0.0, 180.0)?;
                validate_bounded("selection.angle.max_degrees", *max_degrees, 0.0, 180.0)?;
                if min_degrees > max_degrees {
                    return Err(WeaponryDccError::InvalidNumber {
                        field: "selection.angle".to_owned(),
                        reason: "minimum must not exceed maximum".to_owned(),
                    });
                }
                Ok(())
            }
        }
    }
}

/// The initial seed for a deterministic query. Camera visibility is
/// intentionally absent: it belongs to the read-only Viewer projection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "refs")]
pub enum SelectionSeed {
    Explicit(Vec<ElementRef>),
    Scope,
}

/// A query bound to one immutable AuthoringMesh revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectionQuery {
    pub query_id: StableId,
    pub source_revision_id: StableId,
    pub source_revision_sha256: Sha256Hash,
    pub scope: SelectionScope,
    pub element_kind: ElementKind,
    pub seed: SelectionSeed,
    pub predicates: Vec<SelectionPredicate>,
}

impl SelectionQuery {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        query_id: impl AsRef<str>,
        source_revision_id: impl AsRef<str>,
        source_revision_sha256: Sha256Hash,
        scope: SelectionScope,
        element_kind: ElementKind,
        seed: SelectionSeed,
        predicates: Vec<SelectionPredicate>,
    ) -> Result<Self, WeaponryDccError> {
        let query = Self {
            query_id: StableId::new(query_id)?,
            source_revision_id: StableId::new(source_revision_id)?,
            source_revision_sha256,
            scope,
            element_kind,
            seed,
            predicates,
        };
        query.validate()?;
        Ok(query)
    }

    pub fn validate(&self) -> Result<(), WeaponryDccError> {
        if self.query_id.as_str() == SOURCE_REVISION_NODE_ID
            || self.query_id.as_str().starts_with("__selection-")
        {
            return Err(WeaponryDccError::InvalidStableId {
                value: self.query_id.as_str().to_owned(),
                reason: "selection query id uses a reserved internal prefix".to_owned(),
            });
        }
        match &self.seed {
            SelectionSeed::Explicit(elements) => {
                if elements.is_empty() || elements.len() > MAX_SELECTION_REFS {
                    return Err(WeaponryDccError::SelectionBudgetExceeded {
                        count: elements.len(),
                        maximum: MAX_SELECTION_REFS,
                    });
                }
                let mut seen = BTreeSet::new();
                for element in elements {
                    if element.kind != self.element_kind {
                        return Err(WeaponryDccError::SelectionKindMismatch {
                            expected: self.element_kind,
                            actual: element.kind,
                        });
                    }
                    if !seen.insert(element) {
                        return Err(WeaponryDccError::DuplicateSelectionElement {
                            element: element.clone(),
                        });
                    }
                }
            }
            SelectionSeed::Scope => {}
        }
        for predicate in &self.predicates {
            predicate.validate()?;
        }
        Ok(())
    }

    pub fn canonical_sha256(&self) -> Result<Sha256Hash, WeaponryDccError> {
        self.validate()?;
        canonical_hash(self)
    }
}

/// Runtime-derived resolution of a SelectionQuery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionResolution {
    pub query_id: StableId,
    pub query_sha256: Sha256Hash,
    pub source_revision_id: StableId,
    pub source_revision_sha256: Sha256Hash,
    pub resolved_element_refs: Vec<ElementRef>,
    pub topology_digest: Sha256Hash,
}

impl SelectionResolution {
    pub fn new(
        query: &SelectionQuery,
        mut resolved_element_refs: Vec<ElementRef>,
        topology_digest: Sha256Hash,
    ) -> Result<Self, WeaponryDccError> {
        query.validate()?;
        if resolved_element_refs.len() > MAX_SELECTION_REFS {
            return Err(WeaponryDccError::SelectionBudgetExceeded {
                count: resolved_element_refs.len(),
                maximum: MAX_SELECTION_REFS,
            });
        }
        let mut seen = BTreeSet::new();
        for element in &resolved_element_refs {
            if element.kind != query.element_kind {
                return Err(WeaponryDccError::SelectionKindMismatch {
                    expected: query.element_kind,
                    actual: element.kind,
                });
            }
            if !seen.insert(element) {
                return Err(WeaponryDccError::DuplicateSelectionElement {
                    element: element.clone(),
                });
            }
        }
        resolved_element_refs.sort();
        Ok(Self {
            query_id: query.query_id.clone(),
            query_sha256: query.canonical_sha256()?,
            source_revision_id: query.source_revision_id.clone(),
            source_revision_sha256: query.source_revision_sha256.clone(),
            resolved_element_refs,
            topology_digest,
        })
    }

    pub fn validate_against(&self, query: &SelectionQuery) -> Result<(), WeaponryDccError> {
        query.validate()?;
        if self.query_id != query.query_id
            || self.source_revision_id != query.source_revision_id
            || self.source_revision_sha256 != query.source_revision_sha256
            || self.query_sha256 != query.canonical_sha256()?
        {
            return Err(WeaponryDccError::SelectionRevisionMismatch);
        }
        Ok(())
    }

    pub fn canonical_sha256(&self) -> Result<Sha256Hash, WeaponryDccError> {
        canonical_hash(self)
    }
}

/// The first typed modifier vocabulary. Keeping this an enum prevents an
/// arbitrary JSON object from becoming executable geometry policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "operator")]
pub enum ModifierKind {
    Transform {
        translation_m: [f64; 3],
        rotation_rad: [f64; 3],
        scale: [f64; 3],
    },
    Mirror {
        axis: MirrorAxis,
        offset_m: f64,
    },
    Array {
        count: u32,
        offset_m: [f64; 3],
    },
    Bevel {
        width_m: f64,
        segments: u8,
        profile: f64,
        clamp_overlap: bool,
    },
    NormalPolicy {
        crease_angle_rad: f64,
    },
    /// A typed curve source used by a downstream profile/edge operation.
    /// The curve payload stays outside the graph; this node binds its stable
    /// ID and canonical identity without copying any control data into the
    /// modifier schema.
    CurveProfile {
        curve_id: StableId,
        curve_sha256: Sha256Hash,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorAxis {
    X,
    Y,
    Z,
}

impl ModifierKind {
    fn validate(&self) -> Result<(), WeaponryDccError> {
        match self {
            Self::Transform {
                translation_m,
                rotation_rad,
                scale,
            } => {
                validate_vec3(
                    "transform.translation_m",
                    translation_m,
                    -MAX_COORDINATE_M,
                    MAX_COORDINATE_M,
                )?;
                validate_vec3(
                    "transform.rotation_rad",
                    rotation_rad,
                    -std::f64::consts::TAU,
                    std::f64::consts::TAU,
                )?;
                for (index, value) in scale.iter().enumerate() {
                    validate_bounded(&format!("transform.scale[{index}]"), *value, 1.0e-6, 10.0)?;
                }
                Ok(())
            }
            Self::Mirror { offset_m, .. } => validate_bounded(
                "mirror.offset_m",
                *offset_m,
                -MAX_COORDINATE_M,
                MAX_COORDINATE_M,
            ),
            Self::Array { count, offset_m } => {
                if !(1..=MAX_ARRAY_COUNT).contains(count) {
                    return Err(WeaponryDccError::InvalidNumber {
                        field: "array.count".to_owned(),
                        reason: format!("must be in [1, {MAX_ARRAY_COUNT}]"),
                    });
                }
                validate_vec3(
                    "array.offset_m",
                    offset_m,
                    -MAX_COORDINATE_M,
                    MAX_COORDINATE_M,
                )
            }
            Self::Bevel {
                width_m,
                segments,
                profile,
                ..
            } => {
                validate_bounded("bevel.width_m", *width_m, 0.0, MAX_BEVEL_WIDTH_M)?;
                if !(1..=MAX_MODIFIER_SEGMENTS).contains(segments) {
                    return Err(WeaponryDccError::InvalidNumber {
                        field: "bevel.segments".to_owned(),
                        reason: format!("must be in [1, {MAX_MODIFIER_SEGMENTS}]"),
                    });
                }
                validate_bounded("bevel.profile", *profile, 0.25, 0.75)
            }
            Self::NormalPolicy { crease_angle_rad } => validate_bounded(
                "normal_policy.crease_angle_rad",
                *crease_angle_rad,
                0.0,
                std::f64::consts::PI,
            ),
            Self::CurveProfile {
                curve_id,
                curve_sha256,
            } => {
                StableId::new(curve_id.as_str())?;
                Sha256Hash::new(curve_sha256.as_str())?;
                Ok(())
            }
        }
    }

    pub fn curve_profile(curve: &KnifeCurve) -> Result<Self, WeaponryDccError> {
        Ok(Self::CurveProfile {
            curve_id: curve.curve_id.clone(),
            curve_sha256: curve.canonical_sha256()?,
        })
    }
}

/// One immutable modifier node. Inputs are canonicalized by stable ID and the
/// node remains traceable even when enabled is false.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModifierNode {
    pub node_id: StableId,
    pub operator: ModifierKind,
    pub input_node_ids: Vec<StableId>,
    pub selection_query_sha256: Option<Sha256Hash>,
    pub enabled: bool,
}

impl ModifierNode {
    pub fn new(
        node_id: impl AsRef<str>,
        operator: ModifierKind,
        mut input_node_ids: Vec<StableId>,
        selection_query_sha256: Option<Sha256Hash>,
        enabled: bool,
    ) -> Result<Self, WeaponryDccError> {
        let node_id = StableId::new(node_id)?;
        input_node_ids.sort();
        for inputs in input_node_ids.windows(2) {
            if inputs[0] == inputs[1] {
                return Err(WeaponryDccError::DuplicateInput {
                    node_id,
                    input_id: inputs[0].clone(),
                });
            }
        }
        if let Some(selection_hash) = &selection_query_sha256 {
            Sha256Hash::new(selection_hash.as_str())?;
        }
        operator.validate()?;
        Ok(Self {
            node_id,
            operator,
            input_node_ids,
            selection_query_sha256,
            enabled,
        })
    }
}

/// A canonical, acyclic, non-destructive modifier graph attached to one
/// AuthoringMesh revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModifierGraph {
    pub graph_id: StableId,
    pub source_revision_id: StableId,
    pub source_revision_sha256: Sha256Hash,
    pub nodes: Vec<ModifierNode>,
    pub output_node_ids: Vec<StableId>,
    #[serde(skip)]
    topological_order: Vec<StableId>,
}

impl ModifierGraph {
    pub fn new(
        graph_id: impl AsRef<str>,
        source_revision_id: impl AsRef<str>,
        source_revision_sha256: Sha256Hash,
        nodes: Vec<ModifierNode>,
        mut output_node_ids: Vec<StableId>,
    ) -> Result<Self, WeaponryDccError> {
        if nodes.len() > MAX_GRAPH_NODES {
            return Err(WeaponryDccError::GraphNodeBudgetExceeded {
                count: nodes.len(),
                maximum: MAX_GRAPH_NODES,
            });
        }
        let graph_id = StableId::new(graph_id)?;
        let source_revision_id = StableId::new(source_revision_id)?;
        let mut by_id = BTreeMap::new();
        for node in nodes {
            if node.node_id.as_str() == SOURCE_REVISION_NODE_ID
                || node.node_id.as_str().starts_with("__selection-")
                || node.node_id.as_str().starts_with("__curve-")
            {
                return Err(WeaponryDccError::ReservedNodeId {
                    node_id: node.node_id,
                });
            }
            let node_id = node.node_id.clone();
            if by_id.insert(node_id.clone(), node).is_some() {
                return Err(WeaponryDccError::DuplicateNodeId { node_id });
            }
        }
        let edge_count = by_id
            .values()
            .map(|node| node.input_node_ids.len())
            .sum::<usize>();
        if edge_count > MAX_GRAPH_EDGES {
            return Err(WeaponryDccError::GraphEdgeBudgetExceeded {
                count: edge_count,
                maximum: MAX_GRAPH_EDGES,
            });
        }
        let mut dependencies = BTreeMap::new();
        for (node_id, node) in &by_id {
            for input_id in &node.input_node_ids {
                if !by_id.contains_key(input_id) {
                    return Err(WeaponryDccError::MissingInput {
                        node_id: node_id.clone(),
                        input_id: input_id.clone(),
                    });
                }
            }
            dependencies.insert(node_id.clone(), node.input_node_ids.clone());
        }
        for output_node_id in &output_node_ids {
            if !by_id.contains_key(output_node_id) {
                return Err(WeaponryDccError::MissingOutput {
                    node_id: output_node_id.clone(),
                });
            }
        }
        output_node_ids.sort();
        for outputs in output_node_ids.windows(2) {
            if outputs[0] == outputs[1] {
                return Err(WeaponryDccError::DuplicateOutput {
                    node_id: outputs[0].clone(),
                });
            }
        }
        let topological_order = topological_order(&dependencies)?;
        Ok(Self {
            graph_id,
            source_revision_id,
            source_revision_sha256,
            nodes: by_id.into_values().collect(),
            output_node_ids,
            topological_order,
        })
    }

    pub fn node(&self, node_id: impl AsRef<str>) -> Option<&ModifierNode> {
        self.nodes
            .iter()
            .find(|node| node.node_id.as_str() == node_id.as_ref())
    }

    pub fn topological_order(&self) -> &[StableId] {
        &self.topological_order
    }

    pub fn canonical_sha256(&self) -> Result<Sha256Hash, WeaponryDccError> {
        canonical_hash(self)
    }

    pub fn dependency_graph(&self) -> Result<DependencyGraph, WeaponryDccError> {
        DependencyGraph::from_modifier_graph(self)
    }
}

/// The derived execution graph. It includes source and selection roots in
/// addition to ModifierGraph nodes so dirty closure can distinguish a source
/// edit from a parameter or selection edit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyNode {
    pub node_id: StableId,
    pub kind: DependencyNodeKind,
    pub dependencies: Vec<StableId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyNodeKind {
    SourceRevision,
    SelectionQuery,
    CurveSource,
    Modifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyGraph {
    pub nodes: Vec<DependencyNode>,
    #[serde(skip)]
    dependents: BTreeMap<StableId, BTreeSet<StableId>>,
    #[serde(skip)]
    topological_order: Vec<StableId>,
}

impl DependencyGraph {
    fn from_modifier_graph(graph: &ModifierGraph) -> Result<Self, WeaponryDccError> {
        let source_id = StableId::new(SOURCE_REVISION_NODE_ID)?;
        let mut nodes = BTreeMap::new();
        nodes.insert(
            source_id.clone(),
            DependencyNode {
                node_id: source_id.clone(),
                kind: DependencyNodeKind::SourceRevision,
                dependencies: Vec::new(),
            },
        );

        for modifier in &graph.nodes {
            if let Some(selection_hash) = &modifier.selection_query_sha256 {
                let selection_id = selection_node_id(selection_hash)?;
                nodes
                    .entry(selection_id.clone())
                    .or_insert_with(|| DependencyNode {
                        node_id: selection_id,
                        kind: DependencyNodeKind::SelectionQuery,
                        dependencies: vec![source_id.clone()],
                    });
            }
            let mut dependencies = modifier.input_node_ids.clone();
            if dependencies.is_empty() {
                dependencies.push(source_id.clone());
            }
            if let Some(selection_hash) = &modifier.selection_query_sha256 {
                dependencies.push(selection_node_id(selection_hash)?);
            }
            if let ModifierKind::CurveProfile { curve_sha256, .. } = &modifier.operator {
                let curve_id = curve_source_node_id(curve_sha256)?;
                nodes
                    .entry(curve_id.clone())
                    .or_insert_with(|| DependencyNode {
                        node_id: curve_id.clone(),
                        kind: DependencyNodeKind::CurveSource,
                        dependencies: vec![source_id.clone()],
                    });
                dependencies.push(curve_id);
            }
            dependencies.sort();
            if let Some(pair) = dependencies.windows(2).find(|pair| pair[0] == pair[1]) {
                return Err(WeaponryDccError::DuplicateInput {
                    node_id: modifier.node_id.clone(),
                    input_id: pair[0].clone(),
                });
            }
            nodes.insert(
                modifier.node_id.clone(),
                DependencyNode {
                    node_id: modifier.node_id.clone(),
                    kind: DependencyNodeKind::Modifier,
                    dependencies,
                },
            );
        }
        let dependency_map = nodes
            .iter()
            .map(|(node_id, node)| (node_id.clone(), node.dependencies.clone()))
            .collect::<BTreeMap<_, _>>();
        let topological_order = topological_order(&dependency_map)?;
        let dependents = reverse_dependencies(&dependency_map);
        Ok(Self {
            nodes: nodes.into_values().collect(),
            dependents,
            topological_order,
        })
    }

    pub fn node(&self, node_id: impl AsRef<str>) -> Option<&DependencyNode> {
        self.nodes
            .iter()
            .find(|node| node.node_id.as_str() == node_id.as_ref())
    }

    pub fn topological_order(&self) -> &[StableId] {
        &self.topological_order
    }

    pub fn dirty_closure<I, D>(&self, dirty_seeds: I) -> Result<Vec<StableId>, WeaponryDccError>
    where
        I: IntoIterator<Item = D>,
        D: AsRef<str>,
    {
        let node_ids = self
            .nodes
            .iter()
            .map(|node| node.node_id.clone())
            .collect::<BTreeSet<_>>();
        let mut queue = VecDeque::new();
        let mut affected = BTreeSet::new();
        for dirty_seed in dirty_seeds {
            let dirty_seed = StableId::new(dirty_seed)?;
            if !node_ids.contains(&dirty_seed) {
                return Err(WeaponryDccError::UnknownDirtySeed {
                    node_id: dirty_seed,
                });
            }
            if affected.insert(dirty_seed.clone()) {
                queue.push_back(dirty_seed);
            }
        }
        while let Some(node_id) = queue.pop_front() {
            if let Some(children) = self.dependents.get(&node_id) {
                for child in children {
                    if affected.insert(child.clone()) {
                        queue.push_back(child.clone());
                    }
                }
            }
        }
        Ok(affected.into_iter().collect())
    }

    pub fn recompute_plan<I, D>(
        &self,
        dirty_seeds: I,
    ) -> Result<DependencyRecomputePlan, WeaponryDccError>
    where
        I: IntoIterator<Item = D>,
        D: AsRef<str>,
    {
        let dirty_nodes = self.dirty_closure(dirty_seeds)?;
        let dirty_set = dirty_nodes.iter().cloned().collect::<BTreeSet<_>>();
        let recompute_order = self
            .topological_order
            .iter()
            .filter(|node_id| dirty_set.contains(*node_id))
            .cloned()
            .collect();
        Ok(DependencyRecomputePlan {
            dirty_nodes,
            recompute_order,
        })
    }

    pub fn canonical_sha256(&self) -> Result<Sha256Hash, WeaponryDccError> {
        canonical_hash(self)
    }
}

/// A dependency-first recompute plan. It is a plan only; no Worker or
/// persistence side effect occurs in this module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRecomputePlan {
    pub dirty_nodes: Vec<StableId>,
    pub recompute_order: Vec<StableId>,
}

fn selection_node_id(selection_hash: &Sha256Hash) -> Result<StableId, WeaponryDccError> {
    StableId::new(format!("__selection-{}", selection_hash.as_str()))
}

fn curve_source_node_id(curve_hash: &Sha256Hash) -> Result<StableId, WeaponryDccError> {
    StableId::new(format!("__curve-{}", curve_hash.as_str()))
}

fn reverse_dependencies(
    dependencies: &BTreeMap<StableId, Vec<StableId>>,
) -> BTreeMap<StableId, BTreeSet<StableId>> {
    let mut reverse = dependencies
        .keys()
        .cloned()
        .map(|node_id| (node_id, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (node_id, inputs) in dependencies {
        for input in inputs {
            reverse
                .entry(input.clone())
                .or_default()
                .insert(node_id.clone());
        }
    }
    reverse
}

fn topological_order(
    dependencies: &BTreeMap<StableId, Vec<StableId>>,
) -> Result<Vec<StableId>, WeaponryDccError> {
    let mut indegree = BTreeMap::new();
    let reverse = reverse_dependencies(dependencies);
    for (node_id, inputs) in dependencies {
        for input in inputs {
            if !dependencies.contains_key(input) {
                return Err(WeaponryDccError::MissingInput {
                    node_id: node_id.clone(),
                    input_id: input.clone(),
                });
            }
        }
        indegree.insert(node_id.clone(), inputs.len());
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(node_id, _)| node_id.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(dependencies.len());
    while let Some(node_id) = ready.iter().next().cloned() {
        ready.remove(&node_id);
        order.push(node_id.clone());
        if let Some(children) = reverse.get(&node_id) {
            for child in children {
                let degree = indegree
                    .get_mut(child)
                    .expect("reverse dependency must refer to a graph node");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }
    if order.len() != dependencies.len() {
        return Err(WeaponryDccError::Cycle {
            node_ids: indegree
                .into_iter()
                .filter_map(|(node_id, degree)| (degree > 0).then_some(node_id))
                .collect(),
        });
    }
    Ok(order)
}

/// Immutable identity of one evaluated mesh. It deliberately contains only
/// source/graph/input/output hashes; no mesh buffer or external locator is
/// part of the identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatedMeshIdentity {
    pub source_revision_sha256: Sha256Hash,
    pub modifier_graph_sha256: Sha256Hash,
    pub input_evaluation_sha256: Vec<Sha256Hash>,
    pub output_mesh_sha256: Sha256Hash,
}

impl EvaluatedMeshIdentity {
    pub fn new(
        source_revision_sha256: Sha256Hash,
        modifier_graph_sha256: Sha256Hash,
        mut input_evaluation_sha256: Vec<Sha256Hash>,
        output_mesh_sha256: Sha256Hash,
    ) -> Result<Self, WeaponryDccError> {
        input_evaluation_sha256.sort();
        for inputs in input_evaluation_sha256.windows(2) {
            if inputs[0] == inputs[1] {
                return Err(WeaponryDccError::DuplicateEvaluationInput {
                    hash: inputs[0].clone(),
                });
            }
        }
        Ok(Self {
            source_revision_sha256,
            modifier_graph_sha256,
            input_evaluation_sha256,
            output_mesh_sha256,
        })
    }

    pub fn canonical_sha256(&self) -> Result<Sha256Hash, WeaponryDccError> {
        canonical_hash(self)
    }
}

/// A link suitable for a Runtime/CAS adapter. It carries no evaluated mesh
/// buffer and cannot become a second geometry truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatedMeshLink {
    pub identity: EvaluatedMeshIdentity,
}

impl EvaluatedMeshLink {
    pub fn new(identity: EvaluatedMeshIdentity) -> Self {
        Self { identity }
    }

    pub fn canonical_sha256(&self) -> Result<Sha256Hash, WeaponryDccError> {
        canonical_hash(self)
    }
}

/// Pure-kernel representation of an evaluated mesh: identity/link only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatedMesh {
    pub link: EvaluatedMeshLink,
}

impl EvaluatedMesh {
    pub fn new(identity: EvaluatedMeshIdentity) -> Self {
        Self {
            link: EvaluatedMeshLink::new(identity),
        }
    }
}

/// The typed axis used to close a blade section during the bounded sweep.
/// End caps are a property of the sweep plan; this enum only chooses the
/// thickness direction.  No arbitrary vector axis is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnifeThicknessAxis {
    LocalNormal,
    WorldX,
    WorldY,
    WorldZ,
}

impl KnifeThicknessAxis {
    fn vector(self) -> Option<[f64; 3]> {
        match self {
            Self::LocalNormal => None,
            Self::WorldX => Some([1.0, 0.0, 0.0]),
            Self::WorldY => Some([0.0, 1.0, 0.0]),
            Self::WorldZ => Some([0.0, 0.0, 1.0]),
        }
    }
}

/// The semantic side of a generated blade element.  It is closed so lineage
/// cannot be supplied as an arbitrary string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnifeMeshSide {
    Top,
    Bottom,
    Spine,
    Edge,
    StartCap,
    EndCap,
}

/// Stable lineage for a generated mesh element.  Both source curve hashes are
/// retained in every lineage record: this makes a perturbation to either rail
/// change the semantic identity without accepting a caller-provided mesh ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnifeMeshLineage {
    pub source_curve_sha256: Vec<Sha256Hash>,
    pub plan_sha256: Sha256Hash,
    pub station_index: u32,
    pub side: KnifeMeshSide,
    pub role: KnifeCurveRole,
    pub local_index: u8,
}

impl KnifeMeshLineage {
    fn new(
        source_curve_sha256: &[Sha256Hash],
        plan_sha256: &Sha256Hash,
        station_index: usize,
        side: KnifeMeshSide,
        role: KnifeCurveRole,
        local_index: u8,
    ) -> Result<Self, WeaponryDccError> {
        let station_index =
            u32::try_from(station_index).map_err(|_| WeaponryDccError::KnifeBladeInputInvalid {
                reason: "station index does not fit the bounded lineage type".to_owned(),
            })?;
        let lineage = Self {
            source_curve_sha256: source_curve_sha256.to_vec(),
            plan_sha256: plan_sha256.clone(),
            station_index,
            side,
            role,
            local_index,
        };
        lineage.validate()?;
        Ok(lineage)
    }

    pub fn validate(&self) -> Result<(), WeaponryDccError> {
        if self.source_curve_sha256.is_empty() || self.source_curve_sha256.len() > 4 {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "lineage must contain one to four source curve hashes".to_owned(),
            });
        }
        let mut hashes = BTreeSet::new();
        for hash in &self.source_curve_sha256 {
            Sha256Hash::new(hash.as_str())?;
            if !hashes.insert(hash) {
                return Err(WeaponryDccError::KnifeBladeInputInvalid {
                    reason: "lineage repeats a source curve hash".to_owned(),
                });
            }
        }
        Sha256Hash::new(self.plan_sha256.as_str())?;
        if self.station_index as usize >= MAX_KNIFE_CURVE_SAMPLES {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "lineage station exceeds the bounded curve sample range".to_owned(),
            });
        }
        if self.local_index > 3 {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "lineage local index exceeds the closed section range".to_owned(),
            });
        }
        Ok(())
    }

    pub fn canonical_sha256(&self) -> Result<Sha256Hash, WeaponryDccError> {
        self.validate()?;
        canonical_hash(self)
    }
}

fn derived_element_id(
    prefix: &str,
    lineage: &KnifeMeshLineage,
) -> Result<StableId, WeaponryDccError> {
    let lineage_hash = lineage.canonical_sha256()?;
    StableId::new(format!("{prefix}-{lineage_hash}"))
}

/// A generated vertex with an identity that is derived from typed curve and
/// plan lineage.  There is intentionally no public constructor accepting a
/// caller-supplied ID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluatedMeshVertex {
    pub vertex_id: StableId,
    pub position_m: [f64; 3],
    pub lineage: KnifeMeshLineage,
}

impl EvaluatedMeshVertex {
    fn generated(
        position_m: [f64; 3],
        lineage: KnifeMeshLineage,
    ) -> Result<Self, WeaponryDccError> {
        validate_vec3(
            "evaluated_mesh.vertex.position_m",
            &position_m,
            -MAX_EVALUATED_MESH_COORDINATE_M,
            MAX_EVALUATED_MESH_COORDINATE_M,
        )?;
        let position_m = quantize_dcc_position(position_m);
        let vertex_id = derived_element_id("knife-vertex", &lineage)?;
        Ok(Self {
            vertex_id,
            position_m,
            lineage,
        })
    }

    fn validate_at(&self, index: usize) -> Result<(), WeaponryDccError> {
        self.lineage.validate()?;
        validate_vec3(
            &format!("evaluated_mesh.vertices[{index}].position_m"),
            &self.position_m,
            -MAX_EVALUATED_MESH_COORDINATE_M,
            MAX_EVALUATED_MESH_COORDINATE_M,
        )?;
        if self.position_m != quantize_dcc_position(self.position_m) {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: format!(
                    "evaluated_mesh.vertices[{index}].position_m is outside the fixed nanometre grid"
                ),
            });
        }
        if self.vertex_id != derived_element_id("knife-vertex", &self.lineage)? {
            return Err(WeaponryDccError::KnifeEvaluatedMeshLineageMismatch {
                kind: "vertex",
                index,
            });
        }
        Ok(())
    }
}

fn quantize_dcc_position(position_m: [f64; 3]) -> [f64; 3] {
    position_m.map(|coordinate| {
        let quantized = (coordinate * EVALUATED_MESH_COORDINATE_QUANTIZATION_PER_M).round()
            / EVALUATED_MESH_COORDINATE_QUANTIZATION_PER_M;
        if quantized == 0.0 {
            0.0
        } else {
            quantized
        }
    })
}

/// A generated triangle with stable lineage.  Indices refer to the generated
/// vertex array and are checked as part of the mesh's strict validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatedMeshTriangle {
    pub triangle_id: StableId,
    pub indices: [u32; 3],
    pub lineage: KnifeMeshLineage,
}

impl EvaluatedMeshTriangle {
    fn generated(indices: [u32; 3], lineage: KnifeMeshLineage) -> Result<Self, WeaponryDccError> {
        let triangle_id = derived_element_id("knife-triangle", &lineage)?;
        Ok(Self {
            triangle_id,
            indices,
            lineage,
        })
    }

    fn validate_at(
        &self,
        index: usize,
        vertex_count: usize,
        vertices: &[EvaluatedMeshVertex],
    ) -> Result<(), WeaponryDccError> {
        self.lineage.validate()?;
        for vertex_index in self.indices {
            if vertex_index as usize >= vertex_count {
                return Err(WeaponryDccError::KnifeEvaluatedMeshInvalidIndex {
                    triangle_index: index,
                    index: vertex_index,
                    vertex_count,
                });
            }
        }
        if self.indices[0] == self.indices[1]
            || self.indices[1] == self.indices[2]
            || self.indices[0] == self.indices[2]
        {
            return Err(WeaponryDccError::KnifeEvaluatedMeshDegenerateTriangle {
                triangle_index: index,
            });
        }
        let a = vertices[self.indices[0] as usize].position_m;
        let b = vertices[self.indices[1] as usize].position_m;
        let c = vertices[self.indices[2] as usize].position_m;
        let area_vector = cross(subtract(b, a), subtract(c, a));
        let area_squared = dot(area_vector, area_vector);
        if !area_squared.is_finite() || area_squared <= MIN_KNIFE_TRIANGLE_AREA_SQUARED_M2 {
            return Err(WeaponryDccError::KnifeEvaluatedMeshDegenerateTriangle {
                triangle_index: index,
            });
        }
        if self.triangle_id != derived_element_id("knife-triangle", &self.lineage)? {
            return Err(WeaponryDccError::KnifeEvaluatedMeshLineageMismatch {
                kind: "triangle",
                index,
            });
        }
        Ok(())
    }
}

/// The bounded knife profile/loft/sweep request.  The two rails must have the
/// same number of samples; a four-sided closed section is swept between them.
/// It is a typed plan, not a raw mesh input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnifeBladeSweepPlan {
    pub spine_plan: KnifeCurveTessellationPlan,
    pub edge_plan: KnifeCurveTessellationPlan,
    pub thickness_axis: KnifeThicknessAxis,
    pub thickness_m: f64,
    pub root_cap: bool,
    pub tip_cap: bool,
}

pub type KnifeBladeMeshPlan = KnifeBladeSweepPlan;
pub type KnifeBladeProfileSweepPlan = KnifeBladeSweepPlan;

impl KnifeBladeSweepPlan {
    pub fn new(
        spine: &KnifeCurve,
        edge: &KnifeCurve,
        sample_count: u32,
        tolerance_m: f64,
        max_segment_length_m: f64,
        thickness_axis: KnifeThicknessAxis,
        thickness_m: f64,
    ) -> Result<Self, WeaponryDccError> {
        validate_curve_role(spine, KnifeCurveRole::BladeSpine)?;
        validate_curve_role(edge, KnifeCurveRole::BladeEdge)?;
        let plan = Self {
            spine_plan: spine.tessellation_plan(sample_count, tolerance_m, max_segment_length_m)?,
            edge_plan: edge.tessellation_plan(sample_count, tolerance_m, max_segment_length_m)?,
            thickness_axis,
            thickness_m,
            root_cap: true,
            tip_cap: true,
        };
        plan.validate()?;
        Ok(plan)
    }

    /// Convenience constructor for the bounded local-normal profile sweep.
    /// The tolerances are fixed kernel policy, not caller-supplied execution
    /// hooks.
    pub fn from_curves(
        spine: &KnifeCurve,
        edge: &KnifeCurve,
        sample_count: u32,
        thickness_m: f64,
    ) -> Result<Self, WeaponryDccError> {
        Self::new(
            spine,
            edge,
            sample_count,
            1.0e-4,
            1.0,
            KnifeThicknessAxis::LocalNormal,
            thickness_m,
        )
    }

    pub fn validate(&self) -> Result<(), WeaponryDccError> {
        self.spine_plan.validate()?;
        self.edge_plan.validate()?;
        if self.spine_plan.sample_count != self.edge_plan.sample_count {
            return Err(WeaponryDccError::KnifeBladeStationCountMismatch {
                spine: self.spine_plan.sample_count as usize,
                edge: self.edge_plan.sample_count as usize,
            });
        }
        if !self.root_cap || !self.tip_cap {
            return Err(WeaponryDccError::KnifeBladeSweepPlanInvalid {
                reason: "closed knife sweep requires root_cap=true and tip_cap=true".to_owned(),
            });
        }
        validate_bounded(
            "knife_blade.thickness_m",
            self.thickness_m,
            MIN_KNIFE_THICKNESS_M,
            MAX_KNIFE_THICKNESS_M,
        )
        .map_err(|error| WeaponryDccError::KnifeBladeSweepPlanInvalid {
            reason: error.to_string(),
        })?;
        Ok(())
    }

    pub fn canonical_sha256(&self) -> Result<Sha256Hash, WeaponryDccError> {
        self.validate()?;
        canonical_hash(self)
    }

    pub fn evaluate(
        &self,
        spine: &KnifeCurve,
        edge: &KnifeCurve,
    ) -> Result<EvaluatedMeshGeometry, WeaponryDccError> {
        evaluate_knife_blade_profile_sweep(spine, edge, self)
    }
}

/// Generated, disposable geometry from the knife curve kernel.  This is a
/// pure value: it has no CAS/GLB/Store handle, high/low/bake state, or visual
/// quality assertion.  Its semantic hash can be bound to the existing
/// EvaluatedMesh/EvaluatedMeshLink only when the caller supplies the real
/// source revision and modifier graph context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluatedMeshGeometry {
    pub plan_sha256: Sha256Hash,
    pub vertices: Vec<EvaluatedMeshVertex>,
    pub triangles: Vec<EvaluatedMeshTriangle>,
    pub semantic_sha256: Sha256Hash,
}

pub type KnifeEvaluatedMesh = EvaluatedMeshGeometry;
pub type BladeEvaluatedMesh = EvaluatedMeshGeometry;

#[derive(Serialize)]
struct EvaluatedMeshSemantic<'a> {
    plan_sha256: &'a Sha256Hash,
    vertices: &'a [EvaluatedMeshVertex],
    triangles: &'a [EvaluatedMeshTriangle],
}

impl EvaluatedMeshGeometry {
    fn semantic_hash_for(
        plan_sha256: &Sha256Hash,
        vertices: &[EvaluatedMeshVertex],
        triangles: &[EvaluatedMeshTriangle],
    ) -> Result<Sha256Hash, WeaponryDccError> {
        canonical_hash(&EvaluatedMeshSemantic {
            plan_sha256,
            vertices,
            triangles,
        })
    }

    pub fn semantic_hash(&self) -> &Sha256Hash {
        &self.semantic_sha256
    }

    /// Bind this disposable geometry to a caller-owned source/evaluation
    /// context.  The core can only supply `output_mesh_sha256`; it must not
    /// guess an AuthoringMesh revision or ModifierGraph identity.
    pub fn evaluated_mesh_identity(
        &self,
        source_revision_sha256: Sha256Hash,
        modifier_graph_sha256: Sha256Hash,
        input_evaluation_sha256: Vec<Sha256Hash>,
    ) -> Result<EvaluatedMeshIdentity, WeaponryDccError> {
        self.validate()?;
        Sha256Hash::new(source_revision_sha256.as_str())?;
        Sha256Hash::new(modifier_graph_sha256.as_str())?;
        for input in &input_evaluation_sha256 {
            Sha256Hash::new(input.as_str())?;
        }
        EvaluatedMeshIdentity::new(
            source_revision_sha256,
            modifier_graph_sha256,
            input_evaluation_sha256,
            self.semantic_sha256.clone(),
        )
    }

    pub fn evaluated_mesh_link(
        &self,
        source_revision_sha256: Sha256Hash,
        modifier_graph_sha256: Sha256Hash,
        input_evaluation_sha256: Vec<Sha256Hash>,
    ) -> Result<EvaluatedMeshLink, WeaponryDccError> {
        Ok(EvaluatedMeshLink::new(self.evaluated_mesh_identity(
            source_revision_sha256,
            modifier_graph_sha256,
            input_evaluation_sha256,
        )?))
    }

    pub fn positions_m(&self) -> impl Iterator<Item = [f64; 3]> + '_ {
        self.vertices.iter().map(|vertex| vertex.position_m)
    }

    pub fn vertex_ids(&self) -> impl Iterator<Item = &StableId> {
        self.vertices.iter().map(|vertex| &vertex.vertex_id)
    }

    pub fn triangle_ids(&self) -> impl Iterator<Item = &StableId> {
        self.triangles.iter().map(|triangle| &triangle.triangle_id)
    }

    pub fn triangle_indices(&self) -> impl Iterator<Item = [u32; 3]> + '_ {
        self.triangles.iter().map(|triangle| triangle.indices)
    }

    pub fn validate(&self) -> Result<(), WeaponryDccError> {
        Sha256Hash::new(self.plan_sha256.as_str())?;
        Sha256Hash::new(self.semantic_sha256.as_str())?;
        if self.vertices.is_empty() {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "evaluated mesh must contain vertices".to_owned(),
            });
        }
        if self.vertices.len() > MAX_KNIFE_EVALUATED_MESH_VERTICES {
            return Err(WeaponryDccError::KnifeEvaluatedMeshVertexBudgetExceeded {
                count: self.vertices.len(),
                maximum: MAX_KNIFE_EVALUATED_MESH_VERTICES,
            });
        }
        if self.triangles.is_empty() {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "evaluated mesh must contain triangles".to_owned(),
            });
        }
        if self.triangles.len() > MAX_KNIFE_EVALUATED_MESH_TRIANGLES {
            return Err(WeaponryDccError::KnifeEvaluatedMeshTriangleBudgetExceeded {
                count: self.triangles.len(),
                maximum: MAX_KNIFE_EVALUATED_MESH_TRIANGLES,
            });
        }

        let mut vertex_ids = BTreeSet::new();
        let mut source_curve_sha256 = None;
        for (index, vertex) in self.vertices.iter().enumerate() {
            vertex.validate_at(index)?;
            if let Some(expected) = &source_curve_sha256 {
                if expected != &vertex.lineage.source_curve_sha256
                    || vertex.lineage.plan_sha256 != self.plan_sha256
                {
                    return Err(WeaponryDccError::KnifeEvaluatedMeshLineageMismatch {
                        kind: "vertex",
                        index,
                    });
                }
            } else {
                source_curve_sha256 = Some(vertex.lineage.source_curve_sha256.clone());
            }
            if !vertex_ids.insert(vertex.vertex_id.clone()) {
                return Err(WeaponryDccError::KnifeEvaluatedMeshDuplicateId {
                    kind: "vertex",
                    id: vertex.vertex_id.clone(),
                });
            }
        }

        let mut triangle_ids = BTreeSet::new();
        let mut edge_incidence = BTreeMap::<[u32; 2], usize>::new();
        for (index, triangle) in self.triangles.iter().enumerate() {
            triangle.validate_at(index, self.vertices.len(), &self.vertices)?;
            if triangle.lineage.source_curve_sha256
                != source_curve_sha256.clone().unwrap_or_default()
                || triangle.lineage.plan_sha256 != self.plan_sha256
            {
                return Err(WeaponryDccError::KnifeEvaluatedMeshLineageMismatch {
                    kind: "triangle",
                    index,
                });
            }
            if !triangle_ids.insert(triangle.triangle_id.clone()) {
                return Err(WeaponryDccError::KnifeEvaluatedMeshDuplicateId {
                    kind: "triangle",
                    id: triangle.triangle_id.clone(),
                });
            }
            for edge in triangle_edges(triangle.indices) {
                let incidence = edge_incidence.entry(edge).or_default();
                *incidence += 1;
                if *incidence > 2 {
                    return Err(WeaponryDccError::KnifeEvaluatedMeshNonManifoldEdge {
                        edge,
                        incidence: *incidence,
                    });
                }
            }
        }
        if let Some((edge, incidence)) = edge_incidence
            .iter()
            .find(|(_, incidence)| **incidence != 2)
        {
            return Err(WeaponryDccError::KnifeEvaluatedMeshNonManifoldEdge {
                edge: *edge,
                incidence: *incidence,
            });
        }

        let expected_semantic =
            Self::semantic_hash_for(&self.plan_sha256, &self.vertices, &self.triangles)?;
        if self.semantic_sha256 != expected_semantic {
            return Err(WeaponryDccError::KnifeEvaluatedMeshSemanticHashMismatch);
        }
        Ok(())
    }

    pub fn canonical_sha256(&self) -> Result<Sha256Hash, WeaponryDccError> {
        self.validate()?;
        canonical_hash(self)
    }
}

fn validate_curve_role(
    curve: &KnifeCurve,
    expected: KnifeCurveRole,
) -> Result<(), WeaponryDccError> {
    curve.validate()?;
    if curve.role != expected {
        return Err(WeaponryDccError::KnifeCurveRoleMismatch {
            expected,
            actual: curve.role,
        });
    }
    Ok(())
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn add(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn scale(value: [f64; 3], factor: f64) -> [f64; 3] {
    [value[0] * factor, value[1] * factor, value[2] * factor]
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn length_squared(value: [f64; 3]) -> f64 {
    dot(value, value)
}

fn normalized(value: [f64; 3], station_index: usize) -> Result<[f64; 3], WeaponryDccError> {
    let length_squared = length_squared(value);
    if !length_squared.is_finite() || length_squared <= MIN_KNIFE_FRAME_LENGTH_M.powi(2) {
        return Err(WeaponryDccError::KnifeThicknessFrameDegenerate { station_index });
    }
    let inverse_length = length_squared.sqrt().recip();
    Ok(scale(value, inverse_length))
}

fn orthogonalized_axis(
    axis: [f64; 3],
    tangent: [f64; 3],
    width: [f64; 3],
    station_index: usize,
) -> Result<[f64; 3], WeaponryDccError> {
    let projected = subtract(
        subtract(axis, scale(tangent, dot(axis, tangent))),
        scale(width, dot(axis, width)),
    );
    normalized(projected, station_index)
}

fn triangle_edges(indices: [u32; 3]) -> [[u32; 2]; 3] {
    [
        ordered_edge(indices[0], indices[1]),
        ordered_edge(indices[1], indices[2]),
        ordered_edge(indices[2], indices[0]),
    ]
}

fn ordered_edge(left: u32, right: u32) -> [u32; 2] {
    if left < right {
        [left, right]
    } else {
        [right, left]
    }
}

/// Evaluate the typed blade spine/edge loft and close it with a four-sided
/// thickness sweep.  The implementation is pure and deterministic: all
/// topology and IDs are created from the two curves plus the immutable plan.
pub fn evaluate_knife_blade_profile_sweep(
    spine: &KnifeCurve,
    edge: &KnifeCurve,
    plan: &KnifeBladeSweepPlan,
) -> Result<EvaluatedMeshGeometry, WeaponryDccError> {
    validate_curve_role(spine, KnifeCurveRole::BladeSpine)?;
    validate_curve_role(edge, KnifeCurveRole::BladeEdge)?;
    plan.validate()?;
    if plan.spine_plan.curve_sha256 != spine.canonical_sha256()?
        || plan.edge_plan.curve_sha256 != edge.canonical_sha256()?
    {
        return Err(WeaponryDccError::CurvePlanBindingMismatch);
    }
    let spine_samples = spine.sample(&plan.spine_plan)?;
    let edge_samples = edge.sample(&plan.edge_plan)?;
    if spine_samples.points_m.len() != edge_samples.points_m.len() {
        return Err(WeaponryDccError::KnifeBladeStationCountMismatch {
            spine: spine_samples.points_m.len(),
            edge: edge_samples.points_m.len(),
        });
    }

    let station_count = spine_samples.points_m.len();
    let vertex_count = station_count.checked_mul(4).ok_or(
        WeaponryDccError::KnifeEvaluatedMeshVertexBudgetExceeded {
            count: usize::MAX,
            maximum: MAX_KNIFE_EVALUATED_MESH_VERTICES,
        },
    )?;
    let triangle_count = station_count
        .checked_mul(8)
        .and_then(|count| count.checked_sub(4))
        .ok_or(WeaponryDccError::KnifeEvaluatedMeshTriangleBudgetExceeded {
            count: usize::MAX,
            maximum: MAX_KNIFE_EVALUATED_MESH_TRIANGLES,
        })?;
    if vertex_count > MAX_KNIFE_EVALUATED_MESH_VERTICES {
        return Err(WeaponryDccError::KnifeEvaluatedMeshVertexBudgetExceeded {
            count: vertex_count,
            maximum: MAX_KNIFE_EVALUATED_MESH_VERTICES,
        });
    }
    if triangle_count > MAX_KNIFE_EVALUATED_MESH_TRIANGLES {
        return Err(WeaponryDccError::KnifeEvaluatedMeshTriangleBudgetExceeded {
            count: triangle_count,
            maximum: MAX_KNIFE_EVALUATED_MESH_TRIANGLES,
        });
    }

    let spine_hash = spine.canonical_sha256()?;
    let edge_hash = edge.canonical_sha256()?;
    let source_hashes = [spine_hash.clone(), edge_hash.clone()];
    let plan_sha256 = plan.canonical_sha256()?;
    let half_thickness = plan.thickness_m * 0.5;
    let mut vertices = Vec::with_capacity(vertex_count);
    let mut previous_thickness_axis = None;

    for station_index in 0..station_count {
        let spine_point = spine_samples.points_m[station_index];
        let edge_point = edge_samples.points_m[station_index];
        let center = scale(add(spine_point, edge_point), 0.5);
        let tangent_raw = if station_index == 0 {
            subtract(
                scale(
                    add(spine_samples.points_m[1], edge_samples.points_m[1]),
                    0.5,
                ),
                center,
            )
        } else if station_index + 1 == station_count {
            subtract(
                center,
                scale(
                    add(
                        spine_samples.points_m[station_index - 1],
                        edge_samples.points_m[station_index - 1],
                    ),
                    0.5,
                ),
            )
        } else {
            subtract(
                scale(
                    add(
                        spine_samples.points_m[station_index + 1],
                        edge_samples.points_m[station_index + 1],
                    ),
                    0.5,
                ),
                scale(
                    add(
                        spine_samples.points_m[station_index - 1],
                        edge_samples.points_m[station_index - 1],
                    ),
                    0.5,
                ),
            )
        };
        let tangent = normalized(tangent_raw, station_index)?;
        let width_raw = subtract(edge_point, spine_point);
        let width_transverse = subtract(width_raw, scale(tangent, dot(width_raw, tangent)));
        let width = normalized(width_transverse, station_index)?;
        let mut thickness_axis = match plan.thickness_axis.vector() {
            Some(axis) => orthogonalized_axis(axis, tangent, width, station_index)?,
            None => normalized(cross(tangent, width), station_index)?,
        };
        if let Some(previous) = previous_thickness_axis {
            if dot(previous, thickness_axis) < 0.0 {
                thickness_axis = scale(thickness_axis, -1.0);
            }
        }
        previous_thickness_axis = Some(thickness_axis);

        let positions = [
            add(spine_point, scale(thickness_axis, half_thickness)),
            add(edge_point, scale(thickness_axis, half_thickness)),
            subtract(edge_point, scale(thickness_axis, half_thickness)),
            subtract(spine_point, scale(thickness_axis, half_thickness)),
        ];
        let vertex_lineages = [
            (KnifeMeshSide::Top, KnifeCurveRole::BladeSpine, 0_u8),
            (KnifeMeshSide::Top, KnifeCurveRole::BladeEdge, 1_u8),
            (KnifeMeshSide::Bottom, KnifeCurveRole::BladeEdge, 2_u8),
            (KnifeMeshSide::Bottom, KnifeCurveRole::BladeSpine, 3_u8),
        ];
        for (local_index, (position, (side, role, lineage_index))) in
            positions.into_iter().zip(vertex_lineages).enumerate()
        {
            let lineage = KnifeMeshLineage::new(
                &source_hashes,
                &plan_sha256,
                station_index,
                side,
                role,
                lineage_index,
            )?;
            vertices.push(EvaluatedMeshVertex::generated(position, lineage)?);
            debug_assert_eq!(local_index, lineage_index as usize);
        }
    }

    let mut triangles = Vec::with_capacity(triangle_count);
    let add_triangle = |triangles: &mut Vec<EvaluatedMeshTriangle>,
                        indices: [u32; 3],
                        station_index: usize,
                        side: KnifeMeshSide,
                        role: KnifeCurveRole,
                        local_index: u8|
     -> Result<(), WeaponryDccError> {
        let lineage = KnifeMeshLineage::new(
            &source_hashes,
            &plan_sha256,
            station_index,
            side,
            role,
            local_index,
        )?;
        triangles.push(EvaluatedMeshTriangle::generated(indices, lineage)?);
        Ok(())
    };

    for station_index in 0..(station_count - 1) {
        let start = u32::try_from(station_index * 4).map_err(|_| {
            WeaponryDccError::KnifeEvaluatedMeshInvalidIndex {
                triangle_index: triangles.len(),
                index: u32::MAX,
                vertex_count,
            }
        })?;
        let next = start + 4;
        // Loft the two rails on the positive and negative thickness sides.
        add_triangle(
            &mut triangles,
            [start, next, next + 1],
            station_index,
            KnifeMeshSide::Top,
            KnifeCurveRole::Profile,
            0,
        )?;
        add_triangle(
            &mut triangles,
            [start, next + 1, start + 1],
            station_index,
            KnifeMeshSide::Top,
            KnifeCurveRole::Profile,
            1,
        )?;
        add_triangle(
            &mut triangles,
            [start + 2, next + 2, next + 3],
            station_index,
            KnifeMeshSide::Bottom,
            KnifeCurveRole::Profile,
            0,
        )?;
        add_triangle(
            &mut triangles,
            [start + 2, next + 3, start + 3],
            station_index,
            KnifeMeshSide::Bottom,
            KnifeCurveRole::Profile,
            1,
        )?;
        // Sweep thickness around both loft rails, closing the section.
        add_triangle(
            &mut triangles,
            [start, start + 3, next + 3],
            station_index,
            KnifeMeshSide::Spine,
            KnifeCurveRole::BladeSpine,
            0,
        )?;
        add_triangle(
            &mut triangles,
            [start, next + 3, next],
            station_index,
            KnifeMeshSide::Spine,
            KnifeCurveRole::BladeSpine,
            1,
        )?;
        add_triangle(
            &mut triangles,
            [start + 1, next + 1, next + 2],
            station_index,
            KnifeMeshSide::Edge,
            KnifeCurveRole::BladeEdge,
            0,
        )?;
        add_triangle(
            &mut triangles,
            [start + 1, next + 2, start + 2],
            station_index,
            KnifeMeshSide::Edge,
            KnifeCurveRole::BladeEdge,
            1,
        )?;
    }

    let first = [0_u32, 1, 2, 3];
    add_triangle(
        &mut triangles,
        [first[0], first[3], first[2]],
        0,
        KnifeMeshSide::StartCap,
        KnifeCurveRole::Profile,
        0,
    )?;
    add_triangle(
        &mut triangles,
        [first[0], first[2], first[1]],
        0,
        KnifeMeshSide::StartCap,
        KnifeCurveRole::Profile,
        1,
    )?;
    let last = u32::try_from((station_count - 1) * 4).map_err(|_| {
        WeaponryDccError::KnifeEvaluatedMeshInvalidIndex {
            triangle_index: triangles.len(),
            index: u32::MAX,
            vertex_count,
        }
    })?;
    add_triangle(
        &mut triangles,
        [last, last + 1, last + 2],
        station_count - 1,
        KnifeMeshSide::EndCap,
        KnifeCurveRole::Profile,
        0,
    )?;
    add_triangle(
        &mut triangles,
        [last, last + 2, last + 3],
        station_count - 1,
        KnifeMeshSide::EndCap,
        KnifeCurveRole::Profile,
        1,
    )?;

    let semantic_sha256 =
        EvaluatedMeshGeometry::semantic_hash_for(&plan_sha256, &vertices, &triangles)?;
    let mesh = EvaluatedMeshGeometry {
        plan_sha256,
        vertices,
        triangles,
        semantic_sha256,
    };
    mesh.validate()?;
    Ok(mesh)
}

pub fn evaluate_knife_blade(
    spine: &KnifeCurve,
    edge: &KnifeCurve,
    plan: &KnifeBladeSweepPlan,
) -> Result<KnifeEvaluatedMesh, WeaponryDccError> {
    evaluate_knife_blade_profile_sweep(spine, edge, plan)
}

pub fn build_knife_blade_mesh(
    spine: &KnifeCurve,
    edge: &KnifeCurve,
    plan: &KnifeBladeSweepPlan,
) -> Result<KnifeEvaluatedMesh, WeaponryDccError> {
    evaluate_knife_blade_profile_sweep(spine, edge, plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> Sha256Hash {
        let mut value = String::new();
        for byte in label.as_bytes().iter().cycle().take(32) {
            value.push_str(&format!("{byte:02x}"));
        }
        Sha256Hash::new(value).expect("test hash")
    }

    fn id(value: &str) -> StableId {
        StableId::new(value).expect("test id")
    }

    fn transform() -> ModifierKind {
        ModifierKind::Transform {
            translation_m: [0.0, 0.0, 0.0],
            rotation_rad: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
    }

    fn node(node_id: &str, inputs: &[&str], enabled: bool) -> ModifierNode {
        ModifierNode::new(
            node_id,
            transform(),
            inputs.iter().map(|input| id(input)).collect(),
            None,
            enabled,
        )
        .expect("test node")
    }

    fn graph(nodes: Vec<ModifierNode>) -> ModifierGraph {
        let output = nodes
            .iter()
            .find(|node| node.node_id.as_str() == "final")
            .or_else(|| nodes.last())
            .map(|node| node.node_id.clone())
            .into_iter()
            .collect();
        ModifierGraph::new("graph", "revision", hash("revision"), nodes, output)
            .expect("test graph")
    }

    #[test]
    fn selection_resolution_is_bound_to_the_exact_revision_and_sorted() {
        let query = SelectionQuery::new(
            "query",
            "revision",
            hash("revision"),
            SelectionScope::Part(id("receiver")),
            ElementKind::Edge,
            SelectionSeed::Explicit(vec![
                ElementRef::new(ElementKind::Edge, "edge-b").expect("edge"),
                ElementRef::new(ElementKind::Edge, "edge-a").expect("edge"),
            ]),
            vec![SelectionPredicate::Boundary { is_boundary: true }],
        )
        .expect("query");
        let resolution = SelectionResolution::new(
            &query,
            vec![
                ElementRef::new(ElementKind::Edge, "edge-b").expect("edge"),
                ElementRef::new(ElementKind::Edge, "edge-a").expect("edge"),
            ],
            hash("topology"),
        )
        .expect("resolution");
        assert_eq!(
            resolution
                .resolved_element_refs
                .iter()
                .map(|element| element.id.as_str())
                .collect::<Vec<_>>(),
            ["edge-a", "edge-b"]
        );
        resolution.validate_against(&query).expect("same binding");
        let other_query = SelectionQuery::new(
            "other-query",
            "revision-2",
            hash("revision-2"),
            SelectionScope::Any,
            ElementKind::Edge,
            SelectionSeed::Scope,
            Vec::new(),
        )
        .expect("other query");
        assert!(matches!(
            resolution.validate_against(&other_query),
            Err(WeaponryDccError::SelectionRevisionMismatch)
        ));
    }

    #[test]
    fn valid_graph_has_stable_dependency_first_order() {
        let first = graph(vec![
            node("detail", &["panel"], true),
            node("base", &[], true),
            node("panel", &["base"], true),
        ]);
        assert_eq!(
            first
                .topological_order()
                .iter()
                .map(StableId::as_str)
                .collect::<Vec<_>>(),
            ["base", "panel", "detail"]
        );
        let dependency = first.dependency_graph().expect("dependency graph");
        assert_eq!(
            dependency
                .topological_order()
                .iter()
                .map(StableId::as_str)
                .collect::<Vec<_>>(),
            [SOURCE_REVISION_NODE_ID, "base", "panel", "detail"]
        );
    }

    #[test]
    fn missing_input_and_duplicate_node_fail_closed() {
        let missing = ModifierGraph::new(
            "graph",
            "revision",
            hash("revision"),
            vec![node("panel", &["missing"], true)],
            vec![id("panel")],
        );
        assert!(matches!(
            missing,
            Err(WeaponryDccError::MissingInput { node_id, input_id })
                if node_id.as_str() == "panel" && input_id.as_str() == "missing"
        ));

        let duplicate = ModifierGraph::new(
            "graph",
            "revision",
            hash("revision"),
            vec![node("same", &[], true), node("same", &[], true)],
            vec![id("same")],
        );
        assert!(matches!(
            duplicate,
            Err(WeaponryDccError::DuplicateNodeId { node_id })
                if node_id.as_str() == "same"
        ));
    }

    #[test]
    fn cycle_detection_reports_a_closed_failure() {
        let cycle = ModifierGraph::new(
            "graph",
            "revision",
            hash("revision"),
            vec![node("a", &["b"], true), node("b", &["a"], true)],
            vec![id("a")],
        );
        assert!(matches!(cycle, Err(WeaponryDccError::Cycle { .. })));
    }

    #[test]
    fn disabled_nodes_remain_traceable_and_in_the_dependency_graph() {
        let graph = graph(vec![
            node("base", &[], true),
            node("bevel", &["base"], false),
            node("normal", &["bevel"], true),
        ]);
        assert!(!graph.node("bevel").expect("disabled node").enabled);
        assert_eq!(
            graph
                .topological_order()
                .iter()
                .map(StableId::as_str)
                .collect::<Vec<_>>(),
            ["base", "bevel", "normal"]
        );
        let dependency = graph.dependency_graph().expect("dependency graph");
        let dirty = dependency.dirty_closure(["bevel"]).expect("dirty closure");
        assert_eq!(
            dirty.iter().map(StableId::as_str).collect::<Vec<_>>(),
            ["bevel", "normal"]
        );
    }

    #[test]
    fn dirty_closure_and_recompute_order_are_deterministic() {
        let graph = graph(vec![
            node("final", &["panel", "rib"], true),
            node("rib", &["base"], true),
            node("panel", &["base"], true),
            node("base", &[], true),
            node("unrelated", &[], true),
        ]);
        let dependency = graph.dependency_graph().expect("dependency graph");
        let first = dependency
            .recompute_plan(["rib", "panel", "rib"])
            .expect("recompute plan");
        let second = dependency
            .recompute_plan(["panel", "rib"])
            .expect("recompute plan");
        assert_eq!(first, second);
        assert_eq!(
            first
                .dirty_nodes
                .iter()
                .map(StableId::as_str)
                .collect::<Vec<_>>(),
            ["final", "panel", "rib"]
        );
        assert_eq!(
            first
                .recompute_order
                .iter()
                .map(StableId::as_str)
                .collect::<Vec<_>>(),
            ["panel", "rib", "final"]
        );
    }

    #[test]
    fn graph_hash_is_independent_of_node_declaration_order() {
        let first = graph(vec![
            node("final", &["panel", "rib"], true),
            node("panel", &["base"], true),
            node("rib", &["base"], true),
            node("base", &[], true),
        ]);
        let second = graph(vec![
            node("base", &[], true),
            node("rib", &["base"], true),
            node("final", &["rib", "panel"], true),
            node("panel", &["base"], true),
        ]);
        assert_eq!(
            first.canonical_sha256().expect("graph hash"),
            second.canonical_sha256().expect("graph hash")
        );
        assert_eq!(
            first
                .dependency_graph()
                .expect("dependency")
                .canonical_sha256()
                .expect("hash"),
            second
                .dependency_graph()
                .expect("dependency")
                .canonical_sha256()
                .expect("hash")
        );
    }

    #[test]
    fn evaluated_mesh_identity_has_only_typed_hash_lineage() {
        let input_a = hash("input-a");
        let input_b = hash("input-b");
        let identity = EvaluatedMeshIdentity::new(
            hash("revision"),
            hash("graph"),
            vec![input_b.clone(), input_a.clone()],
            hash("output"),
        )
        .expect("identity");
        assert_eq!(
            identity
                .input_evaluation_sha256
                .iter()
                .map(Sha256Hash::as_str)
                .collect::<Vec<_>>(),
            [input_a.as_str(), input_b.as_str()]
        );
        let mesh = EvaluatedMesh::new(identity);
        assert!(mesh.link.canonical_sha256().is_ok());
    }

    #[test]
    fn paths_and_non_finite_modifier_values_are_rejected() {
        assert!(StableId::new("selection/../mesh").is_err());
        assert!(ModifierKind::Transform {
            translation_m: [f64::NAN, 0.0, 0.0],
            rotation_rad: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
        .validate()
        .is_err());
    }

    fn blade_spine_curve() -> KnifeCurve {
        KnifeCurve::new(
            "blade-spine",
            KnifeCurveRole::BladeSpine,
            KnifeCurveBasis::Bezier,
            3,
            vec![
                [0.0, 0.0, 0.0],
                [0.0, 0.2, 0.4],
                [0.0, 0.6, 0.8],
                [0.0, 1.0, 1.0],
            ],
            Vec::new(),
            Vec::new(),
            false,
        )
        .expect("valid blade spine")
    }

    fn blade_edge_curve() -> KnifeCurve {
        KnifeCurve::new(
            "blade-edge",
            KnifeCurveRole::BladeEdge,
            KnifeCurveBasis::Bezier,
            3,
            vec![
                [0.42, 0.0, 0.0],
                [0.42, 0.2, 0.0],
                [0.34, 0.65, 0.0],
                [0.0, 1.0, 0.0],
            ],
            Vec::new(),
            Vec::new(),
            false,
        )
        .expect("valid blade edge")
    }

    fn blade_sweep_plan(spine: &KnifeCurve, edge: &KnifeCurve) -> KnifeBladeSweepPlan {
        KnifeBladeSweepPlan::from_curves(spine, edge, 17, 0.06).expect("valid blade plan")
    }

    #[test]
    fn knife_curve_sampling_plan_and_replay_are_identity_stable() {
        let curve = blade_spine_curve();
        let curve_hash = curve.canonical_sha256().expect("curve hash");
        let plan = curve
            .tessellation_plan(17, 0.001, 0.25)
            .expect("sampling plan");
        assert_eq!(plan.curve_sha256, curve_hash);
        assert_eq!(plan.canonical_sha256(), plan.canonical_sha256());

        let first = curve.sample(&plan).expect("first replay");
        let second = curve.sample(&plan).expect("second replay");
        assert_eq!(first, second);
        assert_eq!(first.canonical_sha256(), second.canonical_sha256());
        assert_eq!(first.points_m.len(), 17);
        assert_eq!(first.points_m[0], [0.0, 0.0, 0.0]);
        assert_eq!(first.points_m[16], [0.0, 1.0, 1.0]);
        let value = serde_json::to_value(&first).expect("sample JSON");
        let bytes = crate::canonical_json_bytes(&value).expect("canonical sample JSON");
        let restored: KnifeCurveSampleSet =
            serde_json::from_slice(&bytes).expect("restored samples");
        restored.validate().expect("restored samples remain valid");
        assert_eq!(restored, first);
    }

    #[test]
    fn knife_curve_nurbs_like_and_curve_profile_dependency_are_typed() {
        let curve = KnifeCurve::new(
            "blade-profile",
            KnifeCurveRole::Profile,
            KnifeCurveBasis::NurbsLike,
            2,
            vec![[0.0, 0.0, 0.0], [0.1, 0.4, 0.0], [0.0, 0.8, 0.0]],
            vec![1.0, 2.0, 1.0],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            true,
        )
        .expect("valid profile");
        let operator = ModifierKind::curve_profile(&curve).expect("curve operator");
        let modifier =
            ModifierNode::new("profile", operator, Vec::new(), None, true).expect("curve modifier");
        let graph = ModifierGraph::new(
            "curve-graph",
            "revision",
            hash("revision"),
            vec![modifier],
            vec![id("profile")],
        )
        .expect("curve graph");
        let dependency = graph.dependency_graph().expect("dependency graph");
        let curve_node = dependency
            .nodes
            .iter()
            .find(|node| node.kind == DependencyNodeKind::CurveSource)
            .expect("curve source node");
        assert!(curve_node.node_id.as_str().starts_with("__curve-"));
        assert_eq!(curve_node.dependencies, vec![id(SOURCE_REVISION_NODE_ID)]);
        let dirty = dependency
            .dirty_closure([curve_node.node_id.as_str()])
            .expect("curve dirty closure");
        assert!(dirty.iter().any(|node_id| node_id.as_str() == "profile"));
    }

    #[test]
    fn knife_curve_rejects_unbounded_or_unbound_inputs() {
        assert!(KnifeCurve::new(
            "bad-curve",
            KnifeCurveRole::BladeEdge,
            KnifeCurveBasis::Bezier,
            3,
            vec![
                [0.0, 0.0, 0.0],
                [0.0, 0.2, 0.4],
                [0.0, f64::NAN, 0.8],
                [0.0, 1.0, 1.0],
            ],
            Vec::new(),
            Vec::new(),
            false,
        )
        .is_err());

        let curve = blade_spine_curve();
        assert!(matches!(
            curve.tessellation_plan(0, 0.001, 0.25),
            Err(WeaponryDccError::CurveSampleBudgetExceeded { .. })
        ));
        let wrong_plan = KnifeCurveTessellationPlan {
            curve_sha256: hash("another-curve"),
            sample_count: 3,
            parameter_start: 0.0,
            parameter_end: 1.0,
            tolerance_m: 0.001,
            max_segment_length_m: 0.25,
        };
        assert!(matches!(
            curve.sample(&wrong_plan),
            Err(WeaponryDccError::CurvePlanBindingMismatch)
        ));
    }

    #[test]
    fn knife_blade_profile_loft_and_thickness_sweep_is_closed_and_lineaged() {
        let spine = blade_spine_curve();
        let edge = blade_edge_curve();
        let plan = blade_sweep_plan(&spine, &edge);
        let mesh = plan.evaluate(&spine, &edge).expect("blade mesh");

        assert_eq!(mesh.vertices.len(), 17 * 4);
        assert_eq!(mesh.triangles.len(), 17 * 8 - 4);
        assert!(mesh.vertices.iter().all(|vertex| {
            vertex.lineage.source_curve_sha256.len() == 2
                && vertex.lineage.plan_sha256 == plan.canonical_sha256().expect("plan hash")
        }));
        assert!(mesh.triangles.iter().all(|triangle| {
            triangle
                .indices
                .iter()
                .all(|index| *index < mesh.vertices.len() as u32)
        }));
        assert!(mesh.validate().is_ok());
        let identity = mesh
            .evaluated_mesh_identity(
                hash("authoring-mesh-revision"),
                hash("modifier-graph"),
                vec![edge.canonical_sha256().expect("edge hash")],
            )
            .expect("context-bound identity");
        assert_eq!(identity.output_mesh_sha256, *mesh.semantic_hash());
        assert_eq!(EvaluatedMesh::new(identity.clone()).link.identity, identity);
        assert_eq!(
            mesh.evaluated_mesh_link(
                hash("authoring-mesh-revision"),
                hash("modifier-graph"),
                vec![edge.canonical_sha256().expect("edge hash")],
            )
            .expect("context-bound link")
            .identity,
            identity
        );

        let mut world_axis_plan = plan.clone();
        world_axis_plan.thickness_axis = KnifeThicknessAxis::WorldY;
        assert!(world_axis_plan.evaluate(&spine, &edge).is_ok());
        world_axis_plan.thickness_axis = KnifeThicknessAxis::WorldX;
        assert!(matches!(
            world_axis_plan.evaluate(&spine, &edge),
            Err(WeaponryDccError::KnifeThicknessFrameDegenerate { .. })
        ));
    }

    #[test]
    fn knife_blade_evaluation_replays_byte_exactly_and_perturbation_changes_lineage() {
        let spine = blade_spine_curve();
        let edge = blade_edge_curve();
        let plan = blade_sweep_plan(&spine, &edge);
        let first = evaluate_knife_blade_profile_sweep(&spine, &edge, &plan).expect("first");
        let second = evaluate_knife_blade_profile_sweep(&spine, &edge, &plan).expect("replay");
        assert_eq!(first, second);
        assert_eq!(first.canonical_sha256(), second.canonical_sha256());

        let mut perturbed_edge = edge.clone();
        perturbed_edge.control_points_m[1][0] += 0.01;
        let perturbed_plan = blade_sweep_plan(&spine, &perturbed_edge);
        let perturbed =
            evaluate_knife_blade_profile_sweep(&spine, &perturbed_edge, &perturbed_plan)
                .expect("perturbed blade");
        assert_ne!(first.semantic_sha256, perturbed.semantic_sha256);
        assert_ne!(
            first.vertices[0].vertex_id, perturbed.vertices[0].vertex_id,
            "vertex identity must include source curve identity"
        );
        assert!(matches!(
            evaluate_knife_blade_profile_sweep(&spine, &perturbed_edge, &plan),
            Err(WeaponryDccError::CurvePlanBindingMismatch)
        ));
    }

    #[test]
    fn knife_blade_mesh_survives_canonical_json_round_trip() {
        let spine = blade_spine_curve();
        let edge = blade_edge_curve();
        let plan = blade_sweep_plan(&spine, &edge);
        let mesh = plan.evaluate(&spine, &edge).expect("blade mesh");
        let value = serde_json::to_value(&mesh).expect("mesh JSON");
        let bytes = crate::canonical_json_bytes(&value).expect("canonical mesh JSON");
        let restored: EvaluatedMeshGeometry =
            serde_json::from_slice(&bytes).expect("restored mesh");

        restored.validate().expect("restored mesh remains valid");
        assert_eq!(restored, mesh);
        assert_eq!(restored.semantic_sha256, mesh.semantic_sha256);
    }

    #[test]
    fn knife_blade_budget_and_degenerate_inputs_fail_closed() {
        let spine = blade_spine_curve();
        let edge = blade_edge_curve();
        let mut open_plan = blade_sweep_plan(&spine, &edge);
        open_plan.root_cap = false;
        assert!(matches!(
            open_plan.validate(),
            Err(WeaponryDccError::KnifeBladeSweepPlanInvalid { .. })
        ));
        assert!(matches!(
            KnifeBladeSweepPlan::from_curves(
                &spine,
                &edge,
                (MAX_KNIFE_CURVE_SAMPLES + 1) as u32,
                0.06,
            ),
            Err(WeaponryDccError::CurveSampleBudgetExceeded { .. })
        ));

        let coincident_edge = KnifeCurve::new(
            "coincident-edge",
            KnifeCurveRole::BladeEdge,
            KnifeCurveBasis::Bezier,
            3,
            spine.control_points_m.clone(),
            Vec::new(),
            Vec::new(),
            false,
        )
        .expect("valid but coincident edge curve");
        let coincident_plan = blade_sweep_plan(&spine, &coincident_edge);
        assert!(matches!(
            coincident_plan.evaluate(&spine, &coincident_edge),
            Err(WeaponryDccError::KnifeThicknessFrameDegenerate { .. })
        ));
    }

    #[test]
    fn knife_blade_tampered_topology_fails_non_manifold_validation() {
        let spine = blade_spine_curve();
        let edge = blade_edge_curve();
        let plan = blade_sweep_plan(&spine, &edge);
        let mesh = plan.evaluate(&spine, &edge).expect("blade mesh");

        let mut non_manifold = mesh.clone();
        non_manifold.triangles.pop();
        assert!(matches!(
            non_manifold.validate(),
            Err(WeaponryDccError::KnifeEvaluatedMeshNonManifoldEdge { .. })
        ));

        let mut degenerate = mesh.clone();
        degenerate.triangles[0].indices[1] = degenerate.triangles[0].indices[0];
        assert!(matches!(
            degenerate.validate(),
            Err(WeaponryDccError::KnifeEvaluatedMeshDegenerateTriangle { .. })
        ));

        let mut invalid_index = mesh;
        invalid_index.triangles[0].indices[0] = invalid_index.vertices.len() as u32;
        assert!(matches!(
            invalid_index.validate(),
            Err(WeaponryDccError::KnifeEvaluatedMeshInvalidIndex { .. })
        ));
    }
}
