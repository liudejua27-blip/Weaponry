use std::{
    collections::BTreeMap,
    sync::{Arc, Barrier},
};

use forgecad_core::{
    semantic_sha256, AgentAssetChangeSet, AgentAssetVersion, AssetStage, AssetVersionStatus,
    ChangeSetStatus, CoreRepository, MigrationRunner, NavigationAction, ObjectReference, Project,
    ProjectStatus, QualityReport, QualityStatus, SnapshotEtag,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tempfile::tempdir;

const PROFILE_ID: &str = "profile_weapon_concept_v1";
const PRODUCTION_GLB: &[u8] = b"glTF\x02\x00\x00\x00k003-production-concept";

fn project(project_id: &str, timestamp: &str) -> Project {
    Project {
        project_id: project_id.to_string(),
        profile_id: PROFILE_ID.to_string(),
        domain_type: "weapon_concept".to_string(),
        name: format!("K003 {project_id}"),
        status: ProjectStatus::Active,
        current_version_id: None,
        created_at: timestamp.to_string(),
        updated_at: timestamp.to_string(),
    }
}

fn asset(
    project_id: &str,
    asset_version_id: &str,
    parent_asset_version_id: Option<&str>,
    version_no: u64,
    shell: &str,
    timestamp: &str,
) -> AgentAssetVersion {
    AgentAssetVersion {
        asset_version_id: asset_version_id.to_string(),
        project_id: project_id.to_string(),
        parent_asset_version_id: parent_asset_version_id.map(str::to_string),
        version_no,
        status: AssetVersionStatus::Committed,
        summary: format!("Production concept {shell}"),
        stage: AssetStage::EditableAsset,
        plan_id: format!("plan_{project_id}"),
        direction_id: "direction_best".to_string(),
        domain_pack_id: "pack_weapon_concept_v1".to_string(),
        artifact_id: format!("artifact_{asset_version_id}"),
        parts: vec![
            json!({"part_id":"part_shell","role":"core_shell"}),
            json!({"part_id":"part_trim","role":"side_accessory"}),
        ],
        shape_program: json!({
            "schema_version":"ShapeProgram@1",
            "program_id":format!("shape_{asset_version_id}"),
            "shell":shell,
            "artifact_profile_id":"production_concept"
        }),
        assembly_graph: json!({
            "schema_version":"AssemblyGraph@1",
            "graph_id":format!("graph_{asset_version_id}"),
            "parts":[
                {"part_id":"part_shell","material_zone_ids":["zone_shell"]},
                {"part_id":"part_trim","material_zone_ids":["zone_trim"]}
            ]
        }),
        material_bindings: BTreeMap::new(),
        created_at: timestamp.to_string(),
    }
}

fn proposed_change(project_id: &str, base_id: &str) -> AgentAssetChangeSet {
    AgentAssetChangeSet {
        change_set_id: "change_k003_acceptance".to_string(),
        project_id: project_id.to_string(),
        base_asset_version_id: base_id.to_string(),
        summary: "Refine the visual shell".to_string(),
        operations: vec![json!({
            "operation_id":"op_transform_shell",
            "op":"set_part_transform",
            "part_id":"part_shell",
            "transform":{
                "position":[0,0,0],
                "rotation":[0,0,0],
                "scale":[1,1,1]
            }
        })],
        protected_part_ids: Vec::new(),
        preview: None,
        status: ChangeSetStatus::Proposed,
        resulting_asset_version_id: None,
        created_at: "2026-07-17T01:00:00Z".to_string(),
        updated_at: "2026-07-17T01:00:00Z".to_string(),
    }
}

#[test]
fn concurrent_cas_project_switch_restart_and_export_hashes_are_authoritative() {
    let root = tempdir().unwrap();
    let db = root.path().join("library.db");
    let repository = CoreRepository::open(&db, root.path(), "k003_acceptance_first").unwrap();
    repository
        .ensure_default_domain_profile("2026-07-17T00:00:00Z")
        .unwrap();
    repository
        .create_project(&project("project_acceptance_a", "2026-07-17T00:00:01Z"))
        .unwrap();
    repository
        .create_project(&project("project_acceptance_b", "2026-07-17T00:00:02Z"))
        .unwrap();

    let a_v1 = asset(
        "project_acceptance_a",
        "asset_acceptance_a_v1",
        None,
        1,
        "shell-a",
        "2026-07-17T00:01:00Z",
    );
    let b_v1 = asset(
        "project_acceptance_b",
        "asset_acceptance_b_v1",
        None,
        1,
        "shell-b",
        "2026-07-17T00:01:01Z",
    );
    repository.commit_initial_asset(&a_v1).unwrap();
    repository.commit_initial_asset(&b_v1).unwrap();

    for version in [&a_v1, &b_v1] {
        for role in ["interactive_preview_glb", "production_glb"] {
            repository
                .attach_object_bytes(
                    &ObjectReference {
                        reference_kind: "asset_version".to_string(),
                        owner_id: version.asset_version_id.clone(),
                        role: role.to_string(),
                    },
                    PRODUCTION_GLB,
                    "glb",
                    "2026-07-17T00:02:00Z",
                )
                .unwrap();
        }
    }

    // Two real SQLite transactions race with one Snapshot ETag. Exactly one
    // may advance the authoritative selection; the loser must be stale.
    let barrier = Arc::new(Barrier::new(3));
    let attempts = [
        ("part_shell", "zone_shell", "2026-07-17T00:03:00Z"),
        ("part_trim", "zone_trim", "2026-07-17T00:03:01Z"),
    ]
    .into_iter()
    .map(|(part_id, zone_id, timestamp)| {
        let repository = repository.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            repository.select(
                "project_acceptance_a",
                SnapshotEtag(1),
                Some(part_id),
                Some(zone_id),
                timestamp,
            )
        })
    })
    .collect::<Vec<_>>();
    barrier.wait();
    let outcomes = attempts
        .into_iter()
        .map(|attempt| attempt.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    let loser = outcomes
        .iter()
        .find_map(|result| result.as_ref().err())
        .unwrap();
    assert_eq!(loser.code(), "ACTIVE_DESIGN_STALE");

    let after_cas = repository
        .snapshot("project_acceptance_a")
        .unwrap()
        .unwrap();
    assert_eq!(after_cas.revision, 2);
    let b_unchanged = repository
        .snapshot("project_acceptance_b")
        .unwrap()
        .unwrap();
    assert_eq!(b_unchanged.revision, 1);
    assert_eq!(
        b_unchanged.active_design.asset_version_id(),
        Some("asset_acceptance_b_v1")
    );

    repository
        .create_change_set(&proposed_change(
            "project_acceptance_a",
            "asset_acceptance_a_v1",
        ))
        .unwrap();
    let preview = asset(
        "project_acceptance_a",
        "asset_acceptance_a_preview",
        Some("asset_acceptance_a_v1"),
        2,
        "shell-refined",
        "2026-07-17T01:01:00Z",
    );
    let before_stale = repository
        .snapshot("project_acceptance_a")
        .unwrap()
        .unwrap();
    let stale = repository
        .preview_change_set(
            "change_k003_acceptance",
            &preview,
            SnapshotEtag(1),
            "2026-07-17T01:01:01Z",
        )
        .unwrap_err();
    assert_eq!(stale.code(), "ACTIVE_DESIGN_STALE");
    assert_eq!(
        repository
            .snapshot("project_acceptance_a")
            .unwrap()
            .unwrap(),
        before_stale
    );
    assert_eq!(
        repository
            .change_set("change_k003_acceptance")
            .unwrap()
            .unwrap()
            .status,
        ChangeSetStatus::Proposed
    );

    let (_, preview_snapshot) = repository
        .preview_change_set(
            "change_k003_acceptance",
            &preview,
            before_stale.etag(),
            "2026-07-17T01:02:00Z",
        )
        .unwrap();
    let mut a_v2 = preview.clone();
    a_v2.asset_version_id = "asset_acceptance_a_v2".into();
    a_v2.created_at = "2026-07-17T01:03:00Z".into();
    let (_, _, confirmed) = repository
        .confirm_change_set("change_k003_acceptance", &a_v2, preview_snapshot.etag())
        .unwrap();
    for role in ["interactive_preview_glb", "production_glb"] {
        repository
            .attach_object_bytes(
                &ObjectReference {
                    reference_kind: "asset_version".to_string(),
                    owner_id: a_v2.asset_version_id.clone(),
                    role: role.to_string(),
                },
                PRODUCTION_GLB,
                "glb",
                "2026-07-17T01:03:01Z",
            )
            .unwrap();
    }

    let undone = repository
        .navigate(
            "project_acceptance_a",
            NavigationAction::Undo,
            "asset_acceptance_a_v3_undo",
            confirmed.etag(),
            "2026-07-17T01:04:00Z",
        )
        .unwrap();
    assert_eq!(undone.version.shape_program, a_v1.shape_program);
    let redone = repository
        .navigate(
            "project_acceptance_a",
            NavigationAction::Redo,
            "asset_acceptance_a_v4_redo",
            undone.snapshot.etag(),
            "2026-07-17T01:05:00Z",
        )
        .unwrap();
    assert_eq!(redone.version.shape_program, a_v2.shape_program);

    let quality = QualityReport {
        quality_report_id: "quality_acceptance_a_v4".to_string(),
        project_id: "project_acceptance_a".to_string(),
        asset_version_id: redone.version.asset_version_id.clone(),
        report: json!({
            "schema_version":"AgentAssetQualityReport@1",
            "artifact_profile_id":"production_concept",
            "glb_sha256":semantic_sha256(&PRODUCTION_GLB).unwrap(),
            "readback_status":"passed"
        }),
        status: QualityStatus::Passed,
        created_at: "2026-07-17T01:06:00Z".to_string(),
    };
    let quality_snapshot = repository
        .attach_quality(&quality, redone.snapshot.etag())
        .unwrap();
    let (export_object, export_snapshot) = repository
        .attach_export_bytes(
            "project_acceptance_a",
            quality_snapshot.etag(),
            "production_glb",
            PRODUCTION_GLB,
            "glb",
            "2026-07-17T01:07:00Z",
        )
        .unwrap();
    assert_eq!(
        repository.read_object(&export_object.sha256).unwrap(),
        PRODUCTION_GLB
    );
    assert_eq!(export_object.ref_count, 11);
    assert_eq!(export_snapshot, quality_snapshot);

    // Switching projects is a read of a different authoritative Snapshot, not
    // a mutation of the active Project's version chain.
    let a_before_b_switch = repository
        .snapshot("project_acceptance_a")
        .unwrap()
        .unwrap();
    let b_selected = repository
        .select(
            "project_acceptance_b",
            b_unchanged.etag(),
            Some("part_shell"),
            Some("zone_shell"),
            "2026-07-17T01:08:00Z",
        )
        .unwrap();
    assert_eq!(b_selected.revision, 2);
    assert_eq!(
        repository
            .snapshot("project_acceptance_a")
            .unwrap()
            .unwrap(),
        a_before_b_switch
    );

    let snapshot_hash = a_before_b_switch.semantic_hash().unwrap();
    let version_hash = repository
        .version("asset_acceptance_a_v4_redo")
        .unwrap()
        .unwrap()
        .semantic_hash()
        .unwrap();
    let glb_hash = export_object.sha256.clone();
    repository.publish().unwrap();
    drop(repository);

    let restarted = CoreRepository::open(&db, root.path(), "k003_acceptance_restarted").unwrap();
    restarted.publish().unwrap();
    assert_eq!(
        restarted
            .snapshot("project_acceptance_a")
            .unwrap()
            .unwrap()
            .semantic_hash()
            .unwrap(),
        snapshot_hash
    );
    assert_eq!(
        restarted
            .version("asset_acceptance_a_v4_redo")
            .unwrap()
            .unwrap()
            .semantic_hash()
            .unwrap(),
        version_hash
    );
    assert_eq!(restarted.read_object(&glb_hash).unwrap(), PRODUCTION_GLB);
    assert_eq!(restarted.object(&glb_hash).unwrap().unwrap().ref_count, 11);
    assert_eq!(
        restarted
            .quality_report("quality_acceptance_a_v4")
            .unwrap()
            .unwrap(),
        quality
    );
}

#[test]
fn legacy_semantic_hash_adapter_is_read_only_across_restart() {
    let root = tempdir().unwrap();
    let db = root.path().join("library.db");
    MigrationRunner::new(&db).run().unwrap();

    let profile: Value = json!({
        "schema_version":"DesignDomainProfile@1",
        "profile_id":"profile_legacy_acceptance",
        "domain_type":"weapon_concept",
        "non_functional_only":true
    });
    let spec: Value = json!({
        "schema_version":"WeaponConceptSpec@1",
        "name":"Legacy read-only concept"
    });
    let graph: Value = json!({
        "schema_version":"ModuleGraph@1",
        "root_node_id":"legacy_root",
        "nodes":[]
    });
    let profile_text = serde_json::to_string(&profile).unwrap();
    let spec_text = serde_json::to_string(&spec).unwrap();
    let graph_text = serde_json::to_string(&graph).unwrap();
    let connection = Connection::open(&db).unwrap();
    connection.execute(
        "INSERT INTO domain_profiles(profile_id, domain_type, schema_version, pack_id, display_name, profile_json, profile_sha256, status, created_at, updated_at) VALUES (?, 'weapon_concept', 'DesignDomainProfile@1', 'pack_legacy_acceptance', 'Legacy', ?, ?, 'active', 't0', 't0')",
        params!["profile_legacy_acceptance", profile_text, semantic_sha256(&profile).unwrap()],
    ).unwrap();
    connection.execute(
        "INSERT INTO projects(project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at) VALUES ('project_legacy_acceptance', 'profile_legacy_acceptance', 'weapon_concept', 'Legacy', 'active', 'version_legacy_acceptance', 't0', 't0')",
        [],
    ).unwrap();
    connection.execute(
        "INSERT INTO project_versions(version_id, project_id, parent_version_id, version_no, status, summary, spec_schema_version, spec_json, spec_sha256, module_graph_id, change_set_id, created_at) VALUES ('version_legacy_acceptance', 'project_legacy_acceptance', NULL, 1, 'committed', 'legacy', 'WeaponConceptSpec@1', ?, ?, 'graph_legacy_acceptance', NULL, 't0')",
        params![spec_text, semantic_sha256(&spec).unwrap()],
    ).unwrap();
    connection.execute(
        "INSERT INTO module_graphs(graph_id, project_id, version_id, root_node_id, schema_version, graph_json, graph_sha256, validation_status, created_at, updated_at) VALUES ('graph_legacy_acceptance', 'project_legacy_acceptance', 'version_legacy_acceptance', 'legacy_root', 'ModuleGraph@1', ?, ?, 'valid', 't0', 't0')",
        params![graph_text, semantic_sha256(&graph).unwrap()],
    ).unwrap();
    drop(connection);

    let expected = semantic_sha256(&json!({
        "project_id":"project_legacy_acceptance",
        "profile":profile,
        "spec":spec,
        "graph":graph
    }))
    .unwrap();
    let repository = CoreRepository::open(&db, root.path(), "legacy_hash_first").unwrap();
    assert_eq!(
        repository
            .legacy_read_only_hash("project_legacy_acceptance")
            .unwrap()
            .as_deref(),
        Some(expected.as_str())
    );
    let connection = Connection::open(&db).unwrap();
    let agent_versions: i64 = connection
        .query_row("SELECT COUNT(*) FROM agent_asset_versions", [], |row| {
            row.get(0)
        })
        .unwrap();
    let snapshots: i64 = connection
        .query_row("SELECT COUNT(*) FROM active_design_snapshots", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!((agent_versions, snapshots), (0, 0));
    drop(connection);
    repository.publish().unwrap();
    drop(repository);

    let restarted = CoreRepository::open(&db, root.path(), "legacy_hash_restarted").unwrap();
    restarted.publish().unwrap();
    assert_eq!(
        restarted
            .legacy_read_only_hash("project_legacy_acceptance")
            .unwrap()
            .as_deref(),
        Some(expected.as_str())
    );
    let connection = Connection::open(&db).unwrap();
    let state: (i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM agent_asset_versions), (SELECT COUNT(*) FROM active_design_snapshots)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(state, (0, 0));
}
