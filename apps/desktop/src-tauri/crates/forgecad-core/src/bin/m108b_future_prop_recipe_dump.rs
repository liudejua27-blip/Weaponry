//! Source-pack-only validator bridge for future-prop M108B authoring.
//!
//! It intentionally accepts a temporary registry path so a parallel domain
//! author can exercise the actual Rust Recipe validator/expander before the
//! release owner rewrites the checked-in aggregate registry and lock.

use std::{env, fs};

use forgecad_core::{
    ComponentRecipeRef, RecipeExpander, RecipeExpansionPolicy, RecipeInstantiationRequest,
    RecipeRegistry, RecipeValidator,
};
use serde_json::{json, Value};

const ROOTS: [&str; 3] = [
    "recipe_prop_scout_compact",
    "recipe_prop_ceremonial_heavy",
    "recipe_prop_racing_streamlined",
];

fn request(registry: &RecipeRegistry, recipe_id: &str) -> RecipeInstantiationRequest {
    let recipe = registry
        .recipe(recipe_id)
        .expect("reviewed future-prop root");
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
        domain_pack_id: "pack_future_weapon_prop".into(),
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
    let path = env::args()
        .nth(1)
        .expect("temporary source-pack registry path");
    let contents = fs::read_to_string(path).expect("read temporary source-pack registry");
    let registry =
        RecipeRegistry::from_json(&contents).expect("validate source-pack RecipeRegistry");
    let candidates: Vec<Value> = ROOTS
        .into_iter()
        .map(|recipe_id| {
            serde_json::to_value(
                RecipeExpander::expand(
                    &registry,
                    &request(&registry, recipe_id),
                    &RecipeExpansionPolicy::default(),
                )
                .expect("expand future-prop source recipe"),
            )
            .expect("serialize candidate")
        })
        .collect();
    println!(
        "{}",
        forgecad_core::canonical_json(&json!({
            "schema_version": "M108BFuturePropSourcePackExpansion@1",
            "registry_sha256": registry.registry_sha256(),
            "candidates": candidates,
        }))
        .expect("canonical output")
    );
}
