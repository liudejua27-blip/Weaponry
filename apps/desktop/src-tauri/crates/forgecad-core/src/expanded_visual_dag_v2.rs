//! FGC-VP202 bounded composition for `ForgeVisualProgram@2`.
//!
//! Composition is data, not code. Rust resolves lexical bindings, rejects
//! recursive macro graphs, expands bounded calls deterministically, and then
//! delegates all geometry validation/lowering to the completed VP201 compiler.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    lower_forge_visual_program_v2, semantic_sha256, CoreError, CoreResult,
    ForgeVisualParameterKindV2, ForgeVisualParameterUnitV2, ForgeVisualParameterV2,
    ForgeVisualProgramLoweringV2, ForgeVisualProgramV2, ForgeVisualUnitSystemV2,
};

pub const FORGE_VISUAL_COMPOSITION_SCHEMA_VERSION: &str = "ForgeVisualComposition@1";
pub const EXPANDED_VISUAL_DAG_SCHEMA_VERSION: &str = "ExpandedVisualDAG@1";
pub const EXPANSION_BUDGET_SCHEMA_VERSION: &str = "ExpansionBudget@1";
pub const VP202_COMPILER_VERSION: &str = "forgecad-core-vp202.1";
pub const VP202_ID_ALGORITHM_VERSION: &str = "expanded-path-v1";

fn invalid(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message.into())
}

fn require_id(field: &str, value: &str, prefix: &str) -> CoreResult<()> {
    if value.starts_with(prefix)
        && value.len() > prefix.len()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
    {
        Ok(())
    } else {
        Err(invalid(
            "FORGE_VISUAL_VP202_ID_INVALID",
            format!("{field} must be a bounded lowercase {prefix} ID"),
        ))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum ScopedScalarV2 {
    Literal(f64),
    Global { parameter_id: String },
    Local { local_parameter_id: String },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum ScopedCountV2 {
    Literal(u16),
    Global { parameter_id: String },
    Local { local_parameter_id: String },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MacroParameterV2 {
    pub local_parameter_id: String,
    pub kind: ForgeVisualParameterKindV2,
    pub unit: ForgeVisualParameterUnitV2,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MacroBindingV2 {
    pub local_parameter_id: String,
    pub value: ScopedScalarV2,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BoundedRepeatV2 {
    pub count: ScopedCountV2,
    pub step: [ScopedScalarV2; 3],
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "primitive_kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MacroPrimitiveV2 {
    Box {
        size: [ScopedScalarV2; 3],
    },
    Cylinder {
        radius: ScopedScalarV2,
        height: ScopedScalarV2,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MacroItemV2 {
    Chain {
        chain_id: String,
        primitive: MacroPrimitiveV2,
        position: [ScopedScalarV2; 3],
        rotation: [ScopedScalarV2; 3],
        role: String,
        material_id: String,
    },
    Invoke {
        call_id: String,
        macro_id: String,
        #[serde(default)]
        bindings: Vec<MacroBindingV2>,
        repeat: BoundedRepeatV2,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VisualMacroV2 {
    pub macro_id: String,
    #[serde(default)]
    pub parameters: Vec<MacroParameterV2>,
    pub items: Vec<MacroItemV2>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RootMacroCallV2 {
    pub call_id: String,
    pub macro_id: String,
    #[serde(default)]
    pub bindings: Vec<MacroBindingV2>,
    pub repeat: BoundedRepeatV2,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExpansionBudgetV2 {
    pub schema_version: String,
    pub max_macros: u16,
    pub max_macro_calls: u16,
    pub max_expansion_depth: u8,
    pub max_expanded_nodes: u16,
    pub max_parts: u16,
    pub max_materials: u16,
    pub max_outputs: u16,
    pub max_primitives: u16,
    pub triangle_budget: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualCompositionV2 {
    pub schema_version: String,
    pub program_id: String,
    pub domain: String,
    pub units: ForgeVisualUnitSystemV2,
    pub seed: u32,
    #[serde(default)]
    pub parameters: Vec<ForgeVisualParameterV2>,
    pub materials: Vec<crate::ForgeVisualMaterialV2>,
    pub macros: Vec<VisualMacroV2>,
    pub calls: Vec<RootMacroCallV2>,
    pub budgets: ExpansionBudgetV2,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExpandedVisualLineageEntryV2 {
    pub expanded_output_id: String,
    pub expanded_node_ids: Vec<String>,
    pub expanded_part_id: String,
    pub expanded_material_zone_id: String,
    pub source_macro_path: Vec<String>,
    pub source_chain_id: String,
    pub instance_indices: Vec<u16>,
    pub parameter_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExpandedVisualBudgetEvidenceV2 {
    pub macro_count: u16,
    pub macro_call_count: u16,
    pub expanded_node_count: u16,
    pub expanded_output_count: u16,
    pub primitive_count: u16,
    pub estimated_triangle_upper_bound: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExpandedVisualDagV2 {
    pub schema_version: String,
    pub compiler_version: String,
    pub id_algorithm_version: String,
    pub source_program_sha256: String,
    pub expanded_program_sha256: String,
    pub lineage_sha256: String,
    pub expanded_dag_sha256: String,
    pub budget_evidence: ExpandedVisualBudgetEvidenceV2,
    pub lineage: Vec<ExpandedVisualLineageEntryV2>,
    pub expanded_program: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExpandedVisualProgramLoweringV2 {
    pub expanded_dag: ExpandedVisualDagV2,
    pub lowering: ForgeVisualProgramLoweringV2,
}

#[derive(Clone)]
struct ResolvedScalar {
    value: f64,
    parameter_ids: BTreeSet<String>,
    kind: ForgeVisualParameterKindV2,
}

struct ExpansionState {
    nodes: Vec<Value>,
    outputs: Vec<Value>,
    lineage: Vec<ExpandedVisualLineageEntryV2>,
    macro_call_count: u16,
    estimated_triangles: u32,
    identities: BTreeSet<String>,
}

#[derive(Default)]
struct PreflightCounts {
    macro_calls: u32,
    chains: u32,
    estimated_triangles: u32,
    reached_macros: BTreeSet<String>,
}

fn expected_unit(kind: ForgeVisualParameterKindV2) -> ForgeVisualParameterUnitV2 {
    match kind {
        ForgeVisualParameterKindV2::Number => ForgeVisualParameterUnitV2::Unitless,
        ForgeVisualParameterKindV2::Integer => ForgeVisualParameterUnitV2::Count,
        ForgeVisualParameterKindV2::Boolean => ForgeVisualParameterUnitV2::Boolean,
        ForgeVisualParameterKindV2::Enum => ForgeVisualParameterUnitV2::EnumValue,
        ForgeVisualParameterKindV2::Length => ForgeVisualParameterUnitV2::Millimeter,
        ForgeVisualParameterKindV2::Angle => ForgeVisualParameterUnitV2::Radian,
        ForgeVisualParameterKindV2::Ratio => ForgeVisualParameterUnitV2::Ratio,
        ForgeVisualParameterKindV2::Color => ForgeVisualParameterUnitV2::LinearRgb,
    }
}

fn resolve_scalar(
    scalar: &ScopedScalarV2,
    expected: ForgeVisualParameterKindV2,
    globals: &BTreeMap<&str, &ForgeVisualParameterV2>,
    locals: &BTreeMap<String, ResolvedScalar>,
) -> CoreResult<ResolvedScalar> {
    let resolved = match scalar {
        ScopedScalarV2::Literal(value) => ResolvedScalar {
            value: *value,
            parameter_ids: BTreeSet::new(),
            kind: expected,
        },
        ScopedScalarV2::Global { parameter_id } => {
            let parameter = globals.get(parameter_id.as_str()).ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP202_BINDING_MISSING",
                    "unknown global parameter",
                )
            })?;
            let value = parameter.default.as_f64().ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP202_BINDING_TYPE",
                    "binding must resolve to a numeric parameter",
                )
            })?;
            ResolvedScalar {
                value,
                parameter_ids: BTreeSet::from([parameter_id.clone()]),
                kind: parameter.kind,
            }
        }
        ScopedScalarV2::Local { local_parameter_id } => {
            locals.get(local_parameter_id).cloned().ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP202_SCOPE_CAPTURE",
                    "local parameter is not bound in the lexical parent scope",
                )
            })?
        }
    };
    if !resolved.value.is_finite() {
        return Err(invalid(
            "FORGE_VISUAL_VP202_NON_FINITE",
            "expanded scalar must be finite",
        ));
    }
    if resolved.kind != expected {
        return Err(invalid(
            "FORGE_VISUAL_VP202_BINDING_TYPE",
            "binding kind does not match the lexical parameter or field",
        ));
    }
    Ok(resolved)
}

fn resolve_count(
    count: &ScopedCountV2,
    globals: &BTreeMap<&str, &ForgeVisualParameterV2>,
    locals: &BTreeMap<String, ResolvedScalar>,
) -> CoreResult<u16> {
    let value = match count {
        ScopedCountV2::Literal(value) => u64::from(*value),
        ScopedCountV2::Global { parameter_id } => {
            let parameter = globals.get(parameter_id.as_str()).ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP202_BINDING_MISSING",
                    "unknown repeat parameter",
                )
            })?;
            if parameter.kind != ForgeVisualParameterKindV2::Integer {
                return Err(invalid(
                    "FORGE_VISUAL_VP202_REPEAT_COUNT",
                    "repeat count must use an integer parameter",
                ));
            }
            parameter.default.as_u64().ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP202_REPEAT_COUNT",
                    "repeat count must be a non-negative integer",
                )
            })?
        }
        ScopedCountV2::Local { local_parameter_id } => {
            let local = locals.get(local_parameter_id).ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP202_SCOPE_CAPTURE",
                    "repeat count local is outside lexical scope",
                )
            })?;
            if local.kind != ForgeVisualParameterKindV2::Integer
                || local.value.fract() != 0.0
                || local.value < 0.0
            {
                return Err(invalid(
                    "FORGE_VISUAL_VP202_REPEAT_COUNT",
                    "repeat count local must resolve to a non-negative integer",
                ));
            }
            local.value as u64
        }
    };
    if !(1..=64).contains(&value) {
        return Err(invalid(
            "FORGE_VISUAL_VP202_REPEAT_COUNT",
            "repeat count must be within 1..=64",
        ));
    }
    Ok(value as u16)
}

fn bind_call(
    definition: &VisualMacroV2,
    bindings: &[MacroBindingV2],
    globals: &BTreeMap<&str, &ForgeVisualParameterV2>,
    parent: &BTreeMap<String, ResolvedScalar>,
) -> CoreResult<BTreeMap<String, ResolvedScalar>> {
    let declarations = definition
        .parameters
        .iter()
        .map(|item| (item.local_parameter_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    if declarations.len() != definition.parameters.len() || bindings.len() != declarations.len() {
        return Err(invalid(
            "FORGE_VISUAL_VP202_BINDING_MISSING",
            "every macro local must have exactly one binding",
        ));
    }
    let mut scope = BTreeMap::new();
    for binding in bindings {
        let declaration = declarations
            .get(binding.local_parameter_id.as_str())
            .ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP202_SCOPE_CAPTURE",
                    "binding targets an undeclared macro local",
                )
            })?;
        if scope.contains_key(&binding.local_parameter_id) {
            return Err(invalid(
                "FORGE_VISUAL_VP202_DUPLICATE_ID",
                "macro local bindings must be unique",
            ));
        }
        scope.insert(
            binding.local_parameter_id.clone(),
            resolve_scalar(&binding.value, declaration.kind, globals, parent)?,
        );
    }
    Ok(scope)
}

fn suffix<'a>(value: &'a str, prefix: &str) -> &'a str {
    value.strip_prefix(prefix).unwrap_or(value)
}

fn validate_macro_graph<'a>(
    macro_id: &'a str,
    macros: &BTreeMap<&'a str, &'a VisualMacroV2>,
    stack: &mut Vec<&'a str>,
    max_depth: u8,
) -> CoreResult<()> {
    if stack.contains(&macro_id) {
        return Err(invalid(
            "FORGE_VISUAL_VP202_RECURSION",
            "recursive and mutually recursive macro calls are forbidden",
        ));
    }
    if stack.len() >= max_depth as usize {
        return Err(invalid(
            "FORGE_VISUAL_VP202_DEPTH_EXCEEDED",
            "declared macro graph exceeds max_expansion_depth",
        ));
    }
    let definition = macros.get(macro_id).copied().ok_or_else(|| {
        invalid(
            "FORGE_VISUAL_VP202_REFERENCE_MISSING",
            "macro graph references an unknown macro",
        )
    })?;
    stack.push(macro_id);
    for item in &definition.items {
        if let MacroItemV2::Invoke { macro_id, .. } = item {
            validate_macro_graph(macro_id, macros, stack, max_depth)?;
        }
    }
    stack.pop();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn preflight_call(
    macro_id: &str,
    bindings: &[MacroBindingV2],
    repeat: &BoundedRepeatV2,
    parent_scope: &BTreeMap<String, ResolvedScalar>,
    macros: &BTreeMap<&str, &VisualMacroV2>,
    globals: &BTreeMap<&str, &ForgeVisualParameterV2>,
    budgets: &ExpansionBudgetV2,
    stack: &mut Vec<String>,
    counts: &mut PreflightCounts,
) -> CoreResult<()> {
    if stack.iter().any(|active| active == macro_id) {
        return Err(invalid(
            "FORGE_VISUAL_VP202_RECURSION",
            "recursive macro reached during preflight",
        ));
    }
    let definition = macros.get(macro_id).copied().ok_or_else(|| {
        invalid(
            "FORGE_VISUAL_VP202_REFERENCE_MISSING",
            "preflight call references an unknown macro",
        )
    })?;
    let scope = bind_call(definition, bindings, globals, parent_scope)?;
    let count = resolve_count(&repeat.count, globals, &scope)?;
    let mut nonzero_step = false;
    for value in &repeat.step {
        let resolved = resolve_scalar(value, ForgeVisualParameterKindV2::Length, globals, &scope)?;
        nonzero_step |= resolved.value != 0.0;
    }
    if count > 1 && !nonzero_step {
        return Err(invalid(
            "FORGE_VISUAL_VP202_REPEAT_LAYOUT",
            "multi-instance repeat requires a non-zero layout step",
        ));
    }
    counts.reached_macros.insert(macro_id.to_string());
    counts.macro_calls = counts
        .macro_calls
        .checked_add(u32::from(count))
        .ok_or_else(|| {
            invalid(
                "FORGE_VISUAL_VP202_ARITHMETIC_OVERFLOW",
                "macro call upper bound overflowed",
            )
        })?;
    stack.push(macro_id.to_string());
    for _ in 0..count {
        for item in &definition.items {
            match item {
                MacroItemV2::Chain { primitive, .. } => {
                    counts.chains = counts.chains.checked_add(1).ok_or_else(|| {
                        invalid(
                            "FORGE_VISUAL_VP202_ARITHMETIC_OVERFLOW",
                            "expanded chain upper bound overflowed",
                        )
                    })?;
                    let triangles = match primitive {
                        MacroPrimitiveV2::Box { .. } => 12,
                        MacroPrimitiveV2::Cylinder { .. } => 256,
                    };
                    counts.estimated_triangles = counts
                        .estimated_triangles
                        .checked_add(triangles)
                        .ok_or_else(|| {
                            invalid(
                                "FORGE_VISUAL_VP202_ARITHMETIC_OVERFLOW",
                                "triangle upper bound overflowed",
                            )
                        })?;
                }
                MacroItemV2::Invoke {
                    macro_id,
                    bindings,
                    repeat,
                    ..
                } => preflight_call(
                    macro_id, bindings, repeat, &scope, macros, globals, budgets, stack, counts,
                )?,
            }
        }
    }
    stack.pop();
    let expanded_nodes = counts.chains.checked_mul(4).ok_or_else(|| {
        invalid(
            "FORGE_VISUAL_VP202_ARITHMETIC_OVERFLOW",
            "expanded node upper bound overflowed",
        )
    })?;
    if counts.macro_calls > u32::from(budgets.max_macro_calls)
        || expanded_nodes > u32::from(budgets.max_expanded_nodes)
        || counts.chains > u32::from(budgets.max_parts)
        || counts.chains > u32::from(budgets.max_outputs)
        || counts.chains > u32::from(budgets.max_primitives)
        || counts.estimated_triangles > budgets.triangle_budget
    {
        return Err(invalid(
            "FORGE_VISUAL_VP202_BUDGET_EXCEEDED",
            "static expansion upper bound exceeds a declared budget",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn expand_call(
    call_id: &str,
    macro_id: &str,
    bindings: &[MacroBindingV2],
    repeat: &BoundedRepeatV2,
    parent_scope: &BTreeMap<String, ResolvedScalar>,
    macros: &BTreeMap<&str, &VisualMacroV2>,
    globals: &BTreeMap<&str, &ForgeVisualParameterV2>,
    budgets: &ExpansionBudgetV2,
    stack: &mut Vec<String>,
    path: &[String],
    indices: &[u16],
    inherited_offset: [f64; 3],
    state: &mut ExpansionState,
) -> CoreResult<()> {
    require_id("call_id", call_id, "call_")?;
    require_id("macro_id", macro_id, "macro_")?;
    if stack.iter().any(|active| active == macro_id) {
        return Err(invalid(
            "FORGE_VISUAL_VP202_RECURSION",
            "recursive and mutually recursive macro calls are forbidden",
        ));
    }
    if stack.len() >= budgets.max_expansion_depth as usize {
        return Err(invalid(
            "FORGE_VISUAL_VP202_DEPTH_EXCEEDED",
            "macro expansion depth exceeds the declared budget",
        ));
    }
    let definition = macros.get(macro_id).copied().ok_or_else(|| {
        invalid(
            "FORGE_VISUAL_VP202_REFERENCE_MISSING",
            "macro call references an unknown macro",
        )
    })?;
    let scope = bind_call(definition, bindings, globals, parent_scope)?;
    let count = resolve_count(&repeat.count, globals, &scope)?;
    state.macro_call_count = state.macro_call_count.checked_add(count).ok_or_else(|| {
        invalid(
            "FORGE_VISUAL_VP202_BUDGET_EXCEEDED",
            "expanded macro call count overflow",
        )
    })?;
    if state.macro_call_count > budgets.max_macro_calls {
        return Err(invalid(
            "FORGE_VISUAL_VP202_BUDGET_EXCEEDED",
            "expanded macro call count exceeds the declared budget",
        ));
    }
    let mut step = [0.0; 3];
    for (axis, value) in repeat.step.iter().enumerate() {
        step[axis] =
            resolve_scalar(value, ForgeVisualParameterKindV2::Length, globals, &scope)?.value;
    }
    stack.push(macro_id.to_string());
    for index in 0..count {
        let mut next_path = path.to_vec();
        next_path.push(call_id.to_string());
        let mut next_indices = indices.to_vec();
        next_indices.push(index);
        let offset = [
            inherited_offset[0] + step[0] * f64::from(index),
            inherited_offset[1] + step[1] * f64::from(index),
            inherited_offset[2] + step[2] * f64::from(index),
        ];
        for item in &definition.items {
            match item {
                MacroItemV2::Invoke {
                    call_id,
                    macro_id,
                    bindings,
                    repeat,
                } => expand_call(
                    call_id,
                    macro_id,
                    bindings,
                    repeat,
                    &scope,
                    macros,
                    globals,
                    budgets,
                    stack,
                    &next_path,
                    &next_indices,
                    offset,
                    state,
                )?,
                MacroItemV2::Chain {
                    chain_id,
                    primitive,
                    position,
                    rotation,
                    role,
                    material_id,
                } => {
                    require_id("chain_id", chain_id, "chain_")?;
                    require_id("material_id", material_id, "mat_")?;
                    let instance_path = next_path
                        .iter()
                        .zip(next_indices.iter())
                        .map(|(id, instance)| format!("{}_r{instance}", suffix(id, "call_")))
                        .collect::<Vec<_>>()
                        .join("_");
                    let identity = format!("{}_{}", instance_path, suffix(chain_id, "chain_"));
                    if identity.len() > 72 {
                        return Err(invalid(
                            "FORGE_VISUAL_VP202_ID_INVALID",
                            "expanded identity exceeds the stable ID budget",
                        ));
                    }
                    if !state.identities.insert(identity.clone()) {
                        return Err(invalid(
                            "FORGE_VISUAL_VP202_DUPLICATE_ID",
                            "two macro expansions produced the same stable identity",
                        ));
                    }
                    let mut parameter_ids = BTreeSet::new();
                    let mut resolved_position = [0.0; 3];
                    let mut resolved_rotation = [0.0; 3];
                    for axis in 0..3 {
                        let position_value = resolve_scalar(
                            &position[axis],
                            ForgeVisualParameterKindV2::Length,
                            globals,
                            &scope,
                        )?;
                        let rotation_value = resolve_scalar(
                            &rotation[axis],
                            ForgeVisualParameterKindV2::Angle,
                            globals,
                            &scope,
                        )?;
                        resolved_position[axis] = position_value.value + offset[axis];
                        resolved_rotation[axis] = rotation_value.value;
                        parameter_ids.extend(position_value.parameter_ids);
                        parameter_ids.extend(rotation_value.parameter_ids);
                    }
                    let primitive_id = format!("node_{identity}_primitive");
                    let transform_id = format!("node_{identity}_transform");
                    let part_node_id = format!("node_{identity}_part");
                    let zone_node_id = format!("node_{identity}_zone");
                    match primitive {
                        MacroPrimitiveV2::Box { size } => {
                            let mut values = [0.0; 3];
                            for axis in 0..3 {
                                let resolved = resolve_scalar(
                                    &size[axis],
                                    ForgeVisualParameterKindV2::Length,
                                    globals,
                                    &scope,
                                )?;
                                values[axis] = resolved.value;
                                parameter_ids.extend(resolved.parameter_ids);
                            }
                            state.nodes.push(
                                json!({"kind":"box","node_id":primitive_id.clone(),"size":values}),
                            );
                            state.estimated_triangles =
                                state.estimated_triangles.checked_add(12).ok_or_else(|| {
                                    invalid(
                                        "FORGE_VISUAL_VP202_BUDGET_EXCEEDED",
                                        "triangle estimate overflow",
                                    )
                                })?;
                        }
                        MacroPrimitiveV2::Cylinder { radius, height } => {
                            let radius = resolve_scalar(
                                radius,
                                ForgeVisualParameterKindV2::Length,
                                globals,
                                &scope,
                            )?;
                            let height = resolve_scalar(
                                height,
                                ForgeVisualParameterKindV2::Length,
                                globals,
                                &scope,
                            )?;
                            parameter_ids.extend(radius.parameter_ids);
                            parameter_ids.extend(height.parameter_ids);
                            state.nodes.push(json!({"kind":"cylinder","node_id":primitive_id.clone(),"radius":radius.value,"height":height.value}));
                            state.estimated_triangles =
                                state.estimated_triangles.checked_add(256).ok_or_else(|| {
                                    invalid(
                                        "FORGE_VISUAL_VP202_BUDGET_EXCEEDED",
                                        "triangle estimate overflow",
                                    )
                                })?;
                        }
                    }
                    state.nodes.push(json!({"kind":"transform","node_id":transform_id.clone(),"input_node_id":primitive_id.clone(),"position":resolved_position,"rotation":resolved_rotation}));
                    state.nodes.push(json!({"kind":"part","node_id":part_node_id.clone(),"input_node_id":transform_id.clone(),"part_id":format!("part_{identity}"),"role":role}));
                    state.nodes.push(json!({"kind":"material_zone","node_id":zone_node_id.clone(),"input_node_id":part_node_id.clone(),"zone_id":format!("zone_{identity}"),"material_id":material_id}));
                    let output_id = format!("output_{identity}");
                    state.outputs.push(
                        json!({"output_id":output_id.clone(),"node_id":zone_node_id.clone()}),
                    );
                    state.lineage.push(ExpandedVisualLineageEntryV2 {
                        expanded_output_id: output_id,
                        expanded_node_ids: vec![
                            primitive_id,
                            transform_id,
                            part_node_id,
                            zone_node_id,
                        ],
                        expanded_part_id: format!("part_{identity}"),
                        expanded_material_zone_id: format!("zone_{identity}"),
                        source_macro_path: stack.clone(),
                        source_chain_id: chain_id.clone(),
                        instance_indices: next_indices.clone(),
                        parameter_ids: parameter_ids.into_iter().collect(),
                    });
                    if state.nodes.len() > budgets.max_expanded_nodes as usize
                        || state.outputs.len() > budgets.max_outputs as usize
                        || state.outputs.len() > budgets.max_parts as usize
                        || state.outputs.len() > budgets.max_primitives as usize
                        || state.estimated_triangles > budgets.triangle_budget
                    {
                        return Err(invalid(
                            "FORGE_VISUAL_VP202_BUDGET_EXCEEDED",
                            "expanded DAG exceeds declared node/output/primitive/triangle budget",
                        ));
                    }
                }
            }
        }
    }
    stack.pop();
    Ok(())
}

pub fn expand_forge_visual_composition_v2(value: &Value) -> CoreResult<ExpandedVisualDagV2> {
    let source: ForgeVisualCompositionV2 =
        serde_json::from_value(value.clone()).map_err(|error| {
            invalid(
                "FORGE_VISUAL_VP202_PARSE_FAILED",
                format!("ForgeVisualComposition@1 failed closed: {error}"),
            )
        })?;
    if source.schema_version != FORGE_VISUAL_COMPOSITION_SCHEMA_VERSION
        || source.budgets.schema_version != EXPANSION_BUDGET_SCHEMA_VERSION
    {
        return Err(invalid(
            "FORGE_VISUAL_VP202_SCHEMA_VERSION",
            "composition and budget schema versions must match VP202",
        ));
    }
    require_id("program_id", &source.program_id, "visual_")?;
    if source.seed > i32::MAX as u32
        || source.macros.is_empty()
        || source.calls.is_empty()
        || source.materials.is_empty()
    {
        return Err(invalid("FORGE_VISUAL_VP202_SOURCE_INVALID", "composition requires bounded macros, calls, materials and a ShapeProgram-compatible seed"));
    }
    if source.budgets.max_macros == 0
        || source.budgets.max_macros > 64
        || source.budgets.max_macro_calls == 0
        || source.budgets.max_macro_calls > 256
        || source.budgets.max_expansion_depth == 0
        || source.budgets.max_expansion_depth > 8
        || source.budgets.max_expanded_nodes == 0
        || source.budgets.max_expanded_nodes > 256
        || source.budgets.max_outputs == 0
        || source.budgets.max_outputs > 128
        || source.budgets.max_parts == 0
        || source.budgets.max_parts > 128
        || source.budgets.max_primitives == 0
        || source.budgets.max_primitives > 256
        || source.budgets.max_materials == 0
        || source.budgets.max_materials > 64
        || !(100..=100_000).contains(&source.budgets.triangle_budget)
    {
        return Err(invalid(
            "FORGE_VISUAL_VP202_BUDGET_INVALID",
            "expansion budget exceeds VP202 ceilings",
        ));
    }
    if source.macros.len() > source.budgets.max_macros as usize
        || source.materials.len() > source.budgets.max_materials as usize
        || source.parameters.len() > 64
    {
        return Err(invalid(
            "FORGE_VISUAL_VP202_BUDGET_EXCEEDED",
            "source declarations exceed expansion budget",
        ));
    }
    let globals = source
        .parameters
        .iter()
        .map(|item| (item.parameter_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    if globals.len() != source.parameters.len() {
        return Err(invalid(
            "FORGE_VISUAL_VP202_DUPLICATE_ID",
            "global parameter IDs must be unique",
        ));
    }
    let mut macro_ids = BTreeSet::new();
    for definition in &source.macros {
        require_id("macro_id", &definition.macro_id, "macro_")?;
        if !macro_ids.insert(definition.macro_id.as_str()) || definition.items.is_empty() {
            return Err(invalid(
                "FORGE_VISUAL_VP202_DUPLICATE_ID",
                "macro IDs must be unique and bodies non-empty",
            ));
        }
        let mut local_ids = BTreeSet::new();
        for parameter in &definition.parameters {
            require_id(
                "local_parameter_id",
                &parameter.local_parameter_id,
                "local_",
            )?;
            if parameter.unit != expected_unit(parameter.kind)
                || !local_ids.insert(parameter.local_parameter_id.as_str())
            {
                return Err(invalid(
                    "FORGE_VISUAL_VP202_BINDING_TYPE",
                    "macro locals require unique IDs and matching units",
                ));
            }
            if matches!(
                parameter.kind,
                ForgeVisualParameterKindV2::Boolean
                    | ForgeVisualParameterKindV2::Enum
                    | ForgeVisualParameterKindV2::Color
            ) {
                return Err(invalid(
                    "FORGE_VISUAL_VP202_CAPABILITY_DENIED",
                    "VP202 macro locals allow only numeric typed values",
                ));
            }
        }
    }
    let macros = source
        .macros
        .iter()
        .map(|item| (item.macro_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    for definition in &source.macros {
        let mut item_ids = BTreeSet::new();
        for item in &definition.items {
            let item_id = match item {
                MacroItemV2::Chain { chain_id, .. } => chain_id,
                MacroItemV2::Invoke { call_id, .. } => call_id,
            };
            if !item_ids.insert(item_id.as_str()) {
                return Err(invalid(
                    "FORGE_VISUAL_VP202_DUPLICATE_ID",
                    "macro item IDs must be unique within lexical scope",
                ));
            }
        }
        validate_macro_graph(
            &definition.macro_id,
            &macros,
            &mut Vec::new(),
            source.budgets.max_expansion_depth,
        )?;
    }
    let source_program_sha256 = semantic_sha256(&source)?;
    let mut preflight = PreflightCounts::default();
    for call in &source.calls {
        preflight_call(
            &call.macro_id,
            &call.bindings,
            &call.repeat,
            &BTreeMap::new(),
            &macros,
            &globals,
            &source.budgets,
            &mut Vec::new(),
            &mut preflight,
        )?;
    }
    if preflight.reached_macros.len() != source.macros.len() {
        return Err(invalid(
            "FORGE_VISUAL_VP202_MACRO_ORPHANED",
            "every declared macro must be reachable from a root call",
        ));
    }
    let capacity = preflight.chains as usize;
    let mut state = ExpansionState {
        nodes: Vec::with_capacity(capacity * 4),
        outputs: Vec::with_capacity(capacity),
        lineage: Vec::with_capacity(capacity),
        macro_call_count: 0,
        estimated_triangles: 0,
        identities: BTreeSet::new(),
    };
    let mut root_call_ids = BTreeSet::new();
    for call in &source.calls {
        if !root_call_ids.insert(call.call_id.as_str()) {
            return Err(invalid(
                "FORGE_VISUAL_VP202_DUPLICATE_ID",
                "root call IDs must be unique",
            ));
        }
        expand_call(
            &call.call_id,
            &call.macro_id,
            &call.bindings,
            &call.repeat,
            &BTreeMap::new(),
            &macros,
            &globals,
            &source.budgets,
            &mut Vec::new(),
            &[],
            &[],
            [0.0; 3],
            &mut state,
        )?;
    }
    state
        .lineage
        .sort_by(|left, right| left.expanded_output_id.cmp(&right.expanded_output_id));
    let expanded_program = json!({
        "schema_version":"ForgeVisualProgram@2",
        "program_id":source.program_id,
        "domain":source.domain,
        "units":source.units,
        "seed":source.seed,
        "parameters":source.parameters,
        "materials":source.materials,
        "nodes":state.nodes,
        "outputs":state.outputs,
        "budgets":{
            "schema_version":"ProgramBudget@1",
            "max_nodes":source.budgets.max_expanded_nodes,
            "max_parts":source.budgets.max_parts,
            "max_materials":source.budgets.max_materials,
            "max_outputs":source.budgets.max_outputs,
            "max_primitives":source.budgets.max_primitives,
            "triangle_budget":source.budgets.triangle_budget
        }
    });
    let expanded_program_sha256 = semantic_sha256(&expanded_program)?;
    ForgeVisualProgramV2::parse_and_validate(&expanded_program)?;
    let lineage_sha256 = semantic_sha256(&state.lineage)?;
    let budget_evidence = ExpandedVisualBudgetEvidenceV2 {
        macro_count: source.macros.len() as u16,
        macro_call_count: state.macro_call_count,
        expanded_node_count: expanded_program["nodes"]
            .as_array()
            .map_or(0, |items| items.len()) as u16,
        expanded_output_count: expanded_program["outputs"]
            .as_array()
            .map_or(0, |items| items.len()) as u16,
        primitive_count: expanded_program["outputs"]
            .as_array()
            .map_or(0, |items| items.len()) as u16,
        estimated_triangle_upper_bound: state.estimated_triangles,
    };
    let expanded_dag_sha256 = semantic_sha256(&json!({
        "schema_version": EXPANDED_VISUAL_DAG_SCHEMA_VERSION,
        "compiler_version": VP202_COMPILER_VERSION,
        "id_algorithm_version": VP202_ID_ALGORITHM_VERSION,
        "source_program_sha256": source_program_sha256.clone(),
        "expanded_program_sha256": expanded_program_sha256.clone(),
        "lineage_sha256": lineage_sha256.clone(),
        "budget_evidence": budget_evidence.clone(),
    }))?;
    Ok(ExpandedVisualDagV2 {
        schema_version: EXPANDED_VISUAL_DAG_SCHEMA_VERSION.into(),
        compiler_version: VP202_COMPILER_VERSION.into(),
        id_algorithm_version: VP202_ID_ALGORITHM_VERSION.into(),
        source_program_sha256,
        expanded_program_sha256,
        lineage_sha256,
        expanded_dag_sha256,
        budget_evidence,
        lineage: state.lineage,
        expanded_program,
    })
}

pub fn expand_and_lower_forge_visual_composition_v2(
    value: &Value,
) -> CoreResult<ExpandedVisualProgramLoweringV2> {
    let expanded_dag = expand_forge_visual_composition_v2(value)?;
    let lowering = lower_forge_visual_program_v2(&expanded_dag.expanded_program)?;
    if lowering.source_program_sha256 != expanded_dag.expanded_program_sha256 {
        return Err(invalid(
            "FORGE_VISUAL_VP202_HASH_MISMATCH",
            "VP201 lowering did not preserve the expanded program hash",
        ));
    }
    Ok(ExpandedVisualProgramLoweringV2 {
        expanded_dag,
        lowering,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        serde_json::from_str(include_str!("../../../../../../packages/concept-spec/fixtures/forge-visual-composition-v2-repeat.json")).unwrap()
    }

    #[test]
    fn vp202_bounded_repeat_expands_and_lowers_with_lineage() {
        let result = expand_and_lower_forge_visual_composition_v2(&fixture()).unwrap();
        assert_eq!(result.expanded_dag.budget_evidence.expanded_output_count, 3);
        assert_eq!(result.expanded_dag.budget_evidence.expanded_node_count, 12);
        assert_eq!(
            result.lowering.shape_program["operations"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(result.expanded_dag.lineage[2].instance_indices, vec![2]);
        assert!(result.expanded_dag.lineage[0]
            .parameter_ids
            .contains(&"param_panel_width".into()));
    }

    #[test]
    fn vp202_expansion_hash_is_key_order_stable_and_semantic() {
        let source = fixture();
        let mut reordered = serde_json::Map::new();
        for key in [
            "budgets",
            "calls",
            "macros",
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
        let left = expand_forge_visual_composition_v2(&source).unwrap();
        let right = expand_forge_visual_composition_v2(&Value::Object(reordered)).unwrap();
        assert_eq!(left.source_program_sha256, right.source_program_sha256);
        assert_eq!(left.expanded_program_sha256, right.expanded_program_sha256);
        let mut changed = source.clone();
        changed["parameters"][0]["default"] = json!(72.0);
        assert_ne!(
            left.expanded_program_sha256,
            expand_forge_visual_composition_v2(&changed)
                .unwrap()
                .expanded_program_sha256
        );
    }

    #[test]
    fn vp202_rejects_direct_and_mutual_recursion() {
        let mut direct = fixture();
        direct["macros"][0]["items"].as_array_mut().unwrap().push(json!({"kind":"invoke","call_id":"call_self","macro_id":"macro_panel","bindings":[{"local_parameter_id":"local_width","value":{"local_parameter_id":"local_width"}}],"repeat":{"count":1,"step":[0.0,0.0,0.0]}}));
        assert_eq!(
            expand_forge_visual_composition_v2(&direct)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP202_RECURSION"
        );

        let mut mutual = fixture();
        mutual["macros"].as_array_mut().unwrap().push(json!({"macro_id":"macro_child","parameters":[],"items":[{"kind":"invoke","call_id":"call_parent","macro_id":"macro_panel","bindings":[{"local_parameter_id":"local_width","value":40.0}],"repeat":{"count":1,"step":[0.0,0.0,0.0]}}]}));
        mutual["macros"][0]["items"].as_array_mut().unwrap().push(json!({"kind":"invoke","call_id":"call_child","macro_id":"macro_child","bindings":[],"repeat":{"count":1,"step":[0.0,0.0,0.0]}}));
        assert_eq!(
            expand_forge_visual_composition_v2(&mutual)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP202_RECURSION"
        );
    }

    #[test]
    fn vp202_rejects_scope_capture_and_missing_binding() {
        let mut capture = fixture();
        capture["calls"][0]["bindings"][0]["value"] = json!({"local_parameter_id":"local_outside"});
        assert_eq!(
            expand_forge_visual_composition_v2(&capture)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP202_SCOPE_CAPTURE"
        );
        let mut missing = fixture();
        missing["calls"][0]["bindings"] = json!([]);
        assert_eq!(
            expand_forge_visual_composition_v2(&missing)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP202_BINDING_MISSING"
        );
    }

    #[test]
    fn vp202_rejects_non_integer_or_unbounded_repeat_before_worker() {
        let mut wrong_kind = fixture();
        wrong_kind["calls"][0]["repeat"]["count"] = json!({"parameter_id":"param_panel_width"});
        assert_eq!(
            expand_forge_visual_composition_v2(&wrong_kind)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP202_REPEAT_COUNT"
        );
        let mut unbounded = fixture();
        unbounded["calls"][0]["repeat"]["count"] = json!(65);
        assert_eq!(
            expand_forge_visual_composition_v2(&unbounded)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP202_REPEAT_COUNT"
        );
    }

    #[test]
    fn vp202_rejects_expanded_node_and_triangle_budget_before_worker() {
        let mut nodes = fixture();
        nodes["budgets"]["max_expanded_nodes"] = json!(8);
        assert_eq!(
            expand_forge_visual_composition_v2(&nodes)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP202_BUDGET_EXCEEDED"
        );
        let mut triangles = fixture();
        triangles["budgets"]["triangle_budget"] = json!(100);
        triangles["macros"][0]["items"][0]["primitive"] =
            json!({"primitive_kind":"cylinder","radius":20.0,"height":40.0});
        assert_eq!(
            expand_forge_visual_composition_v2(&triangles)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP202_BUDGET_EXCEEDED"
        );
    }

    #[test]
    fn vp202_rejects_unknown_schema_and_macro_reference() {
        let mut schema = fixture();
        schema["schema_version"] = json!("ForgeVisualProgram@2");
        assert_eq!(
            expand_forge_visual_composition_v2(&schema)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP202_SCHEMA_VERSION"
        );
        let mut missing = fixture();
        missing["calls"][0]["macro_id"] = json!("macro_missing");
        assert_eq!(
            expand_forge_visual_composition_v2(&missing)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP202_REFERENCE_MISSING"
        );
    }

    #[test]
    fn vp202_rejects_orphan_macro_and_overlapping_repeat() {
        let mut orphan = fixture();
        orphan["macros"].as_array_mut().unwrap().push(json!({
            "macro_id":"macro_unused","parameters":[],
            "items":[{"kind":"chain","chain_id":"chain_unused","primitive":{"primitive_kind":"box","size":[10.0,10.0,10.0]},"position":[0.0,0.0,0.0],"rotation":[0.0,0.0,0.0],"role":"unused_part","material_id":"mat_panel"}]
        }));
        assert_eq!(
            expand_forge_visual_composition_v2(&orphan)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP202_MACRO_ORPHANED"
        );

        let mut overlap = fixture();
        overlap["calls"][0]["repeat"]["step"] = json!([0.0, 0.0, 0.0]);
        assert_eq!(
            expand_forge_visual_composition_v2(&overlap)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP202_REPEAT_LAYOUT"
        );
    }

    #[test]
    fn vp202_nested_bounded_macro_preserves_full_instance_path() {
        let mut source = fixture();
        source["macros"].as_array_mut().unwrap().push(json!({
            "macro_id":"macro_row",
            "parameters":[{"local_parameter_id":"local_child_width","kind":"length","unit":"millimeter"}],
            "items":[{
                "kind":"invoke","call_id":"call_nested_panel","macro_id":"macro_panel",
                "bindings":[{"local_parameter_id":"local_width","value":{"local_parameter_id":"local_child_width"}}],
                "repeat":{"count":2,"step":[0.0,40.0,0.0]}
            }]
        }));
        source["calls"] = json!([{
            "call_id":"call_rows","macro_id":"macro_row",
            "bindings":[{"local_parameter_id":"local_child_width","value":{"parameter_id":"param_panel_width"}}],
            "repeat":{"count":2,"step":[80.0,0.0,0.0]}
        }]);
        source["budgets"]["max_macro_calls"] = json!(8);
        let expanded = expand_forge_visual_composition_v2(&source).unwrap();
        assert_eq!(expanded.budget_evidence.expanded_output_count, 4);
        assert_eq!(expanded.budget_evidence.macro_call_count, 6);
        let outputs = expanded.expanded_program["outputs"].as_array().unwrap();
        let ids = outputs
            .iter()
            .map(|item| item["output_id"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), 4);
        assert!(ids
            .iter()
            .any(|id| id.contains("rows_r1_nested_panel_r1_shell")));
    }
}
