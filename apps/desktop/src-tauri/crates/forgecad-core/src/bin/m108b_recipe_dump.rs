//! Deterministic, read-only bridge for the M108B fixture smoke.
//! It owns no product state and only emits Rust-expanded ShapePrograms.

use forgecad_core::{
    ComponentRecipeRef, RecipeExpander, RecipeExpansionPolicy, RecipeInstantiationRequest,
    RecipeRegistry, RecipeValidator,
};
use serde_json::{json, Value};

fn request(registry: &RecipeRegistry, recipe_id: &str, domain: &str) -> RecipeInstantiationRequest {
    let recipe = registry
        .recipe(recipe_id)
        .expect("reviewed production recipe");
    RecipeInstantiationRequest {
        schema_version: "ComponentRecipeInstantiationRequest@1".into(),
        context_mode: "initial_candidate".into(),
        request_id: format!(
            "recipereq_m108b_{}",
            recipe_id.trim_start_matches("recipe_")
        ),
        project_id: None,
        base_asset_version_id: None,
        snapshot_revision: None,
        domain_pack_id: domain.into(),
        recipe_registry_sha256: registry.registry_sha256().into(),
        recipe: ComponentRecipeRef {
            schema_version: "ComponentRecipeRef@1".into(),
            recipe_id: recipe.recipe_id.clone(),
            version: recipe.version,
            recipe_sha256: RecipeValidator::recipe_sha256(recipe).expect("recipe hash"),
        },
        target_part_id: None,
        slot_bindings: vec![],
        parameter_values: vec![],
        material_zone_overrides: vec![],
    }
}

fn main() {
    let registry = RecipeRegistry::from_embedded_production().expect("production registry");
    let roots = [
        ("recipe_prop_scout_compact", "pack_future_weapon_prop"),
        ("recipe_prop_ceremonial_heavy", "pack_future_weapon_prop"),
        ("recipe_prop_racing_streamlined", "pack_future_weapon_prop"),
        ("recipe_vehicle_compact_coupe", "pack_vehicle_concept"),
        ("recipe_vehicle_utility_crossover", "pack_vehicle_concept"),
        ("recipe_vehicle_track_concept", "pack_vehicle_concept"),
        (
            "recipe_aircraft_streamlined_personal",
            "pack_aircraft_concept",
        ),
        ("recipe_aircraft_explorer_tilt", "pack_aircraft_concept"),
        ("recipe_aircraft_cargo_display", "pack_aircraft_concept"),
        ("recipe_arm_desktop_assistant", "pack_robotic_arm_concept"),
        ("recipe_arm_gallery_industrial", "pack_robotic_arm_concept"),
        ("recipe_arm_service_display", "pack_robotic_arm_concept"),
    ];
    let candidates: Vec<Value> = roots
        .into_iter()
        .map(|(recipe_id, domain)| {
            serde_json::to_value(
                RecipeExpander::expand(
                    &registry,
                    &request(&registry, recipe_id, domain),
                    &RecipeExpansionPolicy::default(),
                )
                .expect("production expansion"),
            )
            .expect("candidate json")
        })
        .collect();
    println!(
        "{}",
        forgecad_core::canonical_json(&json!({
            "schema_version": "M108BProductionRecipeExpansion@1",
            "registry_id": registry.registry_id(),
            "registry_sha256": registry.registry_sha256(),
            "candidates": candidates,
        }))
        .expect("canonical output")
    );
}
