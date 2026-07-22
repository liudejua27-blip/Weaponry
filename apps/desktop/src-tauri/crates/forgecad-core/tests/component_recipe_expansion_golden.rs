use std::{env, fs, path::PathBuf};

use forgecad_core::{
    ComponentRecipeRef, RecipeExpander, RecipeExpansionPolicy, RecipeInstantiationRequest,
    RecipeRegistry, RecipeSlotBinding,
};
use serde_json::{json, Value};

const GOLDEN_SCHEMA: &str = "ComponentRecipeExpansionGolden@1";

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../../packages/concept-spec/fixtures/component-recipe-expanded-golden.json")
}

fn initial_request(
    registry: &RecipeRegistry,
    recipe_id: &str,
    domain_pack_id: &str,
) -> RecipeInstantiationRequest {
    let recipe = registry.recipe(recipe_id).unwrap();
    let mut request = RecipeInstantiationRequest {
        schema_version: "ComponentRecipeInstantiationRequest@1".into(),
        context_mode: "initial_candidate".into(),
        request_id: format!(
            "recipereq_golden_{}",
            recipe_id.trim_start_matches("recipe_")
        ),
        project_id: None,
        base_asset_version_id: None,
        snapshot_revision: None,
        domain_pack_id: domain_pack_id.into(),
        recipe_registry_sha256: registry.registry_sha256().into(),
        recipe: ComponentRecipeRef {
            schema_version: "ComponentRecipeRef@1".into(),
            recipe_id: recipe.recipe_id.clone(),
            version: recipe.version,
            recipe_sha256: forgecad_core::RecipeValidator::recipe_sha256(recipe).unwrap(),
        },
        target_part_id: None,
        slot_bindings: vec![],
        parameter_values: vec![],
        material_zone_overrides: vec![],
    };
    if let Some(slot) = recipe.child_slots.iter().find(|slot| !slot.required) {
        let child = registry.recipe(&slot.child_recipe_id).unwrap();
        request.slot_bindings.push(RecipeSlotBinding {
            slot_id: slot.slot_id.clone(),
            child_recipe: ComponentRecipeRef {
                schema_version: "ComponentRecipeRef@1".into(),
                recipe_id: child.recipe_id.clone(),
                version: child.version,
                recipe_sha256: forgecad_core::RecipeValidator::recipe_sha256(child).unwrap(),
            },
        });
    }
    request
}

fn generated_golden() -> Value {
    let registry = RecipeRegistry::from_embedded().unwrap();
    let candidates = [
        ("recipe_future_prop_shell", "pack_future_weapon_prop"),
        ("recipe_vehicle_body_shell", "pack_vehicle_concept"),
        ("recipe_aircraft_fuselage", "pack_aircraft_concept"),
        ("recipe_robotic_arm_link", "pack_robotic_arm_concept"),
    ]
    .into_iter()
    .map(|(recipe_id, domain)| {
        serde_json::to_value(
            RecipeExpander::expand(
                &registry,
                &initial_request(&registry, recipe_id, domain),
                &RecipeExpansionPolicy::default(),
            )
            .unwrap(),
        )
        .unwrap()
    })
    .collect::<Vec<_>>();
    json!({
        "schema_version": GOLDEN_SCHEMA,
        "registry_sha256": registry.registry_sha256(),
        "candidate_hash_scope": "semantic_sha256(ComponentRecipeCandidate@1 with candidate_sha256 blank; transient in-memory instances omitted)",
        "candidates": candidates,
    })
}

#[test]
fn c105_four_domain_expansion_golden_is_rust_owned_and_deterministic() {
    let generated = generated_golden();
    let path = golden_path();
    if env::var("FORGECAD_REWRITE_COMPONENT_RECIPE_GOLDEN")
        .ok()
        .as_deref()
        == Some("1")
    {
        fs::write(
            &path,
            format!("{}\n", forgecad_core::canonical_json(&generated).unwrap()),
        )
        .unwrap();
    }
    let persisted: Value = serde_json::from_str(&fs::read_to_string(&path).expect(
        "run with FORGECAD_REWRITE_COMPONENT_RECIPE_GOLDEN=1 once to create the Rust-owned golden",
    ))
    .unwrap();
    assert_eq!(
        persisted, generated,
        "golden drift: regenerate only through the Rust expander"
    );
    for candidate in persisted["candidates"].as_array().unwrap() {
        let candidate: forgecad_core::ExpandedComponentCandidate =
            serde_json::from_value(candidate.clone()).unwrap();
        assert_eq!(candidate.context_mode, "initial_candidate");
        assert!(
            candidate.project_id.is_none()
                && candidate.base_asset_version_id.is_none()
                && candidate.snapshot_revision.is_none()
                && candidate.target_part_id.is_none()
        );
        assert_eq!(
            candidate.candidate_sha256,
            RecipeExpander::candidate_sha256(&candidate).unwrap()
        );
        assert_eq!(candidate.status, "expanded");
        for part in candidate.expanded_assembly_graph["parts"]
            .as_array()
            .unwrap()
        {
            assert!(
                part["pivot"]["up"].is_array(),
                "recipe-backed pivots must preserve local roll"
            );
            for connector in part["connectors"].as_array().unwrap() {
                assert!(
                    connector["up"].is_array(),
                    "recipe-backed connectors must preserve local roll"
                );
            }
        }
    }
}
