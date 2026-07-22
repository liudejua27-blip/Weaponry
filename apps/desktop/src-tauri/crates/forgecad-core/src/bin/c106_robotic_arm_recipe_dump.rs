//! Read-only C106 mechanical-arm pack expansion for focused compile and Gate
//! checks. This binary has no repository, provider or product-state access.

use forgecad_core::{
    ComponentRecipeRef, RecipeExpander, RecipeExpansionPolicy, RecipeInstantiationRequest,
    RecipeRegistry, RecipeValidator,
};
use serde_json::{json, Value};

const ROOTS: [(&str, &str); 3] = [
    (
        "recipe_c106_arm_desktop_assistant",
        "pack_robotic_arm_concept",
    ),
    (
        "recipe_c106_arm_gallery_industrial",
        "pack_robotic_arm_concept",
    ),
    (
        "recipe_c106_arm_service_display",
        "pack_robotic_arm_concept",
    ),
];

fn request(
    registry: &RecipeRegistry,
    recipe_id: &str,
    domain_pack_id: &str,
) -> RecipeInstantiationRequest {
    let recipe = registry
        .recipe(recipe_id)
        .expect("reviewed C106 root recipe");
    RecipeInstantiationRequest {
        schema_version: "ComponentRecipeInstantiationRequest@1".into(),
        context_mode: "initial_candidate".into(),
        request_id: format!("recipereq_c106_{}", recipe_id.trim_start_matches("recipe_")),
        project_id: None,
        base_asset_version_id: None,
        snapshot_revision: None,
        domain_pack_id: domain_pack_id.into(),
        recipe_registry_sha256: registry.registry_sha256().into(),
        recipe: ComponentRecipeRef {
            schema_version: "ComponentRecipeRef@1".into(),
            recipe_id: recipe.recipe_id.clone(),
            version: recipe.version,
            recipe_sha256: RecipeValidator::recipe_sha256(recipe).expect("C106 recipe hash"),
        },
        target_part_id: None,
        slot_bindings: vec![],
        parameter_values: vec![],
        material_zone_overrides: vec![],
    }
}

fn main() {
    let registry = RecipeRegistry::from_embedded_c106_robotic_arm().expect("C106 arm registry");
    let candidates: Vec<Value> = ROOTS
        .into_iter()
        .map(|(recipe_id, domain_pack_id)| {
            serde_json::to_value(
                RecipeExpander::expand(
                    &registry,
                    &request(&registry, recipe_id, domain_pack_id),
                    &RecipeExpansionPolicy::default(),
                )
                .expect("C106 arm expansion"),
            )
            .expect("C106 candidate JSON")
        })
        .collect();
    let shape_program_seals = candidates
        .iter()
        .map(|candidate| {
            let recipe_id = candidate
                .pointer("/recipe/recipe_id")
                .and_then(Value::as_str)
                .expect("C106 recipe identity");
            let shape_program = candidate
                .get("expanded_shape_program")
                .expect("C106 expanded ShapeProgram");
            json!({
                "recipe_id": recipe_id,
                "shape_program_canonical_json": forgecad_core::canonical_json(shape_program)
                    .expect("canonical C106 ShapeProgram"),
                "shape_program_sha256": forgecad_core::semantic_sha256(shape_program)
                    .expect("C106 ShapeProgram hash"),
            })
        })
        .collect::<Vec<_>>();
    println!(
        "{}",
        forgecad_core::canonical_json(&json!({
            "schema_version": "C106RoboticArmRecipeExpansion@1",
            "registry_id": registry.registry_id(),
            "registry_sha256": registry.registry_sha256(),
            "candidates": candidates,
            "shape_program_seals": shape_program_seals,
        }))
        .expect("canonical C106 output")
    );
}
