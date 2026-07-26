//! Deterministic bridge from the current C111 reviewed robotic-arm asset into
//! `ForgeVisualProgram@1`.
//!
//! This bridge is deliberately evidence-preserving. The development inventory
//! still contains critical visual blockers, so the emitted program remains a
//! draft and a sealed attempt must fail. PV002 can only seal the same program
//! after those blockers are genuinely closed by geometry/material/surface and
//! fixed-view evidence.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    lower_forge_visual_program, semantic_sha256, CoreError, CoreResult, ExpandedComponentCandidate,
    ForgeVisualDesignToken, ForgeVisualExportProfile, ForgeVisualMaterialBinding, ForgeVisualPart,
    ForgeVisualProgram, ForgeVisualProgramLowering, ForgeVisualProgramStage,
    ForgeVisualSurfaceBinding, RecipeRegistry, SurfaceAdornmentProgram, VisualDetailBinding,
    VisualDetailBindingKind, VisualDetailInventoryItem, VisualDetailLevel, VisualDetailStatus,
};

pub const C111_FORGE_VISUAL_PROGRAM_FIXTURE_SCHEMA_VERSION: &str =
    "C111ForgeVisualProgramFixture@1";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct C111ForgeVisualProgramFixture {
    pub schema_version: String,
    pub fixture_id: String,
    pub registry_id: String,
    pub registry_sha256: String,
    pub inventory_id: String,
    pub inventory_semantic_sha256: String,
    pub program: ForgeVisualProgram,
    pub lowering: ForgeVisualProgramLowering,
    pub surface_adornment_programs: Vec<SurfaceAdornmentProgram>,
    pub fixed_views: Vec<Value>,
    pub expected_production: Value,
    pub critical_unresolved_detail_ids: Vec<String>,
    pub sealed_status: String,
    pub sealed_error_code: String,
}

fn invalid(message: impl Into<String>) -> CoreError {
    CoreError::invalid_data("C111_FORGE_VISUAL_FIXTURE_INVALID", message)
}

fn object<'a>(value: &'a Value, field: &str) -> CoreResult<&'a serde_json::Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| invalid(format!("{field} must be an object")))
}

fn array<'a>(value: &'a Value, field: &str) -> CoreResult<&'a Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| invalid(format!("{field} must be an array")))
}

fn string<'a>(value: &'a Value, field: &str) -> CoreResult<&'a str> {
    value
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(format!("{field} must be a non-empty string")))
}

fn detail_level(value: &str) -> CoreResult<VisualDetailLevel> {
    match value {
        "macro" => Ok(VisualDetailLevel::Macro),
        "meso" => Ok(VisualDetailLevel::Meso),
        "micro" => Ok(VisualDetailLevel::Micro),
        _ => Err(invalid("detail scale_band is unsupported")),
    }
}

fn owner_for_operation<'a>(
    operation_id: &str,
    part_suffixes: &'a BTreeMap<String, String>,
) -> CoreResult<&'a str> {
    let matches = part_suffixes
        .iter()
        .filter(|(suffix, _)| operation_id.starts_with(&format!("op_{suffix}_")))
        .map(|(_, part_id)| part_id.as_str())
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(invalid(format!(
            "operation {operation_id} must have exactly one C111 part owner"
        )));
    }
    Ok(matches[0])
}

pub fn build_c111_forge_visual_program_fixture(
    candidate: &ExpandedComponentCandidate,
    registry: &RecipeRegistry,
    surface_programs: &[SurfaceAdornmentProgram],
    inventory: &Value,
) -> CoreResult<C111ForgeVisualProgramFixture> {
    let inventory_object = object(inventory, "inventory")?;
    if inventory_object
        .get("schema_version")
        .and_then(Value::as_str)
        != Some("C111GoldenSurfaceVisualDetailInventory@1")
        || inventory_object
            .get("root_recipe_id")
            .and_then(Value::as_str)
            != Some("recipe_c111_arm_golden_surface")
        || inventory_object.get("registry_id").and_then(Value::as_str)
            != Some(registry.registry_id())
    {
        return Err(invalid(
            "C111 inventory identity does not match the registry",
        ));
    }

    let assembly_parts = array(
        candidate
            .expanded_assembly_graph
            .get("parts")
            .ok_or_else(|| invalid("assembly_graph.parts is missing"))?,
        "assembly_graph.parts",
    )?;
    let outputs = array(
        candidate
            .expanded_shape_program
            .get("outputs")
            .ok_or_else(|| invalid("shape_program.outputs is missing"))?,
        "shape_program.outputs",
    )?;

    let mut part_suffixes = BTreeMap::new();
    let mut zones_by_part = BTreeMap::<String, BTreeSet<String>>::new();
    let mut parent_by_part = BTreeMap::<String, Option<String>>::new();
    let mut role_by_part = BTreeMap::<String, String>::new();
    for raw_part in assembly_parts {
        let part = object(raw_part, "assembly part")?;
        let part_id = string(
            part.get("part_id")
                .ok_or_else(|| invalid("assembly part_id is missing"))?,
            "assembly part_id",
        )?
        .to_owned();
        let suffix = part_id
            .strip_prefix("part_")
            .ok_or_else(|| invalid("C111 part_id must use part_ prefix"))?
            .to_owned();
        if part_suffixes.insert(suffix, part_id.clone()).is_some() {
            return Err(invalid("C111 part suffixes must be unique"));
        }
        let zones = array(
            part.get("material_zone_ids")
                .ok_or_else(|| invalid("part material_zone_ids is missing"))?,
            "part material_zone_ids",
        )?
        .iter()
        .map(|zone| string(zone, "material zone").map(str::to_owned))
        .collect::<CoreResult<BTreeSet<_>>>()?;
        zones_by_part.insert(part_id.clone(), zones);
        parent_by_part.insert(
            part_id.clone(),
            part.get("parent_part_id")
                .and_then(Value::as_str)
                .map(str::to_owned),
        );
        role_by_part.insert(
            part_id,
            string(
                part.get("role")
                    .ok_or_else(|| invalid("part role is missing"))?,
                "part role",
            )?
            .to_owned(),
        );
    }

    let mut output_owner = BTreeMap::<String, String>::new();
    let mut operation_outputs = Vec::<(String, String, String)>::new();
    for raw_output in outputs {
        let output = object(raw_output, "shape output")?;
        let output_id = string(
            output
                .get("output_id")
                .ok_or_else(|| invalid("shape output_id is missing"))?,
            "shape output_id",
        )?
        .to_owned();
        let operation_id = string(
            output
                .get("operation_id")
                .ok_or_else(|| invalid("shape operation_id is missing"))?,
            "shape operation_id",
        )?
        .to_owned();
        let owner = owner_for_operation(&operation_id, &part_suffixes)?.to_owned();
        if output_owner
            .insert(output_id.clone(), owner.clone())
            .is_some()
        {
            return Err(invalid("shape output identifiers must be unique"));
        }
        operation_outputs.push((operation_id, output_id, owner));
    }

    let parts = role_by_part
        .iter()
        .map(|(part_id, role)| {
            let geometry_output_ids = output_owner
                .iter()
                .filter(|(_, owner)| *owner == part_id)
                .map(|(output_id, _)| output_id.clone())
                .collect::<Vec<_>>();
            ForgeVisualPart {
                part_id: part_id.clone(),
                role: role.clone(),
                parent_part_id: parent_by_part.get(part_id).cloned().flatten(),
                geometry_output_ids,
                material_zone_ids: zones_by_part
                    .get(part_id)
                    .expect("part zone index is complete")
                    .iter()
                    .cloned()
                    .collect(),
            }
        })
        .collect::<Vec<_>>();

    let mut material_graph = Vec::new();
    for instance in &candidate.instances {
        let part_id = format!(
            "part_{}",
            instance.instance_id.trim_start_matches("recipeinst_")
        );
        for raw_zone in &instance.recipe.material_zones {
            let zone = object(raw_zone, "recipe material zone")?;
            material_graph.push(ForgeVisualMaterialBinding {
                part_id: part_id.clone(),
                material_zone_id: string(
                    zone.get("zone_id")
                        .ok_or_else(|| invalid("recipe zone_id is missing"))?,
                    "recipe zone_id",
                )?
                .to_owned(),
                material_id: string(
                    zone.get("material_preset_id")
                        .ok_or_else(|| invalid("recipe material_preset_id is missing"))?,
                    "recipe material_preset_id",
                )?
                .to_owned(),
            });
        }
    }
    material_graph.sort_by(|left, right| {
        (&left.part_id, &left.material_zone_id).cmp(&(&right.part_id, &right.material_zone_id))
    });

    let mut surface_graph = surface_programs
        .iter()
        .map(|program| ForgeVisualSurfaceBinding {
            surface_program_id: program.program_id.clone(),
            part_id: program.target_part_id.clone(),
            material_zone_id: program.target_zone_id.clone(),
        })
        .collect::<Vec<_>>();
    surface_graph.sort_by(|left, right| left.surface_program_id.cmp(&right.surface_program_id));

    let surface_by_id = surface_programs
        .iter()
        .map(|program| (program.program_id.as_str(), program))
        .collect::<BTreeMap<_, _>>();
    let inventory_items = array(
        inventory_object
            .get("items")
            .ok_or_else(|| invalid("inventory.items is missing"))?,
        "inventory.items",
    )?;
    let mut details = Vec::new();
    let mut critical_unresolved_detail_ids = Vec::new();
    for raw_item in inventory_items {
        let item = object(raw_item, "inventory item")?;
        let detail_id = string(
            item.get("detail_id")
                .ok_or_else(|| invalid("detail_id is missing"))?,
            "detail_id",
        )?
        .to_owned();
        let critical = item.get("importance").and_then(Value::as_str) == Some("critical");
        let status = match item.get("status").and_then(Value::as_str) {
            Some("readback_verified") => VisualDetailStatus::Bound,
            Some("unresolved") => VisualDetailStatus::Unresolved,
            _ => return Err(invalid("inventory detail status is unsupported")),
        };
        if critical && status == VisualDetailStatus::Unresolved {
            critical_unresolved_detail_ids.push(detail_id.clone());
        }
        let mut bindings = BTreeMap::<(String, String, String), VisualDetailBinding>::new();
        for raw_mapping in array(
            item.get("maps_to")
                .ok_or_else(|| invalid("detail maps_to is missing"))?,
            "detail maps_to",
        )? {
            let mapping = object(raw_mapping, "detail mapping")?;
            if let Some(suffix) = mapping
                .get("shape_operation_suffix")
                .and_then(Value::as_str)
            {
                for (operation_id, output_id, part_id) in &operation_outputs {
                    if operation_id.ends_with(suffix) {
                        let binding = VisualDetailBinding {
                            kind: VisualDetailBindingKind::GeometryOutput,
                            part_id: part_id.clone(),
                            target_id: output_id.clone(),
                        };
                        bindings.insert(
                            ("geometry_output".into(), part_id.clone(), output_id.clone()),
                            binding,
                        );
                    }
                }
            }
            if let Some(zone_id) = mapping.get("material_zone_id").and_then(Value::as_str) {
                for (part_id, zones) in &zones_by_part {
                    if zones.contains(zone_id) {
                        let binding = VisualDetailBinding {
                            kind: VisualDetailBindingKind::MaterialZone,
                            part_id: part_id.clone(),
                            target_id: zone_id.to_owned(),
                        };
                        bindings.insert(
                            ("material_zone".into(), part_id.clone(), zone_id.to_owned()),
                            binding,
                        );
                    }
                }
            }
            if let Some(program_id) = mapping.get("adornment_program_id").and_then(Value::as_str) {
                let program = surface_by_id
                    .get(program_id)
                    .ok_or_else(|| invalid("detail references a missing surface program"))?;
                let binding = VisualDetailBinding {
                    kind: VisualDetailBindingKind::SurfaceProgram,
                    part_id: program.target_part_id.clone(),
                    target_id: program_id.to_owned(),
                };
                bindings.insert(
                    (
                        "surface_program".into(),
                        program.target_part_id.clone(),
                        program_id.to_owned(),
                    ),
                    binding,
                );
            }
        }
        details.push(VisualDetailInventoryItem {
            detail_id,
            level: detail_level(string(
                item.get("scale_band")
                    .ok_or_else(|| invalid("detail scale_band is missing"))?,
                "detail scale_band",
            )?)?,
            description: string(
                item.get("expected_visual_effect")
                    .ok_or_else(|| invalid("detail expected_visual_effect is missing"))?,
                "detail expected_visual_effect",
            )?
            .to_owned(),
            critical,
            status,
            bindings: bindings.into_values().collect(),
        });
    }
    critical_unresolved_detail_ids.sort();
    let stage = if critical_unresolved_detail_ids.is_empty() {
        ForgeVisualProgramStage::Sealed
    } else {
        ForgeVisualProgramStage::Draft
    };

    let program = ForgeVisualProgram {
        schema_version: "ForgeVisualProgram@1".into(),
        program_id: "visual_program_c111_robotic_arm_iteration_70".into(),
        domain_pack_id: "pack_robotic_arm_concept".into(),
        title: "未来工业机械臂收藏品黄金路径".into(),
        stage,
        visual_only: true,
        design_tokens: vec![
            ForgeVisualDesignToken {
                token_id: "proportion_profile".into(),
                value: "serial_arm_compact_service_display".into(),
            },
            ForgeVisualDesignToken {
                token_id: "surface_language".into(),
                value: "layered_blue_graphite_industrial".into(),
            },
            ForgeVisualDesignToken {
                token_id: "detail_density".into(),
                value: "collectible_high".into(),
            },
        ],
        parts,
        geometry_graph: candidate.expanded_shape_program.clone(),
        assembly_graph: candidate.expanded_assembly_graph.clone(),
        material_graph,
        surface_graph,
        detail_inventory: details,
        export_profile: ForgeVisualExportProfile::ProductionConcept,
    };
    let program_value = serde_json::to_value(&program)
        .map_err(|error| invalid(format!("program serialization failed: {error}")))?;
    let lowering = lower_forge_visual_program(&program_value)?;

    let (sealed_status, sealed_error_code) = if critical_unresolved_detail_ids.is_empty() {
        ("sealed_critical_details_complete".to_owned(), String::new())
    } else {
        let mut sealed_value = program_value;
        sealed_value["stage"] = Value::String("sealed".into());
        let sealed_error = lower_forge_visual_program(&sealed_value)
            .expect_err("C111 development inventory must keep an incomplete seal blocked");
        if sealed_error.code() != "FORGE_VISUAL_PROGRAM_INVALID" {
            return Err(invalid(
                "C111 fixture must expose its real critical visual blockers",
            ));
        }
        (
            "blocked_critical_details".to_owned(),
            sealed_error.code().to_owned(),
        )
    };

    let compiled_evidence = object(
        inventory_object
            .get("compiled_evidence")
            .ok_or_else(|| invalid("compiled_evidence is missing"))?,
        "compiled_evidence",
    )?;
    let fixed_views = array(
        compiled_evidence
            .get("fixed_views")
            .ok_or_else(|| invalid("compiled_evidence.fixed_views is missing"))?,
        "compiled_evidence.fixed_views",
    )?
    .clone();
    if fixed_views.len() != 8 {
        return Err(invalid("C111 fixture must bind exactly eight fixed views"));
    }
    let expected_production = serde_json::json!({
        "shape_program_sha256": compiled_evidence.get("shape_program_sha256"),
        "glb_sha256": compiled_evidence.get("production_glb_sha256"),
        "triangle_count": compiled_evidence.get("production_triangles"),
        "primitive_count": compiled_evidence.get("production_primitives"),
    });

    Ok(C111ForgeVisualProgramFixture {
        schema_version: C111_FORGE_VISUAL_PROGRAM_FIXTURE_SCHEMA_VERSION.into(),
        fixture_id: "fixture_pv002_c111_robotic_arm_iteration_70".into(),
        registry_id: registry.registry_id().into(),
        registry_sha256: registry.registry_sha256().into(),
        inventory_id: string(
            inventory_object
                .get("inventory_id")
                .ok_or_else(|| invalid("inventory_id is missing"))?,
            "inventory_id",
        )?
        .into(),
        inventory_semantic_sha256: semantic_sha256(inventory)?,
        program,
        lowering,
        surface_adornment_programs: surface_programs.to_vec(),
        fixed_views,
        expected_production,
        critical_unresolved_detail_ids,
        sealed_status,
        sealed_error_code,
    })
}
