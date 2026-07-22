use std::{collections::BTreeMap, path::PathBuf};

use forgecad_core::{
    builtin_surface_adornment_manifest, builtin_surface_adornment_manifest_v2, AgentAssetVersion,
    AgentSkillActivation, AgentSkillEvalReport, AgentSkillManifest, AssetStage, AssetVersionStatus,
    CoreRepository, Project, ProjectStatus, SkillEvalStatus, SkillExample, SkillLicense,
    SkillProvenance, SurfaceAdornmentProgram,
};
use rusqlite::Connection;
use serde_json::json;
use tempfile::TempDir;

struct Fixture {
    _root: TempDir,
    db_path: PathBuf,
    repository: CoreRepository,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let db_path = root.path().join("library.db");
        let repository = CoreRepository::open(&db_path, root.path(), "agent-skill-test").unwrap();
        Self {
            _root: root,
            db_path,
            repository,
        }
    }
}

fn manifest(version: u32) -> AgentSkillManifest {
    AgentSkillManifest {
        schema_version: "AgentSkillManifest@1".into(),
        skill_id: "skill_surface_groove".into(),
        version,
        display_name: "Surface groove".into(),
        purpose: "Add a controlled visual groove to an existing material zone.".into(),
        allowed_domains: vec!["pack_future_weapon_prop".into()],
        triggers: vec!["add a subtle surface groove".into()],
        product_tool_ids: vec!["forgecad.preview.prepare.v1".into()],
        g819_operations: vec!["surface_panel".into()],
        recipe_ids: vec!["recipe_future_prop_shell".into()],
        material_preset_ids: vec!["mat_graphite".into()],
        reference_hashes: vec!["a".repeat(64)],
        success_examples: examples("ok"),
        stop_examples: examples("stop"),
        author: SkillProvenance {
            kind: "forgecad_user".into(),
            id: "author_local".into(),
        },
        source: SkillProvenance {
            kind: "forgecad_user".into(),
            id: "source_local".into(),
        },
        license: SkillLicense {
            license_id: "self_declared_original".into(),
            redistributable: false,
        },
        non_functional_only: true,
    }
}

fn examples(kind: &str) -> Vec<SkillExample> {
    (1..=3)
        .map(|number| SkillExample {
            example_id: format!("skillex_{kind}_{number}"),
            brief: format!("{kind} brief {number}"),
            expected_outcome: format!("{kind} result {number}"),
        })
        .collect()
}

fn activation(manifest: &AgentSkillManifest, enabled: bool, suffix: &str) -> AgentSkillActivation {
    AgentSkillActivation {
        schema_version: "AgentSkillActivation@1".into(),
        activation_id: format!("skillact_{suffix}"),
        skill_id: manifest.skill_id.clone(),
        skill_version: manifest.version,
        skill_sha256: manifest.canonical_sha256().unwrap(),
        enabled,
        updated_at: "2026-07-18T10:00:00Z".into(),
    }
}

fn report(
    manifest: &AgentSkillManifest,
    status: SkillEvalStatus,
    suffix: &str,
) -> AgentSkillEvalReport {
    AgentSkillEvalReport {
        schema_version: "AgentSkillEvalReport@1".into(),
        report_id: format!("skilleval_{suffix}"),
        skill_id: manifest.skill_id.clone(),
        skill_version: manifest.version,
        skill_sha256: manifest.canonical_sha256().unwrap(),
        status,
        findings: vec!["deterministic local evaluation".into()],
        evaluated_at: "2026-07-18T10:00:00Z".into(),
    }
}

fn counts(db_path: &PathBuf) -> (i64, i64, i64, i64) {
    let connection = Connection::open(db_path).unwrap();
    let values = [
        "agent_skill_versions",
        "agent_skill_activations",
        "agent_skill_eval_reports",
        "forgecad_core_object_references",
    ]
    .map(|table| {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    });
    (values[0], values[1], values[2], values[3])
}

fn c106_surface_version() -> AgentAssetVersion {
    AgentAssetVersion {
        asset_version_id: "assetver_c106_a005_policy".into(),
        project_id: "project_c106_a005_policy".into(),
        parent_asset_version_id: None,
        version_no: 1,
        status: AssetVersionStatus::Committed,
        summary: "C106 A005 policy fixture".into(),
        stage: AssetStage::EditableAsset,
        plan_id: "plan_c106_a005_policy".into(),
        direction_id: "direction_c106_a005_policy".into(),
        domain_pack_id: "pack_robotic_arm_concept".into(),
        artifact_id: "artifact_c106_a005_policy".into(),
        parts: vec![json!({
            "part_id": "part_c106_link",
            "material_zone_ids": ["zone_c106_link_shell"]
        })],
        shape_program: json!({
            "schema_version": "ShapeProgram@1",
            "program_id": "shape_c106_a005_policy",
            "operations": []
        }),
        assembly_graph: json!({
            "schema_version": "AssemblyGraph@1",
            "graph_id": "graph_c106_a005_policy",
            "root_part_id": "part_c106_link",
            "parts": [{
                "part_id": "part_c106_link",
                "recipe_instance_id": "recipeinst_c106_link",
                "material_zone_ids": ["zone_c106_link_shell"],
                "surface_adornment_slots": [{
                    "slot_id": "adornslot_c106_link_shell",
                    "zone_id": "zone_c106_link_shell",
                    "allowed_kinds": ["flowline"],
                    "allowed_motifs": ["double_flowline"],
                    "allowed_coverages": ["center_band"]
                }]
            }],
            "component_recipe_instances": [{
                "instance_id": "recipeinst_c106_link",
                "recipe": {"recipe_id": "recipe_c106_arm_link_armor"}
            }]
        }),
        material_bindings: BTreeMap::new(),
        created_at: "2026-07-18T10:00:00Z".into(),
    }
}

fn c106_surface_program(manifest: &AgentSkillManifest) -> SurfaceAdornmentProgram {
    SurfaceAdornmentProgram {
        schema_version: "SurfaceAdornmentProgram@1".into(),
        program_id: "adorn_c106_a005_policy".into(),
        target_part_id: "part_c106_link".into(),
        target_zone_id: "zone_c106_link_shell".into(),
        kind: "flowline".into(),
        motif: "double_flowline".into(),
        intensity: "balanced".into(),
        coverage: "center_band".into(),
        seed: 106,
        base_material: "mat_aluminum".into(),
        execution: "texture_bake".into(),
        skill_id: manifest.skill_id.clone(),
        skill_version: manifest.version,
        skill_sha256: manifest.canonical_sha256().unwrap(),
        generator: "a005_v1".into(),
        non_functional_only: true,
    }
}

#[test]
fn dry_run_is_zero_write_and_registry_validation_is_layered() {
    let fixture = Fixture::new();
    let valid = manifest(1);
    let before = counts(&fixture.db_path);
    let dry_run = fixture.repository.dry_run_skill(&valid).unwrap();
    assert!(!dry_run.product_state_write_performed);
    assert_eq!(dry_run.skill_sha256, valid.canonical_sha256().unwrap());
    assert_eq!(counts(&fixture.db_path), before);

    let mut unknown_recipe = valid.clone();
    unknown_recipe.recipe_ids = vec!["recipe_unreviewed".into()];
    assert_eq!(
        fixture
            .repository
            .dry_run_skill(&unknown_recipe)
            .unwrap_err()
            .code(),
        "SKILL_RECIPE_POLICY_INVALID"
    );

    let mut wrong_domain_recipe = valid.clone();
    wrong_domain_recipe.recipe_ids = vec!["recipe_vehicle_body_shell".into()];
    assert_eq!(
        fixture
            .repository
            .dry_run_skill(&wrong_domain_recipe)
            .unwrap_err()
            .code(),
        "SKILL_RECIPE_DOMAIN_INVALID"
    );

    let mut minted_material = valid;
    minted_material.material_preset_ids = vec!["mat_untrusted_texture_source".into()];
    assert_eq!(
        fixture
            .repository
            .dry_run_skill(&minted_material)
            .unwrap_err()
            .code(),
        "SKILL_MATERIAL_POLICY_INVALID"
    );
}

#[test]
fn immutable_version_eval_activation_disable_and_restart_are_separate() {
    let fixture = Fixture::new();
    let v1 = manifest(1);
    fixture
        .repository
        .create_skill_draft(&v1, "2026-07-18T10:00:00Z")
        .unwrap();
    assert_eq!(
        fixture
            .repository
            .create_skill_draft(&v1, "2026-07-18T10:00:01Z")
            .unwrap(),
        v1
    );

    let mut overwritten = v1.clone();
    overwritten.purpose = "different purpose".into();
    assert_eq!(
        fixture
            .repository
            .create_skill_draft(&overwritten, "2026-07-18T10:00:02Z")
            .unwrap_err()
            .code(),
        "SKILL_VERSION_IMMUTABLE"
    );
    let skipped = manifest(3);
    assert_eq!(
        fixture
            .repository
            .create_skill_draft(&skipped, "2026-07-18T10:00:03Z")
            .unwrap_err()
            .code(),
        "SKILL_VERSION_SEQUENCE_INVALID"
    );

    assert_eq!(
        fixture
            .repository
            .set_skill_activation(&activation(&v1, true, "before_eval"))
            .unwrap_err()
            .code(),
        "SKILL_ENABLE_REQUIRES_EVAL"
    );
    fixture
        .repository
        .record_skill_eval(&report(&v1, SkillEvalStatus::Failed, "failed"))
        .unwrap();
    assert_eq!(
        fixture
            .repository
            .set_skill_activation(&activation(&v1, true, "after_failed"))
            .unwrap_err()
            .code(),
        "SKILL_ENABLE_REQUIRES_EVAL"
    );
    fixture
        .repository
        .record_skill_eval(&report(&v1, SkillEvalStatus::Passed, "passed"))
        .unwrap();
    fixture
        .repository
        .set_skill_activation(&activation(&v1, true, "enabled"))
        .unwrap();
    assert_eq!(
        fixture
            .repository
            .active_skill(&v1.skill_id)
            .unwrap()
            .unwrap()
            .skill_sha256,
        v1.canonical_sha256().unwrap()
    );
    assert_eq!(
        fixture
            .repository
            .delete_skill_version(&v1.skill_id, v1.version, "2026-07-18T10:00:04Z")
            .unwrap_err()
            .code(),
        "SKILL_VERSION_REFERENCED"
    );

    fixture
        .repository
        .set_skill_activation(&activation(&v1, false, "disabled"))
        .unwrap();
    assert!(fixture
        .repository
        .active_skill(&v1.skill_id)
        .unwrap()
        .is_none());
    drop(fixture.repository);
    let restarted = CoreRepository::open(
        &fixture.db_path,
        fixture._root.path(),
        "agent-skill-test-restart",
    )
    .unwrap();
    assert!(restarted.active_skill(&v1.skill_id).unwrap().is_none());
    assert_eq!(
        restarted
            .skill_manifest(&v1.skill_id, v1.version)
            .unwrap()
            .unwrap(),
        v1
    );
    assert_eq!(restarted.skill_manifests(&v1.skill_id).unwrap(), vec![v1]);
}

#[test]
fn first_party_starter_is_a_draft_and_never_auto_enables() {
    let fixture = Fixture::new();
    let legacy = builtin_surface_adornment_manifest();
    let builtin = builtin_surface_adornment_manifest_v2();
    fixture.repository.dry_run_skill(&legacy).unwrap();
    fixture.repository.dry_run_skill(&builtin).unwrap();
    assert_eq!(
        fixture
            .repository
            .ensure_builtin_surface_adornment_skill("2026-07-18T10:00:00Z")
            .unwrap(),
        builtin
    );
    assert_ne!(
        legacy.canonical_sha256().unwrap(),
        builtin.canonical_sha256().unwrap()
    );
    assert_eq!(
        fixture
            .repository
            .skill_manifests(&builtin.skill_id)
            .unwrap(),
        vec![legacy, builtin.clone()]
    );
    assert!(fixture
        .repository
        .active_skill(&builtin.skill_id)
        .unwrap()
        .is_none());
    assert!(fixture
        .repository
        .skill_eval_reports(&builtin.skill_id, builtin.version)
        .unwrap()
        .is_empty());
}

#[test]
fn c106_requires_v2_manifest_activation_and_survives_restart_by_exact_hash() {
    let fixture = Fixture::new();
    fixture
        .repository
        .ensure_default_domain_profile("2026-07-18T10:00:00Z")
        .unwrap();
    fixture
        .repository
        .create_project(&Project {
            project_id: "project_c106_a005_policy".into(),
            profile_id: "profile_weapon_concept_v1".into(),
            domain_type: "weapon_concept".into(),
            name: "C106 A005 policy".into(),
            status: ProjectStatus::Active,
            current_version_id: None,
            created_at: "2026-07-18T10:00:00Z".into(),
            updated_at: "2026-07-18T10:00:00Z".into(),
        })
        .unwrap();
    let version = c106_surface_version();
    fixture.repository.commit_initial_asset(&version).unwrap();

    let legacy = builtin_surface_adornment_manifest();
    let current = fixture
        .repository
        .ensure_builtin_surface_adornment_skill("2026-07-18T10:00:01Z")
        .unwrap();
    assert_eq!(current, builtin_surface_adornment_manifest_v2());

    fixture
        .repository
        .record_skill_eval(&report(&legacy, SkillEvalStatus::Passed, "legacy_c105"))
        .unwrap();
    fixture
        .repository
        .set_skill_activation(&activation(&legacy, true, "legacy_c105"))
        .unwrap();
    assert_eq!(
        fixture
            .repository
            .validate_surface_adornment_program(
                &version.asset_version_id,
                &c106_surface_program(&legacy),
            )
            .unwrap_err()
            .code(),
        "SURFACE_ADORNMENT_RECIPE_POLICY_DENIED"
    );

    fixture
        .repository
        .record_skill_eval(&report(&current, SkillEvalStatus::Passed, "current_c106"))
        .unwrap();
    fixture
        .repository
        .set_skill_activation(&activation(&current, true, "current_c106"))
        .unwrap();
    let program = c106_surface_program(&current);
    fixture
        .repository
        .validate_surface_adornment_program(&version.asset_version_id, &program)
        .unwrap();

    let expected_hash = current.canonical_sha256().unwrap();
    drop(fixture.repository);
    let restarted = CoreRepository::open(
        &fixture.db_path,
        fixture._root.path(),
        "agent-skill-c106-restart",
    )
    .unwrap();
    let active = restarted.active_skill(&current.skill_id).unwrap().unwrap();
    assert_eq!(active.skill_version, 2);
    assert_eq!(active.skill_sha256, expected_hash);
    restarted
        .validate_surface_adornment_program(&version.asset_version_id, &program)
        .unwrap();
}

#[test]
fn c106_manifest_policy_rejects_unknown_and_mixed_recipe_sets() {
    let fixture = Fixture::new();
    let mut unknown = builtin_surface_adornment_manifest_v2();
    unknown
        .recipe_ids
        .push("recipe_c106_arm_unreviewed_unknown".into());
    assert_eq!(
        fixture
            .repository
            .dry_run_skill(&unknown)
            .unwrap_err()
            .code(),
        "SKILL_RECIPE_POLICY_INVALID"
    );

    let mut mixed_domain = builtin_surface_adornment_manifest_v2();
    mixed_domain.allowed_domains = vec!["pack_future_weapon_prop".into()];
    assert_eq!(
        fixture
            .repository
            .dry_run_skill(&mixed_domain)
            .unwrap_err()
            .code(),
        "SKILL_RECIPE_DOMAIN_INVALID"
    );
}

#[test]
fn sealed_manifest_rows_reject_direct_identity_tampering() {
    let fixture = Fixture::new();
    let value = manifest(1);
    fixture
        .repository
        .create_skill_draft(&value, "2026-07-18T10:00:00Z")
        .unwrap();
    let connection = Connection::open(&fixture.db_path).unwrap();
    assert!(connection
        .execute(
            "UPDATE agent_skill_versions SET manifest_json='{}' WHERE skill_id=? AND version=?",
            [&value.skill_id, &value.version.to_string()],
        )
        .is_err());
    assert_eq!(
        fixture
            .repository
            .skill_manifest(&value.skill_id, value.version)
            .unwrap()
            .unwrap(),
        value
    );
}
