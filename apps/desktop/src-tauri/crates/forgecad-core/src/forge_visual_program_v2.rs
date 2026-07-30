//! Minimal high-freedom typed source for ADR-0021 / FGC-VP201.
//!
//! This module intentionally coexists with `ForgeVisualProgram@1`. V1 remains
//! readable for persisted assets while V2 establishes a new, fail-closed
//! authoring source. V2 never executes code: Rust validates and lowers its
//! first primitive subset into the existing `ShapeProgram@1` boundary.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    compiled_visual_base_material_id, normalize_persisted_shape_program, semantic_sha256,
    CoreError, CoreResult,
};

pub const FORGE_VISUAL_PROGRAM_V2_SCHEMA_VERSION: &str = "ForgeVisualProgram@2";
pub const FORGE_VISUAL_PROGRAM_V2_LOWERING_SCHEMA_VERSION: &str = "ForgeVisualProgramLowering@2";
pub const FORGE_VISUAL_SOURCE_MAP_SCHEMA_VERSION: &str = "ForgeVisualSourceMap@1";
pub const FORGE_VISUAL_PROGRAM_BUDGET_SCHEMA_VERSION: &str = "ProgramBudget@1";
pub const FORGE_VISUAL_PROGRAM_V2_COMPILER_VERSION: &str = "forgecad-core-vp201.2";

fn invalid(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message.into())
}

fn valid_id(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix)
        && value.len() <= 128
        && value.len() > prefix.len()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
}

fn require_id(field: &str, value: &str, prefix: &str) -> CoreResult<()> {
    if valid_id(value, prefix) {
        Ok(())
    } else {
        Err(invalid(
            "FORGE_VISUAL_V2_ID_INVALID",
            format!("{field} must match the lowercase ShapeProgram-compatible {prefix} ID set"),
        ))
    }
}

fn require_finite(field: &str, value: f64) -> CoreResult<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(invalid(
            "FORGE_VISUAL_V2_NON_FINITE",
            format!("{field} must be finite"),
        ))
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForgeVisualUnitSystemV2 {
    Millimeter,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForgeVisualParameterKindV2 {
    Number,
    Integer,
    Boolean,
    Enum,
    Length,
    Angle,
    Ratio,
    Color,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForgeVisualParameterUnitV2 {
    Unitless,
    Count,
    Boolean,
    EnumValue,
    Millimeter,
    Radian,
    Ratio,
    LinearRgb,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualParameterV2 {
    pub parameter_id: String,
    pub kind: ForgeVisualParameterKindV2,
    pub unit: ForgeVisualParameterUnitV2,
    pub default: Value,
    #[serde(default)]
    pub minimum: Option<f64>,
    #[serde(default)]
    pub maximum: Option<f64>,
    #[serde(default)]
    pub allowed_values: Vec<String>,
}

impl ForgeVisualParameterV2 {
    pub(crate) fn validate(&self) -> CoreResult<()> {
        require_id("parameter_id", &self.parameter_id, "param_")?;
        let expected_unit = match self.kind {
            ForgeVisualParameterKindV2::Number => ForgeVisualParameterUnitV2::Unitless,
            ForgeVisualParameterKindV2::Integer => ForgeVisualParameterUnitV2::Count,
            ForgeVisualParameterKindV2::Boolean => ForgeVisualParameterUnitV2::Boolean,
            ForgeVisualParameterKindV2::Enum => ForgeVisualParameterUnitV2::EnumValue,
            ForgeVisualParameterKindV2::Length => ForgeVisualParameterUnitV2::Millimeter,
            ForgeVisualParameterKindV2::Angle => ForgeVisualParameterUnitV2::Radian,
            ForgeVisualParameterKindV2::Ratio => ForgeVisualParameterUnitV2::Ratio,
            ForgeVisualParameterKindV2::Color => ForgeVisualParameterUnitV2::LinearRgb,
        };
        if self.unit != expected_unit {
            return Err(invalid(
                "FORGE_VISUAL_V2_PARAMETER_UNIT_INVALID",
                "parameter unit must match its declared kind",
            ));
        }
        match self.kind {
            ForgeVisualParameterKindV2::Number
            | ForgeVisualParameterKindV2::Length
            | ForgeVisualParameterKindV2::Angle
            | ForgeVisualParameterKindV2::Ratio => {
                let value = self.default.as_f64().ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_V2_PARAMETER_TYPE_INVALID",
                        "numeric parameter default must be a number",
                    )
                })?;
                require_finite("parameter.default", value)?;
                let minimum = self.minimum.ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_V2_PARAMETER_RANGE_REQUIRED",
                        "numeric parameter minimum is required",
                    )
                })?;
                let maximum = self.maximum.ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_V2_PARAMETER_RANGE_REQUIRED",
                        "numeric parameter maximum is required",
                    )
                })?;
                require_finite("parameter.minimum", minimum)?;
                require_finite("parameter.maximum", maximum)?;
                if minimum > maximum || value < minimum || value > maximum {
                    return Err(invalid(
                        "FORGE_VISUAL_V2_PARAMETER_RANGE_INVALID",
                        "numeric parameter default must be inside an ordered closed range",
                    ));
                }
                if self.kind == ForgeVisualParameterKindV2::Ratio
                    && (minimum < 0.0 || maximum > 1.0)
                {
                    return Err(invalid(
                        "FORGE_VISUAL_V2_RATIO_RANGE_INVALID",
                        "ratio parameter range must remain within 0..=1",
                    ));
                }
            }
            ForgeVisualParameterKindV2::Integer => {
                let value = self.default.as_i64().ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_V2_PARAMETER_TYPE_INVALID",
                        "integer parameter default must be an integer",
                    )
                })? as f64;
                let minimum = self.minimum.ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_V2_PARAMETER_RANGE_REQUIRED",
                        "integer parameter minimum is required",
                    )
                })?;
                let maximum = self.maximum.ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_V2_PARAMETER_RANGE_REQUIRED",
                        "integer parameter maximum is required",
                    )
                })?;
                if minimum.fract() != 0.0
                    || maximum.fract() != 0.0
                    || minimum > maximum
                    || value < minimum
                    || value > maximum
                {
                    return Err(invalid(
                        "FORGE_VISUAL_V2_PARAMETER_RANGE_INVALID",
                        "integer parameter requires an ordered integral range containing default",
                    ));
                }
            }
            ForgeVisualParameterKindV2::Boolean => {
                if !self.default.is_boolean() {
                    return Err(invalid(
                        "FORGE_VISUAL_V2_PARAMETER_TYPE_INVALID",
                        "boolean parameter default must be boolean",
                    ));
                }
            }
            ForgeVisualParameterKindV2::Enum => {
                let value = self.default.as_str().ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_V2_PARAMETER_TYPE_INVALID",
                        "enum parameter default must be a string",
                    )
                })?;
                if self.allowed_values.is_empty()
                    || self.allowed_values.len() > 64
                    || !self.allowed_values.iter().any(|allowed| allowed == value)
                {
                    return Err(invalid(
                        "FORGE_VISUAL_V2_ENUM_INVALID",
                        "enum parameter default must belong to a non-empty bounded allowlist",
                    ));
                }
            }
            ForgeVisualParameterKindV2::Color => {
                let color = self.default.as_array().ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_V2_PARAMETER_TYPE_INVALID",
                        "color parameter default must be an RGB array",
                    )
                })?;
                if color.len() != 3
                    || color
                        .iter()
                        .filter_map(Value::as_f64)
                        .any(|channel| !(0.0..=1.0).contains(&channel))
                    || color.iter().any(|channel| channel.as_f64().is_none())
                {
                    return Err(invalid(
                        "FORGE_VISUAL_V2_COLOR_INVALID",
                        "color parameter must contain three channels within 0..=1",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum ForgeVisualScalarV2 {
    Literal(f64),
    Parameter { parameter_id: String },
}

impl ForgeVisualScalarV2 {
    fn resolve(
        &self,
        parameters: &BTreeMap<&str, &ForgeVisualParameterV2>,
        field: &str,
        expected_kind: ForgeVisualParameterKindV2,
    ) -> CoreResult<(f64, Option<String>)> {
        match self {
            Self::Literal(value) => {
                require_finite(field, *value)?;
                Ok((*value, None))
            }
            Self::Parameter { parameter_id } => {
                require_id(field, parameter_id, "param_")?;
                let parameter = parameters.get(parameter_id.as_str()).ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_V2_REFERENCE_MISSING",
                        format!("{field} references an unknown parameter"),
                    )
                })?;
                if parameter.kind != expected_kind {
                    return Err(invalid(
                        "FORGE_VISUAL_V2_PARAMETER_UNIT_INVALID",
                        format!("{field} references a parameter with incompatible units"),
                    ));
                }
                let value = parameter.default.as_f64().ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_V2_PARAMETER_TYPE_INVALID",
                        format!("{field} requires a numeric parameter"),
                    )
                })?;
                require_finite(field, value)?;
                Ok((value, Some(parameter_id.clone())))
            }
        }
    }
}

fn resolve_vector3(
    values: &[ForgeVisualScalarV2; 3],
    parameters: &BTreeMap<&str, &ForgeVisualParameterV2>,
    field: &str,
    expected_kind: ForgeVisualParameterKindV2,
) -> CoreResult<([f64; 3], Vec<String>)> {
    let mut resolved = [0.0; 3];
    let mut parameter_ids = Vec::new();
    for (index, scalar) in values.iter().enumerate() {
        let (value, parameter_id) =
            scalar.resolve(parameters, &format!("{field}[{index}]"), expected_kind)?;
        resolved[index] = value;
        if let Some(parameter_id) = parameter_id {
            parameter_ids.push(parameter_id);
        }
    }
    Ok((resolved, parameter_ids))
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualMaterialV2 {
    pub material_id: String,
    pub base_material_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ForgeVisualNodeV2 {
    Box {
        node_id: String,
        size: [ForgeVisualScalarV2; 3],
    },
    Cylinder {
        node_id: String,
        radius: ForgeVisualScalarV2,
        height: ForgeVisualScalarV2,
    },
    Transform {
        node_id: String,
        input_node_id: String,
        position: [ForgeVisualScalarV2; 3],
        rotation: [ForgeVisualScalarV2; 3],
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

impl ForgeVisualNodeV2 {
    fn node_id(&self) -> &str {
        match self {
            Self::Box { node_id, .. }
            | Self::Cylinder { node_id, .. }
            | Self::Transform { node_id, .. }
            | Self::Part { node_id, .. }
            | Self::MaterialZone { node_id, .. } => node_id,
        }
    }

    fn input_node_id(&self) -> Option<&str> {
        match self {
            Self::Box { .. } | Self::Cylinder { .. } => None,
            Self::Transform { input_node_id, .. }
            | Self::Part { input_node_id, .. }
            | Self::MaterialZone { input_node_id, .. } => Some(input_node_id),
        }
    }

    fn kind_name(&self) -> &'static str {
        match self {
            Self::Box { .. } => "box",
            Self::Cylinder { .. } => "cylinder",
            Self::Transform { .. } => "transform",
            Self::Part { .. } => "part",
            Self::MaterialZone { .. } => "material_zone",
        }
    }

    fn validate(&self, parameters: &BTreeMap<&str, &ForgeVisualParameterV2>) -> CoreResult<()> {
        require_id("node_id", self.node_id(), "node_")?;
        if let Some(input_node_id) = self.input_node_id() {
            require_id("input_node_id", input_node_id, "node_")?;
        }
        match self {
            Self::Box { size, .. } => {
                let (size, _) = resolve_vector3(
                    size,
                    parameters,
                    "box.size",
                    ForgeVisualParameterKindV2::Length,
                )?;
                for value in size {
                    if value <= 0.0 || value > 100_000.0 {
                        return Err(invalid(
                            "FORGE_VISUAL_V2_DIMENSION_INVALID",
                            "box dimensions must be within 0..=100000 millimeters",
                        ));
                    }
                }
            }
            Self::Cylinder { radius, height, .. } => {
                let (radius, _) = radius.resolve(
                    parameters,
                    "cylinder.radius",
                    ForgeVisualParameterKindV2::Length,
                )?;
                let (height, _) = height.resolve(
                    parameters,
                    "cylinder.height",
                    ForgeVisualParameterKindV2::Length,
                )?;
                if radius <= 0.0 || radius > 100_000.0 || height <= 0.0 || height > 100_000.0 {
                    return Err(invalid(
                        "FORGE_VISUAL_V2_DIMENSION_INVALID",
                        "cylinder dimensions exceed the reviewed subset",
                    ));
                }
            }
            Self::Transform {
                position, rotation, ..
            } => {
                let (position, _) = resolve_vector3(
                    position,
                    parameters,
                    "transform.position",
                    ForgeVisualParameterKindV2::Length,
                )?;
                if position.iter().any(|value| value.abs() > 100_000.0) {
                    return Err(invalid(
                        "FORGE_VISUAL_V2_POSITION_RANGE_INVALID",
                        "transform position must remain within +/-100000 millimeters",
                    ));
                }
                let (rotation, _) = resolve_vector3(
                    rotation,
                    parameters,
                    "transform.rotation",
                    ForgeVisualParameterKindV2::Angle,
                )?;
                if rotation
                    .iter()
                    .any(|value| !(-std::f64::consts::PI..=std::f64::consts::PI).contains(value))
                {
                    return Err(invalid(
                        "FORGE_VISUAL_V2_ROTATION_RANGE_INVALID",
                        "transform rotation must remain within [-pi, pi] radians",
                    ));
                }
            }
            Self::Part { part_id, role, .. } => {
                require_id("part_id", part_id, "part_")?;
                if role.len() < 2
                    || role.len() > 64
                    || !role.starts_with(|character: char| character.is_ascii_lowercase())
                    || !role.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte)
                    })
                {
                    return Err(invalid(
                        "FORGE_VISUAL_V2_ROLE_INVALID",
                        "Part role must match the ShapeProgram lowercase role set",
                    ));
                }
            }
            Self::MaterialZone {
                zone_id,
                material_id,
                ..
            } => {
                require_id("zone_id", zone_id, "zone_")?;
                require_id("material_id", material_id, "mat_")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualOutputV2 {
    pub output_id: String,
    pub node_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualProgramBudgetV2 {
    pub schema_version: String,
    pub max_nodes: u16,
    pub max_parts: u16,
    pub max_materials: u16,
    pub max_outputs: u16,
    pub max_primitives: u16,
    pub triangle_budget: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualProgramV2 {
    pub schema_version: String,
    pub program_id: String,
    pub domain: String,
    pub units: ForgeVisualUnitSystemV2,
    pub seed: u32,
    #[serde(default)]
    pub parameters: Vec<ForgeVisualParameterV2>,
    pub materials: Vec<ForgeVisualMaterialV2>,
    pub nodes: Vec<ForgeVisualNodeV2>,
    pub outputs: Vec<ForgeVisualOutputV2>,
    pub budgets: ForgeVisualProgramBudgetV2,
}

struct ResolvedOutputChainV2<'a> {
    primitive: &'a ForgeVisualNodeV2,
    transform: &'a ForgeVisualNodeV2,
    part: &'a ForgeVisualNodeV2,
    material_zone: &'a ForgeVisualNodeV2,
}

impl ResolvedOutputChainV2<'_> {
    fn node_ids(&self) -> Vec<String> {
        vec![
            self.primitive.node_id().to_string(),
            self.transform.node_id().to_string(),
            self.part.node_id().to_string(),
            self.material_zone.node_id().to_string(),
        ]
    }
}

fn resolve_output_chain_v2<'a>(
    output: &ForgeVisualOutputV2,
    nodes: &'a BTreeMap<&str, &ForgeVisualNodeV2>,
) -> CoreResult<ResolvedOutputChainV2<'a>> {
    let material_zone = nodes.get(output.node_id.as_str()).copied().ok_or_else(|| {
        invalid(
            "FORGE_VISUAL_V2_REFERENCE_MISSING",
            "output node_id must reference a declared node",
        )
    })?;
    let ForgeVisualNodeV2::MaterialZone {
        input_node_id: part_node_id,
        ..
    } = material_zone
    else {
        return Err(invalid(
            "FORGE_VISUAL_V2_GRAPH_ORDER_INVALID",
            "VP201 output must reference a material_zone node",
        ));
    };
    let part = nodes.get(part_node_id.as_str()).copied().ok_or_else(|| {
        invalid(
            "FORGE_VISUAL_V2_REFERENCE_MISSING",
            "material_zone must reference a declared Part node",
        )
    })?;
    let ForgeVisualNodeV2::Part {
        input_node_id: transform_node_id,
        ..
    } = part
    else {
        return Err(invalid(
            "FORGE_VISUAL_V2_GRAPH_ORDER_INVALID",
            "material_zone input must be a Part node",
        ));
    };
    let transform = nodes
        .get(transform_node_id.as_str())
        .copied()
        .ok_or_else(|| {
            invalid(
                "FORGE_VISUAL_V2_REFERENCE_MISSING",
                "Part must reference a declared transform node",
            )
        })?;
    let ForgeVisualNodeV2::Transform {
        input_node_id: primitive_node_id,
        ..
    } = transform
    else {
        return Err(invalid(
            "FORGE_VISUAL_V2_GRAPH_ORDER_INVALID",
            "Part input must be a transform node",
        ));
    };
    let primitive = nodes
        .get(primitive_node_id.as_str())
        .copied()
        .ok_or_else(|| {
            invalid(
                "FORGE_VISUAL_V2_REFERENCE_MISSING",
                "transform must reference a declared primitive node",
            )
        })?;
    if !matches!(
        primitive,
        ForgeVisualNodeV2::Box { .. } | ForgeVisualNodeV2::Cylinder { .. }
    ) {
        return Err(invalid(
            "FORGE_VISUAL_V2_GRAPH_ORDER_INVALID",
            "transform input must be a reviewed primitive node",
        ));
    }
    Ok(ResolvedOutputChainV2 {
        primitive,
        transform,
        part,
        material_zone,
    })
}

impl ForgeVisualProgramV2 {
    pub fn parse_and_validate(value: &Value) -> CoreResult<Self> {
        let program: Self = serde_json::from_value(value.clone()).map_err(|error| {
            invalid(
                "FORGE_VISUAL_V2_PARSE_FAILED",
                format!("ForgeVisualProgram@2 failed closed: {error}"),
            )
        })?;
        program.validate()?;
        Ok(program)
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != FORGE_VISUAL_PROGRAM_V2_SCHEMA_VERSION {
            return Err(invalid(
                "FORGE_VISUAL_V2_SCHEMA_VERSION_INVALID",
                "schema_version must be ForgeVisualProgram@2",
            ));
        }
        require_id("program_id", &self.program_id, "visual_")?;
        if self.seed > i32::MAX as u32 {
            return Err(invalid(
                "FORGE_VISUAL_V2_SEED_INVALID",
                "seed must fit the ShapeProgram signed 31-bit range",
            ));
        }
        if self.budgets.schema_version != FORGE_VISUAL_PROGRAM_BUDGET_SCHEMA_VERSION {
            return Err(invalid(
                "FORGE_VISUAL_V2_BUDGET_SCHEMA_INVALID",
                "budgets.schema_version must be ProgramBudget@1",
            ));
        }
        if self.domain.is_empty() || self.domain.len() > 96 {
            return Err(invalid(
                "FORGE_VISUAL_V2_DOMAIN_INVALID",
                "domain must be non-empty and bounded",
            ));
        }
        if self.budgets.max_nodes == 0
            || self.budgets.max_nodes > 256
            || self.budgets.max_parts == 0
            || self.budgets.max_parts > 256
            || self.budgets.max_materials == 0
            || self.budgets.max_materials > 64
            || self.budgets.max_outputs == 0
            || self.budgets.max_outputs > 128
            || self.budgets.max_primitives == 0
            || self.budgets.max_primitives > 256
            || !(100..=100_000).contains(&self.budgets.triangle_budget)
        {
            return Err(invalid(
                "FORGE_VISUAL_V2_BUDGET_INVALID",
                "program budgets exceed the reviewed VP201 ceiling",
            ));
        }
        if self.nodes.is_empty()
            || self.nodes.len() > self.budgets.max_nodes as usize
            || self.materials.is_empty()
            || self.materials.len() > self.budgets.max_materials as usize
            || self.outputs.is_empty()
            || self.outputs.len() > self.budgets.max_outputs as usize
            || self.parameters.len() > 64
        {
            return Err(invalid(
                "FORGE_VISUAL_V2_BUDGET_EXCEEDED",
                "program contents exceed declared static budgets",
            ));
        }
        let mut parameter_ids = BTreeSet::new();
        for parameter in &self.parameters {
            parameter.validate()?;
            if !parameter_ids.insert(parameter.parameter_id.as_str()) {
                return Err(invalid(
                    "FORGE_VISUAL_V2_DUPLICATE_ID",
                    "parameter IDs must be unique",
                ));
            }
        }
        let parameters = self
            .parameters
            .iter()
            .map(|parameter| (parameter.parameter_id.as_str(), parameter))
            .collect::<BTreeMap<_, _>>();
        let mut material_ids = BTreeSet::new();
        for material in &self.materials {
            require_id("material_id", &material.material_id, "mat_")?;
            require_id("base_material_id", &material.base_material_id, "mat_")?;
            if compiled_visual_base_material_id(&material.base_material_id).is_none() {
                return Err(invalid(
                    "FORGE_VISUAL_V2_CAPABILITY_DENIED",
                    "base_material_id is outside the reviewed PBR material capability",
                ));
            }
            if !material_ids.insert(material.material_id.as_str()) {
                return Err(invalid(
                    "FORGE_VISUAL_V2_DUPLICATE_ID",
                    "material IDs must be unique",
                ));
            }
        }
        let mut node_ids = BTreeSet::new();
        for node in &self.nodes {
            node.validate(&parameters)?;
            if !node_ids.insert(node.node_id()) {
                return Err(invalid(
                    "FORGE_VISUAL_V2_DUPLICATE_ID",
                    "node IDs must be unique",
                ));
            }
        }
        let nodes = self
            .nodes
            .iter()
            .map(|node| (node.node_id(), node))
            .collect::<BTreeMap<_, _>>();
        for node in &self.nodes {
            if let Some(input_node_id) = node.input_node_id() {
                if !nodes.contains_key(input_node_id) {
                    return Err(invalid(
                        "FORGE_VISUAL_V2_REFERENCE_MISSING",
                        format!(
                            "{} node {} references an unknown input node",
                            node.kind_name(),
                            node.node_id()
                        ),
                    ));
                }
            }
            if let ForgeVisualNodeV2::MaterialZone { material_id, .. } = node {
                if !material_ids.contains(material_id.as_str()) {
                    return Err(invalid(
                        "FORGE_VISUAL_V2_REFERENCE_MISSING",
                        "material_zone material_id must reference a declared material",
                    ));
                }
            }
        }
        for start in &self.nodes {
            let mut path = BTreeSet::new();
            let mut current = start;
            loop {
                if !path.insert(current.node_id()) {
                    return Err(invalid(
                        "FORGE_VISUAL_V2_GRAPH_CYCLE",
                        "node input graph must be acyclic",
                    ));
                }
                let Some(input_node_id) = current.input_node_id() else {
                    break;
                };
                current = nodes.get(input_node_id).copied().ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_V2_REFERENCE_MISSING",
                        "node input must reference a declared node",
                    )
                })?;
            }
        }
        let primitive_count = self
            .nodes
            .iter()
            .filter(|node| {
                matches!(
                    node,
                    ForgeVisualNodeV2::Box { .. } | ForgeVisualNodeV2::Cylinder { .. }
                )
            })
            .count();
        let mut part_ids = BTreeSet::new();
        let mut zone_ids = BTreeSet::new();
        for node in &self.nodes {
            match node {
                ForgeVisualNodeV2::Part { part_id, .. } => {
                    if !part_ids.insert(part_id.as_str()) {
                        return Err(invalid(
                            "FORGE_VISUAL_V2_DUPLICATE_ID",
                            "VP201 Part IDs must be unique",
                        ));
                    }
                }
                ForgeVisualNodeV2::MaterialZone { zone_id, .. } => {
                    if !zone_ids.insert(zone_id.as_str()) {
                        return Err(invalid(
                            "FORGE_VISUAL_V2_DUPLICATE_ID",
                            "VP201 Material Zone IDs must be unique",
                        ));
                    }
                }
                _ => {}
            }
        }
        if primitive_count > self.budgets.max_primitives as usize {
            return Err(invalid(
                "FORGE_VISUAL_V2_BUDGET_EXCEEDED",
                "primitive count exceeds declared static budget",
            ));
        }
        if part_ids.len() > self.budgets.max_parts as usize {
            return Err(invalid(
                "FORGE_VISUAL_V2_BUDGET_EXCEEDED",
                "unique Part count exceeds declared static budget",
            ));
        }
        let estimated_triangles = self.nodes.iter().try_fold(0_u32, |total, node| {
            let addition = match node {
                ForgeVisualNodeV2::Box { .. } => 12,
                ForgeVisualNodeV2::Cylinder { .. } => 256,
                _ => 0,
            };
            total.checked_add(addition).ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_V2_BUDGET_EXCEEDED",
                    "static triangle estimate overflowed",
                )
            })
        })?;
        if estimated_triangles > self.budgets.triangle_budget {
            return Err(invalid(
                "FORGE_VISUAL_V2_BUDGET_EXCEEDED",
                "reviewed primitive triangle upper bound exceeds triangle_budget",
            ));
        }
        let mut output_ids = BTreeSet::new();
        let mut used_nodes = BTreeSet::new();
        for output in &self.outputs {
            require_id("output_id", &output.output_id, "output_")?;
            if !output_ids.insert(output.output_id.as_str()) {
                return Err(invalid(
                    "FORGE_VISUAL_V2_DUPLICATE_ID",
                    "output IDs must be unique",
                ));
            }
            let chain = resolve_output_chain_v2(output, &nodes)?;
            for node_id in chain.node_ids() {
                if !used_nodes.insert(node_id) {
                    return Err(invalid(
                        "FORGE_VISUAL_V2_GRAPH_FANOUT_UNSUPPORTED",
                        "VP201 requires one independent linear node chain per output",
                    ));
                }
            }
        }
        if used_nodes.len() != self.nodes.len() {
            return Err(invalid(
                "FORGE_VISUAL_V2_GRAPH_ORPHANED",
                "every VP201 node must be reachable from exactly one output",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualSourceMapEntryV2 {
    pub source_node_ids: Vec<String>,
    pub operation_id: String,
    pub output_id: String,
    pub part_id: String,
    pub material_zone_id: String,
    pub authored_material_id: String,
    pub compiled_material_id: String,
    pub parameter_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualSourceMapV2 {
    pub schema_version: String,
    pub source_program_sha256: String,
    pub entries: Vec<ForgeVisualSourceMapEntryV2>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualProgramLoweringV2 {
    pub schema_version: String,
    pub compiler_version: String,
    pub source_program_sha256: String,
    pub source_map_sha256: String,
    pub shape_program: Value,
    pub source_map: ForgeVisualSourceMapV2,
}

pub fn lower_forge_visual_program_v2(value: &Value) -> CoreResult<ForgeVisualProgramLoweringV2> {
    let program = ForgeVisualProgramV2::parse_and_validate(value)?;
    let source_program_sha256 = semantic_sha256(&program)?;
    let parameters = program
        .parameters
        .iter()
        .map(|parameter| (parameter.parameter_id.as_str(), parameter))
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
    let mut operations = Vec::with_capacity(program.outputs.len());
    let mut outputs = Vec::with_capacity(program.outputs.len());
    let mut source_entries = Vec::with_capacity(program.outputs.len());
    let mut operation_ids = BTreeSet::new();
    for output in &program.outputs {
        let chain = resolve_output_chain_v2(output, &nodes)?;
        let ForgeVisualNodeV2::Transform {
            position, rotation, ..
        } = chain.transform
        else {
            unreachable!("validated VP201 chain contains transform")
        };
        let (position, mut parameter_ids) = resolve_vector3(
            position,
            &parameters,
            "transform.position",
            ForgeVisualParameterKindV2::Length,
        )?;
        let (rotation, rotation_parameters) = resolve_vector3(
            rotation,
            &parameters,
            "transform.rotation",
            ForgeVisualParameterKindV2::Angle,
        )?;
        parameter_ids.extend(rotation_parameters);
        let ForgeVisualNodeV2::Part { part_id, role, .. } = chain.part else {
            unreachable!("validated VP201 chain contains Part")
        };
        let ForgeVisualNodeV2::MaterialZone {
            zone_id,
            material_id,
            ..
        } = chain.material_zone
        else {
            unreachable!("validated VP201 chain contains material zone")
        };
        let authored_material = materials
            .get(material_id.as_str())
            .copied()
            .ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_V2_REFERENCE_MISSING",
                    "material_zone references an unknown authored material",
                )
            })?;
        let compiled_material_id = compiled_visual_base_material_id(
            &authored_material.base_material_id,
        )
        .ok_or_else(|| {
            invalid(
                "FORGE_VISUAL_V2_CAPABILITY_DENIED",
                "authored material has no reviewed compiled PBR base",
            )
        })?;
        let (op, args) = match chain.primitive {
            ForgeVisualNodeV2::Box { size, .. } => {
                let (size, size_parameters) = resolve_vector3(
                    size,
                    &parameters,
                    "box.size",
                    ForgeVisualParameterKindV2::Length,
                )?;
                parameter_ids.extend(size_parameters);
                (
                    "box",
                    json!({
                        "size": size,
                        "position": position,
                        "rotation": rotation,
                        "part_role": role,
                        "zone_id": zone_id,
                        "material_id": compiled_material_id
                    }),
                )
            }
            ForgeVisualNodeV2::Cylinder { radius, height, .. } => {
                let (radius, radius_parameter) = radius.resolve(
                    &parameters,
                    "cylinder.radius",
                    ForgeVisualParameterKindV2::Length,
                )?;
                let (height, height_parameter) = height.resolve(
                    &parameters,
                    "cylinder.height",
                    ForgeVisualParameterKindV2::Length,
                )?;
                parameter_ids.extend(radius_parameter);
                parameter_ids.extend(height_parameter);
                (
                    "cylinder",
                    json!({
                        "radius": radius,
                        "height": height,
                        "position": position,
                        "rotation": rotation,
                        "part_role": role,
                        "zone_id": zone_id,
                        "material_id": compiled_material_id
                    }),
                )
            }
            _ => unreachable!("validated VP201 chain ends in a primitive"),
        };
        parameter_ids.sort();
        parameter_ids.dedup();
        let primitive_suffix = chain
            .primitive
            .node_id()
            .strip_prefix("node_")
            .ok_or_else(|| invalid("FORGE_VISUAL_V2_ID_INVALID", "primitive node ID invalid"))?;
        let operation_id = format!("op_{primitive_suffix}");
        if !operation_ids.insert(operation_id.clone()) {
            return Err(invalid(
                "FORGE_VISUAL_V2_LOWERING_ID_COLLISION",
                "two source nodes lower to the same ShapeProgram operation ID",
            ));
        }
        operations.push(json!({
            "operation_id": operation_id,
            "op": op,
            "inputs": [],
            "args": args
        }));
        outputs.push(json!({
            "output_id": output.output_id,
            "operation_id": operation_id,
            "kind": "mesh",
            "part_role": role
        }));
        source_entries.push(ForgeVisualSourceMapEntryV2 {
            source_node_ids: chain.node_ids(),
            operation_id: operation_id.clone(),
            output_id: output.output_id.clone(),
            part_id: part_id.to_string(),
            material_zone_id: zone_id.to_string(),
            authored_material_id: material_id.to_string(),
            compiled_material_id: compiled_material_id.to_string(),
            parameter_ids,
        });
    }
    source_entries.sort_by(|left, right| left.output_id.cmp(&right.output_id));
    let source_map = ForgeVisualSourceMapV2 {
        schema_version: FORGE_VISUAL_SOURCE_MAP_SCHEMA_VERSION.into(),
        source_program_sha256: source_program_sha256.clone(),
        entries: source_entries,
    };
    let source_map_sha256 = semantic_sha256(&source_map)?;
    let shape_program = normalize_persisted_shape_program(&json!({
        "schema_version": "ShapeProgram@1",
        "program_id": format!("shape_{}", program.program_id.strip_prefix("visual_").unwrap()),
        "units": "millimeter",
        "seed": program.seed,
        "triangle_budget": program.budgets.triangle_budget,
        "parameters": [],
        "operations": operations,
        "outputs": outputs,
        "non_functional_only": true
    }))?;
    Ok(ForgeVisualProgramLoweringV2 {
        schema_version: FORGE_VISUAL_PROGRAM_V2_LOWERING_SCHEMA_VERSION.into(),
        compiler_version: FORGE_VISUAL_PROGRAM_V2_COMPILER_VERSION.into(),
        source_program_sha256: source_program_sha256.clone(),
        source_map_sha256,
        shape_program,
        source_map,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_program() -> Value {
        json!({
            "schema_version": "ForgeVisualProgram@2",
            "program_id": "visual_vp201_minimal",
            "domain": "mechanical_hard_surface",
            "units": "millimeter",
            "seed": 29,
            "parameters": [
                {
                    "parameter_id": "param_shell_length",
                    "kind": "length",
                    "unit": "millimeter",
                    "default": 120.0,
                    "minimum": 24.0,
                    "maximum": 400.0,
                    "allowed_values": []
                },
                {
                    "parameter_id": "param_shell_rotation",
                    "kind": "angle",
                    "unit": "radian",
                    "default": 0.25,
                    "minimum": -3.141592653589793,
                    "maximum": 3.141592653589793,
                    "allowed_values": []
                }
            ],
            "materials": [{
                "material_id": "mat_shell",
                "base_material_id": "mat_graphite"
            }],
            "nodes": [
                {
                    "kind": "box",
                    "node_id": "node_primary_shell",
                    "size": [{"parameter_id":"param_shell_length"}, 48.0, 32.0]
                },
                {
                    "kind": "transform",
                    "node_id": "node_primary_transform",
                    "input_node_id": "node_primary_shell",
                    "position": [16.0, 0.0, 0.0],
                    "rotation": [0.0, {"parameter_id":"param_shell_rotation"}, 0.0]
                },
                {
                    "kind": "part",
                    "node_id": "node_primary_part",
                    "input_node_id": "node_primary_transform",
                    "part_id": "part_primary",
                    "role": "primary_form"
                },
                {
                    "kind": "material_zone",
                    "node_id": "node_primary_zone",
                    "input_node_id": "node_primary_part",
                    "zone_id": "zone_primary",
                    "material_id": "mat_shell"
                }
            ],
            "outputs": [{
                "output_id": "output_primary_shell",
                "node_id": "node_primary_zone"
            }],
            "budgets": {
                "schema_version": "ProgramBudget@1",
                "max_nodes": 8,
                "max_parts": 8,
                "max_materials": 4,
                "max_outputs": 8,
                "max_primitives": 8,
                "triangle_budget": 4000
            }
        })
    }

    #[test]
    fn vp201_v2_lowers_to_shape_program_with_source_map() {
        let lowering = lower_forge_visual_program_v2(&minimal_program()).unwrap();
        assert_eq!(lowering.schema_version, "ForgeVisualProgramLowering@2");
        assert_eq!(lowering.compiler_version, "forgecad-core-vp201.2");
        assert_eq!(
            lowering.source_map.source_program_sha256,
            lowering.source_program_sha256
        );
        assert_eq!(lowering.shape_program["schema_version"], "ShapeProgram@1");
        assert_eq!(lowering.shape_program["operations"][0]["op"], "box");
        assert_eq!(lowering.source_map.entries.len(), 1);
        assert_eq!(
            lowering.source_map.entries[0].source_node_ids,
            vec![
                "node_primary_shell",
                "node_primary_transform",
                "node_primary_part",
                "node_primary_zone"
            ]
        );
        assert_eq!(lowering.source_map.entries[0].part_id, "part_primary");
        assert_eq!(
            lowering.source_map.entries[0].parameter_ids,
            vec!["param_shell_length", "param_shell_rotation"]
        );
        assert_eq!(
            lowering.shape_program["operations"][0]["args"]["material_id"],
            "mat_graphite"
        );
        assert_eq!(
            lowering.source_map.entries[0].authored_material_id,
            "mat_shell"
        );
        assert_eq!(
            lowering.source_map.entries[0].compiled_material_id,
            "mat_graphite"
        );
        assert_eq!(
            lowering.shape_program["operations"][0]["args"]["size"][0],
            120.0
        );
    }

    #[test]
    fn vp201_v2_hash_is_independent_of_json_object_key_order() {
        let source = minimal_program();
        let mut reordered = serde_json::Map::new();
        for key in [
            "budgets",
            "outputs",
            "nodes",
            "materials",
            "parameters",
            "seed",
            "units",
            "domain",
            "program_id",
            "schema_version",
        ] {
            reordered.insert(key.into(), source[key].clone());
        }
        let left = lower_forge_visual_program_v2(&source).unwrap();
        let right = lower_forge_visual_program_v2(&Value::Object(reordered)).unwrap();
        assert_eq!(left.source_program_sha256, right.source_program_sha256);
        assert_eq!(left.shape_program, right.shape_program);
    }

    #[test]
    fn vp201_v2_hash_and_lowering_change_with_parameter_semantics() {
        let source = minimal_program();
        let mut changed = source.clone();
        changed["parameters"][0]["default"] = json!(180.0);
        let left = lower_forge_visual_program_v2(&source).unwrap();
        let right = lower_forge_visual_program_v2(&changed).unwrap();
        assert_ne!(left.source_program_sha256, right.source_program_sha256);
        assert_eq!(
            right.shape_program["operations"][0]["args"]["size"][0],
            180.0
        );
        assert_eq!(
            right.source_map.entries[0].parameter_ids,
            vec!["param_shell_length", "param_shell_rotation"]
        );
    }

    #[test]
    fn vp201_v1_is_not_silently_interpreted_as_v2() {
        let mut source = minimal_program();
        source["schema_version"] = Value::String("ForgeVisualProgram@1".into());
        let error = lower_forge_visual_program_v2(&source).unwrap_err();
        assert_eq!(error.code(), "FORGE_VISUAL_V2_SCHEMA_VERSION_INVALID");
    }

    #[test]
    fn vp201_rejects_duplicate_and_dangling_ids_before_lowering() {
        let mut duplicate = minimal_program();
        let duplicate_node = duplicate["nodes"][0].clone();
        duplicate["nodes"]
            .as_array_mut()
            .unwrap()
            .push(duplicate_node);
        let duplicate_error = lower_forge_visual_program_v2(&duplicate).unwrap_err();
        assert_eq!(duplicate_error.code(), "FORGE_VISUAL_V2_DUPLICATE_ID");

        let mut dangling = minimal_program();
        dangling["nodes"][1]["input_node_id"] = Value::String("node_missing".into());
        let dangling_error = lower_forge_visual_program_v2(&dangling).unwrap_err();
        assert_eq!(dangling_error.code(), "FORGE_VISUAL_V2_REFERENCE_MISSING");
    }

    #[test]
    fn vp201_rejects_cycles_fanout_and_orphaned_nodes() {
        let mut cycle = minimal_program();
        cycle["nodes"][2]["input_node_id"] = Value::String("node_primary_zone".into());
        let cycle_error = lower_forge_visual_program_v2(&cycle).unwrap_err();
        assert_eq!(cycle_error.code(), "FORGE_VISUAL_V2_GRAPH_CYCLE");

        let mut fanout = minimal_program();
        fanout["outputs"].as_array_mut().unwrap().push(json!({
            "output_id": "output_primary_duplicate",
            "node_id": "node_primary_zone"
        }));
        let fanout_error = lower_forge_visual_program_v2(&fanout).unwrap_err();
        assert_eq!(
            fanout_error.code(),
            "FORGE_VISUAL_V2_GRAPH_FANOUT_UNSUPPORTED"
        );

        let mut orphaned = minimal_program();
        orphaned["nodes"].as_array_mut().unwrap().push(json!({
            "kind": "box",
            "node_id": "node_orphaned_shell",
            "size": [20.0, 20.0, 20.0]
        }));
        let orphaned_error = lower_forge_visual_program_v2(&orphaned).unwrap_err();
        assert_eq!(orphaned_error.code(), "FORGE_VISUAL_V2_GRAPH_ORPHANED");
    }

    #[test]
    fn vp201_rejects_declared_budget_overrun() {
        let mut source = minimal_program();
        source["budgets"]["max_nodes"] = json!(3);
        let error = lower_forge_visual_program_v2(&source).unwrap_err();
        assert_eq!(error.code(), "FORGE_VISUAL_V2_BUDGET_EXCEEDED");

        let mut bad_budget_schema = minimal_program();
        bad_budget_schema["budgets"]["schema_version"] = Value::String("ProgramBudget@2".into());
        let schema_error = lower_forge_visual_program_v2(&bad_budget_schema).unwrap_err();
        assert_eq!(schema_error.code(), "FORGE_VISUAL_V2_BUDGET_SCHEMA_INVALID");
    }

    #[test]
    fn vp201_rejects_wrong_units_rotation_seed_and_shape_incompatible_ids() {
        let mut wrong_field_unit = minimal_program();
        wrong_field_unit["nodes"][0]["size"][0] = json!({"parameter_id": "param_shell_rotation"});
        let unit_error = lower_forge_visual_program_v2(&wrong_field_unit).unwrap_err();
        assert_eq!(unit_error.code(), "FORGE_VISUAL_V2_PARAMETER_UNIT_INVALID");

        let mut wrong_declared_unit = minimal_program();
        wrong_declared_unit["parameters"][0]["unit"] = Value::String("radian".into());
        let declared_unit_error = lower_forge_visual_program_v2(&wrong_declared_unit).unwrap_err();
        assert_eq!(
            declared_unit_error.code(),
            "FORGE_VISUAL_V2_PARAMETER_UNIT_INVALID"
        );

        let mut bad_rotation = minimal_program();
        bad_rotation["nodes"][1]["rotation"][0] = json!(4.0);
        let rotation_error = lower_forge_visual_program_v2(&bad_rotation).unwrap_err();
        assert_eq!(
            rotation_error.code(),
            "FORGE_VISUAL_V2_ROTATION_RANGE_INVALID"
        );

        let mut bad_seed = minimal_program();
        bad_seed["seed"] = json!(i32::MAX as u32 + 1);
        let seed_error = lower_forge_visual_program_v2(&bad_seed).unwrap_err();
        assert_eq!(seed_error.code(), "FORGE_VISUAL_V2_SEED_INVALID");

        let mut bad_id = minimal_program();
        bad_id["program_id"] = Value::String("visual_Bad.Id".into());
        let id_error = lower_forge_visual_program_v2(&bad_id).unwrap_err();
        assert_eq!(id_error.code(), "FORGE_VISUAL_V2_ID_INVALID");
    }

    #[test]
    fn vp201_static_triangle_upper_bound_rejects_before_worker() {
        let mut source = minimal_program();
        source["nodes"][0] = json!({
            "kind": "cylinder",
            "node_id": "node_primary_shell",
            "radius": 30.0,
            "height": {"parameter_id": "param_shell_length"}
        });
        source["budgets"]["triangle_budget"] = json!(100);
        let error = lower_forge_visual_program_v2(&source).unwrap_err();
        assert_eq!(error.code(), "FORGE_VISUAL_V2_BUDGET_EXCEEDED");
    }

    #[test]
    fn vp201_source_map_hash_and_serialization_round_trip_are_stable() {
        let lowering = lower_forge_visual_program_v2(&minimal_program()).unwrap();
        assert_eq!(
            lowering.source_map_sha256,
            semantic_sha256(&lowering.source_map).unwrap()
        );
        let serialized = serde_json::to_value(&lowering.source_map).unwrap();
        let round_trip: ForgeVisualSourceMapV2 = serde_json::from_value(serialized).unwrap();
        assert_eq!(round_trip, lowering.source_map);
    }

    #[test]
    fn vp201_rejects_unreviewed_material_capability() {
        let mut source = minimal_program();
        source["materials"][0]["base_material_id"] = Value::String("mat_provider_code".into());
        let error = lower_forge_visual_program_v2(&source).unwrap_err();
        assert_eq!(error.code(), "FORGE_VISUAL_V2_CAPABILITY_DENIED");
    }

    #[test]
    fn vp201_rejects_unknown_nodes_and_non_finite_values() {
        let mut unknown = minimal_program();
        unknown["nodes"][0]["kind"] = Value::String("script".into());
        let unknown_error = lower_forge_visual_program_v2(&unknown).unwrap_err();
        assert_eq!(unknown_error.code(), "FORGE_VISUAL_V2_PARSE_FAILED");

        let mut program = ForgeVisualProgramV2::parse_and_validate(&minimal_program()).unwrap();
        match &mut program.nodes[0] {
            ForgeVisualNodeV2::Box { size, .. } => {
                size[0] = ForgeVisualScalarV2::Literal(f64::NAN);
            }
            _ => unreachable!("fixture begins with a box"),
        }
        let non_finite_error = program.validate().unwrap_err();
        assert_eq!(non_finite_error.code(), "FORGE_VISUAL_V2_NON_FINITE");
    }
}
