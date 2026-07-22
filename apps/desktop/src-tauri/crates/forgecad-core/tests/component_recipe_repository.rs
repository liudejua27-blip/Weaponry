use std::{collections::BTreeMap, path::PathBuf};

use forgecad_core::{
    AgentAssetChangeSet, AgentAssetVersion, AssetStage, AssetVersionStatus, ChangeSetStatus,
    ComponentRecipeRef, CoreRepository, NavigationAction, ObjectReference, Project, ProjectStatus,
    RecipeExpander, RecipeExpansionPolicy, RecipeInstantiationRequest, RecipeRegistry,
    RecipeSlotBinding, RecipeValidator, SnapshotEtag,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use tempfile::TempDir;

const PROJECT_ID: &str = "prj_component_recipe";
const ASSET_ID: &str = "assetver_component_recipe_v1";

struct Fixture {
    _root: TempDir,
    db_path: PathBuf,
    repository: CoreRepository,
    request: RecipeInstantiationRequest,
    root_part_id: String,
}

impl Fixture {
    fn active() -> Self {
        Self::active_for("recipe_future_prop_shell", "pack_future_weapon_prop")
    }

    fn active_c106_robotic_arm() -> Self {
        let registry = RecipeRegistry::from_embedded_c106_robotic_arm().unwrap();
        Self::active_for_registry(
            &registry,
            "recipe_c106_arm_desktop_assistant",
            "pack_robotic_arm_concept",
        )
    }

    fn active_robotic_arm_with_detail() -> Self {
        let mut fixture = Self::active_for("recipe_robotic_arm_link", "pack_robotic_arm_concept");
        let registry = RecipeRegistry::from_embedded().unwrap();
        let child = registry.recipe("recipe_robotic_arm_detail").unwrap();
        fixture.request.slot_bindings = vec![RecipeSlotBinding {
            slot_id: "slot_arm_detail".into(),
            child_recipe: ComponentRecipeRef {
                schema_version: "ComponentRecipeRef@1".into(),
                recipe_id: child.recipe_id.clone(),
                version: child.version,
                recipe_sha256: RecipeValidator::recipe_sha256(child).unwrap(),
            },
        }];
        fixture
    }

    fn active_for(recipe_id: &str, domain_pack_id: &str) -> Self {
        let registry = RecipeRegistry::from_embedded().unwrap();
        Self::active_for_registry(&registry, recipe_id, domain_pack_id)
    }

    fn active_for_registry(
        registry: &RecipeRegistry,
        recipe_id: &str,
        domain_pack_id: &str,
    ) -> Self {
        let root = tempfile::tempdir().unwrap();
        let db_path = root.path().join("library.db");
        let repository =
            CoreRepository::open(&db_path, root.path(), "component-recipe-repository-test")
                .unwrap();
        repository
            .ensure_default_domain_profile("2026-07-18T08:00:00Z")
            .unwrap();
        repository
            .create_project(&Project {
                project_id: PROJECT_ID.into(),
                profile_id: "profile_weapon_concept_v1".into(),
                domain_type: "weapon_concept".into(),
                name: "Component Recipe repository".into(),
                status: ProjectStatus::Active,
                current_version_id: None,
                created_at: "2026-07-18T08:00:01Z".into(),
                updated_at: "2026-07-18T08:00:01Z".into(),
            })
            .unwrap();

        let initial = initial_request_for(
            registry,
            recipe_id,
            domain_pack_id,
            "recipereq_repository_fixture",
        );
        let expanded =
            RecipeExpander::expand(registry, &initial, &RecipeExpansionPolicy::default()).unwrap();
        let root_part_id = expanded.expanded_assembly_graph["root_part_id"]
            .as_str()
            .unwrap()
            .to_string();
        let parts = expanded.expanded_assembly_graph["parts"]
            .as_array()
            .unwrap()
            .clone();
        let version = AgentAssetVersion {
            asset_version_id: ASSET_ID.into(),
            project_id: PROJECT_ID.into(),
            parent_asset_version_id: None,
            version_no: 1,
            status: AssetVersionStatus::Committed,
            summary: "Reviewed Recipe fixture".into(),
            stage: AssetStage::EditableAsset,
            plan_id: "plan_component_recipe".into(),
            direction_id: "direction_component_recipe".into(),
            domain_pack_id: domain_pack_id.into(),
            artifact_id: "artifact_component_recipe".into(),
            parts,
            shape_program: expanded.expanded_shape_program,
            assembly_graph: expanded.expanded_assembly_graph,
            material_bindings: BTreeMap::new(),
            created_at: "2026-07-18T08:00:02Z".into(),
        };
        let snapshot = repository.commit_initial_asset(&version).unwrap();
        let mut request = initial_request_for(
            registry,
            recipe_id,
            domain_pack_id,
            "recipereq_repository_active",
        );
        request.context_mode = "active_asset_edit".into();
        request.project_id = Some(PROJECT_ID.into());
        request.base_asset_version_id = Some(ASSET_ID.into());
        request.snapshot_revision = Some(snapshot.revision);
        request.target_part_id = Some(root_part_id.clone());
        Self {
            _root: root,
            db_path,
            repository,
            request,
            root_part_id,
        }
    }
}

fn initial_request(registry: &RecipeRegistry, request_id: &str) -> RecipeInstantiationRequest {
    initial_request_for(
        registry,
        "recipe_future_prop_shell",
        "pack_future_weapon_prop",
        request_id,
    )
}

fn initial_request_for(
    registry: &RecipeRegistry,
    recipe_id: &str,
    domain_pack_id: &str,
    request_id: &str,
) -> RecipeInstantiationRequest {
    let recipe = registry.recipe(recipe_id).unwrap();
    RecipeInstantiationRequest {
        schema_version: "ComponentRecipeInstantiationRequest@1".into(),
        context_mode: "initial_candidate".into(),
        request_id: request_id.into(),
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
        slot_bindings: Vec::new(),
        parameter_values: Vec::new(),
        material_zone_overrides: Vec::new(),
    }
}

fn table_counts(db_path: &PathBuf) -> Vec<(String, i64)> {
    let connection = Connection::open(db_path).unwrap();
    [
        "projects",
        "agent_asset_versions",
        "agent_asset_heads",
        "active_design_snapshots",
        "agent_asset_change_sets",
        "forgecad_core_objects",
        "forgecad_core_object_references",
    ]
    .into_iter()
    .map(|table| {
        let count = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        (table.to_string(), count)
    })
    .collect()
}

#[test]
fn initial_and_active_recipe_expansion_are_read_only_and_context_bound() {
    let fixture = Fixture::active();
    let registry = RecipeRegistry::from_embedded().unwrap();
    let counts_before = table_counts(&fixture.db_path);
    let snapshot_before = fixture.repository.snapshot(PROJECT_ID).unwrap().unwrap();
    let version_before = fixture.repository.version(ASSET_ID).unwrap().unwrap();

    let initial = fixture
        .repository
        .instantiate_component_recipe_candidate(&initial_request(
            &registry,
            "recipereq_repository_readonly",
        ))
        .unwrap();
    assert_eq!(initial.context_mode, "initial_candidate");
    assert_eq!(initial.project_id, None);
    assert_eq!(initial.base_asset_version_id, None);
    assert_eq!(initial.snapshot_revision, None);
    assert_eq!(initial.target_part_id, None);

    let active = fixture
        .repository
        .instantiate_component_recipe_candidate(&fixture.request)
        .unwrap();
    assert_eq!(active.context_mode, "active_asset_edit");
    assert_eq!(active.project_id.as_deref(), Some(PROJECT_ID));
    assert_eq!(active.base_asset_version_id.as_deref(), Some(ASSET_ID));
    assert_eq!(
        active.target_part_id.as_deref(),
        Some(fixture.root_part_id.as_str())
    );
    assert_eq!(table_counts(&fixture.db_path), counts_before);
    assert_eq!(
        fixture.repository.snapshot(PROJECT_ID).unwrap().unwrap(),
        snapshot_before
    );
    assert_eq!(
        fixture.repository.version(ASSET_ID).unwrap().unwrap(),
        version_before
    );
}

#[test]
fn stale_head_revision_and_locked_target_fail_closed_before_candidate_creation() {
    let mut fixture = Fixture::active();
    let counts_before = table_counts(&fixture.db_path);

    let mut stale_revision = fixture.request.clone();
    stale_revision.snapshot_revision = Some(stale_revision.snapshot_revision.unwrap() + 1);
    assert_eq!(
        fixture
            .repository
            .instantiate_component_recipe_candidate(&stale_revision)
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_CONTEXT_STALE"
    );

    let connection = Connection::open(&fixture.db_path).unwrap();
    connection
        .execute(
            "INSERT INTO agent_asset_versions(asset_version_id, project_id, parent_asset_version_id, version_no, status, summary, stage, plan_id, direction_id, domain_pack_id, artifact_id, parts_json, shape_program_json, assembly_graph_json, material_bindings_json, created_at) SELECT 'assetver_component_recipe_stale', project_id, asset_version_id, 2, status, summary, stage, plan_id, direction_id, domain_pack_id, 'artifact_component_recipe_stale', parts_json, shape_program_json, assembly_graph_json, material_bindings_json, '2026-07-18T08:00:02Z' FROM agent_asset_versions WHERE asset_version_id=?",
            [ASSET_ID],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE agent_asset_heads SET asset_version_id='assetver_component_recipe_stale' WHERE project_id=?",
            [PROJECT_ID],
        )
        .unwrap();
    assert_eq!(
        fixture
            .repository
            .instantiate_component_recipe_candidate(&fixture.request)
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_CONTEXT_STALE"
    );
    connection
        .execute(
            "UPDATE agent_asset_heads SET asset_version_id=? WHERE project_id=?",
            [ASSET_ID, PROJECT_ID],
        )
        .unwrap();
    drop(connection);

    let snapshot = fixture.repository.snapshot(PROJECT_ID).unwrap().unwrap();
    let locked = fixture
        .repository
        .set_part_display_idempotent(
            PROJECT_ID,
            SnapshotEtag(snapshot.revision),
            "lock",
            Some(&fixture.root_part_id),
            "2026-07-18T08:00:03Z",
            "PUT /component-recipe-test/part-display",
            "idem_component_recipe_lock",
            &"a".repeat(64),
        )
        .unwrap();
    fixture.request.snapshot_revision = Some(locked.revision);
    assert_eq!(
        fixture
            .repository
            .instantiate_component_recipe_candidate(&fixture.request)
            .unwrap_err()
            .code(),
        "PART_PROTECTED"
    );
    let counts_after = table_counts(&fixture.db_path);
    assert_eq!(
        counts_after
            .iter()
            .find(|(table, _)| table == "agent_asset_versions")
            .unwrap()
            .1,
        counts_before
            .iter()
            .find(|(table, _)| table == "agent_asset_versions")
            .unwrap()
            .1
            + 1
    );
    assert_eq!(
        counts_after
            .iter()
            .find(|(table, _)| table == "agent_asset_change_sets")
            .unwrap()
            .1,
        0
    );
}

#[test]
fn recipe_ratio_change_writes_only_through_preview_confirm_and_survives_restart_undo_redo() {
    let Fixture {
        _root: root,
        db_path,
        repository,
        request,
        root_part_id,
    } = Fixture::active();
    repository
        .instantiate_component_recipe_candidate(&request)
        .unwrap();
    let base = repository.version(ASSET_ID).unwrap().unwrap();
    let binding = base
        .parts
        .iter()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(root_part_id.as_str()))
        .and_then(|part| part.get("editable_parameter_bindings"))
        .and_then(Value::as_array)
        .and_then(|bindings| bindings.first())
        .cloned()
        .expect("reviewed root Recipe must expose one bounded ratio binding");
    assert_eq!(binding["path"], "transform.scale.y");

    let change_set = AgentAssetChangeSet {
        change_set_id: "changeset_component_recipe_height".into(),
        project_id: PROJECT_ID.into(),
        base_asset_version_id: ASSET_ID.into(),
        summary: "调整主壳体高度比例".into(),
        operations: vec![json!({
            "operation_id":"changeop_component_recipe_height",
            "op":"set_part_parameter",
            "part_id":root_part_id,
            "path":"transform.scale.y",
            "value":1.1
        })],
        protected_part_ids: Vec::new(),
        preview: None,
        status: ChangeSetStatus::Proposed,
        resulting_asset_version_id: None,
        created_at: "2026-07-18T08:00:04Z".into(),
        updated_at: "2026-07-18T08:00:04Z".into(),
    };
    repository.create_change_set(&change_set).unwrap();
    assert_eq!(
        repository.head(PROJECT_ID).unwrap().as_deref(),
        Some(ASSET_ID)
    );
    assert_eq!(
        table_counts(&db_path)
            .iter()
            .find(|(table, _)| table == "agent_asset_versions")
            .unwrap()
            .1,
        1
    );

    let mut preview = base.clone();
    preview.asset_version_id = "assetver_component_recipe_preview".into();
    preview.parent_asset_version_id = Some(ASSET_ID.into());
    preview.version_no = 2;
    preview.summary = "Recipe height preview".into();
    preview.created_at = "2026-07-18T08:00:05Z".into();
    let operation_id = preview
        .assembly_graph
        .get("parts")
        .and_then(Value::as_array)
        .and_then(|parts| {
            parts.iter().find(|part| {
                part.get("part_id").and_then(Value::as_str) == Some(root_part_id.as_str())
            })
        })
        .and_then(|part| part.get("operation_id"))
        .and_then(Value::as_str)
        .unwrap()
        .to_string();
    let operation = preview
        .shape_program
        .get_mut("operations")
        .and_then(Value::as_array_mut)
        .and_then(|operations| {
            operations.iter_mut().find(|operation| {
                operation.get("operation_id").and_then(Value::as_str) == Some(operation_id.as_str())
            })
        })
        .unwrap();
    let profile_scale = operation["args"]["profile_scale"].as_array_mut().unwrap();
    let base_height = profile_scale[1].as_f64().unwrap();
    profile_scale[1] = json!(base_height * 1.1);
    let graph_part = preview
        .assembly_graph
        .get_mut("parts")
        .and_then(Value::as_array_mut)
        .and_then(|parts| {
            parts.iter_mut().find(|part| {
                part.get("part_id").and_then(Value::as_str) == Some(root_part_id.as_str())
            })
        })
        .unwrap();
    graph_part["transform"]["scale"] = json!([1.0, 1.1, 1.0]);

    let (_, preview_snapshot) = repository
        .preview_change_set(
            &change_set.change_set_id,
            &preview,
            SnapshotEtag(1),
            "2026-07-18T08:00:05Z",
        )
        .unwrap();
    assert_eq!(preview_snapshot.revision, 2);
    assert_eq!(
        repository.head(PROJECT_ID).unwrap().as_deref(),
        Some(ASSET_ID)
    );
    assert!(repository
        .version("assetver_component_recipe_preview")
        .unwrap()
        .is_none());

    let mut confirmed: AgentAssetVersion = serde_json::from_value(
        repository
            .change_set(&change_set.change_set_id)
            .unwrap()
            .unwrap()
            .preview
            .unwrap(),
    )
    .unwrap();
    confirmed.asset_version_id = "assetver_component_recipe_v2".into();
    confirmed.created_at = "2026-07-18T08:00:06Z".into();
    let (_, version, snapshot) = repository
        .confirm_change_set(
            &change_set.change_set_id,
            &confirmed,
            preview_snapshot.etag(),
        )
        .unwrap();
    assert_eq!(snapshot.revision, 3);
    assert_eq!(
        snapshot.active_design.asset_version_id(),
        Some(version.asset_version_id.as_str())
    );
    assert_eq!(
        repository.head(PROJECT_ID).unwrap().as_deref(),
        Some("assetver_component_recipe_v2")
    );
    assert!(version.assembly_graph["component_recipe_instances"]
        .as_array()
        .is_some_and(|instances| !instances.is_empty()));

    for asset_version_id in [ASSET_ID, "assetver_component_recipe_v2"] {
        for role in ["interactive_preview_glb", "production_glb"] {
            repository
                .attach_object_bytes(
                    &ObjectReference {
                        reference_kind: "asset_version".into(),
                        owner_id: asset_version_id.into(),
                        role: role.into(),
                    },
                    format!("glTF-{asset_version_id}-{role}").as_bytes(),
                    "glb",
                    "2026-07-18T08:00:06Z",
                )
                .unwrap();
        }
    }

    drop(repository);
    let restarted =
        CoreRepository::open(&db_path, root.path(), "component-recipe-restart").unwrap();
    let restarted_snapshot = restarted.snapshot(PROJECT_ID).unwrap().unwrap();
    assert_eq!(
        restarted_snapshot.active_design.asset_version_id(),
        Some("assetver_component_recipe_v2")
    );
    let undo = restarted
        .navigate(
            PROJECT_ID,
            NavigationAction::Undo,
            "assetver_component_recipe_undo",
            restarted_snapshot.etag(),
            "2026-07-18T08:00:07Z",
        )
        .unwrap();
    assert!(undo.version.assembly_graph["component_recipe_instances"]
        .as_array()
        .is_some_and(|instances| !instances.is_empty()));
    let redo = restarted
        .navigate(
            PROJECT_ID,
            NavigationAction::Redo,
            "assetver_component_recipe_redo",
            undo.snapshot.etag(),
            "2026-07-18T08:00:08Z",
        )
        .unwrap();
    assert!(redo.version.assembly_graph["component_recipe_instances"]
        .as_array()
        .is_some_and(|instances| !instances.is_empty()));
    assert_eq!(
        redo.snapshot.active_design.asset_version_id(),
        Some("assetver_component_recipe_redo")
    );
}

#[test]
fn active_recipe_candidate_bakes_nonroot_target_anchor_before_hashing() {
    let fixture = Fixture::active();
    let registry = RecipeRegistry::from_embedded().unwrap();
    let base = fixture.repository.version(ASSET_ID).unwrap().unwrap();
    let target = base.assembly_graph["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|part| part["parent_part_id"].as_str().is_some())
        .cloned()
        .expect("fixture has a non-root reviewed detail Part");
    let target_part_id = target["part_id"].as_str().unwrap().to_string();
    let target_position = target["transform"]["position"].clone();
    let target_instance_id = target["recipe_instance_id"].as_str().unwrap();
    let target_provenance = base.assembly_graph["component_recipe_instances"]
        .as_array()
        .unwrap()
        .iter()
        .find(|instance| instance["instance_id"].as_str() == Some(target_instance_id))
        .unwrap();
    assert!(target_position
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value.as_f64().unwrap().abs() > 1e-9));

    let trim = registry.recipe("recipe_future_prop_trim").unwrap();
    let mut request = fixture.request.clone();
    request.request_id = "recipereq_repository_nonroot_placement".into();
    request.target_part_id = Some(target_part_id.clone());
    request.recipe = ComponentRecipeRef {
        schema_version: "ComponentRecipeRef@1".into(),
        recipe_id: trim.recipe_id.clone(),
        version: trim.version,
        recipe_sha256: RecipeValidator::recipe_sha256(trim).unwrap(),
    };
    let unplaced =
        RecipeExpander::expand(&registry, &request, &RecipeExpansionPolicy::default()).unwrap();
    let candidate = fixture
        .repository
        .instantiate_component_recipe_candidate(&request)
        .unwrap();
    let root_id = candidate.expanded_assembly_graph["root_part_id"]
        .as_str()
        .unwrap();
    let placed_root = candidate.expanded_assembly_graph["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|part| part["part_id"].as_str() == Some(root_id))
        .unwrap();
    assert_eq!(placed_root["transform"]["position"], target_position);
    assert_eq!(placed_root["parent_part_id"], target["parent_part_id"]);
    let placed_provenance = candidate
        .component_recipe_instances
        .iter()
        .find(|instance| {
            instance.instance_id == placed_root["recipe_instance_id"].as_str().unwrap()
        })
        .unwrap();
    assert_eq!(
        placed_provenance.instance_path,
        target_provenance["instance_path"].as_str().unwrap(),
        "a non-root replacement must inherit its immutable target provenance path"
    );
    assert_eq!(
        serde_json::to_value(&placed_provenance.parent_instance_id).unwrap(),
        target_provenance["parent_instance_id"]
    );
    assert_eq!(
        serde_json::to_value(&placed_provenance.parent_slot_id).unwrap(),
        target_provenance["parent_slot_id"]
    );
    assert_ne!(candidate.candidate_id, unplaced.candidate_id);
    assert_ne!(candidate.candidate_sha256, unplaced.candidate_sha256);
    assert_eq!(
        candidate.candidate_sha256,
        RecipeExpander::candidate_sha256(&candidate).unwrap(),
        "the bridge/Q003 carrier must receive the exact Core-placed candidate hash"
    );
    assert_ne!(
        candidate.expanded_shape_program,
        unplaced.expanded_shape_program,
        "the placement must be baked into the worker ShapeProgram rather than only AssemblyGraph metadata"
    );
}

#[test]
fn active_recipe_candidate_rejects_locked_descendant_without_writing_state() {
    let mut fixture = Fixture::active();
    let base = fixture.repository.version(ASSET_ID).unwrap().unwrap();
    let descendant = base.assembly_graph["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|part| part["parent_part_id"].as_str() == Some(fixture.root_part_id.as_str()))
        .and_then(|part| part["part_id"].as_str())
        .unwrap()
        .to_string();
    let snapshot = fixture.repository.snapshot(PROJECT_ID).unwrap().unwrap();
    let locked = fixture
        .repository
        .set_part_display_idempotent(
            PROJECT_ID,
            snapshot.etag(),
            "lock",
            Some(&descendant),
            "2026-07-18T08:01:00Z",
            "PUT /component-recipe-test/part-display",
            "idem_component_recipe_descendant_lock",
            &"b".repeat(64),
        )
        .unwrap();
    fixture.request.snapshot_revision = Some(locked.revision);
    let counts_before = table_counts(&fixture.db_path);
    assert_eq!(
        fixture
            .repository
            .instantiate_component_recipe_candidate(&fixture.request)
            .unwrap_err()
            .code(),
        "PART_PROTECTED"
    );
    assert_eq!(table_counts(&fixture.db_path), counts_before);
}

#[test]
fn active_optional_slot_candidate_is_zero_write_and_sealed_for_deterministic_replay() {
    let fixture = Fixture::active_robotic_arm_with_detail();
    let counts_before_candidate = table_counts(&fixture.db_path);
    let snapshot_before = fixture.repository.snapshot(PROJECT_ID).unwrap().unwrap();
    let candidate = fixture
        .repository
        .instantiate_component_recipe_candidate(&fixture.request)
        .unwrap();
    assert_eq!(candidate.component_recipe_instances.len(), 2);
    assert!(candidate
        .component_recipe_instances
        .iter()
        .any(|instance| instance.parent_slot_id.as_deref() == Some("slot_arm_detail")));
    assert_eq!(table_counts(&fixture.db_path), counts_before_candidate);
    assert_eq!(
        fixture.repository.snapshot(PROJECT_ID).unwrap().unwrap(),
        snapshot_before
    );

    let operation = json!({
        "operation_id":"changeop_component_recipe_arm_detail",
        "op":"replace_part",
        "part_id":fixture.root_part_id,
        "recipe_request_id":candidate.request_id,
        "component_recipe_ref":candidate.recipe,
        "recipe_registry_sha256":candidate.registry_sha256,
        "recipe_slot_bindings":fixture.request.slot_bindings,
        "recipe_candidate_id":candidate.candidate_id,
        "recipe_candidate_sha256":candidate.candidate_sha256,
        "recipe_snapshot_revision":fixture.request.snapshot_revision,
    });
    let change_set = AgentAssetChangeSet {
        change_set_id: "changeset_component_recipe_arm_detail".into(),
        project_id: PROJECT_ID.into(),
        base_asset_version_id: ASSET_ID.into(),
        summary: "Enable reviewed robotic arm detail".into(),
        operations: vec![operation.clone()],
        protected_part_ids: Vec::new(),
        preview: None,
        status: ChangeSetStatus::Proposed,
        resulting_asset_version_id: None,
        created_at: "2026-07-18T08:03:00Z".into(),
        updated_at: "2026-07-18T08:03:00Z".into(),
    };
    fixture.repository.create_change_set(&change_set).unwrap();
    let counts_after_proposal = table_counts(&fixture.db_path);
    let replayed = fixture
        .repository
        .recipe_replacement_candidate(&change_set, &operation)
        .unwrap();
    assert_eq!(replayed.candidate_id, candidate.candidate_id);
    assert_eq!(replayed.candidate_sha256, candidate.candidate_sha256);
    assert_eq!(
        replayed.component_recipe_instances,
        candidate.component_recipe_instances
    );
    assert_eq!(table_counts(&fixture.db_path), counts_after_proposal);

    let mut missing_binding = operation.clone();
    missing_binding
        .as_object_mut()
        .unwrap()
        .remove("recipe_slot_bindings");
    assert_eq!(
        fixture
            .repository
            .recipe_replacement_candidate(&change_set, &missing_binding)
            .unwrap_err()
            .code(),
        "REPLACE_PART_VARIANT_INVALID"
    );

    let mut duplicate_binding = operation.clone();
    let binding = duplicate_binding["recipe_slot_bindings"][0].clone();
    duplicate_binding["recipe_slot_bindings"] = json!([binding.clone(), binding]);
    assert_eq!(
        fixture
            .repository
            .recipe_replacement_candidate(&change_set, &duplicate_binding)
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_SLOT_BINDING_DUPLICATE"
    );

    let mut tampered_binding = operation;
    tampered_binding["recipe_slot_bindings"] = json!([]);
    assert_eq!(
        fixture
            .repository
            .recipe_replacement_candidate(&change_set, &tampered_binding)
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_CANDIDATE_STALE"
    );
    assert_eq!(table_counts(&fixture.db_path), counts_after_proposal);
}

#[test]
fn c106_active_child_requires_exact_catalog_and_persisted_root_provenance() {
    let c105 = RecipeRegistry::from_embedded().unwrap();
    let c106 = RecipeRegistry::from_embedded_c106_robotic_arm().unwrap();

    // Same robotic-arm Domain Pack remains a C105 request unless it explicitly
    // names the immutable C106 catalog. Domain alone is never a selector.
    let c105_fixture = Fixture::active_robotic_arm_with_detail();
    let c105_candidate = c105_fixture
        .repository
        .instantiate_component_recipe_candidate(&c105_fixture.request)
        .unwrap();
    assert_eq!(c105_candidate.registry_sha256, c105.registry_sha256());

    let mut fixture = Fixture::active_c106_robotic_arm();
    let child = c106.recipe("recipe_c106_arm_link_armor").unwrap();
    let child_part_id = fixture
        .repository
        .version(ASSET_ID)
        .unwrap()
        .unwrap()
        .assembly_graph["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|part| part["role"].as_str() == Some("link_armor"))
        .and_then(|part| part["part_id"].as_str())
        .unwrap()
        .to_string();
    fixture.request.recipe = ComponentRecipeRef {
        schema_version: "ComponentRecipeRef@1".into(),
        recipe_id: child.recipe_id.clone(),
        version: child.version,
        recipe_sha256: RecipeValidator::recipe_sha256(child).unwrap(),
    };
    fixture.request.target_part_id = Some(child_part_id.clone());
    fixture.request.recipe_registry_sha256 = c106.registry_sha256().into();

    let candidate = fixture
        .repository
        .instantiate_component_recipe_candidate(&fixture.request)
        .unwrap();
    assert_eq!(candidate.registry_sha256, c106.registry_sha256());
    assert_eq!(
        candidate.target_part_id.as_deref(),
        Some(child_part_id.as_str())
    );

    let operation = json!({
        "operation_id":"changeop_c106_child_replace",
        "op":"replace_part",
        "part_id":child_part_id,
        "recipe_request_id":candidate.request_id,
        "component_recipe_ref":candidate.recipe,
        "recipe_registry_sha256":candidate.registry_sha256,
        "recipe_slot_bindings":[],
        "recipe_candidate_id":candidate.candidate_id,
        "recipe_candidate_sha256":candidate.candidate_sha256,
        "recipe_snapshot_revision":fixture.request.snapshot_revision,
    });
    let change_set = AgentAssetChangeSet {
        change_set_id: "changeset_c106_child_replace".into(),
        project_id: PROJECT_ID.into(),
        base_asset_version_id: ASSET_ID.into(),
        summary: "Replace a reviewed C106 link-armor component".into(),
        operations: vec![operation.clone()],
        protected_part_ids: Vec::new(),
        preview: None,
        status: ChangeSetStatus::Proposed,
        resulting_asset_version_id: None,
        created_at: "2026-07-18T08:04:00Z".into(),
        updated_at: "2026-07-18T08:04:00Z".into(),
    };
    fixture.repository.create_change_set(&change_set).unwrap();
    let sealed_operation = operation.clone();
    assert_eq!(
        fixture
            .repository
            .recipe_replacement_candidate(&change_set, &operation)
            .unwrap()
            .candidate_sha256,
        candidate.candidate_sha256
    );

    let mut wrong_hash_request = fixture.request.clone();
    wrong_hash_request.recipe_registry_sha256 = c105.registry_sha256().into();
    assert_eq!(
        fixture
            .repository
            .instantiate_component_recipe_candidate(&wrong_hash_request)
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_C106_PROVENANCE_INVALID"
    );
    let mut mixed_operation = operation;
    mixed_operation["recipe_registry_sha256"] = json!(c105.registry_sha256());
    assert_eq!(
        fixture
            .repository
            .recipe_replacement_candidate(&change_set, &mixed_operation)
            .unwrap_err()
            .code(),
        "COMPONENT_RECIPE_C106_PROVENANCE_INVALID"
    );

    // The only persisted truth is the sealed operation plus the immutable
    // base provenance. Reopening must reproduce the candidate exactly.
    let db_path = fixture.db_path.clone();
    let object_root = fixture._root.path().to_path_buf();
    let expected_candidate_sha256 = candidate.candidate_sha256.clone();
    drop(fixture.repository);
    let reopened =
        CoreRepository::open(&db_path, &object_root, "component-recipe-restart").unwrap();
    let persisted = reopened
        .change_set("changeset_c106_child_replace")
        .unwrap()
        .unwrap();
    assert_eq!(
        reopened
            .recipe_replacement_candidate(&persisted, &sealed_operation)
            .unwrap()
            .candidate_sha256,
        expected_candidate_sha256
    );
}

#[test]
fn recipe_replace_change_set_rejects_mixed_operations_before_preview() {
    let fixture = Fixture::active();
    let candidate = fixture
        .repository
        .instantiate_component_recipe_candidate(&fixture.request)
        .unwrap();
    let change_set = AgentAssetChangeSet {
        change_set_id: "changeset_component_recipe_mixed".into(),
        project_id: PROJECT_ID.into(),
        base_asset_version_id: ASSET_ID.into(),
        summary: "sealed Recipe plus transform must fail".into(),
        operations: vec![
            json!({
                "operation_id":"changeop_component_recipe_replace",
                "op":"replace_part",
                "part_id":fixture.root_part_id,
                "recipe_request_id":candidate.request_id,
                "component_recipe_ref":candidate.recipe,
                "recipe_registry_sha256":candidate.registry_sha256,
                "recipe_slot_bindings":[],
                "recipe_candidate_id":candidate.candidate_id,
                "recipe_candidate_sha256":candidate.candidate_sha256,
                "recipe_snapshot_revision":fixture.request.snapshot_revision,
            }),
            json!({
                "operation_id":"changeop_component_recipe_transform",
                "op":"set_part_transform",
                "part_id":fixture.root_part_id,
                "transform":{"position":[1.0,0.0,0.0],"rotation":[0.0,0.0,0.0],"scale":[1.0,1.0,1.0]},
            }),
        ],
        protected_part_ids: Vec::new(),
        preview: None,
        status: ChangeSetStatus::Proposed,
        resulting_asset_version_id: None,
        created_at: "2026-07-18T08:02:00Z".into(),
        updated_at: "2026-07-18T08:02:00Z".into(),
    };
    assert_eq!(
        fixture
            .repository
            .create_change_set(&change_set)
            .unwrap_err()
            .code(),
        "RECIPE_REPLACEMENT_MIXED_OPERATIONS_UNSUPPORTED"
    );
    assert!(fixture
        .repository
        .change_set(&change_set.change_set_id)
        .unwrap()
        .is_none());
}
