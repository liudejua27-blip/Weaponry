//! Bounded V2 language for authoring a production knife blade.
//!
//! The original knife curve façade only swept four points around two rails.
//! That is useful compatibility evidence, but it cannot express the changing
//! root/belly/tip section of a Kukri.  This module adds the smallest typed
//! vocabulary needed for that form:
//!
//! * two separately hash-bound stable curves (`KnifeCurveRole::BladeSpine` and
//!   `KnifeCurveRole::BladeEdge`);
//! * four or more ordered section stations (root, mid, belly, tip);
//! * an eight-point, asymmetric section loft with explicit top/bottom depth;
//! * stable semantic Parts (`spine`, `main_face`, `cutting_edge`, and
//!   `root_transition`); and
//! * optional simultaneous front/top/bottom/left/right envelope constraints.
//!
//! It deliberately emits a disposable evaluated value.  No arbitrary mesh
//! buffers, executable operators, paths, URLs, or persistence handles cross
//! this boundary.  All element identities derive from typed source hashes and
//! the immutable plan; caller-supplied IDs are never accepted for generated
//! elements.

use super::{
    add, canonical_hash, cross, dot, normalized, orthogonalized_axis, quantize_dcc_position, scale,
    subtract, validate_bounded, validate_curve_role, validate_vec3, KnifeCurve, KnifeCurveRole,
    KnifeCurveTessellationPlan, KnifeMeshSide, KnifeThicknessAxis, Sha256Hash, StableId,
    WeaponryDccError, MAX_COORDINATE_M, MIN_KNIFE_TRIANGLE_AREA_SQUARED_M2,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The V2 plan intentionally caps the station count lower than the generic
/// curve sampler.  This keeps validation (including bounded intersection
/// checks) predictable while leaving ample resolution for a knife blade.
pub const MAX_KNIFE_BLADE_LANGUAGE_STATIONS: usize = 256;
pub const MAX_KNIFE_BLADE_LANGUAGE_SECTIONS: usize = 16;
pub const KNIFE_BLADE_LANGUAGE_PROFILE_POINTS: usize = 4;
pub const KNIFE_BLADE_LANGUAGE_RING_POINTS: usize = 8;
pub const MAX_KNIFE_BLADE_LANGUAGE_VERTICES: usize =
    MAX_KNIFE_BLADE_LANGUAGE_STATIONS * KNIFE_BLADE_LANGUAGE_RING_POINTS + 2;
pub const MAX_KNIFE_BLADE_LANGUAGE_TRIANGLES: usize =
    (MAX_KNIFE_BLADE_LANGUAGE_STATIONS - 1) * 16 + 16;
const MIN_SECTION_STATION_GAP: f64 = 1.0e-9;
const MIN_PROFILE_THICKNESS_M: f64 = 1.0e-5;
const MAX_PROFILE_THICKNESS_M: f64 = 2.0;
const MAX_SECTION_CENTER_OFFSET_M: f64 = 1.0;
const MIN_SELF_INTERSECTION_DISTANCE_M: f64 = 1.0e-8;
const MAX_VIEW_CONSTRAINTS: usize = 5;

/// Closed semantic section classes.  Additional stations may repeat a class,
/// but their order must remain root → mid → belly → tip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnifeBladeSectionRole {
    Root,
    Mid,
    Belly,
    Tip,
}

impl KnifeBladeSectionRole {
    fn rank(self) -> u8 {
        match self {
            Self::Root => 0,
            Self::Mid => 1,
            Self::Belly => 2,
            Self::Tip => 3,
        }
    }
}

/// A section station on the normalized dual-curve domain.
///
/// `body_thickness_m` controls the broad body at the spine, while
/// `edge_thickness_m` controls the cutting-edge side.  `center_offset_m`
/// makes the section asymmetric along its thickness axis.  Bevel fractions
/// soften each rail without adding an unbounded bevel operator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifeBladeSection {
    pub section_id: StableId,
    pub role: KnifeBladeSectionRole,
    pub station_t: f64,
    pub body_thickness_m: f64,
    pub edge_thickness_m: f64,
    pub spine_bevel_fraction: f64,
    pub edge_bevel_fraction: f64,
    pub center_offset_m: f64,
}

impl KnifeBladeSection {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        section_id: impl AsRef<str>,
        role: KnifeBladeSectionRole,
        station_t: f64,
        body_thickness_m: f64,
        edge_thickness_m: f64,
        spine_bevel_fraction: f64,
        edge_bevel_fraction: f64,
        center_offset_m: f64,
    ) -> Result<Self, WeaponryDccError> {
        let section = Self {
            section_id: StableId::new(section_id)?,
            role,
            station_t,
            body_thickness_m,
            edge_thickness_m,
            spine_bevel_fraction,
            edge_bevel_fraction,
            center_offset_m,
        };
        section.validate()?;
        Ok(section)
    }

    pub fn validate(&self) -> Result<(), WeaponryDccError> {
        if self.section_id.as_str().starts_with("__") {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: format!("section id {} uses a reserved prefix", self.section_id),
            });
        }
        validate_bounded("knife_blade.section.station_t", self.station_t, 0.0, 1.0)?;
        validate_bounded(
            "knife_blade.section.body_thickness_m",
            self.body_thickness_m,
            MIN_PROFILE_THICKNESS_M,
            MAX_PROFILE_THICKNESS_M,
        )?;
        validate_bounded(
            "knife_blade.section.edge_thickness_m",
            self.edge_thickness_m,
            MIN_PROFILE_THICKNESS_M,
            MAX_PROFILE_THICKNESS_M,
        )?;
        validate_bounded(
            "knife_blade.section.spine_bevel_fraction",
            self.spine_bevel_fraction,
            0.0,
            0.5,
        )?;
        validate_bounded(
            "knife_blade.section.edge_bevel_fraction",
            self.edge_bevel_fraction,
            0.0,
            0.5,
        )?;
        validate_bounded(
            "knife_blade.section.center_offset_m",
            self.center_offset_m,
            -MAX_SECTION_CENTER_OFFSET_M,
            MAX_SECTION_CENTER_OFFSET_M,
        )?;
        // The interpolation uses a linear top/bottom depth.  This endpoint
        // check is sufficient to prove the full interpolant remains positive.
        for (name, thickness) in [
            ("body_thickness_m", self.body_thickness_m),
            ("edge_thickness_m", self.edge_thickness_m),
        ] {
            if thickness * 0.5 <= self.center_offset_m.abs() + MIN_PROFILE_THICKNESS_M {
                return Err(WeaponryDccError::KnifeBladeInputInvalid {
                    reason: format!("section {name} does not leave positive top and bottom depth"),
                });
            }
        }
        Ok(())
    }
}

/// Naming aliases used by callers that describe a station as a cross-section
/// or the generated semantic region as a Part.  They intentionally resolve
/// to the same single V2 types rather than introducing parallel truth.
pub type KnifeBladeCrossSection = KnifeBladeSection;
pub type KnifeBladeProfileSection = KnifeBladeSection;

/// The five fixed review projections used by the knife form language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnifeBladeView {
    Front,
    Top,
    Bottom,
    Left,
    Right,
}

impl KnifeBladeView {
    fn projection(self, point: [f64; 3]) -> [f64; 2] {
        match self {
            // The local blade convention is X/Y front, X/Z top/bottom and
            // Y/Z left/right.  Bottom/right retain the same extent convention
            // so a target envelope cannot be hidden by a sign flip.
            Self::Front => [point[0], point[1]],
            Self::Top | Self::Bottom => [point[0], point[2]],
            Self::Left | Self::Right => [point[1], point[2]],
        }
    }
}

/// A bounded projected envelope.  It is an optional plan input, but if any
/// constraint is supplied the complete five-view set is required.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifeBladeViewConstraint {
    pub view: KnifeBladeView,
    pub min_x_m: f64,
    pub max_x_m: f64,
    pub min_y_m: f64,
    pub max_y_m: f64,
}

impl KnifeBladeViewConstraint {
    pub fn new(
        view: KnifeBladeView,
        min_x_m: f64,
        max_x_m: f64,
        min_y_m: f64,
        max_y_m: f64,
    ) -> Result<Self, WeaponryDccError> {
        let constraint = Self {
            view,
            min_x_m,
            max_x_m,
            min_y_m,
            max_y_m,
        };
        constraint.validate()?;
        Ok(constraint)
    }

    pub fn validate(&self) -> Result<(), WeaponryDccError> {
        validate_bounded("knife_blade.view.min_x_m", self.min_x_m, -20.0, 20.0)?;
        validate_bounded("knife_blade.view.max_x_m", self.max_x_m, -20.0, 20.0)?;
        validate_bounded("knife_blade.view.min_y_m", self.min_y_m, -20.0, 20.0)?;
        validate_bounded("knife_blade.view.max_y_m", self.max_y_m, -20.0, 20.0)?;
        if self.min_x_m >= self.max_x_m || self.min_y_m >= self.max_y_m {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "view constraint min must be less than max".to_owned(),
            });
        }
        Ok(())
    }
}

/// V2 dual-curve, sectioned blade plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifeBladeLanguagePlan {
    pub spine_plan: KnifeCurveTessellationPlan,
    pub edge_plan: KnifeCurveTessellationPlan,
    pub sections: Vec<KnifeBladeSection>,
    pub thickness_axis: KnifeThicknessAxis,
    pub root_cap: bool,
    pub tip_cap: bool,
    pub view_constraints: Vec<KnifeBladeViewConstraint>,
}

pub type KnifeBladeSectionLoftPlan = KnifeBladeLanguagePlan;
pub type KnifeBladeSweepLoftPlan = KnifeBladeLanguagePlan;

impl KnifeBladeLanguagePlan {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        spine: &KnifeCurve,
        edge: &KnifeCurve,
        sample_count: u32,
        tolerance_m: f64,
        max_segment_length_m: f64,
        sections: Vec<KnifeBladeSection>,
        thickness_axis: KnifeThicknessAxis,
        view_constraints: Vec<KnifeBladeViewConstraint>,
    ) -> Result<Self, WeaponryDccError> {
        validate_curve_role(spine, KnifeCurveRole::BladeSpine)?;
        validate_curve_role(edge, KnifeCurveRole::BladeEdge)?;
        let plan = Self {
            spine_plan: spine.tessellation_plan(sample_count, tolerance_m, max_segment_length_m)?,
            edge_plan: edge.tessellation_plan(sample_count, tolerance_m, max_segment_length_m)?,
            sections,
            thickness_axis,
            root_cap: true,
            tip_cap: true,
            view_constraints,
        };
        plan.validate_for_curves(spine, edge)?;
        Ok(plan)
    }

    /// Fixed tolerances for callers that only need the closed typed kernel.
    pub fn from_curves(
        spine: &KnifeCurve,
        edge: &KnifeCurve,
        sample_count: u32,
        sections: Vec<KnifeBladeSection>,
        thickness_axis: KnifeThicknessAxis,
    ) -> Result<Self, WeaponryDccError> {
        Self::new(
            spine,
            edge,
            sample_count,
            1.0e-4,
            1.0,
            sections,
            thickness_axis,
            Vec::new(),
        )
    }

    /// Add all five fixed projections in one typed operation.
    pub fn with_view_constraints(
        mut self,
        constraints: Vec<KnifeBladeViewConstraint>,
    ) -> Result<Self, WeaponryDccError> {
        self.view_constraints = constraints;
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), WeaponryDccError> {
        self.spine_plan.validate()?;
        self.edge_plan.validate()?;
        if self.spine_plan.curve_sha256 == self.edge_plan.curve_sha256 {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "spine and edge must be distinct stable curves".to_owned(),
            });
        }
        if self.spine_plan.sample_count != self.edge_plan.sample_count {
            return Err(WeaponryDccError::KnifeBladeStationCountMismatch {
                spine: self.spine_plan.sample_count as usize,
                edge: self.edge_plan.sample_count as usize,
            });
        }
        let station_count = self.spine_plan.sample_count as usize;
        if !(4..=MAX_KNIFE_BLADE_LANGUAGE_STATIONS).contains(&station_count) {
            return Err(WeaponryDccError::CurveSampleBudgetExceeded {
                count: station_count,
                maximum: MAX_KNIFE_BLADE_LANGUAGE_STATIONS,
            });
        }
        if self.sections.len() < 4 || self.sections.len() > MAX_KNIFE_BLADE_LANGUAGE_SECTIONS {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: format!(
                    "sections must contain 4..={MAX_KNIFE_BLADE_LANGUAGE_SECTIONS} stations"
                ),
            });
        }
        let mut section_ids = BTreeSet::new();
        let mut previous_t = None;
        let mut previous_role: Option<KnifeBladeSectionRole> = None;
        let mut roles = BTreeSet::new();
        for section in &self.sections {
            section.validate()?;
            if !section_ids.insert(section.section_id.clone()) {
                return Err(WeaponryDccError::KnifeBladeInputInvalid {
                    reason: format!("section id {} is repeated", section.section_id),
                });
            }
            if let Some(previous_t) = previous_t {
                if section.station_t - previous_t <= MIN_SECTION_STATION_GAP {
                    return Err(WeaponryDccError::KnifeBladeInputInvalid {
                        reason: "section station_t values must be strictly increasing".to_owned(),
                    });
                }
            }
            if let Some(previous_role) = previous_role {
                if section.role.rank() < previous_role.rank() {
                    return Err(WeaponryDccError::KnifeBladeInputInvalid {
                        reason: "section roles must be ordered root->mid->belly->tip".to_owned(),
                    });
                }
            }
            previous_t = Some(section.station_t);
            previous_role = Some(section.role);
            roles.insert(section.role);
        }
        if self.sections.first().map(|section| section.station_t) != Some(0.0)
            || self.sections.last().map(|section| section.station_t) != Some(1.0)
            || self.sections.first().map(|section| section.role)
                != Some(KnifeBladeSectionRole::Root)
            || self.sections.last().map(|section| section.role) != Some(KnifeBladeSectionRole::Tip)
            || [
                KnifeBladeSectionRole::Root,
                KnifeBladeSectionRole::Mid,
                KnifeBladeSectionRole::Belly,
                KnifeBladeSectionRole::Tip,
            ]
            .iter()
            .any(|role| !roles.contains(role))
        {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "sections must cover root/mid/belly/tip from t=0 to t=1".to_owned(),
            });
        }
        if !self.root_cap || !self.tip_cap {
            return Err(WeaponryDccError::KnifeBladeSweepPlanInvalid {
                reason: "sectioned blade requires root_cap=true and tip_cap=true".to_owned(),
            });
        }
        if self.view_constraints.len() > MAX_VIEW_CONSTRAINTS {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: format!("at most {MAX_VIEW_CONSTRAINTS} view constraints are allowed"),
            });
        }
        let mut views = BTreeSet::new();
        for constraint in &self.view_constraints {
            constraint.validate()?;
            if !views.insert(constraint.view) {
                return Err(WeaponryDccError::KnifeBladeInputInvalid {
                    reason: "view constraints must contain one entry per view".to_owned(),
                });
            }
        }
        if !self.view_constraints.is_empty() && views.len() != MAX_VIEW_CONSTRAINTS {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "front/top/bottom/left/right constraints must be supplied together"
                    .to_owned(),
            });
        }
        Ok(())
    }

    pub fn validate_for_curves(
        &self,
        spine: &KnifeCurve,
        edge: &KnifeCurve,
    ) -> Result<(), WeaponryDccError> {
        self.validate()?;
        validate_curve_role(spine, KnifeCurveRole::BladeSpine)?;
        validate_curve_role(edge, KnifeCurveRole::BladeEdge)?;
        if self.spine_plan.curve_sha256 != spine.canonical_sha256()?
            || self.edge_plan.curve_sha256 != edge.canonical_sha256()?
        {
            return Err(WeaponryDccError::CurvePlanBindingMismatch);
        }
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
    ) -> Result<KnifeBladeLanguageMesh, WeaponryDccError> {
        evaluate_knife_blade_language(spine, edge, self)
    }
}

/// Stable semantic Part language.  Part IDs depend on source curve IDs and
/// role, not generated station indexes, so editing a curve's control points
/// preserves Part identity while changing its geometry hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnifeBladePartRole {
    Spine,
    MainFace,
    CuttingEdge,
    RootTransition,
}

pub type KnifeBladeRegionRole = KnifeBladePartRole;

impl KnifeBladePartRole {
    fn slug(self) -> &'static str {
        match self {
            Self::Spine => "spine",
            Self::MainFace => "main-face",
            Self::CuttingEdge => "cutting-edge",
            Self::RootTransition => "root-transition",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifeBladePartLineage {
    pub source_curve_ids: Vec<StableId>,
    pub source_curve_sha256: Vec<Sha256Hash>,
    pub role: KnifeBladePartRole,
    pub plan_sha256: Sha256Hash,
    pub lineage_sha256: Sha256Hash,
}

impl KnifeBladePartLineage {
    fn new(
        spine: &KnifeCurve,
        edge: &KnifeCurve,
        role: KnifeBladePartRole,
        plan_sha256: Sha256Hash,
    ) -> Result<Self, WeaponryDccError> {
        let source_curve_ids = vec![spine.curve_id.clone(), edge.curve_id.clone()];
        let source_curve_sha256 = vec![spine.canonical_sha256()?, edge.canonical_sha256()?];
        let preimage = (&source_curve_ids, &source_curve_sha256, role, &plan_sha256);
        let lineage_sha256 = canonical_hash(&preimage)?;
        let lineage = Self {
            source_curve_ids,
            source_curve_sha256,
            role,
            plan_sha256,
            lineage_sha256,
        };
        lineage.validate()?;
        Ok(lineage)
    }

    pub fn validate(&self) -> Result<(), WeaponryDccError> {
        if self.source_curve_ids.len() != 2 || self.source_curve_sha256.len() != 2 {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "part lineage must bind exactly spine and edge".to_owned(),
            });
        }
        if self.source_curve_ids[0] == self.source_curve_ids[1]
            || self.source_curve_sha256[0] == self.source_curve_sha256[1]
        {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "part lineage repeats spine or edge".to_owned(),
            });
        }
        for hash in &self.source_curve_sha256 {
            Sha256Hash::new(hash.as_str())?;
        }
        Sha256Hash::new(self.plan_sha256.as_str())?;
        let expected = canonical_hash(&(
            &self.source_curve_ids,
            &self.source_curve_sha256,
            self.role,
            &self.plan_sha256,
        ))?;
        if expected != self.lineage_sha256 {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "part lineage hash is stale".to_owned(),
            });
        }
        Ok(())
    }

    pub fn canonical_sha256(&self) -> Result<Sha256Hash, WeaponryDccError> {
        self.validate()?;
        canonical_hash(self)
    }

    /// Stable across control-point edits, while remaining unique to the
    /// source curve identities and semantic region.
    pub fn stable_part_id(&self) -> Result<StableId, WeaponryDccError> {
        let identity_hash = canonical_hash(&(&self.source_curve_ids, self.role))?;
        StableId::new(format!(
            "knife-blade-part-{}-{}",
            self.role.slug(),
            &identity_hash.as_str()[..24]
        ))
    }

    pub fn material_zone_id(&self) -> Result<StableId, WeaponryDccError> {
        StableId::new(format!("knife-blade-zone-{}", self.role.slug()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifeBladeLanguageVertexLineage {
    pub source_curve_sha256: Vec<Sha256Hash>,
    pub plan_sha256: Sha256Hash,
    pub section_id: StableId,
    pub section_role: KnifeBladeSectionRole,
    pub station_index: u32,
    pub side: KnifeMeshSide,
    pub local_index: u8,
}

impl KnifeBladeLanguageVertexLineage {
    fn new(
        source_curve_sha256: &[Sha256Hash],
        plan_sha256: &Sha256Hash,
        section: &KnifeBladeSection,
        station_index: usize,
        side: KnifeMeshSide,
        local_index: u8,
    ) -> Result<Self, WeaponryDccError> {
        let station_index =
            u32::try_from(station_index).map_err(|_| WeaponryDccError::KnifeBladeInputInvalid {
                reason: "station index exceeds lineage range".to_owned(),
            })?;
        if local_index
            >= (KNIFE_BLADE_LANGUAGE_RING_POINTS + KNIFE_BLADE_LANGUAGE_PROFILE_POINTS) as u8
        {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "vertex local index exceeds section ring".to_owned(),
            });
        }
        let lineage = Self {
            source_curve_sha256: source_curve_sha256.to_vec(),
            plan_sha256: plan_sha256.clone(),
            section_id: section.section_id.clone(),
            section_role: section.role,
            station_index,
            side,
            local_index,
        };
        lineage.validate()?;
        Ok(lineage)
    }

    pub fn validate(&self) -> Result<(), WeaponryDccError> {
        if self.source_curve_sha256.len() != 2 {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "vertex lineage must bind two source curves".to_owned(),
            });
        }
        for hash in &self.source_curve_sha256 {
            Sha256Hash::new(hash.as_str())?;
        }
        Sha256Hash::new(self.plan_sha256.as_str())?;
        StableId::new(self.section_id.as_str())?;
        if self.station_index as usize >= MAX_KNIFE_BLADE_LANGUAGE_STATIONS
            || self.local_index
                >= (KNIFE_BLADE_LANGUAGE_RING_POINTS + KNIFE_BLADE_LANGUAGE_PROFILE_POINTS) as u8
        {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "vertex local index exceeds section ring".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifeBladeLanguageVertex {
    pub vertex_id: StableId,
    pub position_m: [f64; 3],
    pub lineage: KnifeBladeLanguageVertexLineage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifeBladeLanguageTriangleLineage {
    pub source_curve_sha256: Vec<Sha256Hash>,
    pub plan_sha256: Sha256Hash,
    pub part_role: KnifeBladePartRole,
    pub station_index: u32,
    pub segment_index: u32,
    pub local_index: u8,
}

impl KnifeBladeLanguageTriangleLineage {
    fn new(
        source_curve_sha256: &[Sha256Hash],
        plan_sha256: &Sha256Hash,
        part_role: KnifeBladePartRole,
        station_index: usize,
        segment_index: usize,
        local_index: u8,
    ) -> Result<Self, WeaponryDccError> {
        let lineage = Self {
            source_curve_sha256: source_curve_sha256.to_vec(),
            plan_sha256: plan_sha256.clone(),
            part_role,
            station_index: u32::try_from(station_index).map_err(|_| {
                WeaponryDccError::KnifeBladeInputInvalid {
                    reason: "triangle station index exceeds lineage range".to_owned(),
                }
            })?,
            segment_index: u32::try_from(segment_index).map_err(|_| {
                WeaponryDccError::KnifeBladeInputInvalid {
                    reason: "triangle segment index exceeds lineage range".to_owned(),
                }
            })?,
            local_index,
        };
        lineage.validate()?;
        Ok(lineage)
    }

    fn validate(&self) -> Result<(), WeaponryDccError> {
        if self.source_curve_sha256.len() != 2
            || self.local_index >= 16
            || self.station_index as usize >= MAX_KNIFE_BLADE_LANGUAGE_STATIONS + 2
            || self.segment_index as usize >= MAX_KNIFE_BLADE_LANGUAGE_STATIONS + 2
        {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "triangle lineage is outside the closed section range".to_owned(),
            });
        }
        for hash in &self.source_curve_sha256 {
            Sha256Hash::new(hash.as_str())?;
        }
        Sha256Hash::new(self.plan_sha256.as_str())?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifeBladeLanguageTriangle {
    pub triangle_id: StableId,
    pub indices: [u32; 3],
    pub lineage: KnifeBladeLanguageTriangleLineage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifeBladeMeshPart {
    pub part_id: StableId,
    pub material_zone_id: StableId,
    pub role: KnifeBladePartRole,
    pub lineage: KnifeBladePartLineage,
    pub vertex_indices: Vec<u32>,
    pub triangle_indices: Vec<u32>,
}

pub type KnifeBladePart = KnifeBladeMeshPart;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifeBladeLanguageMesh {
    pub plan_sha256: Sha256Hash,
    pub source_curve_sha256: Vec<Sha256Hash>,
    pub vertices: Vec<KnifeBladeLanguageVertex>,
    pub triangles: Vec<KnifeBladeLanguageTriangle>,
    pub parts: Vec<KnifeBladeMeshPart>,
    pub semantic_sha256: Sha256Hash,
}

pub type KnifeBladeSectionLoftMesh = KnifeBladeLanguageMesh;
pub type KnifeBladeSweepLoftMesh = KnifeBladeLanguageMesh;
pub type KnifeBladeMesh = KnifeBladeLanguageMesh;

#[derive(Serialize)]
struct MeshSemantic<'a> {
    plan_sha256: &'a Sha256Hash,
    source_curve_sha256: &'a [Sha256Hash],
    vertices: &'a [KnifeBladeLanguageVertex],
    triangles: &'a [KnifeBladeLanguageTriangle],
    parts: &'a [KnifeBladeMeshPart],
}

impl KnifeBladeLanguageMesh {
    fn semantic_hash_for(
        plan_sha256: &Sha256Hash,
        source_curve_sha256: &[Sha256Hash],
        vertices: &[KnifeBladeLanguageVertex],
        triangles: &[KnifeBladeLanguageTriangle],
        parts: &[KnifeBladeMeshPart],
    ) -> Result<Sha256Hash, WeaponryDccError> {
        canonical_hash(&MeshSemantic {
            plan_sha256,
            source_curve_sha256,
            vertices,
            triangles,
            parts,
        })
    }

    pub fn semantic_hash(&self) -> &Sha256Hash {
        &self.semantic_sha256
    }

    pub fn canonical_sha256(&self) -> Result<Sha256Hash, WeaponryDccError> {
        self.validate().and_then(|_| canonical_hash(self))
    }

    pub fn validate(&self) -> Result<(), WeaponryDccError> {
        if self.source_curve_sha256.len() != 2 {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "mesh must bind spine and edge source hashes".to_owned(),
            });
        }
        for hash in &self.source_curve_sha256 {
            Sha256Hash::new(hash.as_str())?;
        }
        Sha256Hash::new(self.plan_sha256.as_str())?;
        Sha256Hash::new(self.semantic_sha256.as_str())?;
        if self.vertices.is_empty() || self.vertices.len() > MAX_KNIFE_BLADE_LANGUAGE_VERTICES {
            return Err(WeaponryDccError::KnifeEvaluatedMeshVertexBudgetExceeded {
                count: self.vertices.len(),
                maximum: MAX_KNIFE_BLADE_LANGUAGE_VERTICES,
            });
        }
        if self.triangles.is_empty() || self.triangles.len() > MAX_KNIFE_BLADE_LANGUAGE_TRIANGLES {
            return Err(WeaponryDccError::KnifeEvaluatedMeshTriangleBudgetExceeded {
                count: self.triangles.len(),
                maximum: MAX_KNIFE_BLADE_LANGUAGE_TRIANGLES,
            });
        }
        let mut vertex_ids = BTreeSet::new();
        for (index, vertex) in self.vertices.iter().enumerate() {
            vertex.lineage.validate()?;
            if vertex.lineage.source_curve_sha256 != self.source_curve_sha256
                || vertex.lineage.plan_sha256 != self.plan_sha256
            {
                return Err(WeaponryDccError::KnifeEvaluatedMeshLineageMismatch {
                    kind: "vertex",
                    index,
                });
            }
            validate_vec3(
                &format!("knife_blade.vertices[{index}].position_m"),
                &vertex.position_m,
                -MAX_COORDINATE_M * 2.0,
                MAX_COORDINATE_M * 2.0,
            )?;
            if vertex.position_m != quantize_dcc_position(vertex.position_m) {
                return Err(WeaponryDccError::KnifeBladeInputInvalid {
                    reason: format!("vertex {index} is outside the fixed nanometre grid"),
                });
            }
            if vertex.vertex_id != derived_vertex_id(&vertex.lineage)? {
                return Err(WeaponryDccError::KnifeEvaluatedMeshLineageMismatch {
                    kind: "vertex",
                    index,
                });
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
            triangle.lineage.validate()?;
            if triangle.lineage.source_curve_sha256 != self.source_curve_sha256
                || triangle.lineage.plan_sha256 != self.plan_sha256
            {
                return Err(WeaponryDccError::KnifeEvaluatedMeshLineageMismatch {
                    kind: "triangle",
                    index,
                });
            }
            for vertex_index in triangle.indices {
                if vertex_index as usize >= self.vertices.len() {
                    return Err(WeaponryDccError::KnifeEvaluatedMeshInvalidIndex {
                        triangle_index: index,
                        index: vertex_index,
                        vertex_count: self.vertices.len(),
                    });
                }
            }
            if triangle.indices[0] == triangle.indices[1]
                || triangle.indices[1] == triangle.indices[2]
                || triangle.indices[0] == triangle.indices[2]
            {
                return Err(WeaponryDccError::KnifeEvaluatedMeshDegenerateTriangle {
                    triangle_index: index,
                });
            }
            let a = self.vertices[triangle.indices[0] as usize].position_m;
            let b = self.vertices[triangle.indices[1] as usize].position_m;
            let c = self.vertices[triangle.indices[2] as usize].position_m;
            let area = cross(subtract(b, a), subtract(c, a));
            if !dot(area, area).is_finite() || dot(area, area) <= MIN_KNIFE_TRIANGLE_AREA_SQUARED_M2
            {
                return Err(WeaponryDccError::KnifeEvaluatedMeshDegenerateTriangle {
                    triangle_index: index,
                });
            }
            if !triangle_ids.insert(triangle.triangle_id.clone()) {
                return Err(WeaponryDccError::KnifeEvaluatedMeshDuplicateId {
                    kind: "triangle",
                    id: triangle.triangle_id.clone(),
                });
            }
            let expected_id = derived_triangle_id(&triangle.lineage)?;
            if triangle.triangle_id != expected_id {
                return Err(WeaponryDccError::KnifeEvaluatedMeshLineageMismatch {
                    kind: "triangle",
                    index,
                });
            }
            for edge in triangle_edges(triangle.indices) {
                *edge_incidence.entry(edge).or_default() += 1;
            }
        }
        if let Some((edge, incidence)) = edge_incidence.iter().find(|(_, count)| **count != 2) {
            return Err(WeaponryDccError::KnifeEvaluatedMeshNonManifoldEdge {
                edge: *edge,
                incidence: *incidence,
            });
        }
        validate_parts(self)?;
        if let Some((first, second)) = mesh_self_intersection(self) {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: format!(
                    "generated blade self-intersects between triangles {first} and {second}"
                ),
            });
        }
        let expected_semantic = Self::semantic_hash_for(
            &self.plan_sha256,
            &self.source_curve_sha256,
            &self.vertices,
            &self.triangles,
            &self.parts,
        )?;
        if expected_semantic != self.semantic_sha256 {
            return Err(WeaponryDccError::KnifeEvaluatedMeshSemanticHashMismatch);
        }
        Ok(())
    }

    pub fn validate_view_constraints(
        &self,
        constraints: &[KnifeBladeViewConstraint],
    ) -> Result<(), WeaponryDccError> {
        if constraints.is_empty() {
            return Ok(());
        }
        if constraints.len() != MAX_VIEW_CONSTRAINTS {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: "all five view constraints are required".to_owned(),
            });
        }
        let mut seen = BTreeSet::new();
        for constraint in constraints {
            constraint.validate()?;
            if !seen.insert(constraint.view) {
                return Err(WeaponryDccError::KnifeBladeInputInvalid {
                    reason: "view constraints repeat a projection".to_owned(),
                });
            }
            let mut extent = [
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
            ];
            for vertex in &self.vertices {
                let projected = constraint.view.projection(vertex.position_m);
                extent[0] = extent[0].min(projected[0]);
                extent[1] = extent[1].max(projected[0]);
                extent[2] = extent[2].min(projected[1]);
                extent[3] = extent[3].max(projected[1]);
            }
            if extent[0] < constraint.min_x_m
                || extent[1] > constraint.max_x_m
                || extent[2] < constraint.min_y_m
                || extent[3] > constraint.max_y_m
            {
                return Err(WeaponryDccError::KnifeBladeInputInvalid {
                    reason: format!(
                        "{} view envelope rejected generated blade",
                        constraint.view_name()
                    ),
                });
            }
        }
        Ok(())
    }
}

impl KnifeBladeViewConstraint {
    fn view_name(&self) -> &'static str {
        match self.view {
            KnifeBladeView::Front => "front",
            KnifeBladeView::Top => "top",
            KnifeBladeView::Bottom => "bottom",
            KnifeBladeView::Left => "left",
            KnifeBladeView::Right => "right",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct InterpolatedSection {
    section_id_index: usize,
    body_thickness_m: f64,
    edge_thickness_m: f64,
    spine_bevel_fraction: f64,
    edge_bevel_fraction: f64,
    center_offset_m: f64,
}

fn interpolate_section(sections: &[KnifeBladeSection], station_t: f64) -> InterpolatedSection {
    if station_t >= 1.0 {
        let index = sections.len() - 1;
        let section = &sections[index];
        return InterpolatedSection {
            section_id_index: index,
            body_thickness_m: section.body_thickness_m,
            edge_thickness_m: section.edge_thickness_m,
            spine_bevel_fraction: section.spine_bevel_fraction,
            edge_bevel_fraction: section.edge_bevel_fraction,
            center_offset_m: section.center_offset_m,
        };
    }
    let upper = sections
        .iter()
        .position(|section| section.station_t > station_t)
        .unwrap_or(sections.len() - 1);
    let lower = upper.saturating_sub(1);
    let first = &sections[lower];
    let second = &sections[upper];
    let denominator = second.station_t - first.station_t;
    let alpha = ((station_t - first.station_t) / denominator).clamp(0.0, 1.0);
    let lerp = |left: f64, right: f64| left + (right - left) * alpha;
    InterpolatedSection {
        section_id_index: lower,
        body_thickness_m: lerp(first.body_thickness_m, second.body_thickness_m),
        edge_thickness_m: lerp(first.edge_thickness_m, second.edge_thickness_m),
        spine_bevel_fraction: lerp(first.spine_bevel_fraction, second.spine_bevel_fraction),
        edge_bevel_fraction: lerp(first.edge_bevel_fraction, second.edge_bevel_fraction),
        center_offset_m: lerp(first.center_offset_m, second.center_offset_m),
    }
}

fn source_rail_self_intersects(points: &[[f64; 3]]) -> bool {
    for first in 0..points.len().saturating_sub(1) {
        for second in (first + 2)..points.len().saturating_sub(1) {
            if second == first + 1 {
                continue;
            }
            if segment_distance_squared(
                points[first],
                points[first + 1],
                points[second],
                points[second + 1],
            ) <= MIN_SELF_INTERSECTION_DISTANCE_M.powi(2)
            {
                return true;
            }
        }
    }
    false
}

fn dual_rails_intersect(spine: &[[f64; 3]], edge: &[[f64; 3]]) -> bool {
    for spine_segment in 0..spine.len().saturating_sub(1) {
        for edge_segment in 0..edge.len().saturating_sub(1) {
            if segment_distance_squared(
                spine[spine_segment],
                spine[spine_segment + 1],
                edge[edge_segment],
                edge[edge_segment + 1],
            ) <= MIN_SELF_INTERSECTION_DISTANCE_M.powi(2)
            {
                return true;
            }
        }
    }
    false
}

fn segment_distance_squared(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> f64 {
    let u = subtract(b, a);
    let v = subtract(d, c);
    let w = subtract(a, c);
    let uu = dot(u, u);
    let vv = dot(v, v);
    let uv = dot(u, v);
    let uw = dot(u, w);
    let vw = dot(v, w);
    let denominator = uu * vv - uv * uv;
    let (mut s, mut t) = if denominator.abs() > f64::EPSILON {
        (
            (uv * vw - vv * uw) / denominator,
            (uu * vw - uv * uw) / denominator,
        )
    } else {
        (0.0, if vv > f64::EPSILON { vw / vv } else { 0.0 })
    };
    s = s.clamp(0.0, 1.0);
    t = t.clamp(0.0, 1.0);
    let first = add(a, scale(u, s));
    let second = add(c, scale(v, t));
    let delta = subtract(first, second);
    dot(delta, delta)
}

fn derived_vertex_id(
    lineage: &KnifeBladeLanguageVertexLineage,
) -> Result<StableId, WeaponryDccError> {
    let hash = canonical_hash(lineage)?;
    StableId::new(format!("knife-blade-vertex-{}", &hash.as_str()[..24]))
}

fn derived_triangle_id(
    lineage: &KnifeBladeLanguageTriangleLineage,
) -> Result<StableId, WeaponryDccError> {
    let hash = canonical_hash(lineage)?;
    StableId::new(format!("knife-blade-triangle-{}", &hash.as_str()[..24]))
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

fn validate_parts(mesh: &KnifeBladeLanguageMesh) -> Result<(), WeaponryDccError> {
    let required = [
        KnifeBladePartRole::Spine,
        KnifeBladePartRole::MainFace,
        KnifeBladePartRole::CuttingEdge,
        KnifeBladePartRole::RootTransition,
    ];
    if mesh.parts.len() != required.len() {
        return Err(WeaponryDccError::KnifeBladeInputInvalid {
            reason: "mesh must contain the four semantic blade Parts".to_owned(),
        });
    }
    let mut roles = BTreeSet::new();
    let mut part_ids = BTreeSet::new();
    let mut zone_ids = BTreeSet::new();
    let mut triangle_owners = BTreeSet::new();
    for part in &mesh.parts {
        part.lineage.validate()?;
        if part.lineage.role != part.role
            || part.lineage.plan_sha256 != mesh.plan_sha256
            || part.lineage.source_curve_sha256 != mesh.source_curve_sha256
        {
            return Err(WeaponryDccError::KnifeEvaluatedMeshLineageMismatch {
                kind: "part",
                index: 0,
            });
        }
        if part.part_id != part.lineage.stable_part_id()?
            || part.material_zone_id != part.lineage.material_zone_id()?
            || !roles.insert(part.role)
            || !part_ids.insert(part.part_id.clone())
            || !zone_ids.insert(part.material_zone_id.clone())
        {
            return Err(WeaponryDccError::KnifeEvaluatedMeshDuplicateId {
                kind: "part",
                id: part.part_id.clone(),
            });
        }
        if part.triangle_indices.is_empty() || part.vertex_indices.is_empty() {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: format!("blade Part {} is empty", part.role.slug()),
            });
        }
        let mut vertices = BTreeSet::new();
        for index in &part.vertex_indices {
            if *index as usize >= mesh.vertices.len() {
                return Err(WeaponryDccError::KnifeEvaluatedMeshInvalidIndex {
                    triangle_index: 0,
                    index: *index,
                    vertex_count: mesh.vertices.len(),
                });
            }
            vertices.insert(*index);
        }
        for index in &part.triangle_indices {
            if *index as usize >= mesh.triangles.len() {
                return Err(WeaponryDccError::KnifeEvaluatedMeshInvalidIndex {
                    triangle_index: *index as usize,
                    index: *index,
                    vertex_count: mesh.triangles.len(),
                });
            }
            let triangle = &mesh.triangles[*index as usize];
            if triangle.lineage.part_role != part.role
                || triangle
                    .indices
                    .iter()
                    .any(|vertex| !vertices.contains(vertex))
                || !triangle_owners.insert(*index)
            {
                return Err(WeaponryDccError::KnifeEvaluatedMeshLineageMismatch {
                    kind: "part-triangle",
                    index: *index as usize,
                });
            }
        }
    }
    if required.iter().any(|role| !roles.contains(role))
        || triangle_owners.len() != mesh.triangles.len()
    {
        return Err(WeaponryDccError::KnifeBladeInputInvalid {
            reason: "blade Parts must partition every triangle".to_owned(),
        });
    }
    Ok(())
}

fn point_aabb(point: [f64; 3], min: &mut [f64; 3], max: &mut [f64; 3]) {
    for component in 0..3 {
        let old_min = min[component];
        let old_max = max[component];
        min[component] = old_min.min(point[component]);
        max[component] = old_max.max(point[component]);
    }
}

fn triangle_aabb(
    mesh: &KnifeBladeLanguageMesh,
    triangle: &KnifeBladeLanguageTriangle,
) -> ([f64; 3], [f64; 3]) {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for index in triangle.indices {
        point_aabb(mesh.vertices[index as usize].position_m, &mut min, &mut max);
    }
    (min, max)
}

fn aabb_overlap(first: ([f64; 3], [f64; 3]), second: ([f64; 3], [f64; 3])) -> bool {
    (0..3).all(|axis| first.0[axis] <= second.1[axis] && second.0[axis] <= first.1[axis])
}

fn triangles_share_vertex(
    first: &KnifeBladeLanguageTriangle,
    second: &KnifeBladeLanguageTriangle,
) -> bool {
    first
        .indices
        .iter()
        .any(|left| second.indices.iter().any(|right| left == right))
}

fn segment_intersects_triangle(
    start: [f64; 3],
    end: [f64; 3],
    a: [f64; 3],
    b: [f64; 3],
    c: [f64; 3],
) -> bool {
    let direction = subtract(end, start);
    let edge1 = subtract(b, a);
    let edge2 = subtract(c, a);
    let perpendicular = cross(direction, edge2);
    let determinant = dot(edge1, perpendicular);
    if determinant.abs() <= f64::EPSILON {
        return false;
    }
    let inverse = determinant.recip();
    let offset = subtract(start, a);
    let u = dot(offset, perpendicular) * inverse;
    if !(-1.0e-8..=1.0 + 1.0e-8).contains(&u) {
        return false;
    }
    let cross_offset = cross(offset, edge1);
    let v = dot(direction, cross_offset) * inverse;
    if v < -1.0e-8 || u + v > 1.0 + 1.0e-8 {
        return false;
    }
    let distance = dot(edge2, cross_offset) * inverse;
    (-1.0e-8..=1.0 + 1.0e-8).contains(&distance)
}

fn triangles_intersect(
    mesh: &KnifeBladeLanguageMesh,
    first: &KnifeBladeLanguageTriangle,
    second: &KnifeBladeLanguageTriangle,
) -> bool {
    let first_points = first
        .indices
        .map(|index| mesh.vertices[index as usize].position_m);
    let second_points = second
        .indices
        .map(|index| mesh.vertices[index as usize].position_m);
    for edge in [[0, 1], [1, 2], [2, 0]] {
        if segment_intersects_triangle(
            first_points[edge[0]],
            first_points[edge[1]],
            second_points[0],
            second_points[1],
            second_points[2],
        ) || segment_intersects_triangle(
            second_points[edge[0]],
            second_points[edge[1]],
            first_points[0],
            first_points[1],
            first_points[2],
        ) {
            return true;
        }
    }
    false
}

fn mesh_self_intersection(mesh: &KnifeBladeLanguageMesh) -> Option<(usize, usize)> {
    for first in 0..mesh.triangles.len() {
        let first_triangle = &mesh.triangles[first];
        let first_aabb = triangle_aabb(mesh, first_triangle);
        for second in (first + 1)..mesh.triangles.len() {
            let second_triangle = &mesh.triangles[second];
            if triangles_share_vertex(first_triangle, second_triangle)
                || !aabb_overlap(first_aabb, triangle_aabb(mesh, second_triangle))
            {
                continue;
            }
            if triangles_intersect(mesh, first_triangle, second_triangle) {
                return Some((first, second));
            }
        }
    }
    None
}

fn push_vertex(
    vertices: &mut Vec<KnifeBladeLanguageVertex>,
    point: [f64; 3],
    source_hashes: &[Sha256Hash],
    plan_hash: &Sha256Hash,
    section: &KnifeBladeSection,
    station_index: usize,
    side: KnifeMeshSide,
    local_index: u8,
) -> Result<(), WeaponryDccError> {
    let point = quantize_dcc_position(point);
    validate_vec3(
        "knife_blade.generated_vertex.position_m",
        &point,
        -MAX_COORDINATE_M * 2.0,
        MAX_COORDINATE_M * 2.0,
    )?;
    let lineage = KnifeBladeLanguageVertexLineage::new(
        source_hashes,
        plan_hash,
        section,
        station_index,
        side,
        local_index,
    )?;
    let vertex_id = derived_vertex_id(&lineage)?;
    vertices.push(KnifeBladeLanguageVertex {
        vertex_id,
        position_m: point,
        lineage,
    });
    Ok(())
}

fn push_triangle(
    triangles: &mut Vec<KnifeBladeLanguageTriangle>,
    owners: &mut BTreeMap<KnifeBladePartRole, Vec<u32>>,
    indices: [u32; 3],
    source_hashes: &[Sha256Hash],
    plan_hash: &Sha256Hash,
    role: KnifeBladePartRole,
    station_index: usize,
    segment_index: usize,
    local_index: u8,
) -> Result<(), WeaponryDccError> {
    let lineage = KnifeBladeLanguageTriangleLineage::new(
        source_hashes,
        plan_hash,
        role,
        station_index,
        segment_index,
        local_index,
    )?;
    let triangle_id = derived_triangle_id(&lineage)?;
    let index =
        u32::try_from(triangles.len()).map_err(|_| WeaponryDccError::KnifeBladeInputInvalid {
            reason: "triangle index exceeds u32".to_owned(),
        })?;
    triangles.push(KnifeBladeLanguageTriangle {
        triangle_id,
        indices,
        lineage,
    });
    owners.entry(role).or_default().push(index);
    Ok(())
}

/// Evaluate the V2 section loft.  The generated ring is ordered as top
/// spine→edge followed by bottom spine→edge.  The three width bands become
/// spine/main-face/cutting-edge, with the root span and root cap assigned to
/// root-transition.
pub fn evaluate_knife_blade_language(
    spine: &KnifeCurve,
    edge: &KnifeCurve,
    plan: &KnifeBladeLanguagePlan,
) -> Result<KnifeBladeLanguageMesh, WeaponryDccError> {
    plan.validate_for_curves(spine, edge)?;
    let spine_samples = spine.sample(&plan.spine_plan)?;
    let edge_samples = edge.sample(&plan.edge_plan)?;
    let station_count = spine_samples.points_m.len();
    if source_rail_self_intersects(&spine_samples.points_m)
        || source_rail_self_intersects(&edge_samples.points_m)
        || dual_rails_intersect(&spine_samples.points_m, &edge_samples.points_m)
    {
        return Err(WeaponryDccError::KnifeBladeInputInvalid {
            reason: "spine or edge curve self-intersects".to_owned(),
        });
    }
    let plan_sha256 = plan.canonical_sha256()?;
    let source_hashes = vec![spine.canonical_sha256()?, edge.canonical_sha256()?];
    let vertex_count = station_count
        .checked_mul(KNIFE_BLADE_LANGUAGE_RING_POINTS)
        .and_then(|count| count.checked_add(2))
        .ok_or(WeaponryDccError::KnifeEvaluatedMeshVertexBudgetExceeded {
            count: usize::MAX,
            maximum: MAX_KNIFE_BLADE_LANGUAGE_VERTICES,
        })?;
    let triangle_count = station_count
        .saturating_sub(1)
        .saturating_mul(16)
        .saturating_add(16);
    if vertex_count > MAX_KNIFE_BLADE_LANGUAGE_VERTICES {
        return Err(WeaponryDccError::KnifeEvaluatedMeshVertexBudgetExceeded {
            count: vertex_count,
            maximum: MAX_KNIFE_BLADE_LANGUAGE_VERTICES,
        });
    }
    if triangle_count > MAX_KNIFE_BLADE_LANGUAGE_TRIANGLES {
        return Err(WeaponryDccError::KnifeEvaluatedMeshTriangleBudgetExceeded {
            count: triangle_count,
            maximum: MAX_KNIFE_BLADE_LANGUAGE_TRIANGLES,
        });
    }
    let mut vertices = Vec::with_capacity(vertex_count);
    let mut previous_thickness_axis = None;
    let mut previous_tangent = None;
    let profile_u = [0.0, 0.2, 0.8, 1.0];
    for station_index in 0..station_count {
        let station_t = station_index as f64 / (station_count - 1) as f64;
        let interpolated = interpolate_section(&plan.sections, station_t);
        let section = &plan.sections[interpolated.section_id_index];
        let spine_point = spine_samples.points_m[station_index];
        let edge_point = edge_samples.points_m[station_index];
        let center = scale(add(spine_point, edge_point), 0.5);
        let previous_center = if station_index == 0 {
            scale(
                add(spine_samples.points_m[0], edge_samples.points_m[0]),
                0.5,
            )
        } else {
            scale(
                add(
                    spine_samples.points_m[station_index - 1],
                    edge_samples.points_m[station_index - 1],
                ),
                0.5,
            )
        };
        let next_center = if station_index + 1 == station_count {
            center
        } else {
            scale(
                add(
                    spine_samples.points_m[station_index + 1],
                    edge_samples.points_m[station_index + 1],
                ),
                0.5,
            )
        };
        let tangent_raw = if station_index == 0 {
            subtract(next_center, center)
        } else if station_index + 1 == station_count {
            subtract(center, previous_center)
        } else {
            subtract(next_center, previous_center)
        };
        let tangent = normalized(tangent_raw, station_index)?;
        if previous_tangent.is_some_and(|previous| dot(previous, tangent) <= 0.0) {
            return Err(WeaponryDccError::KnifeBladeInputInvalid {
                reason: format!("blade centerline reverses at station {station_index}"),
            });
        }
        previous_tangent = Some(tangent);
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

        let points = profile_u.map(|u| {
            let rail_point = add(spine_point, scale(width_raw, u));
            let thickness = interpolated.body_thickness_m
                + (interpolated.edge_thickness_m - interpolated.body_thickness_m) * u;
            let bevel_factor = if u <= f64::EPSILON {
                1.0 - interpolated.spine_bevel_fraction * 0.5
            } else if (u - 1.0).abs() <= f64::EPSILON {
                1.0 - interpolated.edge_bevel_fraction * 0.5
            } else {
                1.0
            };
            let half = thickness * 0.5;
            let top_depth = (half + interpolated.center_offset_m) * bevel_factor;
            let bottom_depth = (half - interpolated.center_offset_m) * bevel_factor;
            if top_depth <= MIN_PROFILE_THICKNESS_M || bottom_depth <= MIN_PROFILE_THICKNESS_M {
                // The section constructor validates endpoints and interpolation
                // is linear; this branch is an explicit finite guard for any
                // future profile policy change.
                return Err(WeaponryDccError::KnifeBladeInputInvalid {
                    reason: format!("section depth collapsed at station {station_index}"),
                });
            }
            Ok((
                add(rail_point, scale(thickness_axis, top_depth)),
                subtract(rail_point, scale(thickness_axis, bottom_depth)),
            ))
        });
        let points = points.into_iter().collect::<Result<Vec<_>, _>>()?;
        for (local_index, point) in points.iter().enumerate() {
            push_vertex(
                &mut vertices,
                point.0,
                &source_hashes,
                &plan_sha256,
                section,
                station_index,
                KnifeMeshSide::Top,
                local_index as u8,
            )?;
        }
        for (local_index, point) in points.iter().enumerate() {
            push_vertex(
                &mut vertices,
                point.1,
                &source_hashes,
                &plan_sha256,
                section,
                station_index,
                KnifeMeshSide::Bottom,
                (KNIFE_BLADE_LANGUAGE_PROFILE_POINTS + local_index) as u8,
            )?;
        }
    }

    // Two cap centers make the eight-point section a properly closed polygon.
    let first_section = &plan.sections[0];
    let last_section = &plan.sections[plan.sections.len() - 1];
    let first_center = scale(
        add(spine_samples.points_m[0], edge_samples.points_m[0]),
        0.5,
    );
    let last_center = scale(
        add(
            spine_samples.points_m[station_count - 1],
            edge_samples.points_m[station_count - 1],
        ),
        0.5,
    );
    // At cap centers the local normal is derived from the first/last ring's
    // centerline and rail, matching the neighboring section deterministically.
    let cap_axis = |station_index: usize| -> Result<[f64; 3], WeaponryDccError> {
        let center = if station_index == 0 {
            first_center
        } else {
            last_center
        };
        let next = if station_index == 0 {
            scale(
                add(spine_samples.points_m[1], edge_samples.points_m[1]),
                0.5,
            )
        } else {
            scale(
                add(
                    spine_samples.points_m[station_count - 2],
                    edge_samples.points_m[station_count - 2],
                ),
                0.5,
            )
        };
        let tangent = normalized(
            if station_index == 0 {
                subtract(next, center)
            } else {
                subtract(center, next)
            },
            station_index,
        )?;
        let rail = if station_index == 0 {
            subtract(edge_samples.points_m[0], spine_samples.points_m[0])
        } else {
            subtract(
                edge_samples.points_m[station_count - 1],
                spine_samples.points_m[station_count - 1],
            )
        };
        let width = normalized(
            subtract(rail, scale(tangent, dot(rail, tangent))),
            station_index,
        )?;
        match plan.thickness_axis.vector() {
            Some(axis) => orthogonalized_axis(axis, tangent, width, station_index),
            None => normalized(cross(tangent, width), station_index),
        }
    };
    let first_axis = cap_axis(0)?;
    let last_axis = cap_axis(station_count - 1)?;
    push_vertex(
        &mut vertices,
        add(
            first_center,
            scale(first_axis, first_section.center_offset_m),
        ),
        &source_hashes,
        &plan_sha256,
        first_section,
        0,
        KnifeMeshSide::StartCap,
        KNIFE_BLADE_LANGUAGE_RING_POINTS as u8,
    )?;
    push_vertex(
        &mut vertices,
        add(last_center, scale(last_axis, last_section.center_offset_m)),
        &source_hashes,
        &plan_sha256,
        last_section,
        station_count - 1,
        KnifeMeshSide::EndCap,
        (KNIFE_BLADE_LANGUAGE_RING_POINTS + 1) as u8,
    )?;

    let mut triangles = Vec::with_capacity(triangle_count);
    let mut owners = BTreeMap::<KnifeBladePartRole, Vec<u32>>::new();
    for segment_index in 0..station_count - 1 {
        let start =
            u32::try_from(segment_index * KNIFE_BLADE_LANGUAGE_RING_POINTS).map_err(|_| {
                WeaponryDccError::KnifeBladeInputInvalid {
                    reason: "vertex index exceeds u32".to_owned(),
                }
            })?;
        let next = start + KNIFE_BLADE_LANGUAGE_RING_POINTS as u32;
        let band_role = |band: usize| {
            if segment_index == 0 {
                KnifeBladePartRole::RootTransition
            } else {
                match band {
                    0 => KnifeBladePartRole::Spine,
                    1 => KnifeBladePartRole::MainFace,
                    _ => KnifeBladePartRole::CuttingEdge,
                }
            }
        };
        let mut local_index = 0_u8;
        for band in 0..3 {
            let role = band_role(band);
            let top_left = start + band as u32;
            let top_right = top_left + 1;
            let next_left = next + band as u32;
            let next_right = next_left + 1;
            push_triangle(
                &mut triangles,
                &mut owners,
                [top_left, next_left, next_right],
                &source_hashes,
                &plan_sha256,
                role,
                segment_index,
                segment_index,
                local_index,
            )?;
            local_index += 1;
            push_triangle(
                &mut triangles,
                &mut owners,
                [top_left, next_right, top_right],
                &source_hashes,
                &plan_sha256,
                role,
                segment_index,
                segment_index,
                local_index,
            )?;
            local_index += 1;
            let bottom_left = start + KNIFE_BLADE_LANGUAGE_PROFILE_POINTS as u32 + band as u32;
            let bottom_right = bottom_left + 1;
            let next_bottom_left = next + KNIFE_BLADE_LANGUAGE_PROFILE_POINTS as u32 + band as u32;
            let next_bottom_right = next_bottom_left + 1;
            push_triangle(
                &mut triangles,
                &mut owners,
                [bottom_left, next_bottom_right, next_bottom_left],
                &source_hashes,
                &plan_sha256,
                role,
                segment_index,
                segment_index,
                local_index,
            )?;
            local_index += 1;
            push_triangle(
                &mut triangles,
                &mut owners,
                [bottom_left, bottom_right, next_bottom_right],
                &source_hashes,
                &plan_sha256,
                role,
                segment_index,
                segment_index,
                local_index,
            )?;
            local_index += 1;
        }
        let spine_role = if segment_index == 0 {
            KnifeBladePartRole::RootTransition
        } else {
            KnifeBladePartRole::Spine
        };
        push_triangle(
            &mut triangles,
            &mut owners,
            [start, next + 4, next],
            &source_hashes,
            &plan_sha256,
            spine_role,
            segment_index,
            segment_index,
            local_index,
        )?;
        local_index += 1;
        push_triangle(
            &mut triangles,
            &mut owners,
            [start, start + 4, next + 4],
            &source_hashes,
            &plan_sha256,
            spine_role,
            segment_index,
            segment_index,
            local_index,
        )?;
        local_index += 1;
        let edge_role = KnifeBladePartRole::CuttingEdge;
        push_triangle(
            &mut triangles,
            &mut owners,
            [start + 3, next + 3, next + 7],
            &source_hashes,
            &plan_sha256,
            edge_role,
            segment_index,
            segment_index,
            local_index,
        )?;
        local_index += 1;
        push_triangle(
            &mut triangles,
            &mut owners,
            [start + 3, next + 7, start + 7],
            &source_hashes,
            &plan_sha256,
            edge_role,
            segment_index,
            segment_index,
            local_index,
        )?;
    }
    let start_center = (station_count * KNIFE_BLADE_LANGUAGE_RING_POINTS) as u32;
    let end_center = start_center + 1;
    let start_ring = [0_u32, 1, 2, 3, 7, 6, 5, 4];
    for index in 0..8 {
        push_triangle(
            &mut triangles,
            &mut owners,
            [start_center, start_ring[index], start_ring[(index + 1) % 8]],
            &source_hashes,
            &plan_sha256,
            KnifeBladePartRole::RootTransition,
            0,
            station_count,
            index as u8,
        )?;
        let end_ring = [
            (station_count as u32 - 1) * KNIFE_BLADE_LANGUAGE_RING_POINTS as u32,
            (station_count as u32 - 1) * KNIFE_BLADE_LANGUAGE_RING_POINTS as u32 + 1,
            (station_count as u32 - 1) * KNIFE_BLADE_LANGUAGE_RING_POINTS as u32 + 2,
            (station_count as u32 - 1) * KNIFE_BLADE_LANGUAGE_RING_POINTS as u32 + 3,
            (station_count as u32 - 1) * KNIFE_BLADE_LANGUAGE_RING_POINTS as u32 + 7,
            (station_count as u32 - 1) * KNIFE_BLADE_LANGUAGE_RING_POINTS as u32 + 6,
            (station_count as u32 - 1) * KNIFE_BLADE_LANGUAGE_RING_POINTS as u32 + 5,
            (station_count as u32 - 1) * KNIFE_BLADE_LANGUAGE_RING_POINTS as u32 + 4,
        ];
        push_triangle(
            &mut triangles,
            &mut owners,
            [end_center, end_ring[(index + 1) % 8], end_ring[index]],
            &source_hashes,
            &plan_sha256,
            KnifeBladePartRole::CuttingEdge,
            station_count - 1,
            station_count + 1,
            index as u8,
        )?;
    }

    let mut parts = Vec::with_capacity(4);
    for role in [
        KnifeBladePartRole::Spine,
        KnifeBladePartRole::MainFace,
        KnifeBladePartRole::CuttingEdge,
        KnifeBladePartRole::RootTransition,
    ] {
        let lineage = KnifeBladePartLineage::new(spine, edge, role, plan_sha256.clone())?;
        let triangle_indices = owners.remove(&role).unwrap_or_default();
        let mut vertex_indices = BTreeSet::new();
        for index in &triangle_indices {
            for vertex in triangles[*index as usize].indices {
                vertex_indices.insert(vertex);
            }
        }
        parts.push(KnifeBladeMeshPart {
            part_id: lineage.stable_part_id()?,
            material_zone_id: lineage.material_zone_id()?,
            role,
            lineage,
            vertex_indices: vertex_indices.into_iter().collect(),
            triangle_indices,
        });
    }
    let semantic_sha256 = KnifeBladeLanguageMesh::semantic_hash_for(
        &plan_sha256,
        &source_hashes,
        &vertices,
        &triangles,
        &parts,
    )?;
    let mesh = KnifeBladeLanguageMesh {
        plan_sha256,
        source_curve_sha256: source_hashes,
        vertices,
        triangles,
        parts,
        semantic_sha256,
    };
    mesh.validate()?;
    mesh.validate_view_constraints(&plan.view_constraints)?;
    Ok(mesh)
}

pub fn build_knife_blade_language_mesh(
    spine: &KnifeCurve,
    edge: &KnifeCurve,
    plan: &KnifeBladeLanguagePlan,
) -> Result<KnifeBladeLanguageMesh, WeaponryDccError> {
    evaluate_knife_blade_language(spine, edge, plan)
}

pub fn evaluate_knife_blade_section_loft(
    spine: &KnifeCurve,
    edge: &KnifeCurve,
    plan: &KnifeBladeLanguagePlan,
) -> Result<KnifeBladeLanguageMesh, WeaponryDccError> {
    evaluate_knife_blade_language(spine, edge, plan)
}

pub fn evaluate_knife_blade_profile_loft(
    spine: &KnifeCurve,
    edge: &KnifeCurve,
    plan: &KnifeBladeLanguagePlan,
) -> Result<KnifeBladeLanguageMesh, WeaponryDccError> {
    evaluate_knife_blade_language(spine, edge, plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weaponry_dcc::KnifeCurveBasis;

    fn curves() -> (KnifeCurve, KnifeCurve) {
        let spine = KnifeCurve::new(
            "language-spine",
            KnifeCurveRole::BladeSpine,
            KnifeCurveBasis::Bezier,
            3,
            vec![
                [0.0, 0.0, 0.0],
                [0.02, 0.25, 0.0],
                [0.04, 0.7, 0.01],
                [0.0, 1.0, 0.0],
            ],
            Vec::new(),
            Vec::new(),
            false,
        )
        .expect("spine");
        let edge = KnifeCurve::new(
            "language-edge",
            KnifeCurveRole::BladeEdge,
            KnifeCurveBasis::Bezier,
            3,
            vec![
                [0.36, 0.0, 0.02],
                [0.52, 0.28, 0.03],
                [0.48, 0.7, 0.01],
                [0.08, 1.0, 0.0],
            ],
            Vec::new(),
            Vec::new(),
            false,
        )
        .expect("edge");
        (spine, edge)
    }

    fn sections() -> Vec<KnifeBladeSection> {
        vec![
            KnifeBladeSection::new(
                "root",
                KnifeBladeSectionRole::Root,
                0.0,
                0.12,
                0.035,
                0.25,
                0.08,
                0.01,
            )
            .expect("root"),
            KnifeBladeSection::new(
                "mid",
                KnifeBladeSectionRole::Mid,
                0.32,
                0.09,
                0.022,
                0.18,
                0.12,
                0.005,
            )
            .expect("mid"),
            KnifeBladeSection::new(
                "belly",
                KnifeBladeSectionRole::Belly,
                0.68,
                0.1,
                0.018,
                0.12,
                0.2,
                -0.004,
            )
            .expect("belly"),
            KnifeBladeSection::new(
                "tip",
                KnifeBladeSectionRole::Tip,
                1.0,
                0.035,
                0.008,
                0.1,
                0.3,
                0.0,
            )
            .expect("tip"),
        ]
    }

    #[test]
    fn sectioned_language_is_closed_lineaged_and_deterministic() {
        let (spine, edge) = curves();
        let plan = KnifeBladeLanguagePlan::from_curves(
            &spine,
            &edge,
            17,
            sections(),
            KnifeThicknessAxis::LocalNormal,
        )
        .expect("plan");
        let first = plan.evaluate(&spine, &edge).expect("mesh");
        let second = plan.evaluate(&spine, &edge).expect("replay");
        assert_eq!(first, second);
        assert_eq!(first.canonical_sha256(), second.canonical_sha256());
        assert_eq!(first.vertices.len(), 17 * 8 + 2);
        assert_eq!(first.triangles.len(), (17 - 1) * 16 + 16);
        assert_eq!(first.parts.len(), 4);
        assert!(first
            .parts
            .iter()
            .all(|part| !part.triangle_indices.is_empty()));
        assert!(first.validate().is_ok());
    }

    #[test]
    fn sectioned_language_rejects_partial_views_and_degenerate_or_crossing_rails() {
        let (spine, edge) = curves();
        let mut plan = KnifeBladeLanguagePlan::from_curves(
            &spine,
            &edge,
            17,
            sections(),
            KnifeThicknessAxis::LocalNormal,
        )
        .expect("plan");
        let front = KnifeBladeViewConstraint::new(KnifeBladeView::Front, -1.0, 1.0, -1.0, 2.0)
            .expect("front");
        assert!(plan.clone().with_view_constraints(vec![front]).is_err());
        plan.view_constraints = [
            (KnifeBladeView::Front, [-1.0, 1.0, -1.0, 2.0]),
            (KnifeBladeView::Top, [-1.0, 1.0, -1.0, 1.0]),
            (KnifeBladeView::Bottom, [-1.0, 1.0, -1.0, 1.0]),
            (KnifeBladeView::Left, [-1.0, 2.0, -1.0, 1.0]),
            (KnifeBladeView::Right, [-1.0, 2.0, -1.0, 1.0]),
        ]
        .into_iter()
        .map(|(view, bounds)| {
            KnifeBladeViewConstraint::new(view, bounds[0], bounds[1], bounds[2], bounds[3])
                .expect("view")
        })
        .collect();
        let constrained = plan.evaluate(&spine, &edge);
        assert!(constrained.is_ok(), "{constrained:?}");
        let bad = KnifeCurve::new(
            "crossing-edge",
            KnifeCurveRole::BladeEdge,
            KnifeCurveBasis::Bezier,
            3,
            vec![
                [0.36, 0.0, 0.02],
                [-1.0, 0.3, 0.03],
                [-1.0, 0.7, 0.01],
                [0.08, 1.0, 0.0],
            ],
            Vec::new(),
            Vec::new(),
            false,
        )
        .expect("bad curve remains typed");
        let bad_plan = KnifeBladeLanguagePlan::from_curves(
            &spine,
            &bad,
            17,
            sections(),
            KnifeThicknessAxis::LocalNormal,
        )
        .expect("bad plan");
        assert!(bad_plan.evaluate(&spine, &bad).is_err());
    }
}
