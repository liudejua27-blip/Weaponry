use forgecad_core::{
    build_c111_forge_visual_program_fixture, build_c111_structural_detail_contract,
    builtin_surface_adornment_manifest_v2, builtin_surface_adornment_manifest_v3,
    c111_golden_surface_adornment_programs, c111_golden_surface_layer_program, ComponentRecipeRef,
    ForgeVisualProgramStage, RecipeExpander, RecipeExpansionPolicy, RecipeInstantiationRequest,
    RecipeRegistry, RecipeValidator, VisualBuildPass, VisualBuildStage, VisualConvergenceInput,
    VisualDetailCoverage, VisualDetailLevel, VisualDetailStatus, VisualFixedViewEvidence,
    VisualGlbReadbackEvidence, DESIGN_BUILD_LEDGER_SCHEMA_VERSION,
    VISUAL_CONVERGENCE_INPUT_SCHEMA_VERSION,
};
use std::collections::{BTreeMap, BTreeSet};

const ROOT_RECIPE_ID: &str = "recipe_c111_arm_golden_surface";
const C111_REVIEWED_ITERATION: u64 = 79;
const C111_REVIEWED_OPERATION_COUNT: usize = 204;
const C111_OUTPUT_BUDGET: usize = 128;
const C111B_GRIPPER_STRUCTURE_SUFFIXES: [&str; 12] = [
    "_gripper_knuckle_a",
    "_gripper_distal_knuckle_a",
    "_gripper_finger_a_armor",
    "_gripper_finger_a_pad",
    "_gripper_knuckle_b",
    "_gripper_distal_knuckle_b",
    "_gripper_finger_b_armor",
    "_gripper_finger_b_pad",
    "_gripper_knuckle_c",
    "_gripper_distal_knuckle_c",
    "_gripper_finger_c_armor",
    "_gripper_finger_c_panel",
];
const DETAIL_INVENTORY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/fixtures/c111-golden-surface-robotic-arm-visual-detail-inventory.json"
));

#[test]
fn pv002_c111_forge_visual_program_preserves_truth_and_seals_complete_inventory() {
    let registry = RecipeRegistry::from_embedded_c111_golden_surface_robotic_arm().unwrap();
    let candidate = RecipeExpander::expand(
        &registry,
        &request(&registry, "pack_robotic_arm_concept"),
        &RecipeExpansionPolicy::default(),
    )
    .unwrap();
    let programs = c111_golden_surface_adornment_programs(&candidate, &registry).unwrap();
    let inventory = serde_json::from_str(DETAIL_INVENTORY).unwrap();
    let fixture =
        build_c111_forge_visual_program_fixture(&candidate, &registry, &programs, &inventory)
            .unwrap();

    assert_eq!(
        inventory["compiled_evidence"]["iteration"].as_u64(),
        Some(C111_REVIEWED_ITERATION)
    );
    assert_eq!(fixture.program.stage, ForgeVisualProgramStage::Sealed);
    assert_eq!(fixture.program.parts.len(), 10);
    let geometry_outputs = fixture.program.geometry_graph["outputs"]
        .as_array()
        .unwrap();
    assert!(
        geometry_outputs.len() <= C111_OUTPUT_BUDGET,
        "iteration {} exported {} outputs, above the lightweight budget of {}",
        C111_REVIEWED_ITERATION,
        geometry_outputs.len(),
        C111_OUTPUT_BUDGET
    );
    let geometry_output_ids = geometry_outputs
        .iter()
        .map(|output| output["output_id"].as_str().expect("geometry output id"))
        .collect::<BTreeSet<_>>();
    let owned_output_ids = fixture
        .program
        .parts
        .iter()
        .flat_map(|part| part.geometry_output_ids.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    assert_eq!(geometry_output_ids.len(), geometry_outputs.len());
    assert_eq!(owned_output_ids, geometry_output_ids);
    assert_eq!(fixture.program.surface_graph.len(), 6);
    assert_eq!(fixture.program.detail_inventory.len(), 27);
    assert_eq!(fixture.fixed_views.len(), 8);
    assert_eq!(fixture.lowering.source_program_sha256.len(), 64);
    assert_eq!(
        fixture.lowering.shape_program,
        candidate.expanded_shape_program
    );
    assert_eq!(
        fixture.lowering.assembly_graph,
        candidate.expanded_assembly_graph
    );
    assert_eq!(fixture.sealed_status, "sealed_critical_details_complete");
    assert!(fixture.sealed_error_code.is_empty());
    assert!(fixture.critical_unresolved_detail_ids.is_empty());
    assert!(fixture
        .program
        .detail_inventory
        .iter()
        .filter(|detail| detail.critical && detail.status == VisualDetailStatus::Unresolved)
        .all(|detail| !detail.bindings.is_empty()));
    assert!(fixture
        .program
        .detail_inventory
        .iter()
        .filter(|detail| detail.status == VisualDetailStatus::Bound)
        .all(|detail| !detail.bindings.is_empty()));
}

#[test]
fn pv004_c111_real_glb_lineage_closes_fixed_build_and_eight_view_contract() {
    let registry = RecipeRegistry::from_embedded_c111_golden_surface_robotic_arm().unwrap();
    let candidate = RecipeExpander::expand(
        &registry,
        &request(&registry, "pack_robotic_arm_concept"),
        &RecipeExpansionPolicy::default(),
    )
    .unwrap();
    let programs = c111_golden_surface_adornment_programs(&candidate, &registry).unwrap();
    let inventory: serde_json::Value = serde_json::from_str(DETAIL_INVENTORY).unwrap();
    let fixture =
        build_c111_forge_visual_program_fixture(&candidate, &registry, &programs, &inventory)
            .unwrap();
    let compiled = &inventory["compiled_evidence"];
    let glb_sha256 = compiled["production_glb_sha256"]
        .as_str()
        .unwrap()
        .to_string();
    let mut previous = fixture.lowering.source_program_sha256.clone();
    let mut passes = Vec::new();
    for (index, stage) in VisualBuildStage::ORDERED.into_iter().enumerate() {
        let output = if index + 1 == VisualBuildStage::ORDERED.len() {
            glb_sha256.clone()
        } else {
            forgecad_core::semantic_sha256(&serde_json::json!({
                "stage": stage,
                "input_sha256": previous,
                "shape_program_sha256": fixture.expected_production["shape_program_sha256"],
            }))
            .unwrap()
        };
        passes.push(VisualBuildPass {
            stage,
            input_sha256: previous,
            output_sha256: output.clone(),
            completed: true,
        });
        previous = output;
    }
    let fixed_views = fixture
        .fixed_views
        .iter()
        .map(|view| VisualFixedViewEvidence {
            view_id: view["view_id"].as_str().unwrap().into(),
            glb_sha256: glb_sha256.clone(),
            renderer_id: "c111_fixed_eight_view_v2".into(),
            image_sha256: view["sha256"].as_str().unwrap().into(),
            readback_passed: true,
        })
        .collect();
    let bound_count = |level| {
        fixture
            .program
            .detail_inventory
            .iter()
            .filter(|detail| detail.level == level && detail.status == VisualDetailStatus::Bound)
            .count() as u32
    };
    let report = VisualConvergenceInput {
        schema_version: VISUAL_CONVERGENCE_INPUT_SCHEMA_VERSION.into(),
        ledger: forgecad_core::DesignBuildLedger {
            schema_version: DESIGN_BUILD_LEDGER_SCHEMA_VERSION.into(),
            source_program_sha256: fixture.lowering.source_program_sha256,
            source_revision: 1,
            passes,
        },
        readback: VisualGlbReadbackEvidence {
            glb_sha256,
            shape_program_sha256: fixture.expected_production["shape_program_sha256"]
                .as_str()
                .unwrap()
                .into(),
            triangle_count: fixture.expected_production["triangle_count"]
                .as_u64()
                .unwrap(),
            primitive_count: fixture.expected_production["primitive_count"]
                .as_u64()
                .unwrap(),
            material_zone_count: fixture.program.material_graph.len() as u64,
            closed_manifold: true,
            surface_provenance_present: true,
            pbr_channels_complete: true,
        },
        fixed_views,
        fixed_view_profile: forgecad_core::VisualFixedViewProfile::LegacyC111,
        detail_coverage: VisualDetailCoverage {
            macro_bound: bound_count(VisualDetailLevel::Macro),
            meso_bound: bound_count(VisualDetailLevel::Meso),
            micro_bound: bound_count(VisualDetailLevel::Micro),
            critical_unresolved: fixture.critical_unresolved_detail_ids.len() as u32,
        },
        reference_comparison: None,
        repairs: Vec::new(),
    }
    .evaluate()
    .unwrap();

    assert!(report.passed, "{:?}", report.failure_codes);
    assert_eq!(report.glb_sha256, fixture.expected_production["glb_sha256"]);
    assert_eq!(report.completed_stage_count, 7);
    assert_eq!(report.fixed_view_count, 8);
}

fn request(registry: &RecipeRegistry, domain_pack_id: &str) -> RecipeInstantiationRequest {
    let recipe = registry.recipe(ROOT_RECIPE_ID).expect("C111 reviewed root");
    RecipeInstantiationRequest {
        schema_version: "ComponentRecipeInstantiationRequest@1".into(),
        context_mode: "initial_candidate".into(),
        request_id: "recipereq_c111_golden_surface_contract".into(),
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
fn c111_golden_surface_registry_is_independent_and_expands_one_reviewed_arm() {
    let registry = RecipeRegistry::from_embedded_c111_golden_surface_robotic_arm().unwrap();
    let c106 = RecipeRegistry::from_embedded_c106_robotic_arm().unwrap();
    assert_eq!(
        registry.registry_id(),
        "registry_c111_golden_surface_robotic_arm_v1"
    );
    assert_eq!(registry.recipes().count(), 8);
    assert_ne!(registry.registry_sha256(), c106.registry_sha256());

    let candidate = RecipeExpander::expand(
        &registry,
        &request(&registry, "pack_robotic_arm_concept"),
        &RecipeExpansionPolicy::default(),
    )
    .unwrap();
    let parts = candidate.expanded_assembly_graph["parts"]
        .as_array()
        .unwrap();
    let connections = candidate.expanded_assembly_graph["connections"]
        .as_array()
        .unwrap();
    let operations = candidate.expanded_shape_program["operations"]
        .as_array()
        .unwrap();
    let outputs = candidate.expanded_shape_program["outputs"]
        .as_array()
        .unwrap();

    assert_eq!(parts.len(), 10);
    assert_eq!(connections.len(), 9);
    assert_eq!(candidate.component_recipe_instances.len(), 10);
    assert_eq!(operations.len(), C111_REVIEWED_OPERATION_COUNT);
    assert!(
        outputs.len() <= C111_OUTPUT_BUDGET,
        "iteration {} exported {} outputs, above the lightweight budget of {}",
        C111_REVIEWED_ITERATION,
        outputs.len(),
        C111_OUTPUT_BUDGET
    );
    let output_ids = outputs
        .iter()
        .map(|output| output["output_id"].as_str().expect("shape output id"))
        .collect::<BTreeSet<_>>();
    assert_eq!(output_ids.len(), outputs.len());
    assert!(connections
        .iter()
        .all(|connection| connection["status"] == "connected"));
    assert!(candidate.component_recipe_instances.iter().all(|instance| {
        instance.source["source_kind"] == "forgecad_first_party"
            && instance.source["source_id"] == "source_c111_arm"
            && instance.license["license_id"] == "ForgeCAD-Internal-Visual-Only"
            && instance.quality_status == "passed"
            && instance.non_functional_only
    }));

    let roles = parts
        .iter()
        .filter_map(|part| part["role"].as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        roles,
        BTreeSet::from([
            "base_form",
            "cable_harness",
            "end_effector_form",
            "joint_housing",
            "link_armor",
            "surface_trim",
            "turntable",
        ])
    );
    let operation_names = operations
        .iter()
        .filter_map(|operation| operation["op"].as_str())
        .collect::<BTreeSet<_>>();
    for required in [
        "loft",
        "sweep",
        "revolve",
        "bevel_approx",
        "surface_panel",
        "radial_array",
    ] {
        assert!(
            operation_names.contains(required),
            "C111 must retain {required}"
        );
    }
    for required_suffix in [
        "_base_shell",
        "_plinth_fastener_array",
        "_plinth_guard_array",
        "_plinth_service_panel",
        "_joint_inner_bearing",
        "_joint_signal_core",
        "_joint_outer_ring_secondary",
        "_joint_upper_guard",
        "_joint_lower_guard",
        "_wrist_outer_ring_secondary",
        "_wrist_upper_guard",
        "_wrist_lower_guard",
        "_link_continuous_shell",
        "_link_panel_seam_a",
        "_link_upper_tension_rod",
        "_link_frame_rail_upper",
        "_link_frame_rail_lower",
        "_link_frame_cross_brace",
        "_cable_a",
        "_cable_clamp_bridge",
        "_gripper_wrist_collar",
        "_gripper_palm_armor_a",
        "_gripper_knuckle_a",
        "_gripper_knuckle_c",
        "_gripper_finger_tip_a",
        "_gripper_finger_c_panel",
        "_gripper_finger_a_sweep",
        "_gripper_finger_b_sweep",
        "_gripper_finger_c_sweep",
        "_gripper_finger_tip_c",
    ] {
        assert!(
            operations.iter().any(|operation| {
                operation["operation_id"]
                    .as_str()
                    .is_some_and(|operation_id| operation_id.ends_with(required_suffix))
            }),
            "C111 visible layer must retain {required_suffix}"
        );
    }
}

#[test]
fn c111_golden_surface_is_an_explicit_a005_v3_grant_and_fails_closed_cross_domain() {
    let registry = RecipeRegistry::from_embedded_c111_golden_surface_robotic_arm().unwrap();
    let c111_recipe_ids = registry
        .recipes()
        .map(|recipe| recipe.recipe_id.as_str())
        .collect::<BTreeSet<_>>();
    let c111_surface_recipe_ids = registry
        .recipes()
        .filter(|recipe| !recipe.surface_adornment_slots.is_empty())
        .map(|recipe| recipe.recipe_id.as_str())
        .collect::<BTreeSet<_>>();
    let v2 = builtin_surface_adornment_manifest_v2();
    let v3 = builtin_surface_adornment_manifest_v3();
    assert!(c111_recipe_ids
        .iter()
        .all(|recipe_id| !v2.recipe_ids.iter().any(|id| id == recipe_id)));
    assert!(c111_surface_recipe_ids
        .iter()
        .all(|recipe_id| v3.recipe_ids.iter().any(|id| id == recipe_id)));
    assert!(
        !v3.recipe_ids
            .iter()
            .any(|id| id == "recipe_c111_arm_wrist_housing"),
        "the dedicated wrist has no A005 slot and must not consume a Skill grant"
    );
    assert_ne!(
        v2.canonical_sha256().unwrap(),
        v3.canonical_sha256().unwrap()
    );

    let error = RecipeExpander::expand(
        &registry,
        &request(&registry, "pack_vehicle_concept"),
        &RecipeExpansionPolicy::default(),
    )
    .unwrap_err();
    assert_eq!(error.code(), "COMPONENT_RECIPE_DOMAIN_INCOMPATIBLE");
}

#[test]
fn c111_golden_surface_binds_exactly_six_reviewed_a005_programs() {
    let registry = RecipeRegistry::from_embedded_c111_golden_surface_robotic_arm().unwrap();
    let candidate = RecipeExpander::expand(
        &registry,
        &request(&registry, "pack_robotic_arm_concept"),
        &RecipeExpansionPolicy::default(),
    )
    .unwrap();
    let programs = c111_golden_surface_adornment_programs(&candidate, &registry).unwrap();
    let surface_layer = c111_golden_surface_layer_program(&candidate, &registry).unwrap();
    let manifest = builtin_surface_adornment_manifest_v3();
    let manifest_sha256 = manifest.canonical_sha256().unwrap();

    assert_eq!(programs.len(), 6);
    let program_ids = programs
        .iter()
        .map(|program| program.program_id.as_str())
        .collect::<BTreeSet<_>>();
    for stable_id in [
        "adorn_c111_base_flowline",
        "adorn_c111_gripper_chevron",
        "adorn_c111_gripper_microgrid",
        "adorn_c111_joint_microgrid",
        "adorn_c111_link_groove",
    ] {
        assert!(program_ids.contains(stable_id));
    }
    assert_eq!(
        programs
            .iter()
            .filter(|program| program.target_zone_id == "zone_arm_link_armor")
            .count(),
        1
    );
    assert!(programs.iter().all(|program| {
        program.skill_id == manifest.skill_id
            && program.skill_version == 3
            && program.skill_sha256 == manifest_sha256
            && program.execution == "texture_bake"
            && program.non_functional_only
    }));
    assert_eq!(surface_layer.target_zone_id, "zone_arm_link_armor");
    assert_eq!(surface_layer.decal_layers.len(), 2);
    assert_eq!(surface_layer.roughness_masks.len(), 2);
    assert_eq!(surface_layer.uv_frame.rotation_degrees, 90.0);
    assert!(surface_layer
        .decal_layers
        .iter()
        .any(|layer| layer.text_token == "A-01"));
    assert!(surface_layer
        .roughness_masks
        .iter()
        .any(|mask| mask.motif == "edge_wear"));
    assert!(surface_layer
        .roughness_masks
        .iter()
        .any(|mask| mask.motif == "linear_brush"));
    assert_eq!(
        programs
            .iter()
            .map(|program| program.target_zone_id.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "zone_arm_base_paint",
            "zone_arm_gripper",
            "zone_arm_gripper_paint",
            "zone_arm_joint_shell",
            "zone_arm_link_armor",
            "zone_arm_link_shell",
        ])
    );
}

#[test]
fn c111b_structural_detail_contract_is_exact_and_fails_closed() {
    let registry = RecipeRegistry::from_embedded_c111_golden_surface_robotic_arm().unwrap();
    let candidate = RecipeExpander::expand(
        &registry,
        &request(&registry, "pack_robotic_arm_concept"),
        &RecipeExpansionPolicy::default(),
    )
    .unwrap();
    let programs = c111_golden_surface_adornment_programs(&candidate, &registry).unwrap();
    let surface_layer = c111_golden_surface_layer_program(&candidate, &registry).unwrap();
    let inventory = serde_json::from_str(DETAIL_INVENTORY).unwrap();
    let fixture =
        build_c111_forge_visual_program_fixture(&candidate, &registry, &programs, &inventory)
            .unwrap();
    let contract =
        build_c111_structural_detail_contract(&fixture.program, &programs, &surface_layer).unwrap();
    assert_eq!(contract.schema_version, "C111StructuralDetailContract@1");
    assert_eq!(contract.lineages.len(), 7);
    assert_eq!(
        contract
            .lineages
            .iter()
            .map(|lineage| lineage.detail_class.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "auxiliary_linkage",
            "cable_clamps",
            "decal",
            "gripper_hinges",
            "joint_stack",
            "service_panel",
            "wear",
        ])
    );
    let cable_lineage = contract
        .lineages
        .iter()
        .find(|lineage| lineage.detail_class == "cable_clamps")
        .unwrap();
    assert!(
        cable_lineage
            .geometry_output_ids
            .iter()
            .filter(|output_id| {
                output_id.ends_with("_cable_a") || output_id.ends_with("_cable_b")
            })
            .count()
            >= 2
    );
    assert!(
        cable_lineage
            .geometry_output_ids
            .iter()
            .filter(|output_id| output_id.contains("_cable_clamp_"))
            .count()
            >= 2
    );
    let gripper_lineage = contract
        .lineages
        .iter()
        .find(|lineage| lineage.detail_class == "gripper_hinges")
        .unwrap();
    assert_eq!(
        gripper_lineage.geometry_output_ids.len(),
        C111B_GRIPPER_STRUCTURE_SUFFIXES.len()
    );
    for required_suffix in C111B_GRIPPER_STRUCTURE_SUFFIXES {
        assert_eq!(
            gripper_lineage
                .geometry_output_ids
                .iter()
                .filter(|output_id| output_id.ends_with(required_suffix))
                .count(),
            1,
            "iteration {} must retain exactly one {required_suffix} output",
            C111_REVIEWED_ITERATION
        );
    }

    let mut duplicate_lineage_output = contract.clone();
    let cable_lineage = duplicate_lineage_output
        .lineages
        .iter_mut()
        .find(|lineage| lineage.detail_class == "cable_clamps")
        .unwrap();
    cable_lineage
        .geometry_output_ids
        .push(cable_lineage.geometry_output_ids[0].clone());
    let error = duplicate_lineage_output
        .validate(&fixture.program, &surface_layer)
        .unwrap_err();
    assert_eq!(error.code(), "C111_STRUCTURAL_DETAIL_MISSING");

    let mut one_clamp = fixture.program.clone();
    let retained_clamp_id = one_clamp.geometry_graph["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|output| {
            output["output_id"]
                .as_str()
                .filter(|output_id| output_id.contains("_cable_clamp_"))
        })
        .unwrap()
        .to_owned();
    for part in &mut one_clamp.parts {
        part.geometry_output_ids.retain(|output_id| {
            !output_id.contains("_cable_clamp_") || output_id == &retained_clamp_id
        });
    }
    one_clamp.geometry_graph["outputs"]
        .as_array_mut()
        .unwrap()
        .retain(|output| {
            output["output_id"].as_str().is_none_or(|output_id| {
                !output_id.contains("_cable_clamp_") || output_id == retained_clamp_id
            })
        });
    let error =
        build_c111_structural_detail_contract(&one_clamp, &programs, &surface_layer).unwrap_err();
    assert_eq!(error.code(), "C111_STRUCTURAL_DETAIL_MISSING");
    assert!(error
        .to_string()
        .contains("at least 2 distinct clamp outputs"));

    let mut one_rubber_cable = fixture.program.clone();
    let retained_rubber_output_id = one_rubber_cable.geometry_graph["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|output| {
            output["output_id"]
                .as_str()
                .filter(|output_id| output_id.ends_with("_cable_a"))
        })
        .unwrap()
        .to_owned();
    for part in &mut one_rubber_cable.parts {
        part.geometry_output_ids.retain(|output_id| {
            output_id == &retained_rubber_output_id
                || (!output_id.ends_with("_cable_a") && !output_id.ends_with("_cable_b"))
        });
    }
    one_rubber_cable.geometry_graph["outputs"]
        .as_array_mut()
        .unwrap()
        .retain(|output| {
            output["output_id"].as_str().is_none_or(|output_id| {
                output_id == retained_rubber_output_id
                    || (!output_id.ends_with("_cable_a") && !output_id.ends_with("_cable_b"))
            })
        });
    let error = build_c111_structural_detail_contract(&one_rubber_cable, &programs, &surface_layer)
        .unwrap_err();
    assert_eq!(error.code(), "C111_STRUCTURAL_DETAIL_MISSING");
    assert!(error
        .to_string()
        .contains("at least 2 distinct rubber cable outputs"));

    let mut missing_panel = fixture.program.clone();
    for part in &mut missing_panel.parts {
        part.geometry_output_ids
            .retain(|output_id| !output_id.ends_with("_plinth_service_panel"));
    }
    missing_panel.geometry_graph["outputs"]
        .as_array_mut()
        .unwrap()
        .retain(|output| {
            !output["output_id"]
                .as_str()
                .is_some_and(|output_id| output_id.ends_with("_plinth_service_panel"))
        });
    let error = build_c111_structural_detail_contract(&missing_panel, &programs, &surface_layer)
        .unwrap_err();
    assert_eq!(error.code(), "C111_STRUCTURAL_DETAIL_MISSING");

    let mut missing_wear = surface_layer.clone();
    missing_wear
        .roughness_masks
        .retain(|mask| mask.motif != "edge_wear");
    let error = build_c111_structural_detail_contract(&fixture.program, &programs, &missing_wear)
        .unwrap_err();
    assert_eq!(error.code(), "C111_STRUCTURAL_DETAIL_MISSING");

    for required_suffix in C111B_GRIPPER_STRUCTURE_SUFFIXES {
        let mut missing_gripper_detail = fixture.program.clone();
        for part in &mut missing_gripper_detail.parts {
            part.geometry_output_ids
                .retain(|output_id| !output_id.ends_with(required_suffix));
        }
        missing_gripper_detail.geometry_graph["outputs"]
            .as_array_mut()
            .unwrap()
            .retain(|output| {
                !output["output_id"]
                    .as_str()
                    .is_some_and(|output_id| output_id.ends_with(required_suffix))
            });
        let error = build_c111_structural_detail_contract(
            &missing_gripper_detail,
            &programs,
            &surface_layer,
        )
        .unwrap_err();
        assert_eq!(
            error.code(),
            "C111_STRUCTURAL_DETAIL_MISSING",
            "missing {required_suffix} must fail closed"
        );
    }

    let mut wrong_cable_material = fixture.program.clone();
    wrong_cable_material.geometry_graph["operations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|operation| {
            operation["operation_id"]
                .as_str()
                .is_some_and(|operation_id| operation_id.ends_with("_cable_b"))
        })
        .unwrap()["args"]["material_id"] = serde_json::json!("mat_aluminum");
    let error =
        build_c111_structural_detail_contract(&wrong_cable_material, &programs, &surface_layer)
            .unwrap_err();
    assert_eq!(error.code(), "C111_STRUCTURAL_DETAIL_MISSING");

    let mut detached_contact = fixture.program.clone();
    detached_contact.geometry_graph["operations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|operation| {
            operation["operation_id"]
                .as_str()
                .is_some_and(|operation_id| operation_id.ends_with("_gripper_finger_c_panel"))
        })
        .unwrap()["inputs"] = serde_json::json!([]);
    let error = build_c111_structural_detail_contract(&detached_contact, &programs, &surface_layer)
        .unwrap_err();
    assert_eq!(error.code(), "C111_STRUCTURAL_DETAIL_MISSING");

    let mut non_mesh_contact = fixture.program.clone();
    non_mesh_contact.geometry_graph["outputs"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|output| {
            output["output_id"]
                .as_str()
                .is_some_and(|output_id| output_id.ends_with("_gripper_finger_a_pad"))
        })
        .unwrap()["kind"] = serde_json::json!("profile");
    let error = build_c111_structural_detail_contract(&non_mesh_contact, &programs, &surface_layer)
        .unwrap_err();
    assert_eq!(error.code(), "C111_STRUCTURAL_DETAIL_MISSING");

    let mut detached_clamp = fixture.program.clone();
    let detached_clamp_operation_id = detached_clamp.geometry_graph["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|output| {
            output["output_id"]
                .as_str()
                .filter(|output_id| output_id.contains("_cable_clamp_"))
                .and_then(|_| output["operation_id"].as_str())
        })
        .unwrap()
        .to_owned();
    detached_clamp.geometry_graph["operations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|operation| {
            operation["operation_id"]
                .as_str()
                .is_some_and(|operation_id| operation_id == detached_clamp_operation_id)
        })
        .unwrap()["inputs"] = serde_json::json!([]);
    let error = build_c111_structural_detail_contract(&detached_clamp, &programs, &surface_layer)
        .unwrap_err();
    assert_eq!(error.code(), "C111_STRUCTURAL_DETAIL_MISSING");
}

#[test]
fn c111_authored_operations_match_their_declared_material_zones() {
    let registry = RecipeRegistry::from_embedded_c111_golden_surface_robotic_arm().unwrap();
    for recipe in registry.recipes() {
        let materials_by_zone = recipe
            .material_zones
            .iter()
            .map(|zone| {
                (
                    zone["zone_id"].as_str().expect("material zone id"),
                    zone["material_preset_id"]
                        .as_str()
                        .expect("material preset id"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let operations = recipe.shape_program_template["operations"]
            .as_array()
            .expect("shape operations");
        for operation in operations {
            let args = &operation["args"];
            let (Some(zone_id), Some(material_id)) =
                (args["zone_id"].as_str(), args["material_id"].as_str())
            else {
                continue;
            };
            assert_eq!(
                materials_by_zone.get(zone_id).copied(),
                Some(material_id),
                "{} operation {} must use the material declared by {}",
                recipe.recipe_id,
                operation["operation_id"].as_str().unwrap_or("unknown"),
                zone_id
            );
        }
    }
}

#[test]
fn c111_every_declared_material_zone_has_a_rendered_output() {
    let registry = RecipeRegistry::from_embedded_c111_golden_surface_robotic_arm().unwrap();
    for recipe in registry.recipes() {
        let operations = recipe.shape_program_template["operations"]
            .as_array()
            .expect("shape operations")
            .iter()
            .map(|operation| {
                (
                    operation["operation_id"].as_str().expect("operation id"),
                    &operation["args"],
                )
            })
            .collect::<BTreeMap<_, _>>();
        let rendered_zones = recipe.shape_program_template["outputs"]
            .as_array()
            .expect("shape outputs")
            .iter()
            .map(|output| {
                let operation_id = output["operation_id"]
                    .as_str()
                    .expect("output operation id");
                operations[operation_id]["zone_id"]
                    .as_str()
                    .expect("output material zone")
            })
            .collect::<BTreeSet<_>>();
        for zone in &recipe.material_zones {
            let zone_id = zone["zone_id"].as_str().expect("material zone id");
            assert!(
                rendered_zones.contains(zone_id),
                "{} declares {}, but no rendered output preserves that zone",
                recipe.recipe_id,
                zone_id
            );
        }
    }
}
