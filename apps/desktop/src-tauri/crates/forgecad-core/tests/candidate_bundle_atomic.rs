use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Barrier},
};

use forgecad_core::{
    semantic_sha256, AgentAssetVersion, AssetStage, AssetVersionStatus, BlockoutCandidate,
    CandidateStatus, CoreRepository, ObjectReference, Project, ProjectStatus, QualityReport,
    QualityStatus,
};
use rusqlite::Connection;
use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const PROJECT_ID: &str = "project_atomic_bundle";
const ARTIFACT_ID: &str = "artifact_atomic_bundle";
const VERSION_ID: &str = "assetver_atomic_bundle_v1";
const QUALITY_ID: &str = "quality_atomic_bundle_v1";

struct Fixture {
    root: TempDir,
    db: PathBuf,
    repository: CoreRepository,
}

impl Fixture {
    fn new(instance_id: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let db = root.path().join("library.db");
        let repository = CoreRepository::open(&db, root.path(), instance_id).unwrap();
        repository
            .ensure_default_domain_profile("2026-07-17T08:00:00Z")
            .unwrap();
        repository
            .create_project(&Project {
                project_id: PROJECT_ID.into(),
                profile_id: "profile_weapon_concept_v1".into(),
                domain_type: "weapon_concept".into(),
                name: "Atomic candidate bundle".into(),
                status: ProjectStatus::Active,
                current_version_id: None,
                created_at: "2026-07-17T08:00:01Z".into(),
                updated_at: "2026-07-17T08:00:01Z".into(),
            })
            .unwrap();
        Self {
            root,
            db,
            repository,
        }
    }
}

fn candidate() -> BlockoutCandidate {
    BlockoutCandidate {
        artifact_id: ARTIFACT_ID.into(),
        project_id: Some(PROJECT_ID.into()),
        plan_id: "plan_atomic_bundle".into(),
        direction_id: "direction_best".into(),
        domain_pack_id: "pack_weapon_concept_v1".into(),
        status: CandidateStatus::Candidate,
        candidate: json!({"score":0.99,"selection":"internal_best"}),
        shape_program: shape_program(),
        assembly_graph: assembly_graph(),
        material_bindings: BTreeMap::new(),
        glb_sha256: String::new(),
        created_at: "2026-07-17T08:00:02Z".into(),
        updated_at: "2026-07-17T08:00:02Z".into(),
    }
}

fn version() -> AgentAssetVersion {
    AgentAssetVersion {
        asset_version_id: VERSION_ID.into(),
        project_id: PROJECT_ID.into(),
        parent_asset_version_id: None,
        version_no: 1,
        status: AssetVersionStatus::Committed,
        summary: "Selected production concept".into(),
        stage: AssetStage::EditableAsset,
        plan_id: "plan_atomic_bundle".into(),
        direction_id: "direction_best".into(),
        domain_pack_id: "pack_weapon_concept_v1".into(),
        artifact_id: ARTIFACT_ID.into(),
        parts: vec![json!({"part_id":"part_shell","role":"core_shell"})],
        shape_program: shape_program(),
        assembly_graph: assembly_graph(),
        material_bindings: BTreeMap::new(),
        created_at: "2026-07-17T08:00:03Z".into(),
    }
}

fn shape_program() -> serde_json::Value {
    json!({
        "schema_version":"ShapeProgram@1",
        "program_id":"shape_atomic_bundle",
        "shell":"production",
    })
}

fn assembly_graph() -> serde_json::Value {
    json!({
        "schema_version":"AssemblyGraph@1",
        "graph_id":"graph_atomic_bundle",
        "parts":[{"part_id":"part_shell","material_zone_ids":["zone_shell"]}],
    })
}

fn quality(version: &AgentAssetVersion, production_glb: &[u8]) -> QualityReport {
    let production_sha = format!("{:x}", Sha256::digest(production_glb));
    let shape_sha = semantic_sha256(&version.shape_program).unwrap();
    QualityReport {
        quality_report_id: QUALITY_ID.into(),
        project_id: PROJECT_ID.into(),
        asset_version_id: VERSION_ID.into(),
        report: json!({
            "schema_version":"AgentAssetQualityReport@1",
            "quality_report_id":QUALITY_ID,
            "asset_version_id":VERSION_ID,
            "status":"passed",
            "evidence_source":"geometry_compile_readback",
            "triangle_count":8192,
            "compile_readback":{
                "schema_version":"GeometryCompileReadback@2",
                "artifact_profile":{
                    "artifact_profile_id":"production_concept",
                    "profile_sha256":"a".repeat(64),
                },
                "shape_program_sha256":shape_sha,
                "glb_sha256":production_sha,
                "glb_byte_size":production_glb.len(),
                "triangle_count":8192,
                "closed_manifold":true,
                "surface_provenance_present":true,
            },
        }),
        status: QualityStatus::Passed,
        created_at: "2026-07-17T08:00:04Z".into(),
    }
}

fn glb(label: &str) -> Vec<u8> {
    let mut json_chunk = serde_json::to_vec(&json!({
        "asset":{"version":"2.0"},
        "extras":{"label":label},
    }))
    .unwrap();
    while json_chunk.len() % 4 != 0 {
        json_chunk.push(b' ');
    }
    let total_length = 12 + 8 + json_chunk.len();
    let mut bytes = Vec::with_capacity(total_length);
    bytes.extend_from_slice(b"glTF");
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&(total_length as u32).to_le_bytes());
    bytes.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"JSON");
    bytes.extend_from_slice(&json_chunk);
    bytes
}

fn table_count(db: &Path, table: &str) -> i64 {
    let connection = Connection::open(db).unwrap();
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn regular_file_count(path: &Path) -> usize {
    if !path.exists() {
        return 0;
    }
    fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap())
        .map(|entry| {
            if entry.file_type().unwrap().is_dir() {
                regular_file_count(&entry.path())
            } else {
                1
            }
        })
        .sum()
}

#[test]
fn bundle_is_atomic_replayable_and_restart_readable_with_two_distinct_hashes() {
    let Fixture {
        root,
        db,
        repository,
    } = Fixture::new("bundle_distinct_first");
    let production = glb("production");
    let interactive = glb("interactive");
    let candidate = candidate();
    let version = version();
    let quality = quality(&version, &production);

    let first = repository
        .commit_candidate_bundle(
            candidate.clone(),
            &production,
            &interactive,
            &version,
            &quality,
        )
        .unwrap();
    assert_eq!(first.candidate.status, CandidateStatus::Committed);
    assert_eq!(first.snapshot.revision, 2);
    assert_eq!(first.production_glb.ref_count, 1);
    assert_eq!(first.interactive_preview_glb.ref_count, 1);
    assert_ne!(
        first.production_glb.sha256,
        first.interactive_preview_glb.sha256
    );
    repository.validate_candidate_bundle(&first).unwrap();
    assert_eq!(
        repository
            .read_candidate_bundle(ARTIFACT_ID, VERSION_ID, QUALITY_ID)
            .unwrap(),
        Some(first.clone())
    );

    let replay = repository
        .commit_candidate_bundle(candidate, &production, &interactive, &version, &quality)
        .unwrap();
    assert_eq!(replay, first);
    for (table, expected) in [
        ("agent_blockout_candidates", 1),
        ("agent_asset_versions", 1),
        ("agent_asset_heads", 1),
        ("active_design_snapshots", 1),
        ("agent_asset_quality_reports", 1),
        ("forgecad_core_objects", 2),
        ("forgecad_core_object_references", 2),
    ] {
        assert_eq!(table_count(&db, table), expected, "{table}");
    }

    drop(repository);
    let restarted = CoreRepository::open(&db, root.path(), "bundle_distinct_restart").unwrap();
    let after_restart = restarted
        .read_candidate_bundle(ARTIFACT_ID, VERSION_ID, QUALITY_ID)
        .unwrap()
        .unwrap();
    assert_eq!(after_restart, first);
    restarted.validate_candidate_bundle(&after_restart).unwrap();
}

#[test]
fn concurrent_same_bundle_has_one_state_and_both_callers_get_complete_readback() {
    let fixture = Fixture::new("bundle_concurrent");
    let repository = Arc::new(fixture.repository);
    let production = Arc::new(glb("production-concurrent"));
    let interactive = Arc::new(glb("interactive-concurrent"));
    let candidate = candidate();
    let version = version();
    let quality = quality(&version, &production);
    let barrier = Arc::new(Barrier::new(3));

    let attempts = (0..2)
        .map(|_| {
            let repository = repository.clone();
            let production = production.clone();
            let interactive = interactive.clone();
            let candidate = candidate.clone();
            let version = version.clone();
            let quality = quality.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                repository.commit_candidate_bundle(
                    candidate,
                    &production,
                    &interactive,
                    &version,
                    &quality,
                )
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let results = attempts
        .into_iter()
        .map(|attempt| attempt.join().unwrap().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results[0], results[1]);
    assert_eq!(table_count(&fixture.db, "agent_asset_versions"), 1);
    assert_eq!(table_count(&fixture.db, "forgecad_core_objects"), 2);
    assert_eq!(
        table_count(&fixture.db, "forgecad_core_object_references"),
        2
    );
}

#[test]
fn late_transaction_failure_rolls_back_every_row_and_cleans_promoted_files() {
    let fixture = Fixture::new("bundle_failure");
    let connection = Connection::open(&fixture.db).unwrap();
    connection
        .execute_batch(&format!(
            "CREATE TRIGGER fail_atomic_bundle_quality BEFORE INSERT ON agent_asset_quality_reports WHEN NEW.quality_report_id='{QUALITY_ID}' BEGIN SELECT RAISE(ABORT, 'injected bundle failure'); END;"
        ))
        .unwrap();
    drop(connection);
    let production = glb("production-failure");
    let interactive = glb("interactive-failure");
    let version = version();
    let quality = quality(&version, &production);
    let error = fixture
        .repository
        .commit_candidate_bundle(candidate(), &production, &interactive, &version, &quality)
        .unwrap_err();
    assert_eq!(error.code(), "SQLITE_OPERATION_FAILED");
    assert_eq!(
        fixture
            .repository
            .read_candidate_bundle(ARTIFACT_ID, VERSION_ID, QUALITY_ID)
            .unwrap(),
        None
    );
    for table in [
        "agent_blockout_candidates",
        "forgecad_core_candidate_objects",
        "agent_asset_versions",
        "agent_asset_heads",
        "active_design_snapshots",
        "agent_asset_quality_reports",
        "forgecad_core_objects",
        "forgecad_core_object_references",
    ] {
        assert_eq!(table_count(&fixture.db, table), 0, "{table}");
    }
    assert_eq!(
        regular_file_count(&fixture.root.path().join("objects/.staging")),
        0
    );
    assert_eq!(
        regular_file_count(&fixture.root.path().join("objects/.pending")),
        0
    );
    assert_eq!(
        regular_file_count(&fixture.root.path().join("objects/sha256")),
        0
    );
}

#[test]
fn identical_profile_bytes_deduplicate_to_one_object_with_two_role_references() {
    let fixture = Fixture::new("bundle_deduplicate");
    let glb = glb("shared-profile");
    let version = version();
    let quality = quality(&version, &glb);
    let bundle = fixture
        .repository
        .commit_candidate_bundle(candidate(), &glb, &glb, &version, &quality)
        .unwrap();
    assert_eq!(
        bundle.production_glb.sha256,
        bundle.interactive_preview_glb.sha256
    );
    assert_eq!(bundle.production_glb.ref_count, 2);
    assert_eq!(bundle.interactive_preview_glb.ref_count, 2);
    assert_eq!(table_count(&fixture.db, "forgecad_core_objects"), 1);
    assert_eq!(
        table_count(&fixture.db, "forgecad_core_object_references"),
        2
    );
}

#[test]
fn incomplete_existing_candidate_and_invalid_quality_never_masquerade_as_success() {
    let fixture = Fixture::new("bundle_incomplete");
    let production = glb("production-incomplete");
    let interactive = glb("interactive-incomplete");
    let stored_candidate = fixture
        .repository
        .create_candidate(candidate(), &production)
        .unwrap();
    let mut replay_candidate = candidate();
    replay_candidate.glb_sha256 = stored_candidate.glb_sha256;
    let partial_version = version();
    let partial_quality = quality(&partial_version, &production);
    let incomplete = fixture
        .repository
        .commit_candidate_bundle(
            replay_candidate,
            &production,
            &interactive,
            &partial_version,
            &partial_quality,
        )
        .unwrap_err();
    assert_eq!(incomplete.code(), "CANDIDATE_BUNDLE_INCOMPLETE");
    assert!(fixture.repository.version(VERSION_ID).unwrap().is_none());
    assert!(fixture
        .repository
        .quality_report(QUALITY_ID)
        .unwrap()
        .is_none());

    let fresh = Fixture::new("bundle_bad_quality");
    let invalid_version = version();
    let mut invalid_quality = quality(&invalid_version, &production);
    invalid_quality.report["compile_readback"]["closed_manifold"] = json!(false);
    let invalid = fresh
        .repository
        .commit_candidate_bundle(
            candidate(),
            &production,
            &interactive,
            &invalid_version,
            &invalid_quality,
        )
        .unwrap_err();
    assert_eq!(invalid.code(), "CANDIDATE_BUNDLE_QUALITY_INVALID");
    assert_eq!(table_count(&fresh.db, "forgecad_core_objects"), 0);
    assert_eq!(regular_file_count(&fresh.root.path().join("objects")), 0);
}

#[test]
fn replay_with_different_interactive_hash_is_an_idempotency_conflict() {
    let fixture = Fixture::new("bundle_replay_conflict");
    let production = glb("production-replay");
    let interactive = glb("interactive-replay");
    let version = version();
    let quality = quality(&version, &production);
    fixture
        .repository
        .commit_candidate_bundle(candidate(), &production, &interactive, &version, &quality)
        .unwrap();
    let conflict = fixture
        .repository
        .commit_candidate_bundle(
            candidate(),
            &production,
            &glb("interactive-different"),
            &version,
            &quality,
        )
        .unwrap_err();
    assert_eq!(conflict.code(), "CANDIDATE_BUNDLE_IDEMPOTENCY_CONFLICT");
    assert_eq!(table_count(&fixture.db, "forgecad_core_objects"), 2);
}

#[test]
fn required_object_roles_are_addressable_by_the_existing_reference_api() {
    let fixture = Fixture::new("bundle_roles");
    let production = glb("production-roles");
    let interactive = glb("interactive-roles");
    let version = version();
    let quality = quality(&version, &production);
    let bundle = fixture
        .repository
        .commit_candidate_bundle(candidate(), &production, &interactive, &version, &quality)
        .unwrap();
    for (role, expected_sha) in [
        ("production_glb", bundle.production_glb.sha256),
        (
            "interactive_preview_glb",
            bundle.interactive_preview_glb.sha256,
        ),
    ] {
        let object = fixture
            .repository
            .object_for_reference(&ObjectReference {
                reference_kind: "asset_version".into(),
                owner_id: VERSION_ID.into(),
                role: role.into(),
            })
            .unwrap()
            .unwrap();
        assert_eq!(object.sha256, expected_sha);
    }
}
