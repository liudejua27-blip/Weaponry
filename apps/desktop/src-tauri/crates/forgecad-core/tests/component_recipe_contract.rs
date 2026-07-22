use forgecad_core::{
    ComponentRecipeRef, RecipeExpander, RecipeExpansionPolicy, RecipeInstantiationRequest,
    RecipeMaterialZoneOverride, RecipeParameterValue, RecipeRegistry, RecipeSlotBinding,
};
use serde_json::json;

fn request(
    registry: &RecipeRegistry,
    recipe_id: &str,
    domain_pack_id: &str,
) -> RecipeInstantiationRequest {
    let recipe = registry.recipe(recipe_id).unwrap();
    RecipeInstantiationRequest {
        schema_version: "ComponentRecipeInstantiationRequest@1".into(),
        context_mode: "initial_candidate".into(),
        request_id: "recipereq_contract_four_domain".into(),
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
    }
}

#[test]
fn c105_embedded_registry_expands_four_roots_and_child_components_deterministically() {
    let registry = RecipeRegistry::from_embedded().unwrap();
    assert_eq!(registry.registry_sha256().len(), 64);
    for (recipe_id, domain_pack_id) in [
        ("recipe_future_prop_shell", "pack_future_weapon_prop"),
        ("recipe_vehicle_body_shell", "pack_vehicle_concept"),
        ("recipe_aircraft_fuselage", "pack_aircraft_concept"),
        ("recipe_robotic_arm_link", "pack_robotic_arm_concept"),
    ] {
        let mut request = request(&registry, recipe_id, domain_pack_id);
        if recipe_id == "recipe_robotic_arm_link" {
            let child = registry.recipe("recipe_robotic_arm_detail").unwrap();
            request.slot_bindings.push(RecipeSlotBinding {
                slot_id: "slot_arm_detail".into(),
                child_recipe: ComponentRecipeRef {
                    schema_version: "ComponentRecipeRef@1".into(),
                    recipe_id: child.recipe_id.clone(),
                    version: child.version,
                    recipe_sha256: forgecad_core::RecipeValidator::recipe_sha256(child).unwrap(),
                },
            });
        }
        let first =
            RecipeExpander::expand(&registry, &request, &RecipeExpansionPolicy::default()).unwrap();
        let second =
            RecipeExpander::expand(&registry, &request, &RecipeExpansionPolicy::default()).unwrap();
        assert_eq!(first.candidate_sha256, second.candidate_sha256);
        assert_eq!(first.instances.len(), 2);
        assert_eq!(first.component_recipe_instances.len(), 2);
        assert_eq!(first.instances[0].recipe.parameter_bindings.len(), 1);
        assert_eq!(first.instances[0].instance_path, "root");
        assert!(first.instances[1].instance_path.starts_with("root/slot_"));
        assert_eq!(
            first.expanded_shape_program["schema_version"],
            "ShapeProgram@1"
        );
        assert_eq!(
            first.expanded_assembly_graph["schema_version"],
            "AssemblyGraph@1"
        );
        assert_eq!(
            first.expanded_shape_program["operations"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            first.expanded_assembly_graph["connections"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let root_part = &first.expanded_assembly_graph["parts"][0];
        assert!(root_part["operation_id"]
            .as_str()
            .unwrap()
            .starts_with("op_"));
        assert!(root_part["output_id"]
            .as_str()
            .unwrap()
            .starts_with("output_"));
        assert_eq!(
            root_part["editable_parameter_bindings"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            root_part["editable_parameters"].as_array().unwrap()[0],
            first.instances[0].recipe.parameter_bindings[0]["parameter_id"]
        );
        assert_recipe_frames_are_persisted(&first.expanded_assembly_graph);
        for (part, instance) in first.expanded_assembly_graph["parts"]
            .as_array()
            .unwrap()
            .iter()
            .zip(&first.instances)
        {
            assert_eq!(
                part["pivot"],
                serde_json::to_value(&instance.recipe.pivot).unwrap(),
                "recipe-backed pivot must round-trip without losing its roll"
            );
            let persisted = part["connectors"].as_array().unwrap();
            assert_eq!(persisted.len(), instance.recipe.connectors.len());
            for (connector, recipe_connector) in persisted.iter().zip(&instance.recipe.connectors) {
                assert_eq!(connector["up"], serde_json::json!(recipe_connector.up));
                assert_eq!(
                    connector["normal"],
                    serde_json::json!(recipe_connector.normal)
                );
            }
        }
        let child_position = first.expanded_shape_program["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|operation| operation["op"] == "box")
            .and_then(|operation| operation["args"]["position"].as_array())
            .unwrap();
        assert!(child_position.iter().any(|value| value.as_f64().unwrap().abs() > 1e-9), "slot connector translation must be baked into the child geometry, not only AssemblyGraph");
    }
}

#[test]
fn recipe_surface_adornment_slots_are_bounded_projected_and_never_become_geometry_operations() {
    let mut document: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../packages/concept-spec/fixtures/editable-component-recipe-registry.json"
    )))
    .unwrap();
    {
        let recipe = document["recipes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|recipe| recipe["recipe_id"] == "recipe_robotic_arm_link")
            .unwrap();
        recipe["surface_adornment_slots"] = json!([{
            "slot_id": "adornslot_arm_link_shell",
            "zone_id": "zone_arm_shell",
            "allowed_kinds": ["normal_relief", "flowline"],
            "allowed_motifs": ["parallel_groove", "double_flowline"],
            "allowed_coverages": ["center_band", "edge_band"]
        }]);
    }
    let registry = RecipeRegistry::from_json(&serde_json::to_string(&document).unwrap()).unwrap();
    let candidate = RecipeExpander::expand(
        &registry,
        &request(
            &registry,
            "recipe_robotic_arm_link",
            "pack_robotic_arm_concept",
        ),
        &RecipeExpansionPolicy::default(),
    )
    .unwrap();
    let root = &candidate.expanded_assembly_graph["parts"][0];
    assert_eq!(
        root["surface_adornment_slots"][0]["slot_id"],
        "adornslot_arm_link_shell"
    );
    assert_eq!(
        root["surface_adornment_slots"][0]["zone_id"],
        "zone_arm_shell"
    );
    assert!(candidate.expanded_shape_program["operations"]
        .as_array()
        .unwrap()
        .iter()
        .all(|operation| operation["op"] != "surface_adornment"));

    document["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["recipe_id"] == "recipe_robotic_arm_link")
        .unwrap()["surface_adornment_slots"][0]["zone_id"] = json!("zone_not_owned");
    assert_eq!(
        RecipeRegistry::from_json(&serde_json::to_string(&document).unwrap())
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_SURFACE_ADORNMENT_SLOT_INVALID"
    );
}

fn assert_recipe_frames_are_persisted(graph: &serde_json::Value) {
    for part in graph["parts"].as_array().expect("recipe graph parts") {
        assert_orthonormal_frame(&part["pivot"]);
        for connector in part["connectors"]
            .as_array()
            .expect("recipe part connectors")
        {
            assert_orthonormal_frame(connector);
        }
    }
}

fn assert_orthonormal_frame(frame: &serde_json::Value) {
    let normal = vec3(&frame["normal"]);
    let up = vec3(&frame["up"]);
    let norm = |vector: [f64; 3]| vector.iter().map(|value| value * value).sum::<f64>().sqrt();
    assert!(
        (norm(normal) - 1.0).abs() <= 1e-9,
        "normal must remain unit"
    );
    assert!((norm(up) - 1.0).abs() <= 1e-9, "up must remain unit");
    assert!(
        normal
            .iter()
            .zip(up)
            .map(|(left, right)| left * right)
            .sum::<f64>()
            .abs()
            <= 1e-9,
        "normal/up must remain orthogonal"
    );
}

fn vec3(value: &serde_json::Value) -> [f64; 3] {
    let values = value.as_array().expect("persisted vec3");
    assert_eq!(values.len(), 3);
    [
        values[0].as_f64().expect("finite x"),
        values[1].as_f64().expect("finite y"),
        values[2].as_f64().expect("finite z"),
    ]
}

#[test]
fn c105_rejects_recipe_reference_tampering_and_cross_domain_use() {
    let registry = RecipeRegistry::from_embedded().unwrap();
    let mut tampered = request(
        &registry,
        "recipe_vehicle_body_shell",
        "pack_vehicle_concept",
    );
    tampered.recipe.recipe_sha256 = "0".repeat(64);
    assert_eq!(
        RecipeExpander::expand(&registry, &tampered, &RecipeExpansionPolicy::default())
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_REFERENCE_STALE"
    );

    let cross_domain = request(
        &registry,
        "recipe_vehicle_body_shell",
        "pack_aircraft_concept",
    );
    assert_eq!(
        RecipeExpander::expand(&registry, &cross_domain, &RecipeExpansionPolicy::default())
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_DOMAIN_INCOMPATIBLE"
    );
}

#[test]
fn c105_rejects_unknown_or_cross_domain_material_override_without_silent_fallback() {
    let registry = RecipeRegistry::from_embedded().unwrap();
    let mut unknown = request(
        &registry,
        "recipe_vehicle_body_shell",
        "pack_vehicle_concept",
    );
    unknown
        .material_zone_overrides
        .push(RecipeMaterialZoneOverride {
            zone_id: "zone_vehicle_shell".into(),
            material_preset_id: "mat_not_reviewed".into(),
        });
    assert_eq!(
        RecipeExpander::expand(&registry, &unknown, &RecipeExpansionPolicy::default())
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_ZONE_OVERRIDE_INVALID"
    );

    let mut wrong_domain = request(
        &registry,
        "recipe_robotic_arm_link",
        "pack_robotic_arm_concept",
    );
    wrong_domain
        .material_zone_overrides
        .push(RecipeMaterialZoneOverride {
            zone_id: "zone_arm_shell".into(),
            material_preset_id: "mat_dark_glass".into(),
        });
    assert_eq!(
        RecipeExpander::expand(&registry, &wrong_domain, &RecipeExpansionPolicy::default())
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_ZONE_OVERRIDE_INVALID"
    );
}

#[test]
fn c105_context_modes_reject_forged_or_incomplete_snapshot_context() {
    let registry = RecipeRegistry::from_embedded().unwrap();
    let mut forged_initial = request(
        &registry,
        "recipe_vehicle_body_shell",
        "pack_vehicle_concept",
    );
    forged_initial.snapshot_revision = Some(1);
    assert_eq!(
        RecipeExpander::expand(
            &registry,
            &forged_initial,
            &RecipeExpansionPolicy::default()
        )
        .unwrap_err()
        .code(),
        "COMPONENT_RECIPE_CONTEXT_INVALID"
    );

    let mut incomplete_active = request(
        &registry,
        "recipe_vehicle_body_shell",
        "pack_vehicle_concept",
    );
    incomplete_active.context_mode = "active_asset_edit".into();
    incomplete_active.project_id = Some("prj_contract_active".into());
    incomplete_active.base_asset_version_id = Some("assetver_contract_active".into());
    incomplete_active.snapshot_revision = Some(0);
    assert_eq!(
        RecipeExpander::expand(
            &registry,
            &incomplete_active,
            &RecipeExpansionPolicy::default()
        )
        .unwrap_err()
        .code(),
        "COMPONENT_RECIPE_CONTEXT_INVALID"
    );
}

#[test]
fn c105_declared_parameter_override_fails_closed_until_bridge_bakes_the_exact_sweep_dimension() {
    let registry = RecipeRegistry::from_embedded().unwrap();
    let mut request = request(
        &registry,
        "recipe_vehicle_body_shell",
        "pack_vehicle_concept",
    );
    request.parameter_values.push(RecipeParameterValue {
        parameter_id: "editparam_vehicle_body_profile_height".into(),
        value: 1.1,
    });
    assert_eq!(
        RecipeExpander::expand(&registry, &request, &RecipeExpansionPolicy::default())
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_PARAMETER_UNSUPPORTED"
    );
}

#[test]
fn c105_optional_slot_binding_is_fixed_and_cannot_be_repeated() {
    let registry = RecipeRegistry::from_embedded().unwrap();
    let child = registry.recipe("recipe_robotic_arm_detail").unwrap();
    let binding = RecipeSlotBinding {
        slot_id: "slot_arm_detail".into(),
        child_recipe: ComponentRecipeRef {
            schema_version: "ComponentRecipeRef@1".into(),
            recipe_id: child.recipe_id.clone(),
            version: child.version,
            recipe_sha256: forgecad_core::RecipeValidator::recipe_sha256(child).unwrap(),
        },
    };
    let mut duplicate = request(
        &registry,
        "recipe_robotic_arm_link",
        "pack_robotic_arm_concept",
    );
    duplicate.slot_bindings = vec![binding.clone(), binding];
    assert_eq!(
        RecipeExpander::expand(&registry, &duplicate, &RecipeExpansionPolicy::default())
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_SLOT_BINDING_DUPLICATE"
    );
}

#[test]
fn c105_optional_slot_binding_rejects_unknown_stale_and_required_slots() {
    let registry = RecipeRegistry::from_embedded().unwrap();
    let child = registry.recipe("recipe_robotic_arm_detail").unwrap();
    let binding = RecipeSlotBinding {
        slot_id: "slot_arm_detail".into(),
        child_recipe: ComponentRecipeRef {
            schema_version: "ComponentRecipeRef@1".into(),
            recipe_id: child.recipe_id.clone(),
            version: child.version,
            recipe_sha256: forgecad_core::RecipeValidator::recipe_sha256(child).unwrap(),
        },
    };

    let mut unknown_slot = request(
        &registry,
        "recipe_robotic_arm_link",
        "pack_robotic_arm_concept",
    );
    unknown_slot.slot_bindings = vec![RecipeSlotBinding {
        slot_id: "slot_not_reviewed".into(),
        child_recipe: binding.child_recipe.clone(),
    }];
    assert_eq!(
        RecipeExpander::expand(&registry, &unknown_slot, &RecipeExpansionPolicy::default())
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_SLOT_BINDING_INVALID"
    );

    let mut stale_child = request(
        &registry,
        "recipe_robotic_arm_link",
        "pack_robotic_arm_concept",
    );
    let mut stale_binding = binding.clone();
    stale_binding.child_recipe.recipe_sha256 = "0".repeat(64);
    stale_child.slot_bindings = vec![stale_binding];
    assert_eq!(
        RecipeExpander::expand(&registry, &stale_child, &RecipeExpansionPolicy::default())
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_SLOT_BINDING_STALE"
    );

    let mut required_registry_json: serde_json::Value =
        serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../packages/concept-spec/fixtures/editable-component-recipe-registry.json"
    )))
        .unwrap();
    required_registry_json["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["recipe_id"] == "recipe_robotic_arm_link")
        .unwrap()["child_slots"][0]["required"] = serde_json::json!(true);
    let required_registry =
        RecipeRegistry::from_json(&serde_json::to_string(&required_registry_json).unwrap())
            .unwrap();
    let required_child = required_registry
        .recipe("recipe_robotic_arm_detail")
        .unwrap();
    let mut required_request = request(
        &required_registry,
        "recipe_robotic_arm_link",
        "pack_robotic_arm_concept",
    );
    required_request.slot_bindings = vec![RecipeSlotBinding {
        slot_id: "slot_arm_detail".into(),
        child_recipe: ComponentRecipeRef {
            schema_version: "ComponentRecipeRef@1".into(),
            recipe_id: required_child.recipe_id.clone(),
            version: required_child.version,
            recipe_sha256: forgecad_core::RecipeValidator::recipe_sha256(required_child).unwrap(),
        },
    }];
    assert_eq!(
        RecipeExpander::expand(
            &required_registry,
            &required_request,
            &RecipeExpansionPolicy::default(),
        )
        .unwrap_err()
        .code(),
        "COMPONENT_RECIPE_SLOT_BINDING_INVALID"
    );
}

#[test]
fn c105_recipe_version_upgrade_preserves_old_candidate_hash_and_rejects_stale_ref() {
    let v1_registry = RecipeRegistry::from_embedded().unwrap();
    let v1_request = request(
        &v1_registry,
        "recipe_vehicle_body_shell",
        "pack_vehicle_concept",
    );
    let v1_candidate =
        RecipeExpander::expand(&v1_registry, &v1_request, &RecipeExpansionPolicy::default())
            .unwrap();
    let v1_candidate_json = serde_json::to_value(&v1_candidate).unwrap();
    let v1_candidate_sha = v1_candidate.candidate_sha256.clone();
    let v1_shape_sha =
        forgecad_core::semantic_sha256(&v1_candidate.expanded_shape_program).unwrap();
    let v1_registry_sha = v1_registry.registry_sha256().to_string();

    let mut upgraded: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../packages/concept-spec/fixtures/editable-component-recipe-registry.json"
    )))
    .unwrap();
    let recipe = upgraded["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["recipe_id"] == "recipe_vehicle_body_shell")
        .unwrap();
    recipe["version"] = serde_json::json!(2);
    recipe["display_name"] = serde_json::json!("车辆主壳配方 v2");
    let v2_registry =
        RecipeRegistry::from_json(&serde_json::to_string(&upgraded).unwrap()).unwrap();
    assert_ne!(v2_registry.registry_sha256(), v1_registry_sha);

    let stale =
        RecipeExpander::expand(&v2_registry, &v1_request, &RecipeExpansionPolicy::default())
            .unwrap_err();
    assert_eq!(stale.code(), "COMPONENT_RECIPE_REGISTRY_STALE");

    let mut stale_reference_request = v1_request.clone();
    stale_reference_request.recipe_registry_sha256 = v2_registry.registry_sha256().into();
    assert_eq!(
        RecipeExpander::expand(
            &v2_registry,
            &stale_reference_request,
            &RecipeExpansionPolicy::default(),
        )
        .unwrap_err()
        .code(),
        "COMPONENT_RECIPE_REFERENCE_STALE"
    );

    let v2_request = request(
        &v2_registry,
        "recipe_vehicle_body_shell",
        "pack_vehicle_concept",
    );
    assert_eq!(v2_request.recipe.version, 2);
    assert_ne!(
        v2_request.recipe.recipe_sha256,
        v1_request.recipe.recipe_sha256
    );
    let v2_candidate =
        RecipeExpander::expand(&v2_registry, &v2_request, &RecipeExpansionPolicy::default())
            .unwrap();
    assert_ne!(v2_candidate.candidate_sha256, v1_candidate_sha);
    assert_ne!(
        forgecad_core::semantic_sha256(&v2_candidate.expanded_shape_program).unwrap(),
        v1_shape_sha
    );

    // Upgrading the reviewed catalog creates a distinct candidate; it never
    // rewrites an already persisted v1 candidate or its immutable hashes.
    assert_eq!(
        serde_json::to_value(&v1_candidate).unwrap(),
        v1_candidate_json
    );
    assert_eq!(v1_candidate.candidate_sha256, v1_candidate_sha);
    assert_eq!(
        forgecad_core::semantic_sha256(&v1_candidate.expanded_shape_program).unwrap(),
        v1_shape_sha
    );
}

fn registry_document() -> serde_json::Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../packages/concept-spec/fixtures/editable-component-recipe-registry.json"
    )))
    .unwrap()
}

fn recipe_mut<'a>(
    document: &'a mut serde_json::Value,
    recipe_id: &str,
) -> &'a mut serde_json::Value {
    document["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["recipe_id"] == recipe_id)
        .unwrap()
}

#[test]
fn m108b_rotated_recipe_frames_bake_deterministically_and_preserve_local_connector_roll() {
    let mut document = registry_document();
    let root = recipe_mut(&mut document, "recipe_vehicle_body_shell");
    root["root_local_transform"]["rotation"] = serde_json::json!([0.35, -0.55, 1.1]);
    // This parent connector roll is intentionally different from the child
    // frame; the Rust expander must include it in the baked child operation.
    root["connectors"][0]["up"] = serde_json::json!([0.0, 1.0, 0.0]);
    let registry = RecipeRegistry::from_json(&serde_json::to_string(&document).unwrap()).unwrap();
    let request = request(
        &registry,
        "recipe_vehicle_body_shell",
        "pack_vehicle_concept",
    );
    let first =
        RecipeExpander::expand(&registry, &request, &RecipeExpansionPolicy::default()).unwrap();
    let second =
        RecipeExpander::expand(&registry, &request, &RecipeExpansionPolicy::default()).unwrap();
    assert_eq!(first.candidate_sha256, second.candidate_sha256);
    assert_ne!(first.candidate_sha256, "");
    for operation in first.expanded_shape_program["operations"]
        .as_array()
        .unwrap()
    {
        if matches!(operation["op"].as_str(), Some("sweep" | "box")) {
            assert!(operation["args"]["rotation"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value.as_f64().unwrap().abs() > 1e-7));
        }
    }
    for part in first.expanded_assembly_graph["parts"].as_array().unwrap() {
        assert!(part["transform"]["rotation"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_f64().unwrap().abs() > 1e-7));
        // The graph records a world transform and still preserves the local
        // connector/pivot frames used to reconstruct a connector pose.
        assert!(part["pivot"]["up"].is_array());
        assert!(part["connectors"]
            .as_array()
            .unwrap()
            .iter()
            .all(|connector| connector["up"].is_array()));
    }
}

#[test]
fn m108b_scale_and_unsafe_transform_operations_fail_closed_before_candidate_creation() {
    let mut scaled = registry_document();
    recipe_mut(&mut scaled, "recipe_vehicle_body_shell")["root_local_transform"]["scale"] =
        serde_json::json!([1.25, 1.25, 1.25]);
    let scaled_registry =
        RecipeRegistry::from_json(&serde_json::to_string(&scaled).unwrap()).unwrap();
    let scaled_request = request(
        &scaled_registry,
        "recipe_vehicle_body_shell",
        "pack_vehicle_concept",
    );
    assert_eq!(
        RecipeExpander::expand(
            &scaled_registry,
            &scaled_request,
            &RecipeExpansionPolicy::default()
        )
        .unwrap_err()
        .code(),
        "COMPONENT_RECIPE_TEMPLATE_SCALE_UNSUPPORTED"
    );

    let mut shear = registry_document();
    let transform =
        &mut recipe_mut(&mut shear, "recipe_vehicle_body_shell")["root_local_transform"];
    transform["rotation"] = serde_json::json!([0.0, 0.0, 1.5707963267948966]);
    transform["scale"] = serde_json::json!([1.0, 1.2, 1.0]);
    assert_eq!(
        RecipeRegistry::from_json(&serde_json::to_string(&shear).unwrap())
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_SHEAR_UNSUPPORTED"
    );

    let mut unsupported = registry_document();
    let recipe = recipe_mut(&mut unsupported, "recipe_vehicle_body_shell");
    recipe["root_local_transform"]["rotation"] = serde_json::json!([0.0, 0.0, 1.5707963267948966]);
    recipe["shape_program_template"]["operations"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "operation_id": "op_forbidden_array", "op": "array", "inputs": ["op_vehicle_body"],
            "args": {"axis": [1, 0, 0], "count": 2, "spacing": 10}
        }));
    let unsupported_registry =
        RecipeRegistry::from_json(&serde_json::to_string(&unsupported).unwrap()).unwrap();
    let unsupported_request = request(
        &unsupported_registry,
        "recipe_vehicle_body_shell",
        "pack_vehicle_concept",
    );
    assert_eq!(
        RecipeExpander::expand(
            &unsupported_registry,
            &unsupported_request,
            &RecipeExpansionPolicy::default()
        )
        .unwrap_err()
        .code(),
        "COMPONENT_RECIPE_TEMPLATE_TRANSFORM_OPERATION_UNSUPPORTED"
    );
}
