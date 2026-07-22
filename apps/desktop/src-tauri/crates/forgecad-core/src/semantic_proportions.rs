//! Rust-owned D005/G811 semantic proportion resolution.
//!
//! The catalog is visual-only and read-only. Resolution never recompiles
//! geometry and never writes a version or Snapshot: it accepts an option only
//! when the active immutable asset, its G808 binding, the AssemblyGraph
//! transform, the Snapshot-bound Q003 report and the exact production GLB in
//! CAS all agree.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    semantic_sha256, verify_forgecad_glb, ActiveDesignSnapshot, AgentAssetVersion, CoreError,
    CoreRepository, CoreResult, ForgeCadGlbReadback, ObjectReference, QualityStatus,
};

const RUNTIME_MANIFEST_VERSION: &str = "ShapeProgramRuntimeManifest@1";
const ALL_DOMAINS: [&str; 4] = [
    "pack_future_weapon_prop",
    "pack_vehicle_concept",
    "pack_aircraft_concept",
    "pack_robotic_arm_concept",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MechanicalStyleToken {
    pub schema_version: String,
    pub token_id: String,
    pub version: String,
    pub display_name: String,
    pub description: String,
    pub proportion_profile: String,
    pub edge_language: String,
    pub surface_tension: String,
    pub detail_density: String,
    pub symmetry: String,
    pub material_palette: String,
    pub lighting_profile: String,
    pub allowed_domains: Vec<String>,
    pub visual_only: bool,
    pub provenance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResolvedSemanticProportionOption {
    pub schema_version: String,
    pub recipe_id: String,
    pub style_token: MechanicalStyleToken,
    pub display_name: String,
    pub description: String,
    pub path: String,
    pub current_value: f64,
    pub target_value: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub unit: String,
    pub source_operation_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResolvedSemanticProportionOptions {
    pub schema_version: String,
    pub asset_version_id: String,
    pub part_id: String,
    pub domain_pack_id: String,
    pub runtime_manifest_version: String,
    pub shape_program_sha256: String,
    pub glb_sha256: String,
    pub locked: bool,
    pub options: Vec<ResolvedSemanticProportionOption>,
    pub unavailable_message: Option<String>,
}

#[derive(Clone, Copy)]
struct StyleTokenDefinition {
    token_id: &'static str,
    display_name: &'static str,
    description: &'static str,
    proportion_profile: &'static str,
    edge_language: &'static str,
    surface_tension: &'static str,
    detail_density: &'static str,
    symmetry: &'static str,
    material_palette: &'static str,
    lighting_profile: &'static str,
}

#[derive(Clone, Copy)]
struct RecipeDefinition {
    recipe_id: &'static str,
    domain_pack_id: &'static str,
    style_token_id: &'static str,
    display_name: &'static str,
    description: &'static str,
    role_selector: &'static str,
    path: &'static str,
    step_delta: f64,
}

#[derive(Clone, Copy)]
struct EditableBinding {
    path: &'static str,
    default: f64,
    min: f64,
    max: f64,
    step: f64,
}

const STYLE_TOKENS: [StyleTokenDefinition; 4] = [
    StyleTokenDefinition {
        token_id: "style_compact_rounded",
        display_name: "紧凑圆润",
        description: "缩短主要视觉跨度，保持轻量、亲和的概念外观。",
        proportion_profile: "compact",
        edge_language: "soft",
        surface_tension: "relaxed",
        detail_density: "low",
        symmetry: "bilateral",
        material_palette: "clean_coating",
        lighting_profile: "soft_studio",
    },
    StyleTokenDefinition {
        token_id: "style_aerodynamic_sleek",
        display_name: "修长流线",
        description: "延展主方向比例，形成更连贯的速度感轮廓。",
        proportion_profile: "elongated",
        edge_language: "controlled",
        surface_tension: "taut",
        detail_density: "low",
        symmetry: "bilateral",
        material_palette: "technical_composite",
        lighting_profile: "concept_contrast",
    },
    StyleTokenDefinition {
        token_id: "style_industrial_substantial",
        display_name: "厚重稳定",
        description: "增加承载视觉量感，仅表达外观，不代表结构或性能。",
        proportion_profile: "substantial",
        edge_language: "crisp",
        surface_tension: "neutral",
        detail_density: "medium",
        symmetry: "assembly_driven",
        material_palette: "mixed_industrial",
        lighting_profile: "cad_neutral",
    },
    StyleTokenDefinition {
        token_id: "style_clean_balanced",
        display_name: "简洁协调",
        description: "收敛次要部件比例，让主次关系更清楚。",
        proportion_profile: "balanced",
        edge_language: "controlled",
        surface_tension: "neutral",
        detail_density: "low",
        symmetry: "assembly_driven",
        material_palette: "dark_metal",
        lighting_profile: "cad_neutral",
    },
];

const RECIPES: [RecipeDefinition; 16] = [
    recipe(
        "proportion_prop_compact",
        "pack_future_weapon_prop",
        "style_compact_rounded",
        "主体更紧凑",
        "收短展示道具主体的视觉长度。",
        "primary_form",
        "transform.scale.x",
        -1.0,
    ),
    recipe(
        "proportion_prop_sleek",
        "pack_future_weapon_prop",
        "style_aerodynamic_sleek",
        "主体更修长",
        "延展展示道具主体的视觉长度。",
        "primary_form",
        "transform.scale.x",
        1.0,
    ),
    recipe(
        "proportion_prop_substantial",
        "pack_future_weapon_prop",
        "style_industrial_substantial",
        "主体更厚重",
        "增加主体的视觉高度，不表达功能能力。",
        "primary_form",
        "transform.scale.y",
        1.0,
    ),
    recipe(
        "proportion_prop_clean",
        "pack_future_weapon_prop",
        "style_clean_balanced",
        "辅助体更简洁",
        "收窄次要外壳，突出主体层级。",
        "secondary_form",
        "transform.scale.z",
        -1.0,
    ),
    recipe(
        "proportion_vehicle_compact",
        "pack_vehicle_concept",
        "style_compact_rounded",
        "车身更紧凑",
        "收短车身主壳体的视觉长度。",
        "primary_form",
        "transform.scale.x",
        -1.0,
    ),
    recipe(
        "proportion_vehicle_sleek",
        "pack_vehicle_concept",
        "style_aerodynamic_sleek",
        "车身更修长",
        "延展车身主壳体的视觉长度。",
        "primary_form",
        "transform.scale.x",
        1.0,
    ),
    recipe(
        "proportion_vehicle_substantial",
        "pack_vehicle_concept",
        "style_industrial_substantial",
        "车身更厚重",
        "增加车身主壳体的视觉高度。",
        "primary_form",
        "transform.scale.y",
        1.0,
    ),
    recipe(
        "proportion_vehicle_clean",
        "pack_vehicle_concept",
        "style_clean_balanced",
        "座舱更简洁",
        "收窄座舱视觉比例，强化车身主次。",
        "cabin_form",
        "transform.scale.z",
        -1.0,
    ),
    recipe(
        "proportion_aircraft_compact",
        "pack_aircraft_concept",
        "style_compact_rounded",
        "机身更紧凑",
        "收短机身的视觉长度。",
        "primary_form",
        "transform.scale.x",
        -1.0,
    ),
    recipe(
        "proportion_aircraft_sleek",
        "pack_aircraft_concept",
        "style_aerodynamic_sleek",
        "机身更修长",
        "延展机身的视觉长度。",
        "primary_form",
        "transform.scale.x",
        1.0,
    ),
    recipe(
        "proportion_aircraft_substantial",
        "pack_aircraft_concept",
        "style_industrial_substantial",
        "机身更厚重",
        "增加机身的视觉高度，不代表适航或结构性能。",
        "primary_form",
        "transform.scale.y",
        1.0,
    ),
    recipe(
        "proportion_aircraft_clean",
        "pack_aircraft_concept",
        "style_clean_balanced",
        "座舱盖更简洁",
        "收窄座舱盖视觉比例。",
        "cabin_form",
        "transform.scale.z",
        -1.0,
    ),
    recipe(
        "proportion_arm_compact",
        "pack_robotic_arm_concept",
        "style_compact_rounded",
        "上臂更紧凑",
        "收短上臂连杆的视觉跨度。",
        "upper_link_form",
        "transform.scale.y",
        -1.0,
    ),
    recipe(
        "proportion_arm_sleek",
        "pack_robotic_arm_concept",
        "style_aerodynamic_sleek",
        "上臂更修长",
        "延展上臂连杆的视觉跨度。",
        "upper_link_form",
        "transform.scale.y",
        1.0,
    ),
    recipe(
        "proportion_arm_substantial",
        "pack_robotic_arm_concept",
        "style_industrial_substantial",
        "底座更厚重",
        "增加底座的视觉宽度，不代表负载能力。",
        "base_form",
        "transform.scale.x",
        1.0,
    ),
    recipe(
        "proportion_arm_clean",
        "pack_robotic_arm_concept",
        "style_clean_balanced",
        "末端更简洁",
        "收窄末端部件的视觉比例。",
        "end_effector_form",
        "transform.scale.z",
        -1.0,
    ),
];

const fn recipe(
    recipe_id: &'static str,
    domain_pack_id: &'static str,
    style_token_id: &'static str,
    display_name: &'static str,
    description: &'static str,
    role_selector: &'static str,
    path: &'static str,
    step_delta: f64,
) -> RecipeDefinition {
    RecipeDefinition {
        recipe_id,
        domain_pack_id,
        style_token_id,
        display_name,
        description,
        role_selector,
        path,
        step_delta,
    }
}

pub fn resolve_semantic_proportions(
    repository: &CoreRepository,
    asset_version_id: &str,
    part_id: &str,
) -> CoreResult<ResolvedSemanticProportionOptions> {
    let version = repository
        .version(asset_version_id)?
        .ok_or_else(|| CoreError::not_found("AgentAssetVersion"))?;
    let snapshot = repository
        .snapshot(&version.project_id)?
        .ok_or_else(|| CoreError::not_found("ActiveDesignSnapshot"))?;
    require_current_asset(repository, &version, &snapshot)?;

    if is_external_reference(repository, &version)? {
        return Err(CoreError::conflict(
            "EXTERNAL_REFERENCE_NOT_EDITABLE",
            "导入 GLB 当前仅作为参考模型；请让 Agent 重建后再使用外观比例配方。",
        ));
    }

    let part = version
        .parts
        .iter()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(part_id))
        .ok_or_else(|| CoreError::not_found("Agent asset part"))?;
    let part_role = part
        .get("role")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            CoreError::invalid_data(
                "PART_ROLE_INVALID",
                "Semantic proportions require a stable persisted part role.",
            )
        })?;
    let graph_part = graph_part(&version, part_id);
    let locked = part.get("locked").and_then(Value::as_bool) == Some(true)
        || graph_part
            .and_then(|item| item.get("locked"))
            .and_then(Value::as_bool)
            == Some(true)
        || snapshot
            .part_display
            .as_ref()
            .is_some_and(|display| display.locked_part_ids.iter().any(|id| id == part_id));

    let verified = verified_production_artifact(repository, &version, &snapshot)?;
    let zones = part_material_zones(part, graph_part);
    let operation_ids = shape_operation_ids(&version.shape_program);
    let source_operation_ids =
        surface_source_operations(&verified.document, part_role, &zones, &operation_ids);
    let bindings = editable_bindings(part)?;
    let graph_scale = graph_scale(graph_part);
    let tokens = style_token_map();
    let mut options = Vec::new();

    if !locked && !source_operation_ids.is_empty() {
        for recipe in RECIPES
            .iter()
            .filter(|recipe| recipe.domain_pack_id == version.domain_pack_id)
        {
            if resolve_role_selector(&version, recipe.role_selector).as_deref() != Some(part_id) {
                continue;
            }
            let Some(binding) = bindings.get(recipe.path) else {
                continue;
            };
            let Some(style_token) = tokens.get(recipe.style_token_id) else {
                continue;
            };
            if !style_token
                .allowed_domains
                .iter()
                .any(|domain| domain == &version.domain_pack_id)
            {
                continue;
            }
            let Some(axis) = scale_axis(recipe.path) else {
                continue;
            };
            let Some(current) = graph_scale.and_then(|scale| scale.get(axis).copied()) else {
                continue;
            };
            if !in_declared_grid(current, *binding) {
                continue;
            }
            let target = round_ten(current + binding.step * recipe.step_delta);
            if !in_declared_grid(target, *binding) {
                continue;
            }
            options.push(ResolvedSemanticProportionOption {
                schema_version: "ResolvedSemanticProportionOption@1".into(),
                recipe_id: recipe.recipe_id.into(),
                style_token: style_token.clone(),
                display_name: recipe.display_name.into(),
                description: recipe.description.into(),
                path: binding.path.into(),
                current_value: current,
                target_value: target,
                min: binding.min,
                max: binding.max,
                step: binding.step,
                unit: "ratio".into(),
                source_operation_ids: source_operation_ids.clone(),
            });
        }
    }

    let unavailable_message = if locked {
        Some("该部件已锁定。解除锁定后才能预览外观比例配方。".into())
    } else if source_operation_ids.is_empty() {
        Some("真实编译结果没有找到该部件的稳定表面来源，未提供比例配方。".into())
    } else if bindings.is_empty() {
        Some("该部件没有受限比例参数，Agent 不会猜测或创建自由参数。".into())
    } else if options.is_empty() {
        Some("当前部件没有适用且仍在安全范围内的领域比例配方。".into())
    } else {
        None
    };

    Ok(ResolvedSemanticProportionOptions {
        schema_version: "ResolvedSemanticProportionOptions@1".into(),
        asset_version_id: version.asset_version_id,
        part_id: part_id.into(),
        domain_pack_id: version.domain_pack_id,
        runtime_manifest_version: RUNTIME_MANIFEST_VERSION.into(),
        shape_program_sha256: verified.shape_program_sha256,
        glb_sha256: verified.readback.glb_sha256,
        locked,
        options,
        unavailable_message,
    })
}

fn require_current_asset(
    repository: &CoreRepository,
    version: &AgentAssetVersion,
    snapshot: &ActiveDesignSnapshot,
) -> CoreResult<()> {
    let head = repository.head(&version.project_id)?;
    if snapshot.active_design.asset_version_id() != Some(version.asset_version_id.as_str())
        || head.as_deref() != Some(version.asset_version_id.as_str())
    {
        return Err(CoreError::conflict(
            "ACTIVE_DESIGN_STALE",
            "该资产不是当前活动设计，请刷新后重新检查。",
        ));
    }
    Ok(())
}

fn is_external_reference(
    repository: &CoreRepository,
    version: &AgentAssetVersion,
) -> CoreResult<bool> {
    if version
        .shape_program
        .get("schema_version")
        .and_then(Value::as_str)
        == Some("ExternalGLBReference@1")
    {
        return Ok(true);
    }
    Ok(repository
        .object_for_reference(&ObjectReference {
            reference_kind: "asset_version".into(),
            owner_id: version.asset_version_id.clone(),
            role: "external_reference_glb".into(),
        })?
        .is_some())
}

struct VerifiedProductionArtifact {
    readback: ForgeCadGlbReadback,
    document: Value,
    shape_program_sha256: String,
}

fn verified_production_artifact(
    repository: &CoreRepository,
    version: &AgentAssetVersion,
    snapshot: &ActiveDesignSnapshot,
) -> CoreResult<VerifiedProductionArtifact> {
    let quality_reference = snapshot.quality.as_ref().ok_or_else(|| {
        readback_conflict("The active Snapshot has no Q003 compile readback quality.")
    })?;
    if quality_reference.asset_version_id != version.asset_version_id {
        return Err(readback_conflict(
            "Snapshot quality belongs to another Agent asset version.",
        ));
    }
    let quality = repository
        .quality_report(&quality_reference.quality_report_id)?
        .ok_or_else(|| readback_conflict("Snapshot quality report is missing."))?;
    if quality.project_id != version.project_id
        || quality.asset_version_id != version.asset_version_id
        || quality.status != QualityStatus::Passed
    {
        return Err(readback_conflict(
            "Snapshot quality is unavailable, failed or bound to another asset.",
        ));
    }

    let object = repository
        .object_for_reference(&ObjectReference {
            reference_kind: "asset_version".into(),
            owner_id: version.asset_version_id.clone(),
            role: "production_glb".into(),
        })?
        .ok_or_else(|| readback_conflict("The active asset has no production GLB in CAS."))?;
    let bytes = repository.read_object(&object.sha256)?;
    let readback = verify_forgecad_glb(&bytes, Some("production_concept"))?;
    let shape_program_sha256 = semantic_sha256(&version.shape_program)?;
    let compile = quality
        .report
        .get("compile_readback")
        .and_then(Value::as_object)
        .ok_or_else(|| readback_conflict("Q003 quality has no compile_readback object."))?;
    let profile = compile
        .get("artifact_profile")
        .and_then(Value::as_object)
        .ok_or_else(|| readback_conflict("Q003 readback has no artifact profile."))?;
    let exact = quality
        .report
        .get("evidence_source")
        .and_then(Value::as_str)
        == Some("geometry_compile_readback")
        && compile.get("schema_version").and_then(Value::as_str)
            == Some("GeometryCompileReadback@2")
        && compile
            .get("runtime_manifest_version")
            .and_then(Value::as_str)
            == Some(readback.runtime_manifest_version.as_str())
        && profile.get("artifact_profile_id").and_then(Value::as_str) == Some("production_concept")
        && profile.get("profile_sha256").and_then(Value::as_str)
            == Some(readback.artifact_profile_sha256.as_str())
        && compile.get("shape_program_sha256").and_then(Value::as_str)
            == Some(shape_program_sha256.as_str())
        && compile.get("glb_sha256").and_then(Value::as_str) == Some(readback.glb_sha256.as_str())
        && object.sha256 == readback.glb_sha256
        && compile.get("glb_byte_size").and_then(Value::as_u64) == Some(readback.glb_byte_size)
        && compile.get("triangle_count").and_then(Value::as_u64) == Some(readback.triangle_count)
        && compile.get("bounds_mm") == Some(&serde_json::json!(readback.bounds_mm))
        && compile.get("mesh_count").and_then(Value::as_u64) == Some(readback.mesh_count)
        && compile.get("primitive_count").and_then(Value::as_u64) == Some(readback.primitive_count)
        && compile.get("material_count").and_then(Value::as_u64) == Some(readback.material_count)
        && compile.get("uv0_primitive_count").and_then(Value::as_u64)
            == Some(readback.uv0_primitive_count)
        && compile
            .get("normal_primitive_count")
            .and_then(Value::as_u64)
            == Some(readback.normal_primitive_count)
        && compile
            .get("tangent_primitive_count")
            .and_then(Value::as_u64)
            == Some(readback.tangent_primitive_count)
        && compile.get("closed_manifold").and_then(Value::as_bool) == Some(true)
        && compile
            .get("surface_provenance_present")
            .and_then(Value::as_bool)
            == Some(true)
        && readback.closed_manifold
        && readback.surface_provenance_present;
    if !exact {
        return Err(readback_conflict(
            "Q003 readback, ShapeProgram and canonical production GLB do not match.",
        ));
    }
    let document = glb_json_document(&bytes)?;
    Ok(VerifiedProductionArtifact {
        readback,
        document,
        shape_program_sha256,
    })
}

fn readback_conflict(message: impl Into<String>) -> CoreError {
    CoreError::conflict("GEOMETRY_READBACK_FAILED", message)
}

fn graph_part<'a>(version: &'a AgentAssetVersion, part_id: &str) -> Option<&'a Value> {
    version
        .assembly_graph
        .get("parts")
        .and_then(Value::as_array)?
        .iter()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(part_id))
}

fn part_material_zones(part: &Value, graph_part: Option<&Value>) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for source in [Some(part), graph_part].into_iter().flatten() {
        for key in ["material_zone_ids", "material_zones"] {
            if let Some(zones) = source.get(key).and_then(Value::as_array) {
                for zone in zones {
                    if let Some(id) = zone
                        .as_str()
                        .or_else(|| zone.get("zone_id").and_then(Value::as_str))
                        .filter(|id| !id.is_empty())
                    {
                        result.insert(id.to_string());
                    }
                }
            }
        }
    }
    result
}

fn shape_operation_ids(shape_program: &Value) -> BTreeSet<String> {
    shape_program
        .get("operations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|operation| operation.get("operation_id").and_then(Value::as_str))
        .filter(|operation_id| operation_id.starts_with("op_") && !operation_id.contains('\0'))
        .map(str::to_string)
        .collect()
}

fn surface_source_operations(
    document: &Value,
    part_role: &str,
    zones: &BTreeSet<String>,
    shape_operation_ids: &BTreeSet<String>,
) -> Vec<String> {
    let mut result = BTreeSet::new();
    let Some(meshes) = document.get("meshes").and_then(Value::as_array) else {
        return Vec::new();
    };
    for primitive in meshes
        .iter()
        .filter_map(|mesh| mesh.get("primitives").and_then(Value::as_array))
        .flatten()
    {
        let Some(extras) = primitive.get("extras").and_then(Value::as_object) else {
            continue;
        };
        if extras.get("forgecad_part_role").and_then(Value::as_str) != Some(part_role) {
            continue;
        }
        let Some(zone_id) = extras
            .get("forgecad_material_zone_id")
            .and_then(Value::as_str)
        else {
            continue;
        };
        if !zones.contains(zone_id) {
            continue;
        }
        let feature_node = extras
            .get("forgecad_feature_node_id")
            .and_then(Value::as_str);
        let source_ids = extras
            .get("forgecad_csg_provenance")
            .and_then(|value| value.get("source_operation_ids"))
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
            .unwrap_or_else(|| feature_node.into_iter().collect());
        if source_ids.is_empty()
            || source_ids
                .iter()
                .any(|id| !shape_operation_ids.contains(*id))
        {
            continue;
        }
        result.extend(source_ids.into_iter().map(str::to_string));
    }
    result.into_iter().collect()
}

fn editable_bindings(part: &Value) -> CoreResult<BTreeMap<&'static str, EditableBinding>> {
    let mut result = BTreeMap::new();
    let bindings = part
        .get("editable_parameter_bindings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for binding in bindings {
        let Some(path) = binding.get("path").and_then(Value::as_str) else {
            continue;
        };
        let Some(path) = canonical_scale_path(path) else {
            continue;
        };
        if binding.get("unit").and_then(Value::as_str) != Some("ratio") {
            continue;
        }
        let default = finite_number(&binding, "default")?;
        let min = finite_number(&binding, "min")?;
        let max = finite_number(&binding, "max")?;
        let step = finite_number(&binding, "step")?;
        if max <= min
            || step <= 0.0
            || step > max - min
            || default < min
            || default > max
            || !is_grid_value(default, min, step)
        {
            return Err(CoreError::invalid_data(
                "EDITABLE_PARAMETER_BINDING_INVALID",
                "Semantic proportion binding range, default or step is invalid.",
            ));
        }
        if result
            .insert(
                path,
                EditableBinding {
                    path,
                    default,
                    min,
                    max,
                    step,
                },
            )
            .is_some()
        {
            return Err(CoreError::invalid_data(
                "EDITABLE_PARAMETER_BINDING_INVALID",
                "Semantic proportion binding paths must be unique.",
            ));
        }
    }
    Ok(result)
}

fn finite_number(value: &Value, key: &str) -> CoreResult<f64> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
        .ok_or_else(|| {
            CoreError::invalid_data(
                "EDITABLE_PARAMETER_BINDING_INVALID",
                format!("Semantic proportion binding {key} must be finite."),
            )
        })
}

fn canonical_scale_path(path: &str) -> Option<&'static str> {
    match path {
        "transform.scale.x" => Some("transform.scale.x"),
        "transform.scale.y" => Some("transform.scale.y"),
        "transform.scale.z" => Some("transform.scale.z"),
        _ => None,
    }
}

fn graph_scale(graph_part: Option<&Value>) -> Option<[f64; 3]> {
    let scale = graph_part?
        .get("transform")?
        .get("scale")?
        .as_array()
        .filter(|values| values.len() == 3)?;
    let values = [scale[0].as_f64()?, scale[1].as_f64()?, scale[2].as_f64()?];
    values
        .iter()
        .all(|value| value.is_finite())
        .then_some(values)
}

fn resolve_role_selector(version: &AgentAssetVersion, selector: &str) -> Option<String> {
    let parts = version
        .parts
        .iter()
        .filter_map(|part| {
            Some((
                part.get("part_id")?.as_str()?.to_string(),
                part.get("role")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            ))
        })
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return None;
    }
    let root_id = version
        .assembly_graph
        .get("root_part_id")
        .and_then(Value::as_str);
    if matches!(selector, "primary_form" | "base_form") {
        return root_id
            .filter(|root| parts.iter().any(|(id, _)| id == root))
            .map(str::to_string)
            .or_else(|| Some(parts[0].0.clone()));
    }
    if selector == "secondary_form" {
        return parts.get(1).map(|part| part.0.clone());
    }
    let keywords: &[&str] = match selector {
        "cabin_form" => &["cabin", "cockpit", "canopy"],
        "upper_link_form" => &[
            "upper",
            "link_1",
            "boom_a",
            "desktop_link",
            "rail_link",
            "welding_arm",
        ],
        "end_effector_form" => &["tool", "gripper", "claw", "probe", "camera", "sensor"],
        _ => &[],
    };
    let matches = parts
        .iter()
        .filter(|(_, role)| keywords.iter().any(|keyword| role.contains(keyword)))
        .collect::<Vec<_>>();
    if let Some(found) = if selector == "end_effector_form" {
        matches.last()
    } else {
        matches.first()
    } {
        return Some(found.0.clone());
    }
    match selector {
        "upper_link_form" => parts.get(2.min(parts.len() - 1)).map(|part| part.0.clone()),
        "end_effector_form" => parts.last().map(|part| part.0.clone()),
        _ => parts.get(1).map(|part| part.0.clone()),
    }
}

fn style_token_map() -> BTreeMap<&'static str, MechanicalStyleToken> {
    STYLE_TOKENS
        .iter()
        .map(|definition| {
            (
                definition.token_id,
                MechanicalStyleToken {
                    schema_version: "MechanicalStyleToken@1".into(),
                    token_id: definition.token_id.into(),
                    version: "1".into(),
                    display_name: definition.display_name.into(),
                    description: definition.description.into(),
                    proportion_profile: definition.proportion_profile.into(),
                    edge_language: definition.edge_language.into(),
                    surface_tension: definition.surface_tension.into(),
                    detail_density: definition.detail_density.into(),
                    symmetry: definition.symmetry.into(),
                    material_palette: definition.material_palette.into(),
                    lighting_profile: definition.lighting_profile.into(),
                    allowed_domains: ALL_DOMAINS.iter().map(|value| (*value).into()).collect(),
                    visual_only: true,
                    provenance: "forgecad_builtin".into(),
                },
            )
        })
        .collect()
}

fn scale_axis(path: &str) -> Option<usize> {
    match path {
        "transform.scale.x" => Some(0),
        "transform.scale.y" => Some(1),
        "transform.scale.z" => Some(2),
        _ => None,
    }
}

fn in_declared_grid(value: f64, binding: EditableBinding) -> bool {
    value.is_finite()
        && value >= binding.min - 1e-9
        && value <= binding.max + 1e-9
        && is_grid_value(value, binding.min, binding.step)
        && binding.default.is_finite()
}

fn is_grid_value(value: f64, min: f64, step: f64) -> bool {
    let steps = (value - min) / step;
    (steps - steps.round()).abs() <= 1e-8
}

fn round_ten(value: f64) -> f64 {
    (value * 10_000_000_000.0).round() / 10_000_000_000.0
}

fn glb_json_document(bytes: &[u8]) -> CoreResult<Value> {
    if bytes.len() < 20 || bytes.get(..4) != Some(b"glTF") {
        return Err(readback_conflict("Production object is not a binary glTF."));
    }
    let version = read_u32_le(bytes, 4)?;
    let declared = read_u32_le(bytes, 8)? as usize;
    if version != 2 || declared != bytes.len() {
        return Err(readback_conflict(
            "Production GLB header does not match its CAS bytes.",
        ));
    }
    let mut cursor = 12usize;
    let mut document = None;
    while cursor < bytes.len() {
        if cursor.checked_add(8).is_none_or(|end| end > bytes.len()) {
            return Err(readback_conflict(
                "Production GLB chunk header is truncated.",
            ));
        }
        let length = read_u32_le(bytes, cursor)? as usize;
        let kind = read_u32_le(bytes, cursor + 4)?;
        let start = cursor + 8;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| readback_conflict("Production GLB chunk is truncated."))?;
        if kind == 0x4e4f_534a {
            if document.is_some() {
                return Err(readback_conflict(
                    "Production GLB contains duplicate JSON chunks.",
                ));
            }
            document = Some(
                serde_json::from_slice(&bytes[start..end])
                    .map_err(|_| readback_conflict("Production GLB JSON cannot be decoded."))?,
            );
        }
        cursor = end;
    }
    document.ok_or_else(|| readback_conflict("Production GLB has no JSON document."))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> CoreResult<u32> {
    let raw: [u8; 4] = bytes
        .get(offset..offset + 4)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| readback_conflict("Production GLB integer is truncated."))?;
    Ok(u32::from_le_bytes(raw))
}
