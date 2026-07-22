use forgecad_core::{
    ComponentRecipeRef, RecipeExpander, RecipeExpansionPolicy, RecipeInstantiationRequest,
    RecipeRegistry, RecipeValidator,
};

fn transform_point(matrix: [[f64; 4]; 4], point: [f64; 3]) -> [f64; 3] {
    [
        matrix[0][0] * point[0] + matrix[0][1] * point[1] + matrix[0][2] * point[2] + matrix[0][3],
        matrix[1][0] * point[0] + matrix[1][1] * point[1] + matrix[1][2] * point[2] + matrix[1][3],
        matrix[2][0] * point[0] + matrix[2][1] * point[1] + matrix[2][2] * point[2] + matrix[2][3],
    ]
}

fn assert_near(actual: [f64; 3], expected: [f64; 3], recipe_id: &str, slot_id: &str) {
    for axis in 0..3 {
        assert!(
            (actual[axis] - expected[axis]).abs() <= 1.0e-7,
            "{recipe_id}:{slot_id}: axis {axis}: {actual:?} != {expected:?}"
        );
    }
}

fn request(
    registry: &RecipeRegistry,
    recipe_id: &str,
    domain_pack_id: &str,
) -> RecipeInstantiationRequest {
    let recipe = registry
        .recipe(recipe_id)
        .expect("reviewed production root");
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
        domain_pack_id: domain_pack_id.into(),
        recipe_registry_sha256: registry.registry_sha256().into(),
        recipe: ComponentRecipeRef {
            schema_version: "ComponentRecipeRef@1".into(),
            recipe_id: recipe.recipe_id.clone(),
            version: recipe.version,
            recipe_sha256: RecipeValidator::recipe_sha256(recipe).unwrap(),
        },
        target_part_id: None,
        slot_bindings: vec![],
        parameter_values: vec![],
        material_zone_overrides: vec![],
    }
}

#[test]
fn m108b_production_registry_is_separate_from_c105_and_expands_twelve_roots() {
    let c105 = RecipeRegistry::from_embedded().unwrap();
    let production = RecipeRegistry::from_embedded_production().unwrap();
    assert_eq!(c105.recipes().count(), 8);
    assert_eq!(
        production.registry_id(),
        "registry_m108b_production_concept_v1"
    );
    assert_ne!(production.registry_sha256(), c105.registry_sha256());
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
    for (recipe_id, domain) in roots {
        let root = production.recipe(recipe_id).unwrap();
        let expected_root_slots = if domain == "pack_aircraft_concept" {
            7
        } else {
            6
        };
        let expected_root_zones =
            if matches!(domain, "pack_vehicle_concept" | "pack_robotic_arm_concept") {
                5
            } else {
                4
            };
        let expected_instances = expected_root_slots + 1;
        assert_eq!(root.child_slots.len(), expected_root_slots, "{recipe_id}");
        assert_eq!(
            root.material_zones.len(),
            expected_root_zones,
            "{recipe_id}"
        );
        assert!(root.child_slots.iter().all(|slot| slot.required));
        assert!(
            root.child_slots
                .iter()
                .all(|slot| slot.parent_local_transform.position == [0.0, 0.0, 0.0]),
            "{recipe_id}: benchmark root slots must not duplicate parent connector positions"
        );
        let candidate = RecipeExpander::expand(
            &production,
            &request(&production, recipe_id, domain),
            &RecipeExpansionPolicy::default(),
        )
        .unwrap();
        assert_eq!(candidate.instances.len(), expected_instances, "{recipe_id}");
        assert_eq!(candidate.quality_profile, "production_concept");
        assert_eq!(
            candidate.component_recipe_instances.len(),
            expected_instances
        );

        for child in candidate.instances.iter().skip(1) {
            let parent_id = child.provenance.parent_instance_id.as_deref().unwrap();
            let slot_id = child.provenance.parent_slot_id.as_deref().unwrap();
            let parent = candidate
                .instances
                .iter()
                .find(|instance| instance.instance_id == parent_id)
                .unwrap();
            let slot = parent
                .recipe
                .child_slots
                .iter()
                .find(|slot| slot.slot_id == slot_id)
                .unwrap();
            let parent_connector = parent
                .recipe
                .connectors
                .iter()
                .find(|connector| connector.connector_id == slot.parent_connector_id)
                .unwrap();
            let child_connector = child
                .recipe
                .connectors
                .iter()
                .find(|connector| connector.connector_id == slot.child_connector_id)
                .unwrap();
            assert_eq!(
                child_connector.position,
                [0.0, 0.0, 0.0],
                "{recipe_id}:{slot_id}"
            );
            let expected_mount = transform_point(parent.world_transform, parent_connector.position);
            let actual_mount = transform_point(child.world_transform, child_connector.position);
            assert_near(actual_mount, expected_mount, recipe_id, slot_id);
        }
    }
}
