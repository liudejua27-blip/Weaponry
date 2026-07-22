use std::collections::{BTreeSet, HashSet};

use serde_json::Value;

use crate::{semantic_sha256, CoreError, CoreResult};

use super::{
    transform::{connector_frame, validate_transform},
    EditableComponentRecipe, RecipeExpansionPolicy, RecipeInstantiationRequest, RecipeRegistry,
};

const RUNTIME_OPERATIONS: &[&str] = &[
    "box",
    "cylinder",
    "capsule",
    "wedge",
    "profile",
    "extrude",
    "revolve",
    "loft",
    "sweep",
    "mirror",
    "array",
    "radial_array",
    "union",
    "subtract",
    "bevel_approx",
    "surface_panel",
];
const DOMAINS: &[&str] = &[
    "pack_future_weapon_prop",
    "pack_vehicle_concept",
    "pack_aircraft_concept",
    "pack_robotic_arm_concept",
];

pub struct RecipeValidator;

impl RecipeValidator {
    pub fn validate_registry(registry: &RecipeRegistry) -> CoreResult<()> {
        if registry.recipe_map().is_empty() {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_REGISTRY_EMPTY",
                "Component Recipe registry must contain reviewed entries.",
            ));
        }
        for recipe in registry.recipes() {
            Self::validate_recipe(recipe)?;
            for slot in &recipe.child_slots {
                let child = registry.recipe(&slot.child_recipe_id).ok_or_else(|| {
                    CoreError::invalid_data(
                        "COMPONENT_RECIPE_CHILD_MISSING",
                        "Child slot references a recipe outside the reviewed registry.",
                    )
                })?;
                if !recipe
                    .allowed_domains
                    .iter()
                    .all(|domain| child.allowed_domains.contains(domain))
                    || !child
                        .allowed_domains
                        .iter()
                        .all(|domain| recipe.allowed_domains.contains(domain))
                {
                    return Err(CoreError::invalid_data(
                        "COMPONENT_RECIPE_CHILD_DOMAIN_INCOMPATIBLE",
                        "Parent and child Recipe allowed_domains must match exactly.",
                    ));
                }
                if !slot.accepted_roles.contains(&child.component_role) {
                    return Err(CoreError::invalid_data(
                        "COMPONENT_RECIPE_CHILD_ROLE_INCOMPATIBLE",
                        "Child Recipe role is not accepted by its parent slot.",
                    ));
                }
                if !recipe
                    .connectors
                    .iter()
                    .any(|connector| connector.connector_id == slot.parent_connector_id)
                    || !child
                        .connectors
                        .iter()
                        .any(|connector| connector.connector_id == slot.child_connector_id)
                {
                    return Err(CoreError::invalid_data(
                        "COMPONENT_RECIPE_CONNECTOR_MISSING",
                        "Child slot must reference reviewed parent and child connectors.",
                    ));
                }
                validate_transform(&slot.parent_local_transform)?;
            }
        }
        detect_cycles(registry)?;
        Ok(())
    }

    pub fn validate_request(
        registry: &RecipeRegistry,
        request: &RecipeInstantiationRequest,
        policy: &RecipeExpansionPolicy,
    ) -> CoreResult<()> {
        if request.schema_version != "ComponentRecipeInstantiationRequest@1" {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_REQUEST_INVALID",
                "Component Recipe request must use ComponentRecipeInstantiationRequest@1.",
            ));
        }
        validate_policy(policy)?;
        if !valid_prefixed_id(&request.request_id, "recipereq_")
            || request.recipe.schema_version != "ComponentRecipeRef@1"
            || request.recipe.recipe_sha256.len() != 64
            || request.recipe_registry_sha256.len() != 64
            || !request
                .recipe_registry_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_REQUEST_INVALID",
                "Component Recipe request identity or immutable recipe reference is invalid.",
            ));
        }
        if request.recipe_registry_sha256 != registry.registry_sha256() {
            return Err(CoreError::conflict(
                "COMPONENT_RECIPE_REGISTRY_STALE",
                "Recipe request registry identity does not match the selected reviewed catalog.",
            ));
        }
        match request.context_mode.as_str() {
            "initial_candidate" if request.project_id.is_none()
                && request.base_asset_version_id.is_none()
                && request.snapshot_revision.is_none()
                && request.target_part_id.is_none() => {}
            "active_asset_edit" if request.project_id.as_deref().is_some_and(|id| valid_prefixed_id(id, "prj_"))
                && request.base_asset_version_id.as_deref().is_some_and(|id| valid_prefixed_id(id, "assetver_"))
                && request.snapshot_revision.is_some_and(|revision| revision >= 1) => {}
            _ => return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_CONTEXT_INVALID",
                "initial_candidate may not forge an asset context; active_asset_edit requires project/base/revision and may target one part.",
            )),
        }
        if request
            .target_part_id
            .as_deref()
            .is_some_and(|part_id| !valid_prefixed_id(part_id, "part_"))
        {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_REQUEST_INVALID",
                "Recipe target_part_id must be a stable part ID or null.",
            ));
        }
        let recipe = registry
            .recipe(&request.recipe.recipe_id)
            .ok_or_else(|| CoreError::not_found("reviewed Component Recipe"))?;
        if recipe.version != request.recipe.version
            || Self::recipe_sha256(recipe)? != request.recipe.recipe_sha256
        {
            return Err(CoreError::conflict(
                "COMPONENT_RECIPE_REFERENCE_STALE",
                "Recipe reference does not match the reviewed registry entry.",
            ));
        }
        if !recipe
            .allowed_domains
            .iter()
            .any(|domain| domain == &request.domain_pack_id)
        {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_DOMAIN_INCOMPATIBLE",
                "The requested recipe is not allowed in this domain pack.",
            ));
        }
        for parameter in &request.parameter_values {
            if !valid_prefixed_id(&parameter.parameter_id, "editparam_")
                || !parameter.value.is_finite()
            {
                return Err(CoreError::invalid_data(
                    "COMPONENT_RECIPE_PARAMETER_INVALID",
                    "Recipe parameter values must be bounded IDs and finite numbers.",
                ));
            }
        }
        if !request.parameter_values.is_empty() {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_PARAMETER_UNSUPPORTED",
                "C105 v1 rejects non-empty parameter_values until a reviewed binding can be applied to the exact ShapeProgram template.",
            ));
        }
        for override_value in &request.material_zone_overrides {
            let domain = domain_for_pack(&request.domain_pack_id)
                .expect("domain checked by Recipe registry");
            if !valid_prefixed_id(&override_value.zone_id, "zone_")
                || !valid_prefixed_id(&override_value.material_preset_id, "mat_")
                || !recipe.material_zones.iter().any(|zone| {
                    zone.get("zone_id").and_then(Value::as_str) == Some(&override_value.zone_id)
                })
                || !crate::repository::material_allowed_domains(&override_value.material_preset_id)
                    .is_some_and(|allowed| allowed.contains(&domain))
            {
                return Err(CoreError::invalid_data(
                    "COMPONENT_RECIPE_ZONE_OVERRIDE_INVALID",
                    "Material zone overrides must target a declared stable Recipe zone.",
                ));
            }
        }
        if !request.material_zone_overrides.is_empty() {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_ZONE_OVERRIDE_UNSUPPORTED",
                "C105 v1 rejects material overrides rather than silently changing recipe provenance outside ChangeSet preview.",
            ));
        }
        let mut bound_slot_ids = BTreeSet::new();
        for binding in &request.slot_bindings {
            if !bound_slot_ids.insert(binding.slot_id.as_str()) {
                return Err(CoreError::invalid_data(
                    "COMPONENT_RECIPE_SLOT_BINDING_DUPLICATE",
                    "An optional reviewed child slot may be enabled at most once.",
                ));
            }
            let slot = recipe
                .child_slots
                .iter()
                .find(|slot| slot.slot_id == binding.slot_id)
                .ok_or_else(|| {
                    CoreError::invalid_data(
                        "COMPONENT_RECIPE_SLOT_BINDING_INVALID",
                        "Slot binding is not declared by the root Recipe.",
                    )
                })?;
            if slot.required {
                return Err(CoreError::invalid_data(
                    "COMPONENT_RECIPE_SLOT_BINDING_INVALID",
                    "Only an optional reviewed child slot may be explicitly enabled.",
                ));
            }
            let child = registry
                .recipe(&binding.child_recipe.recipe_id)
                .ok_or_else(|| CoreError::not_found("reviewed child Component Recipe"))?;
            if binding.child_recipe.schema_version != "ComponentRecipeRef@1"
                || binding.child_recipe.recipe_id != slot.child_recipe_id
                || binding.child_recipe.version != child.version
                || binding.child_recipe.recipe_sha256 != Self::recipe_sha256(child)?
            {
                return Err(CoreError::conflict(
                    "COMPONENT_RECIPE_SLOT_BINDING_STALE",
                    "Slot binding does not match the reviewed child Recipe reference.",
                ));
            }
        }
        Ok(())
    }

    pub fn recipe_sha256(recipe: &EditableComponentRecipe) -> CoreResult<String> {
        semantic_sha256(recipe)
    }

    fn validate_recipe(recipe: &EditableComponentRecipe) -> CoreResult<()> {
        if recipe.schema_version != "EditableComponentRecipe@1"
            || !valid_prefixed_id(&recipe.recipe_id, "recipe_")
            || !valid_identifier(&recipe.component_role)
            || recipe.version == 0
            || recipe.display_name.is_empty()
            || recipe.display_name.len() > 80
            || recipe.description.is_empty()
            || recipe.description.len() > 400
            || !recipe.non_functional_only
        {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_INVALID",
                "Recipe identity or visual-only contract is invalid.",
            ));
        }
        if recipe.feature_template.is_empty()
            || recipe.feature_template.len() > 32
            || recipe.connectors.is_empty()
            || recipe.connectors.len() > 12
            || recipe.material_zones.is_empty()
            || recipe.material_zones.len() > 12
            || recipe.surface_adornment_slots.len() > 8
            || recipe.profiles.len() > 12
            || recipe.section_sets.len() > 12
            || recipe.child_slots.len() > 12
            || recipe.parameter_bindings.len() > 16
            || recipe.triangle_estimate < 100
            || recipe.triangle_estimate > 100_000
        {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_BUDGET_INVALID",
                "Recipe exceeds the C105 bounded component budget.",
            ));
        }
        let domains = recipe.allowed_domains.iter().collect::<BTreeSet<_>>();
        if domains.is_empty()
            || domains.len() != recipe.allowed_domains.len()
            || !recipe
                .allowed_domains
                .iter()
                .all(|value| DOMAINS.contains(&value.as_str()))
        {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_DOMAIN_INVALID",
                "Recipe allowed_domains must be unique registered domain packs.",
            ));
        }
        if recipe.quality_status != "passed"
            || recipe.source.get("source_kind").and_then(Value::as_str)
                != Some("forgecad_first_party")
            || !recipe
                .source
                .get("source_id")
                .and_then(Value::as_str)
                .is_some_and(|id| valid_prefixed_id(id, "source_"))
            || recipe.license.get("license_id").and_then(Value::as_str)
                != Some("ForgeCAD-Internal-Visual-Only")
            || recipe
                .license
                .get("redistributable")
                .and_then(Value::as_bool)
                != Some(false)
            || !recipe
                .review_state
                .get("review_id")
                .and_then(Value::as_str)
                .is_some_and(|id| valid_prefixed_id(id, "review_"))
            || recipe
                .review_state
                .get("reviewer_kind")
                .and_then(Value::as_str)
                != Some("forgecad_internal")
            || recipe
                .review_state
                .get("reviewed_at")
                .and_then(Value::as_str)
                .filter(|date| !date.is_empty())
                .is_none()
        {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_REVIEW_REQUIRED",
                "Only reviewed, passed, first-party visual-only Recipes with an allowed license may enter the registry.",
            ));
        }
        validate_transform(&recipe.root_local_transform)?;
        connector_frame(recipe.pivot.position, recipe.pivot.normal, recipe.pivot.up)?;
        let mut connector_ids = HashSet::new();
        for connector in &recipe.connectors {
            if !valid_prefixed_id(&connector.connector_id, "connector_")
                || !matches!(
                    connector.kind.as_str(),
                    "visual_mount" | "shell_join" | "trim_join" | "display_support"
                )
                || !connector_ids.insert(&connector.connector_id)
            {
                return Err(CoreError::invalid_data(
                    "COMPONENT_RECIPE_CONNECTOR_INVALID",
                    "Recipe connector IDs must be unique, reviewed and from the bounded visual catalog.",
                ));
            }
            connector_frame(connector.position, connector.normal, connector.up)?;
        }
        let mut slot_ids = HashSet::new();
        for slot in &recipe.child_slots {
            if !valid_prefixed_id(&slot.slot_id, "slot_")
                || slot.count == 0
                || slot.count > 16
                || slot.accepted_roles.is_empty()
                || slot.accepted_roles.len() > 8
                || slot
                    .accepted_roles
                    .iter()
                    .any(|role| !valid_identifier(role))
                || !slot_ids.insert(&slot.slot_id)
            {
                return Err(CoreError::invalid_data(
                    "COMPONENT_RECIPE_CHILD_SLOT_INVALID",
                    "Recipe child slots must be bounded and have unique stable IDs.",
                ));
            }
        }
        let material_zone_ids = recipe
            .material_zones
            .iter()
            .filter_map(|zone| zone.get("zone_id").and_then(Value::as_str))
            .collect::<BTreeSet<_>>();
        let mut adornment_slot_ids = HashSet::new();
        for slot in &recipe.surface_adornment_slots {
            let motifs_match_kinds = slot.allowed_kinds.iter().any(|kind| {
                slot.allowed_motifs.iter().any(|motif| {
                    matches!(
                        (kind.as_str(), motif.as_str()),
                        ("normal_relief", "parallel_groove" | "chevron_relief")
                            | ("pattern", "hex_microgrid")
                            | ("flowline", "double_flowline")
                            | ("micro_surface", "hex_microgrid" | "parallel_groove")
                    )
                })
            });
            if !valid_prefixed_id(&slot.slot_id, "adornslot_")
                || !valid_prefixed_id(&slot.zone_id, "zone_")
                || !adornment_slot_ids.insert(&slot.slot_id)
                || !material_zone_ids.contains(slot.zone_id.as_str())
                || slot.allowed_kinds.is_empty()
                || slot.allowed_kinds.len() > 4
                || slot.allowed_kinds.iter().collect::<BTreeSet<_>>().len()
                    != slot.allowed_kinds.len()
                || !slot.allowed_kinds.iter().all(|kind| {
                    matches!(
                        kind.as_str(),
                        "normal_relief" | "pattern" | "flowline" | "micro_surface"
                    )
                })
                || slot.allowed_motifs.is_empty()
                || slot.allowed_motifs.len() > 4
                || slot.allowed_motifs.iter().collect::<BTreeSet<_>>().len()
                    != slot.allowed_motifs.len()
                || !slot.allowed_motifs.iter().all(|motif| {
                    matches!(
                        motif.as_str(),
                        "parallel_groove" | "chevron_relief" | "double_flowline" | "hex_microgrid"
                    )
                })
                || !motifs_match_kinds
                || slot.allowed_coverages.is_empty()
                || slot.allowed_coverages.len() > 4
                || slot.allowed_coverages.iter().collect::<BTreeSet<_>>().len()
                    != slot.allowed_coverages.len()
                || !slot.allowed_coverages.iter().all(|coverage| {
                    matches!(
                        coverage.as_str(),
                        "full_zone" | "center_band" | "edge_band" | "symmetric_pair"
                    )
                })
            {
                return Err(CoreError::invalid_data(
                    "COMPONENT_RECIPE_SURFACE_ADORNMENT_SLOT_INVALID",
                    "Surface adornment slots must be unique bounded A005 choices on a declared Recipe material zone.",
                ));
            }
        }
        let template_operation_ids = recipe
            .shape_program_template
            .get("operations")
            .and_then(Value::as_array)
            .map(|operations| {
                operations
                    .iter()
                    .filter_map(|operation| operation.get("operation_id").and_then(Value::as_str))
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        for feature in &recipe.feature_template {
            if !feature
                .get("feature_id")
                .and_then(Value::as_str)
                .is_some_and(|id| valid_prefixed_id(id, "feature_"))
                || !feature
                    .get("operation_id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| template_operation_ids.contains(id))
                || !feature
                    .get("role")
                    .and_then(Value::as_str)
                    .is_some_and(valid_identifier)
            {
                return Err(CoreError::invalid_data(
                    "COMPONENT_RECIPE_FEATURE_BINDING_INVALID",
                    "Recipe feature_template must bind stable features to an executable reviewed ShapeProgram operation.",
                ));
            }
            validate_safe_json(feature)?;
        }
        let mut parameter_ids = BTreeSet::new();
        for binding in &recipe.parameter_bindings {
            let parameter_id = binding.get("parameter_id").and_then(Value::as_str);
            let path = binding.get("path").and_then(Value::as_str);
            let default = binding.get("default").and_then(Value::as_f64);
            let minimum = binding.get("min").and_then(Value::as_f64);
            let maximum = binding.get("max").and_then(Value::as_f64);
            let step = binding.get("step").and_then(Value::as_f64);
            if binding.get("schema_version").and_then(Value::as_str)
                != Some("EditableParameterBinding@1")
                || !parameter_id.is_some_and(|id| valid_prefixed_id(id, "editparam_"))
                || !parameter_id.is_some_and(|id| parameter_ids.insert(id))
                || !matches!(
                    path,
                    Some("transform.scale.x" | "transform.scale.y" | "transform.scale.z")
                )
                || binding.get("unit").and_then(Value::as_str) != Some("ratio")
                || !binding
                    .get("display_name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| !name.is_empty() && name.len() <= 60)
                || !bounded_ratio(default, minimum, maximum, step)
            {
                return Err(CoreError::invalid_data(
                    "COMPONENT_RECIPE_PARAMETER_BINDING_INVALID",
                    "Recipe parameter bindings must reuse the frozen G808 bounded ratio contract.",
                ));
            }
        }
        validate_shape_program_template(&recipe.shape_program_template)?;
        for value in recipe
            .profiles
            .iter()
            .chain(recipe.section_sets.iter())
            .chain(recipe.parameter_bindings.iter())
            .chain(recipe.material_zones.iter())
            .chain(
                recipe
                    .surface_adornment_slots
                    .iter()
                    .map(|slot| {
                        // The typed slot has already rejected arbitrary fields.  This
                        // JSON conversion keeps the shared no-URL/path/code audit.
                        // Serialization cannot fail for this closed data type.
                        serde_json::to_value(slot).expect("surface adornment slot serializes")
                    })
                    .collect::<Vec<_>>()
                    .iter(),
            )
            .chain([&recipe.source, &recipe.license, &recipe.review_state])
        {
            validate_safe_json(value)?;
        }
        Ok(())
    }
}

fn validate_shape_program_template(template: &Value) -> CoreResult<()> {
    let object = template.as_object().ok_or_else(|| {
        CoreError::invalid_data(
            "COMPONENT_RECIPE_EXECUTION_TEMPLATE_INCOMPLETE",
            "Recipe must contain a complete ShapeProgram@1 template.",
        )
    })?;
    if object.get("schema_version").and_then(Value::as_str) != Some("ShapeProgram@1")
        || object.get("units").and_then(Value::as_str) != Some("millimeter")
        || object.get("non_functional_only").and_then(Value::as_bool) != Some(true)
        || !object
            .get("program_id")
            .and_then(Value::as_str)
            .is_some_and(|id| valid_prefixed_id(id, "shape_"))
        || object
            .get("triangle_budget")
            .and_then(Value::as_u64)
            .filter(|budget| (100..=100_000).contains(budget))
            .is_none()
    {
        return Err(CoreError::invalid_data(
            "COMPONENT_RECIPE_EXECUTION_TEMPLATE_INCOMPLETE",
            "Recipe ShapeProgram template must be a bounded visual-only ShapeProgram@1.",
        ));
    }
    let operations = object
        .get("operations")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| {
            CoreError::invalid_data(
                "COMPONENT_RECIPE_EXECUTION_TEMPLATE_INCOMPLETE",
                "Recipe ShapeProgram template needs executable operations.",
            )
        })?;
    let mut operation_ids = BTreeSet::new();
    for operation in operations {
        let operation_id = operation
            .get("operation_id")
            .and_then(Value::as_str)
            .filter(|id| valid_prefixed_id(id, "op_"))
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "COMPONENT_RECIPE_EXECUTION_TEMPLATE_INCOMPLETE",
                    "Template operations require stable op_ IDs.",
                )
            })?;
        if !operation_ids.insert(operation_id)
            || !operation
                .get("op")
                .and_then(Value::as_str)
                .is_some_and(|op| RUNTIME_OPERATIONS.contains(&op))
            || operation.get("args").and_then(Value::as_object).is_none()
            || !operation
                .get("inputs")
                .and_then(Value::as_array)
                .is_some_and(|inputs| {
                    inputs
                        .iter()
                        .all(|input| input.as_str().is_some_and(|id| id.starts_with("op_")))
                })
        {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_EXECUTION_TEMPLATE_INCOMPLETE",
                "Template operations must use complete bounded args and only resolved op_ inputs.",
            ));
        }
        validate_safe_json(operation)?;
    }
    let outputs = object
        .get("outputs")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| {
            CoreError::invalid_data(
                "COMPONENT_RECIPE_EXECUTION_TEMPLATE_INCOMPLETE",
                "Recipe ShapeProgram template needs output declarations.",
            )
        })?;
    for output in outputs {
        if !output
            .get("output_id")
            .and_then(Value::as_str)
            .is_some_and(|id| valid_prefixed_id(id, "output_"))
            || !output
                .get("operation_id")
                .and_then(Value::as_str)
                .is_some_and(|id| operation_ids.contains(id))
            || output.get("kind").and_then(Value::as_str) != Some("mesh")
            || !output
                .get("part_role")
                .and_then(Value::as_str)
                .is_some_and(valid_identifier)
        {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_EXECUTION_TEMPLATE_INCOMPLETE",
                "Template output declarations must reference an executable mesh operation.",
            ));
        }
    }
    for input in object
        .get("profile_inputs")
        .and_then(Value::as_array)
        .unwrap_or(&Vec::new())
    {
        let payload = input.get("canonical_payload").ok_or_else(|| {
            CoreError::invalid_data(
                "COMPONENT_RECIPE_EXECUTION_TEMPLATE_INCOMPLETE",
                "Template profile input needs an immutable canonical payload.",
            )
        })?;
        let expected = input
            .get("input_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "COMPONENT_RECIPE_EXECUTION_TEMPLATE_INCOMPLETE",
                    "Template profile input needs its canonical SHA-256.",
                )
            })?;
        if expected.len() != 64 || semantic_sha256(payload)? != expected {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_TEMPLATE_HASH_MISMATCH",
                "Template profile input hash does not match its canonical payload.",
            ));
        }
    }
    Ok(())
}

fn validate_policy(policy: &RecipeExpansionPolicy) -> CoreResult<()> {
    if policy.schema_version != "RecipeExpansionPolicy@1"
        || policy.max_depth == 0
        || policy.max_depth > 6
        || policy.max_instances == 0
        || policy.max_instances > 64
        || policy.max_operations == 0
        || policy.max_operations > 512
        || policy.max_profiles == 0
        || policy.max_profiles > 128
        || policy.max_sections == 0
        || policy.max_sections > 512
        || policy.max_material_zones == 0
        || policy.max_material_zones > 128
        || policy.max_triangles == 0
        || policy.max_triangles > 100_000
    {
        return Err(CoreError::invalid_data(
            "COMPONENT_RECIPE_POLICY_INVALID",
            "Component Recipe expansion policy may only use bounded C105 v1 caps.",
        ));
    }
    Ok(())
}

fn detect_cycles(registry: &RecipeRegistry) -> CoreResult<()> {
    fn visit(
        registry: &RecipeRegistry,
        recipe_id: &str,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
    ) -> CoreResult<()> {
        if visited.contains(recipe_id) {
            return Ok(());
        }
        if !visiting.insert(recipe_id.to_owned()) {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_CYCLE",
                "Component Recipe child slots must form an acyclic graph.",
            ));
        }
        let recipe = registry
            .recipe(recipe_id)
            .ok_or_else(|| CoreError::not_found("Component Recipe"))?;
        for slot in &recipe.child_slots {
            visit(registry, &slot.child_recipe_id, visiting, visited)?;
        }
        visiting.remove(recipe_id);
        visited.insert(recipe_id.to_owned());
        Ok(())
    }
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for recipe in registry.recipes() {
        visit(registry, &recipe.recipe_id, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn validate_safe_json(value: &Value) -> CoreResult<()> {
    match value {
        Value::Null | Value::Bool(_) => Ok(()),
        Value::Number(number) if number.as_f64().is_some_and(f64::is_finite) => Ok(()),
        Value::Number(_) => Err(CoreError::invalid_data(
            "COMPONENT_RECIPE_NUMBER_INVALID",
            "Recipe numbers must be finite.",
        )),
        Value::String(text) => {
            let lowered = text.to_ascii_lowercase();
            if lowered.contains("://")
                || lowered.starts_with("file:")
                || lowered.contains("../")
                || lowered.starts_with('/')
                || lowered.contains('\\')
                || lowered.contains("<script")
                || lowered.contains("javascript:")
            {
                return Err(CoreError::invalid_data(
                    "COMPONENT_RECIPE_UNSAFE_CONTENT",
                    "Recipes cannot contain executable text, URLs or filesystem paths.",
                ));
            }
            Ok(())
        }
        Value::Array(values) => values.iter().try_for_each(validate_safe_json),
        Value::Object(values) => values.values().try_for_each(validate_safe_json),
    }
}

fn valid_prefixed_id(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix.len() <= 120
            && suffix.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            })
    })
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn domain_for_pack(pack_id: &str) -> Option<&'static str> {
    match pack_id {
        "pack_future_weapon_prop" => Some("future_weapon_prop"),
        "pack_vehicle_concept" => Some("vehicle_concept"),
        "pack_aircraft_concept" => Some("aircraft_concept"),
        "pack_robotic_arm_concept" => Some("robotic_arm_concept"),
        _ => None,
    }
}

fn bounded_ratio(
    default: Option<f64>,
    minimum: Option<f64>,
    maximum: Option<f64>,
    step: Option<f64>,
) -> bool {
    match (default, minimum, maximum, step) {
        (Some(default), Some(minimum), Some(maximum), Some(step)) => {
            default.is_finite()
                && minimum.is_finite()
                && maximum.is_finite()
                && step.is_finite()
                && (0.1..=10.0).contains(&minimum)
                && maximum >= minimum
                && maximum <= 10.0
                && (minimum..=maximum).contains(&default)
                && step > 0.0
                && step <= maximum - minimum
        }
        _ => false,
    }
}
