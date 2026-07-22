use forgecad_core::{
    lower_assembly_delta, materialize_assembly_delta, AgentAssetVersion, AssemblyDeltaProgram,
    AssetStage, AssetVersionStatus, ComponentRecipeRef, RecipeExpander, RecipeExpansionPolicy,
    RecipeInstantiationRequest, RecipeRegistry, RecipeValidator,
};
use serde_json::json;

fn transform() -> serde_json::Value {
    json!({"position":[0.0, 12.0, 0.0], "rotation":[0.0, 0.2, 0.0], "scale":[1.0, 1.0, 1.0]})
}

fn c110c_base() -> AgentAssetVersion {
    let registry = RecipeRegistry::from_embedded_c106_robotic_arm().unwrap();
    let root = registry.recipe("recipe_c106_arm_service_display").unwrap();
    let initial = RecipeExpander::expand(
        &registry,
        &RecipeInstantiationRequest {
            schema_version: "ComponentRecipeInstantiationRequest@1".into(),
            context_mode: "initial_candidate".into(),
            request_id: "recipereq_c110c_base".into(),
            project_id: None,
            base_asset_version_id: None,
            snapshot_revision: None,
            domain_pack_id: "pack_robotic_arm_concept".into(),
            recipe_registry_sha256: registry.registry_sha256().into(),
            recipe: ComponentRecipeRef {
                schema_version: "ComponentRecipeRef@1".into(),
                recipe_id: root.recipe_id.clone(),
                version: root.version,
                recipe_sha256: RecipeValidator::recipe_sha256(root).unwrap(),
            },
            target_part_id: None,
            slot_bindings: vec![],
            parameter_values: vec![],
            material_zone_overrides: vec![],
        },
        &RecipeExpansionPolicy::default(),
    )
    .unwrap();
    AgentAssetVersion {
        asset_version_id: "assetver_c110c_base".into(),
        project_id: "prj_c110c".into(),
        parent_asset_version_id: None,
        version_no: 1,
        status: AssetVersionStatus::Committed,
        summary: "C110C base".into(),
        stage: AssetStage::EditableAsset,
        plan_id: "plan_c110c".into(),
        direction_id: "direction_c110c".into(),
        domain_pack_id: "pack_robotic_arm_concept".into(),
        artifact_id: "artifact_c110c".into(),
        parts: initial.expanded_assembly_graph["parts"]
            .as_array()
            .unwrap()
            .clone(),
        shape_program: initial.expanded_shape_program,
        assembly_graph: initial.expanded_assembly_graph,
        material_bindings: Default::default(),
        created_at: "2026-07-21T00:00:00Z".into(),
    }
}

fn c110g_base() -> AgentAssetVersion {
    let registry = RecipeRegistry::from_embedded_c110g_parallel_link().unwrap();
    let root = registry.recipe("recipe_c110g_parallel_link_root").unwrap();
    let initial = RecipeExpander::expand(
        &registry,
        &RecipeInstantiationRequest {
            schema_version: "ComponentRecipeInstantiationRequest@1".into(),
            context_mode: "initial_candidate".into(),
            request_id: "recipereq_c110g_base".into(),
            project_id: None,
            base_asset_version_id: None,
            snapshot_revision: None,
            domain_pack_id: "pack_robotic_arm_concept".into(),
            recipe_registry_sha256: registry.registry_sha256().into(),
            recipe: ComponentRecipeRef {
                schema_version: "ComponentRecipeRef@1".into(),
                recipe_id: root.recipe_id.clone(),
                version: root.version,
                recipe_sha256: RecipeValidator::recipe_sha256(root).unwrap(),
            },
            target_part_id: None,
            slot_bindings: vec![],
            parameter_values: vec![],
            material_zone_overrides: vec![],
        },
        &RecipeExpansionPolicy::default(),
    )
    .unwrap();
    AgentAssetVersion {
        asset_version_id: "assetver_c110g_base".into(),
        project_id: "prj_c110g".into(),
        parent_asset_version_id: None,
        version_no: 1,
        status: AssetVersionStatus::Committed,
        summary: "C110G base".into(),
        stage: AssetStage::EditableAsset,
        plan_id: "plan_c110g".into(),
        direction_id: "direction_c110g".into(),
        domain_pack_id: "pack_robotic_arm_concept".into(),
        artifact_id: "artifact_c110g".into(),
        parts: initial.expanded_assembly_graph["parts"]
            .as_array()
            .unwrap()
            .clone(),
        shape_program: initial.expanded_shape_program,
        assembly_graph: initial.expanded_assembly_graph,
        material_bindings: Default::default(),
        created_at: "2026-07-22T00:00:00Z".into(),
    }
}

#[test]
fn lowers_add_pose_and_snap_as_one_bounded_delta() {
    let value = json!({
        "schema_version": "AssemblyDeltaProgram@1",
        "domain_pack_id": "pack_robotic_arm_concept",
        "base_asset_version_id": "assetver_arm_v1",
        "summary": "Add a sensor pod and pose the wrist for inspection.",
        "visual_only": true,
        "operations": [
            {"op":"add_reviewed_recipe", "operation_id":"delta_add_sensor", "new_part_id":"part_sensor_pod", "parent_part_id":"part_upper_link", "parent_connector_id":"connector_arm_sensor", "child_connector_id":"connector_cable_mount", "recipe_id":"recipe_c106_arm_cable_harness", "slot_id":"slot_arm_sensor_pod", "transform": transform()},
            {"op":"set_joint_pose", "operation_id":"delta_pose_wrist", "part_id":"part_wrist", "joint_id":"joint_wrist", "pose":{"rotation":[0.0,0.4,0.0],"translation":[0.0,0.0,0.0]}},
            {"op":"snap_part_to_connector", "operation_id":"delta_snap_sensor", "part_id":"part_sensor_pod", "target_part_id":"part_upper_link", "target_connector_id":"connector_arm_sensor", "connector_id":"connector_cable_mount"}
        ]
    });
    let lowering = lower_assembly_delta(&value).expect("bounded delta should lower");
    assert_eq!(lowering.status, "lowered");
    assert_eq!(lowering.operations.len(), 3);
    assert_eq!(lowering.operations[0]["op"], "add_reviewed_recipe");
    assert_eq!(lowering.intent_sha256.len(), 64);
    let parsed: AssemblyDeltaProgram = serde_json::from_value(value).unwrap();
    parsed.validate().unwrap();
}

#[test]
fn rejects_unknown_recipe_and_non_visual_fields() {
    let value = json!({
        "schema_version": "AssemblyDeltaProgram@1",
        "domain_pack_id": "pack_robotic_arm_concept",
        "base_asset_version_id": "assetver_arm_v1",
        "summary": "unsafe",
        "visual_only": true,
        "operations": [{"op":"replace_reviewed_recipe", "operation_id":"replace", "part_id":"part_wrist", "recipe_id":"recipe_not_reviewed"}]
    });
    let error = lower_assembly_delta(&value).unwrap_err();
    assert_eq!(error.code(), "ASSEMBLY_DELTA_INVALID");
}

#[test]
fn rejects_duplicate_operation_ids_and_unknown_fields() {
    let value = json!({
        "schema_version": "AssemblyDeltaProgram@1",
        "domain_pack_id": "pack_robotic_arm_concept",
        "base_asset_version_id": "assetver_arm_v1",
        "summary": "pose twice",
        "visual_only": true,
        "operations": [
            {"op":"set_part_transform", "operation_id":"same", "part_id":"part_wrist", "transform": transform()},
            {"op":"set_part_transform", "operation_id":"same", "part_id":"part_elbow", "transform": transform(), "engineering_dimension": 4}
        ]
    });
    let error = lower_assembly_delta(&value).unwrap_err();
    assert_eq!(error.code(), "ASSEMBLY_DELTA_INVALID");
}

#[test]
fn materializes_sensor_attachment_into_shape_and_assembly_graph() {
    let base = c110c_base();
    let parent_part_id = base.assembly_graph["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|part| {
            part["connectors"].as_array().is_some_and(|connectors| {
                connectors
                    .iter()
                    .any(|connector| connector["connector_id"] == "connector_service_upper_link")
            })
        })
        .expect("C110C fixture exposes the upper-link connector")["part_id"]
        .as_str()
        .unwrap();
    let delta = json!({
        "schema_version": "AssemblyDeltaProgram@1",
        "domain_pack_id": "pack_robotic_arm_concept",
        "base_asset_version_id": "assetver_c110c_base",
        "summary": "Add a sensor pod to the upper link.",
        "visual_only": true,
        "operations": [{
            "op":"add_reviewed_recipe", "operation_id":"delta_add_sensor", "new_part_id":"part_c110c_sensor_pod", "parent_part_id": parent_part_id, "parent_connector_id":"connector_service_upper_link", "child_connector_id":"connector_sensor_pod_mount", "recipe_id":"recipe_c110c_arm_sensor_pod", "slot_id":"slot_arm_sensor_pod", "transform": transform()
        }]
    });
    let materialized = materialize_assembly_delta(&base, &delta).unwrap();
    assert_eq!(materialized.parts.len(), base.parts.len() + 1);
    assert!(materialized.shape_program["operations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|operation| operation["operation_id"]
            .as_str()
            .unwrap()
            .contains("sensor_pod")));
    assert!(materialized.assembly_graph["parts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|part| part["part_id"] == "part_c110c_sensor_pod"));
    assert!(materialized.assembly_graph["connections"]
        .as_array()
        .unwrap()
        .iter()
        .any(|connection| connection["to_part_id"] == "part_c110c_sensor_pod"));
}

#[test]
fn materializes_c110g_attachment_on_the_independent_parallel_link_arm() {
    let base = c110g_base();
    let root = base.assembly_graph["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|part| part["parent_part_id"].is_null())
        .unwrap();
    let root_part_id = root["part_id"].as_str().unwrap();
    let delta = json!({
        "schema_version": "AssemblyDeltaProgram@1",
        "domain_pack_id": "pack_robotic_arm_concept",
        "base_asset_version_id": base.asset_version_id,
        "summary": "Add a reviewed parallel-link visual strut.",
        "visual_only": true,
        "operations": [{
            "op":"add_reviewed_recipe",
            "operation_id":"delta_c110g_add_link",
            "new_part_id":"part_c110g_added_link",
            "parent_part_id":root_part_id,
            "parent_connector_id":"connector_parallel_carriage",
            "child_connector_id":"connector_parallel_link_mount",
            "recipe_id":"recipe_c110g_parallel_link",
            "slot_id":"slot_c110g_parallel_link",
            "transform":transform()
        }]
    });
    let materialized = materialize_assembly_delta(&base, &delta).unwrap();
    assert_eq!(materialized.parts.len(), base.parts.len() + 1);
    let added = materialized.assembly_graph["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|part| part["part_id"] == "part_c110g_added_link")
        .unwrap();
    assert_eq!(added["role"], "link_armor");
    assert!(materialized.assembly_graph["connections"]
        .as_array()
        .unwrap()
        .iter()
        .any(
            |connection| connection["to_part_id"] == "part_c110g_added_link"
                && connection["slot_id"] == "slot_c110g_parallel_link"
        ));
    assert!(materialized.assembly_graph["component_recipe_instances"]
        .as_array()
        .unwrap()
        .iter()
        .any(|instance| instance["recipe"]["recipe_id"] == "recipe_c110g_parallel_link"));
}

#[test]
fn c110d_attachment_registry_exposes_three_composable_visual_recipes() {
    let registry = RecipeRegistry::from_embedded_c110c_robotic_arm_attachments().unwrap();
    let recipes = [
        ("recipe_c110c_arm_sensor_pod", "connector_sensor_pod_mount"),
        (
            "recipe_c110d_arm_actuator_cover",
            "connector_actuator_cover_mount",
        ),
        (
            "recipe_c110d_arm_cable_guide",
            "connector_cable_guide_mount",
        ),
        (
            "recipe_c110d_arm_wrist_tool_mount",
            "connector_wrist_tool_mount",
        ),
    ];
    assert_eq!(registry.recipes().count(), recipes.len());
    for (recipe_id, connector_id) in recipes {
        let recipe = registry.recipe(recipe_id).expect("reviewed C110D Recipe");
        assert_eq!(recipe.allowed_domains, ["pack_robotic_arm_concept"]);
        assert!(recipe.non_functional_only);
        assert!(recipe.triangle_estimate >= 100);
        assert!(recipe
            .connectors
            .iter()
            .any(|connector| connector.connector_id == connector_id));
    }
}

#[test]
fn materializes_two_different_c110d_attachments_on_the_same_arm() {
    let base = c110c_base();
    let parent_part_id = base.assembly_graph["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|part| {
            part["connectors"].as_array().is_some_and(|connectors| {
                connectors
                    .iter()
                    .any(|connector| connector["connector_id"] == "connector_service_upper_link")
            })
        })
        .unwrap()["part_id"]
        .as_str()
        .unwrap();
    let delta = json!({
        "schema_version": "AssemblyDeltaProgram@1",
        "domain_pack_id": "pack_robotic_arm_concept",
        "base_asset_version_id": base.asset_version_id,
        "summary": "Add an actuator cover and cable guide to the same arm.",
        "visual_only": true,
        "operations": [
            {"op":"add_reviewed_recipe","operation_id":"delta_add_actuator_cover","new_part_id":"part_c110d_actuator_cover","parent_part_id":parent_part_id,"parent_connector_id":"connector_service_upper_link","child_connector_id":"connector_actuator_cover_mount","recipe_id":"recipe_c110d_arm_actuator_cover","slot_id":"slot_arm_guard_rail","transform":transform()},
            {"op":"add_reviewed_recipe","operation_id":"delta_add_cable_guide","new_part_id":"part_c110d_cable_guide","parent_part_id":parent_part_id,"parent_connector_id":"connector_service_upper_link","child_connector_id":"connector_cable_guide_mount","recipe_id":"recipe_c110d_arm_cable_guide","slot_id":"slot_arm_camera_boom","transform":{"position":[0.0,24.0,0.0],"rotation":[0.0,0.1,0.0],"scale":[1.0,1.0,1.0]}}
        ]
    });
    let materialized = materialize_assembly_delta(&base, &delta).unwrap();
    assert_eq!(materialized.parts.len(), base.parts.len() + 2);
    for part_id in ["part_c110d_actuator_cover", "part_c110d_cable_guide"] {
        assert!(materialized.assembly_graph["parts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|part| part["part_id"] == part_id));
        assert!(materialized.assembly_graph["connections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|connection| connection["to_part_id"] == part_id));
    }
    assert!(materialized.shape_program["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|output| output["output_id"]
            .as_str()
            .is_some_and(|id| id.contains("actuator_cover"))));
    assert!(materialized.shape_program["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|output| output["output_id"]
            .as_str()
            .is_some_and(|id| id.contains("cable_guide"))));
}

#[test]
fn materializes_transform_pose_and_connector_snap_with_geometry_changes() {
    let base = c110c_base();
    let target_part = base.assembly_graph["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|part| part["role"] == "joint_housing")
        .expect("C106 arm exposes a reviewed joint housing");
    let part_id = target_part["part_id"].as_str().unwrap();
    let source_connector_id = target_part["connectors"][0]["connector_id"]
        .as_str()
        .unwrap();
    let root = base.assembly_graph["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|part| part["parent_part_id"].is_null())
        .unwrap();
    let root_connector_id = root["connectors"][0]["connector_id"].as_str().unwrap();
    let delta = json!({
        "schema_version": "AssemblyDeltaProgram@1",
        "domain_pack_id": "pack_robotic_arm_concept",
        "base_asset_version_id": base.asset_version_id,
        "summary": "Pose and snap a reviewed arm joint.",
        "visual_only": true,
        "operations": [
            {"op":"set_part_transform","operation_id":"delta_transform_joint","part_id":part_id,"transform":{"position":[20.0,30.0,40.0],"rotation":[0.0,0.0,0.2],"scale":[1.0,1.0,1.0]}},
            {"op":"set_joint_pose","operation_id":"delta_pose_joint","part_id":part_id,"joint_id":"joint_visual_wrist","pose":{"rotation":[0.0,0.15,0.0],"translation":[5.0,0.0,0.0]}},
            {"op":"snap_part_to_connector","operation_id":"delta_snap_joint","part_id":part_id,"target_part_id":root["part_id"],"target_connector_id":root_connector_id,"connector_id":source_connector_id}
        ]
    });
    let materialized = materialize_assembly_delta(&base, &delta).unwrap();
    let moved = materialized.assembly_graph["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|part| part["part_id"] == part_id)
        .unwrap();
    assert!(moved["joints"]
        .as_array()
        .is_some_and(|joints| !joints.is_empty()));
    assert_ne!(moved["transform"], target_part["transform"]);
    let base_operations = base.shape_program["operations"].as_array().unwrap();
    let moved_operations = materialized.shape_program["operations"].as_array().unwrap();
    assert!(moved_operations.iter().any(|operation| {
        let Some(before) = base_operations
            .iter()
            .find(|candidate| candidate["operation_id"] == operation["operation_id"])
        else {
            return false;
        };
        operation["operation_id"]
            .as_str()
            .is_some_and(|id| id.starts_with("op_"))
            && (operation["args"]["position"] != before["args"]["position"]
                || operation["args"]["rotation"] != before["args"]["rotation"])
    }));
}

#[test]
fn materializes_reviewed_recipe_replacement_without_changing_part_identity() {
    let base = c110c_base();
    let target = base.assembly_graph["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|part| part["role"] == "link_armor")
        .expect("C106 arm exposes a replaceable link");
    let part_id = target["part_id"].as_str().unwrap();
    let old_operation = target["operation_id"].as_str().unwrap();
    let delta = json!({
        "schema_version": "AssemblyDeltaProgram@1",
        "domain_pack_id": "pack_robotic_arm_concept",
        "base_asset_version_id": base.asset_version_id,
        "summary": "Replace one visual link with a reviewed joint housing.",
        "visual_only": true,
        "operations": [{
            "op":"replace_reviewed_recipe",
            "operation_id":"delta_replace_link",
            "part_id":part_id,
            "recipe_id":"recipe_c106_arm_joint_housing"
        }]
    });
    let materialized = materialize_assembly_delta(&base, &delta).unwrap();
    let replaced = materialized.assembly_graph["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|part| part["part_id"] == part_id)
        .unwrap();
    assert_eq!(replaced["role"], "joint_housing");
    assert_eq!(replaced["part_id"], part_id);
    assert_ne!(replaced["operation_id"], old_operation);
    assert!(materialized.shape_program["operations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|operation| operation["operation_id"] == replaced["operation_id"]));
    assert!(materialized.assembly_graph["connections"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|connection| connection["to_part_id"] == part_id)
        .all(|connection| connection["to_connector_id"] == "connector_joint_mount"));
}
