use forgecad_core::{
    ComponentRecipeRef, RecipeExpander, RecipeExpansionPolicy, RecipeInstantiationRequest,
    RecipeRegistry, RecipeValidator,
};
use std::collections::{BTreeMap, BTreeSet};

const ROOTS: [&str; 3] = [
    "recipe_c106_arm_desktop_assistant",
    "recipe_c106_arm_gallery_industrial",
    "recipe_c106_arm_service_display",
];

const C106_REGISTRY_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/fixtures/c106-robotic-arm-component-recipe-registry.json"
));

const A005_KINDS: [&str; 4] = ["normal_relief", "pattern", "flowline", "micro_surface"];
const A005_MOTIFS: [&str; 4] = [
    "parallel_groove",
    "chevron_relief",
    "double_flowline",
    "hex_microgrid",
];
const A005_COVERAGES: [&str; 4] = ["full_zone", "center_band", "edge_band", "symmetric_pair"];

fn expected_visual_parameter_bindings() -> BTreeMap<&'static str, (&'static str, &'static str)> {
    BTreeMap::from([
        (
            "recipe_c106_arm_joint_housing",
            (
                "editparam_joint_housing_visual_fullness",
                "transform.scale.z",
            ),
        ),
        (
            "recipe_c106_arm_link_armor",
            ("editparam_link_armor_visual_fullness", "transform.scale.y"),
        ),
        (
            "recipe_c106_arm_surface_trim",
            ("editparam_surface_trim_visual_span", "transform.scale.x"),
        ),
    ])
}

fn request(
    registry: &RecipeRegistry,
    recipe_id: &str,
    domain_pack_id: &str,
) -> RecipeInstantiationRequest {
    let recipe = registry.recipe(recipe_id).expect("C106 reviewed root");
    RecipeInstantiationRequest {
        schema_version: "ComponentRecipeInstantiationRequest@1".into(),
        context_mode: "initial_candidate".into(),
        request_id: format!("recipereq_test_{}", recipe_id.trim_start_matches("recipe_")),
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
fn c106_arm_pack_is_isolated_and_expands_the_semantic_ten_part_assembly() {
    let c106 = RecipeRegistry::from_embedded_c106_robotic_arm().unwrap();
    let m108b = RecipeRegistry::from_embedded_production().unwrap();
    assert_eq!(c106.registry_id(), "registry_c106_robotic_arm_concept_v1");
    assert_eq!(c106.recipes().count(), 9);
    assert_ne!(c106.registry_sha256(), m108b.registry_sha256());

    let expected_parameters = expected_visual_parameter_bindings();
    for recipe in c106.recipes() {
        let material_zone_ids = recipe
            .material_zones
            .iter()
            .map(|zone| zone["zone_id"].as_str().expect("declared material zone"))
            .collect::<BTreeSet<_>>();
        assert!(
            !recipe.surface_adornment_slots.is_empty(),
            "{} must expose at least one bounded A005 surface slot",
            recipe.recipe_id
        );
        for slot in &recipe.surface_adornment_slots {
            assert!(
                material_zone_ids.contains(slot.zone_id.as_str()),
                "{} slot {} must target its own declared zone",
                recipe.recipe_id,
                slot.slot_id
            );
            assert!(slot
                .allowed_kinds
                .iter()
                .all(|value| A005_KINDS.contains(&value.as_str())));
            assert!(slot
                .allowed_motifs
                .iter()
                .all(|value| A005_MOTIFS.contains(&value.as_str())));
            assert!(slot
                .allowed_coverages
                .iter()
                .all(|value| A005_COVERAGES.contains(&value.as_str())));
        }
        assert_eq!(recipe.quality_status, "passed");
        assert!(recipe.non_functional_only);
        assert_eq!(recipe.source["source_kind"], "forgecad_first_party");
        assert_eq!(recipe.source["source_id"], "source_c106_arm");
        assert_eq!(
            recipe.license["license_id"],
            "ForgeCAD-Internal-Visual-Only"
        );
        assert_eq!(recipe.license["redistributable"], false);
        assert_eq!(recipe.review_state["reviewer_kind"], "forgecad_internal");

        match expected_parameters.get(recipe.recipe_id.as_str()) {
            Some((parameter_id, path)) => {
                assert_eq!(recipe.parameter_bindings.len(), 1, "{}", recipe.recipe_id);
                let binding = &recipe.parameter_bindings[0];
                assert_eq!(binding["schema_version"], "EditableParameterBinding@1");
                assert_eq!(binding["parameter_id"], *parameter_id);
                assert_eq!(binding["path"], *path);
                assert_eq!(binding["unit"], "ratio");
                assert_eq!(binding["default"], 1.0);
                assert!(binding["display_name"]
                    .as_str()
                    .is_some_and(|name| name.contains("视觉")));
            }
            None => assert!(recipe.parameter_bindings.is_empty(), "{}", recipe.recipe_id),
        }
    }

    for root in ROOTS {
        let candidate = RecipeExpander::expand(
            &c106,
            &request(&c106, root, "pack_robotic_arm_concept"),
            &RecipeExpansionPolicy::default(),
        )
        .unwrap();
        let graph = candidate.expanded_assembly_graph.as_object().unwrap();
        let parts = graph["parts"].as_array().unwrap();
        let connections = graph["connections"].as_array().unwrap();
        assert_eq!(parts.len(), 10, "{root}");
        assert_eq!(connections.len(), 9, "{root}");
        assert_eq!(candidate.component_recipe_instances.len(), 10, "{root}");
        assert!(candidate.component_recipe_instances.iter().all(|instance| {
            instance.source["source_kind"] == "forgecad_first_party"
                && instance.source["source_id"] == "source_c106_arm"
                && instance.license["license_id"] == "ForgeCAD-Internal-Visual-Only"
                && instance.non_functional_only
                && instance.quality_status == "passed"
        }));
        let mut roles = parts
            .iter()
            .filter_map(|part| part["role"].as_str())
            .collect::<Vec<_>>();
        roles.sort_unstable();
        assert_eq!(
            roles,
            vec![
                "base_form",
                "cable_harness",
                "end_effector_form",
                "joint_housing",
                "joint_housing",
                "joint_housing",
                "link_armor",
                "link_armor",
                "surface_trim",
                "turntable",
            ],
            "{root}"
        );
        assert!(parts
            .iter()
            .all(|part| part["operation_id"].as_str().is_some()));
        assert!(parts
            .iter()
            .all(|part| part["output_id"].as_str().is_some()));
        assert!(connections
            .iter()
            .all(|connection| connection["status"] == "connected"));
        let material_zone_ids = parts
            .iter()
            .flat_map(|part| part["material_zone_ids"].as_array().unwrap())
            .filter_map(|zone| zone.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            material_zone_ids.len(),
            if root == "recipe_c106_arm_service_display" {
                19
            } else {
                16
            },
            "{root}"
        );
        assert!(parts.iter().all(|part| {
            let zones = part["material_zone_ids"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|zone| zone.as_str())
                .collect::<BTreeSet<_>>();
            part["surface_adornment_slots"]
                .as_array()
                .is_some_and(|slots| {
                    !slots.is_empty()
                        && slots.iter().all(|slot| {
                            slot["zone_id"]
                                .as_str()
                                .is_some_and(|zone_id| zones.contains(zone_id))
                        })
                })
        }));

        for (role, expected_recipe_id) in [
            ("joint_housing", "recipe_c106_arm_joint_housing"),
            ("link_armor", "recipe_c106_arm_link_armor"),
            ("surface_trim", "recipe_c106_arm_surface_trim"),
        ] {
            let (expected_parameter_id, expected_path) = expected_parameters[expected_recipe_id];
            let matching_parts = parts
                .iter()
                .filter(|part| part["role"] == role)
                .collect::<Vec<_>>();
            assert!(!matching_parts.is_empty(), "{root}:{role}");
            assert!(matching_parts.iter().all(|part| {
                let bindings = part["editable_parameter_bindings"].as_array().unwrap();
                bindings.len() == 1
                    && bindings[0]["parameter_id"] == expected_parameter_id
                    && bindings[0]["path"] == expected_path
                    && part["editable_parameters"] == serde_json::json!([expected_parameter_id])
            }));
        }
    }
}

#[test]
fn c106_arm_pack_rejects_cross_domain_root_request_before_expansion() {
    let c106 = RecipeRegistry::from_embedded_c106_robotic_arm().unwrap();
    let error = RecipeExpander::expand(
        &c106,
        &request(&c106, ROOTS[0], "pack_vehicle_concept"),
        &RecipeExpansionPolicy::default(),
    )
    .unwrap_err();
    assert_eq!(error.code(), "COMPONENT_RECIPE_DOMAIN_INCOMPATIBLE");
}

#[test]
fn c106_arm_pack_rejects_engineering_parameter_paths() {
    let mut registry: serde_json::Value = serde_json::from_str(C106_REGISTRY_JSON).unwrap();
    let joint_housing = registry["recipes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|recipe| recipe["recipe_id"] == "recipe_c106_arm_joint_housing")
        .unwrap();
    joint_housing["parameter_bindings"][0]["path"] = serde_json::json!("joint.torque");

    let error = RecipeRegistry::from_json(&serde_json::to_string(&registry).unwrap()).unwrap_err();
    assert_eq!(error.code(), "COMPONENT_RECIPE_PARAMETER_BINDING_INVALID");
}

#[test]
fn c106_service_display_bakes_a_layered_s_pose_and_curved_visual_cables() {
    let registry = RecipeRegistry::from_embedded_c106_robotic_arm().unwrap();
    let candidate = RecipeExpander::expand(
        &registry,
        &request(
            &registry,
            "recipe_c106_arm_service_display",
            "pack_robotic_arm_concept",
        ),
        &RecipeExpansionPolicy::default(),
    )
    .unwrap();

    let parts = candidate.expanded_assembly_graph["parts"]
        .as_array()
        .unwrap();
    let link_turns = parts
        .iter()
        .filter(|part| part["role"] == "link_armor")
        .map(|part| part["transform"]["rotation"][2].as_f64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(link_turns.len(), 2);
    assert!(
        link_turns.iter().any(|turn| *turn > 0.9) && link_turns.iter().any(|turn| *turn < -0.6),
        "the service root must retain its reviewed rising-and-returning S pose"
    );

    let operations = candidate.expanded_shape_program["operations"]
        .as_array()
        .unwrap();
    let outputs = candidate.expanded_shape_program["outputs"]
        .as_array()
        .unwrap();
    assert_eq!(
        outputs.len(),
        48,
        "the reviewed service-display pack must use the full bounded output budget for visible hard-surface layers"
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| operation["op"] == "sweep")
            .count(),
        3,
        "the visual cable harness must retain two structural curves and one signal curve"
    );
    assert!(operations
        .iter()
        .any(|operation| operation["op"] == "bevel_approx"));
    assert!(operations
        .iter()
        .any(|operation| operation["op"] == "surface_panel"));
    for required_suffix in [
        "_plinth_service_panel",
        "_plinth_fastener_array",
        "_plinth_signal_array",
        "_joint_bearing_box",
        "_joint_bearing_face",
        "_joint_hub_cap",
        "_link_side_rail_b",
        "_cable_clamp_bridge",
        "_gripper_wrist_profile",
        "_gripper_wrist_collar",
    ] {
        assert!(
            operations.iter().any(|operation| {
                operation["operation_id"]
                    .as_str()
                    .is_some_and(|operation_id| operation_id.ends_with(required_suffix))
            }),
            "the production visual pack must retain {required_suffix}"
        );
    }
    assert!(operations.iter().any(|operation| {
        operation["operation_id"]
            .as_str()
            .is_some_and(|operation_id| operation_id.ends_with("_link_body"))
            && operation["op"] == "box"
            && operation["args"]["size"][1]
                .as_f64()
                .is_some_and(|depth| depth <= 90.0)
            && operation["args"]["size"][2]
                .as_f64()
                .is_some_and(|width| width <= 80.0)
    }));
    assert_eq!(
        operations
            .iter()
            .filter(|operation| {
                operation["operation_id"]
                    .as_str()
                    .is_some_and(|operation_id| operation_id.ends_with("_joint_bearing_face"))
                    && operation["op"] == "surface_panel"
                    && operation["args"]["material_id"] == "mat_automotive_paint"
            })
            .count(),
        3,
        "each visible joint must retain one painted face on a distinct rounded bearing box"
    );
    assert!(
        operations.iter().any(|operation| {
            operation["operation_id"]
                .as_str()
                .is_some_and(|operation_id| operation_id.ends_with("_gripper_wrist_profile"))
                && operation["op"] == "profile"
                && operation["args"]["points"]
                    .as_array()
                    .is_some_and(|points| points.len() == 11)
        }),
        "the wrist must keep its stepped multi-ring section instead of reverting to one cylinder"
    );
    assert!(
        operations.iter().any(|operation| {
            operation["operation_id"]
                .as_str()
                .is_some_and(|operation_id| operation_id.ends_with("_gripper_wrist_collar"))
                && operation["op"] == "revolve"
        }),
        "the stepped wrist section must compile as one closed revolved production mesh"
    );
    assert!(operations.iter().any(|operation| {
        operation["operation_id"]
            .as_str()
            .is_some_and(|operation_id| operation_id.ends_with("_link_armor_right"))
            && operation["op"] == "bevel_approx"
    }));
    assert!(
        operations.iter().any(|operation| {
            operation["operation_id"]
                .as_str()
                .is_some_and(|operation_id| operation_id.ends_with("_cable_clamp_bridge"))
                && operation["op"] == "surface_panel"
        }),
        "the cable clamp must keep its distinct graphite carrier and metallic face panel"
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| {
                operation["operation_id"]
                    .as_str()
                    .is_some_and(|operation_id| {
                        operation_id.ends_with("_gripper_finger_a")
                            || operation_id.ends_with("_gripper_finger_b")
                    })
                    && operation["op"] == "bevel_approx"
            })
            .count(),
        2,
        "the service gripper must retain two hard-surface proximal finger shells"
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| operation["op"] == "radial_array")
            .count(),
        1,
        "only the root-local, non-rotated maintenance fasteners may use a radial array"
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| operation["op"] == "array")
            .count(),
        3,
        "only the root-local base panel/signal rows and unrotated turntable ring stack may use linear arrays"
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| operation["op"] == "wedge")
            .count(),
        2,
        "the gripper must retain two separately baked distal finger shells"
    );
    assert!(
        operations
            .iter()
            .filter(|operation| operation["op"] == "wedge")
            .all(|operation| operation["args"]["size"][0]
                .as_f64()
                .is_some_and(|length| length >= 220.0)),
        "each distal finger shell must remain a visually distinct second segment"
    );
    assert!(
        operations
            .iter()
            .any(|operation| { operation["args"]["material_id"] == "mat_automotive_paint" }),
        "the service root must carry a real opaque blue painted hard-surface zone"
    );
    assert!(
        operations.iter().any(|operation| {
            operation["operation_id"]
                .as_str()
                .is_some_and(|operation_id| operation_id.ends_with("_plinth_lower"))
        }),
        "the service root must retain its layered rectangular equipment plinth"
    );
    assert!(
        operations.iter().any(|operation| {
            operation["operation_id"]
                .as_str()
                .is_some_and(|operation_id| operation_id.ends_with("_plinth_blue_front"))
                && operation["op"] == "array"
                && operation["args"]["count"] == 4
        }),
        "the C108 base must retain four bounded front maintenance panels in one existing output"
    );
    assert!(
        operations.iter().any(|operation| {
            operation["operation_id"]
                .as_str()
                .is_some_and(|operation_id| operation_id.ends_with("_turntable_accent_ring_stack"))
                && operation["op"] == "array"
                && operation["args"]["count"] == 3
        }),
        "the unrotated turntable may carry a three-layer visual ring stack"
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| {
                operation["operation_id"]
                    .as_str()
                    .is_some_and(|operation_id| {
                        operation_id.ends_with("_link_armor_plate_base")
                            || operation_id.ends_with("_link_armor_right_base")
                    })
                    && operation["args"]["position"][2]
                        .as_f64()
                        .is_some_and(|offset| offset.abs() <= 100.0)
                    && operation["args"]["size"][1]
                        .as_f64()
                        .is_some_and(|depth| depth <= 200.0)
            })
            .count(),
        4,
        "both repeated links must keep their armor embedded around the inner frame instead of floating slabs"
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| {
                operation["operation_id"]
                    .as_str()
                    .is_some_and(|operation_id| {
                        operation_id.ends_with("_gripper_finger_a_pad")
                            || operation_id.ends_with("_gripper_finger_b_pad")
                    })
                    && operation["op"] == "surface_panel"
            })
            .count(),
        2,
        "the gripper must retain two bounded contact-surface panels without extra outputs"
    );
}
