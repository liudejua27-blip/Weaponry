//! Read-only C111A golden-surface arm expansion for focused visual compile
//! and fixed-view evidence.  This binary has no provider, repository, or
//! product-state write access.

use forgecad_core::{
    build_c111_forge_visual_program_fixture, builtin_surface_adornment_manifest_v3,
    c111_golden_surface_adornment_programs, ComponentRecipeRef, RecipeExpander,
    RecipeExpansionPolicy, RecipeInstantiationRequest, RecipeRegistry, RecipeValidator,
};
use serde_json::json;

const ROOT_RECIPE_ID: &str = "recipe_c111_arm_golden_surface";
const DOMAIN_PACK_ID: &str = "pack_robotic_arm_concept";
const DETAIL_INVENTORY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/fixtures/c111-golden-surface-robotic-arm-visual-detail-inventory.json"
));

fn main() {
    let registry = RecipeRegistry::from_embedded_c111_golden_surface_robotic_arm()
        .expect("C111A golden-surface arm registry");
    let recipe = registry
        .recipe(ROOT_RECIPE_ID)
        .expect("reviewed C111A root recipe");
    let request = RecipeInstantiationRequest {
        schema_version: "ComponentRecipeInstantiationRequest@1".into(),
        context_mode: "initial_candidate".into(),
        request_id: "recipereq_c111_golden_surface".into(),
        project_id: None,
        base_asset_version_id: None,
        snapshot_revision: None,
        domain_pack_id: DOMAIN_PACK_ID.into(),
        recipe_registry_sha256: registry.registry_sha256().into(),
        recipe: ComponentRecipeRef {
            schema_version: "ComponentRecipeRef@1".into(),
            recipe_id: recipe.recipe_id.clone(),
            version: recipe.version,
            recipe_sha256: RecipeValidator::recipe_sha256(recipe).expect("C111A recipe hash"),
        },
        target_part_id: None,
        slot_bindings: vec![],
        parameter_values: vec![],
        material_zone_overrides: vec![],
    };
    let candidate = RecipeExpander::expand(&registry, &request, &RecipeExpansionPolicy::default())
        .expect("C111A arm expansion");
    let surface_adornment_programs = c111_golden_surface_adornment_programs(&candidate, &registry)
        .expect("C111A reviewed surface programs");
    let inventory: serde_json::Value =
        serde_json::from_str(DETAIL_INVENTORY).expect("C111A detail inventory JSON");
    let forge_visual_program_fixture = build_c111_forge_visual_program_fixture(
        &candidate,
        &registry,
        &surface_adornment_programs,
        &inventory,
    )
    .expect("C111A ForgeVisualProgram development fixture");
    let candidate_value = serde_json::to_value(candidate).expect("C111A candidate JSON");
    let skill = builtin_surface_adornment_manifest_v3();
    skill.validate().expect("C111A A005 v3 manifest");
    let skill_sha256 = skill.canonical_sha256().expect("C111A A005 v3 hash");
    let shape_program = candidate_value
        .get("expanded_shape_program")
        .expect("C111A expanded ShapeProgram");
    println!(
        "{}",
        forgecad_core::canonical_json(&json!({
            "schema_version": "C111GoldenSurfaceRecipeExpansion@1",
            "registry_id": registry.registry_id(),
            "registry_sha256": registry.registry_sha256(),
            "candidate": candidate_value,
            "surface_adornment_manifest": {
                "skill_id": skill.skill_id,
                "skill_version": skill.version,
                "skill_sha256": skill_sha256,
            },
            "surface_adornment_programs": surface_adornment_programs,
            "forge_visual_program_fixture": forge_visual_program_fixture,
            "shape_program_canonical_json": forgecad_core::canonical_json(shape_program)
                .expect("canonical C111A ShapeProgram"),
            "shape_program_sha256": forgecad_core::semantic_sha256(shape_program)
                .expect("C111A ShapeProgram hash"),
        }))
        .expect("canonical C111A output")
    );
}
