use std::{collections::BTreeMap, path::PathBuf};

use forgecad_core::{
    semantic_sha256, ActiveDesign, AgentAssetVersion, AssetStage, AssetVersionStatus,
    BlockoutCandidate, CandidateStatus, CoreRepository, Project, ProjectStatus, QualityReport,
    QualityStatus, SnapshotEtag,
};
use rusqlite::{params, Connection};
use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const PROJECT_ID: &str = "prj_legacy_conversion";
const LEGACY_VERSION_ID: &str = "ver_legacy_conversion_v1";
const LEGACY_GRAPH_ID: &str = "mg_legacy_conversion_v1";
const ARTIFACT_ID: &str = "artifact_legacy_conversion_rebuild";
const ASSET_VERSION_ID: &str = "assetver_legacy_conversion_rebuild";
const QUALITY_ID: &str = "quality_legacy_conversion_rebuild";
const NOW: &str = "2026-07-17T12:00:00Z";

struct Fixture {
    root: TempDir,
    db: PathBuf,
    repository: CoreRepository,
}

impl Fixture {
    fn new(instance_id: &str, seed_legacy: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let db = root.path().join("library.db");
        let repository = CoreRepository::open(&db, root.path(), instance_id).unwrap();
        repository.ensure_default_domain_profile(NOW).unwrap();
        repository
            .create_project(&Project {
                project_id: PROJECT_ID.into(),
                profile_id: "profile_weapon_concept_v1".into(),
                domain_type: "weapon_concept".into(),
                name: "Legacy conversion fixture".into(),
                status: ProjectStatus::Active,
                current_version_id: None,
                created_at: NOW.into(),
                updated_at: NOW.into(),
            })
            .unwrap();
        if seed_legacy {
            seed_legacy_source(&db);
        }
        Self {
            root,
            db,
            repository,
        }
    }
}

fn seed_legacy_source(db: &PathBuf) {
    let connection = Connection::open(db).unwrap();
    connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    let graph = json!({
        "schema_version":"ModuleGraph@1",
        "graph_id":LEGACY_GRAPH_ID,
        "project_id":PROJECT_ID,
        "root_node_id":"node_root",
        "nodes":[{
            "node_id":"node_root",
            "module_id":"module_legacy_shell",
            "transform":{
                "position":[0.0,0.0,0.0],
                "rotation":[0.0,0.0,0.0],
                "scale":[1.0,1.0,1.0]
            },
            "mirror_axis":"none",
            "locked":false,
            "visible":true
        }],
        "edges":[]
    });
    let graph_json = serde_json::to_string(&graph).unwrap();
    let graph_sha256 = semantic_sha256(&graph).unwrap();
    let spec = json!({
        "schema_version":"WeaponConceptSpec@1",
        "project_id":PROJECT_ID,
        "profile_id":"profile_weapon_concept_v1",
        "name":"Legacy conversion fixture",
        "archetype":"future_modular_sidearm",
        "intended_uses":["game_asset","film_prop"],
        "style":{
            "keywords":["future","mechanical"],
            "palette":["graphite","signal_red"],
            "detail_density":0.8
        },
        "proportions":{
            "overall_length_mm":320.0,
            "body_height_mm":120.0,
            "grip_angle_deg":12.0
        },
        "required_slots":["core.front","core.rear","core.grip"],
        "optional_slots":["core.top"],
        "constraints":{
            "symmetry":"mostly_symmetric",
            "max_triangle_count":250000
        },
        "assumptions":["Visual-only historical concept fixture."]
    });
    let spec_json = serde_json::to_string(&spec).unwrap();
    let spec_sha256 = semantic_sha256(&spec).unwrap();
    connection
        .execute(
            "INSERT INTO module_graphs(graph_id, project_id, version_id, root_node_id, schema_version, graph_json, graph_sha256, validation_status, created_at, updated_at) VALUES (?, ?, NULL, 'node_root', 'ModuleGraph@1', ?, ?, 'valid', ?, ?)",
            params![
                LEGACY_GRAPH_ID,
                PROJECT_ID,
                graph_json,
                graph_sha256,
                NOW,
                NOW,
            ],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO project_versions(version_id, project_id, parent_version_id, version_no, status, summary, spec_schema_version, spec_json, spec_sha256, module_graph_id, change_set_id, created_at) VALUES (?, ?, NULL, 1, 'committed', 'legacy source', 'WeaponConceptSpec@1', ?, ?, ?, NULL, ?)",
            params![
                LEGACY_VERSION_ID,
                PROJECT_ID,
                spec_json,
                spec_sha256,
                LEGACY_GRAPH_ID,
                NOW,
            ],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE module_graphs SET version_id=? WHERE graph_id=?",
            params![LEGACY_VERSION_ID, LEGACY_GRAPH_ID],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE projects SET current_version_id=? WHERE project_id=?",
            params![LEGACY_VERSION_ID, PROJECT_ID],
        )
        .unwrap();
}

fn legacy_semantic_hash(db: &PathBuf) -> String {
    let connection = Connection::open(db).unwrap();
    let project = connection
        .query_row(
            "SELECT project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at FROM projects WHERE project_id=?",
            [PROJECT_ID],
            |row| {
                Ok(json!({
                    "project_id": row.get::<_, String>(0)?,
                    "profile_id": row.get::<_, String>(1)?,
                    "domain_type": row.get::<_, String>(2)?,
                    "name": row.get::<_, String>(3)?,
                    "status": row.get::<_, String>(4)?,
                    "current_version_id": row.get::<_, Option<String>>(5)?,
                    "created_at": row.get::<_, String>(6)?,
                    "updated_at": row.get::<_, String>(7)?,
                }))
            },
        )
        .unwrap();
    let version = connection
        .query_row(
            "SELECT version_id, project_id, parent_version_id, version_no, status, summary, spec_schema_version, spec_json, spec_sha256, module_graph_id, change_set_id, created_at FROM project_versions WHERE version_id=?",
            [LEGACY_VERSION_ID],
            |row| {
                Ok(json!({
                    "version_id": row.get::<_, String>(0)?,
                    "project_id": row.get::<_, String>(1)?,
                    "parent_version_id": row.get::<_, Option<String>>(2)?,
                    "version_no": row.get::<_, u64>(3)?,
                    "status": row.get::<_, String>(4)?,
                    "summary": row.get::<_, String>(5)?,
                    "spec_schema_version": row.get::<_, String>(6)?,
                    "spec_json": row.get::<_, String>(7)?,
                    "spec_sha256": row.get::<_, String>(8)?,
                    "module_graph_id": row.get::<_, Option<String>>(9)?,
                    "change_set_id": row.get::<_, Option<String>>(10)?,
                    "created_at": row.get::<_, String>(11)?,
                }))
            },
        )
        .unwrap();
    let graph = connection
        .query_row(
            "SELECT graph_id, project_id, version_id, root_node_id, schema_version, graph_json, graph_sha256, validation_status, created_at, updated_at FROM module_graphs WHERE graph_id=?",
            [LEGACY_GRAPH_ID],
            |row| {
                Ok(json!({
                    "graph_id": row.get::<_, String>(0)?,
                    "project_id": row.get::<_, String>(1)?,
                    "version_id": row.get::<_, Option<String>>(2)?,
                    "root_node_id": row.get::<_, String>(3)?,
                    "schema_version": row.get::<_, String>(4)?,
                    "graph_json": row.get::<_, String>(5)?,
                    "graph_sha256": row.get::<_, String>(6)?,
                    "validation_status": row.get::<_, String>(7)?,
                    "created_at": row.get::<_, String>(8)?,
                    "updated_at": row.get::<_, String>(9)?,
                }))
            },
        )
        .unwrap();
    semantic_sha256(&json!({"project":project,"version":version,"graph":graph})).unwrap()
}

fn request_hash(expected_revision: u64, client_request_id: &str) -> String {
    semantic_sha256(&json!({
        "project_id": PROJECT_ID,
        "expected_revision": expected_revision,
        "request": {
            "client_request_id": client_request_id,
            "snapshot_revision": expected_revision,
        }
    }))
    .unwrap()
}

fn authorize(
    repository: &CoreRepository,
    key: &str,
) -> forgecad_core::LegacyActiveDesignConversionResponse {
    repository
        .authorize_legacy_conversion_idempotent(
            PROJECT_ID,
            SnapshotEtag(1),
            NOW,
            &format!("POST /api/v1/projects/{PROJECT_ID}/active-design:convert-legacy"),
            key,
            &request_hash(1, key),
        )
        .unwrap()
}

fn shape_program() -> serde_json::Value {
    json!({"schema_version":"ShapeProgram@1","program_id":"shape_legacy_rebuild"})
}

fn assembly_graph() -> serde_json::Value {
    json!({
        "schema_version":"AssemblyGraph@1",
        "graph_id":"graph_legacy_rebuild",
        "parts":[{"part_id":"part_shell","material_zone_ids":["zone_shell"]}],
    })
}

fn candidate() -> BlockoutCandidate {
    BlockoutCandidate {
        artifact_id: ARTIFACT_ID.into(),
        project_id: Some(PROJECT_ID.into()),
        plan_id: "plan_legacy_rebuild".into(),
        direction_id: "direction_best".into(),
        domain_pack_id: "pack_weapon_concept_v1".into(),
        status: CandidateStatus::Candidate,
        candidate: json!({"selection":"internal_best"}),
        shape_program: shape_program(),
        assembly_graph: assembly_graph(),
        material_bindings: BTreeMap::new(),
        glb_sha256: String::new(),
        created_at: NOW.into(),
        updated_at: NOW.into(),
    }
}

fn version() -> AgentAssetVersion {
    AgentAssetVersion {
        asset_version_id: ASSET_VERSION_ID.into(),
        project_id: PROJECT_ID.into(),
        parent_asset_version_id: None,
        version_no: 1,
        status: AssetVersionStatus::Committed,
        summary: "Explicit Agent rebuild".into(),
        stage: AssetStage::EditableAsset,
        plan_id: "plan_legacy_rebuild".into(),
        direction_id: "direction_best".into(),
        domain_pack_id: "pack_weapon_concept_v1".into(),
        artifact_id: ARTIFACT_ID.into(),
        parts: vec![json!({"part_id":"part_shell","role":"core_shell"})],
        shape_program: shape_program(),
        assembly_graph: assembly_graph(),
        material_bindings: BTreeMap::new(),
        created_at: "2026-07-17T12:00:01Z".into(),
    }
}

fn quality(version: &AgentAssetVersion, production_glb: &[u8]) -> QualityReport {
    QualityReport {
        quality_report_id: QUALITY_ID.into(),
        project_id: PROJECT_ID.into(),
        asset_version_id: ASSET_VERSION_ID.into(),
        report: json!({
            "schema_version":"AgentAssetQualityReport@1",
            "quality_report_id":QUALITY_ID,
            "asset_version_id":ASSET_VERSION_ID,
            "status":"passed",
            "evidence_source":"geometry_compile_readback",
            "triangle_count":8192,
            "compile_readback":{
                "schema_version":"GeometryCompileReadback@2",
                "artifact_profile":{"artifact_profile_id":"production_concept","profile_sha256":"a".repeat(64)},
                "shape_program_sha256":semantic_sha256(&version.shape_program).unwrap(),
                "glb_sha256":format!("{:x}", Sha256::digest(production_glb)),
                "glb_byte_size":production_glb.len(),
                "triangle_count":8192,
                "closed_manifold":true,
                "surface_provenance_present":true,
            },
        }),
        status: QualityStatus::Passed,
        created_at: "2026-07-17T12:00:02Z".into(),
    }
}

fn glb(label: &str) -> Vec<u8> {
    let mut json_chunk =
        serde_json::to_vec(&json!({"asset":{"version":"2.0"},"extras":{"label":label}})).unwrap();
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

fn table_count(db: &PathBuf, table: &str) -> i64 {
    Connection::open(db)
        .unwrap()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

#[test]
fn authorization_is_cas_idempotent_restart_durable_and_geometry_free() {
    let Fixture {
        root,
        db,
        repository,
    } = Fixture::new("legacy_authorize_first", true);
    let legacy_before = legacy_semantic_hash(&db);

    let stale = repository
        .authorize_legacy_conversion_idempotent(
            PROJECT_ID,
            SnapshotEtag(2),
            NOW,
            &format!("POST /api/v1/projects/{PROJECT_ID}/active-design:convert-legacy"),
            "legacy_stale",
            &request_hash(2, "legacy_stale"),
        )
        .unwrap_err();
    assert_eq!(stale.code(), "ACTIVE_DESIGN_STALE");
    assert!(repository.snapshot(PROJECT_ID).unwrap().is_none());

    let first = authorize(&repository, "legacy_authorize");
    assert_eq!(first.schema_version, "LegacyActiveDesignConversion@1");
    assert_eq!(first.status, "ready_for_agent_rebuild");
    assert_eq!(first.snapshot_revision, 1);
    assert_eq!(first.source.source, "legacy_concept_read_only");
    let snapshot = repository.snapshot(PROJECT_ID).unwrap().unwrap();
    assert!(matches!(
        snapshot.active_design,
        ActiveDesign::LegacyConceptReadOnly { .. }
    ));
    assert_eq!(snapshot.revision, 1);
    assert_eq!(
        repository
            .legacy_conversion_intent(PROJECT_ID)
            .unwrap()
            .unwrap()
            .snapshot_revision,
        1
    );
    assert_eq!(legacy_semantic_hash(&db), legacy_before);
    for table in [
        "agent_asset_versions",
        "agent_asset_heads",
        "agent_blockout_candidates",
    ] {
        assert_eq!(table_count(&db, table), 0, "{table}");
    }

    assert_eq!(authorize(&repository, "legacy_authorize"), first);
    let conflict = repository
        .authorize_legacy_conversion_idempotent(
            PROJECT_ID,
            SnapshotEtag(1),
            NOW,
            &format!("POST /api/v1/projects/{PROJECT_ID}/active-design:convert-legacy"),
            "legacy_authorize",
            &"f".repeat(64),
        )
        .unwrap_err();
    assert_eq!(conflict.code(), "IDEMPOTENCY_CONFLICT");

    drop(repository);
    let restarted = CoreRepository::open(&db, root.path(), "legacy_authorize_restart").unwrap();
    assert_eq!(
        restarted
            .legacy_conversion_intent(PROJECT_ID)
            .unwrap()
            .unwrap()
            .snapshot_revision,
        1
    );
    assert_eq!(authorize(&restarted, "legacy_authorize"), first);
    assert_eq!(legacy_semantic_hash(&db), legacy_before);
}

#[test]
fn first_agent_bundle_requires_exact_intent_then_activates_and_consumes_it_atomically() {
    let fixture = Fixture::new("legacy_bundle", true);
    let legacy_before = legacy_semantic_hash(&fixture.db);
    let production = glb("legacy-production");
    let interactive = glb("legacy-interactive");
    let version = version();
    let quality = quality(&version, &production);

    let unauthorized = fixture
        .repository
        .commit_candidate_bundle(candidate(), &production, &interactive, &version, &quality)
        .unwrap_err();
    assert_eq!(unauthorized.code(), "LEGACY_CONVERSION_NOT_AUTHORIZED");
    // No Snapshot existed yet, so a bundle cannot infer conversion consent
    // from legacy project data alone.
    assert_eq!(table_count(&fixture.db, "agent_asset_versions"), 0);
    assert_eq!(table_count(&fixture.db, "forgecad_core_objects"), 0);

    authorize(&fixture.repository, "legacy_bundle_authorize");
    let bundle = fixture
        .repository
        .commit_candidate_bundle(candidate(), &production, &interactive, &version, &quality)
        .unwrap();
    assert_eq!(bundle.snapshot.revision, 2);
    assert_eq!(
        bundle.snapshot.active_design.asset_version_id(),
        Some(ASSET_VERSION_ID)
    );
    assert_eq!(
        bundle.snapshot.quality.as_ref().unwrap().quality_report_id,
        QUALITY_ID
    );
    assert!(fixture
        .repository
        .legacy_conversion_intent(PROJECT_ID)
        .unwrap()
        .is_none());
    assert_eq!(legacy_semantic_hash(&fixture.db), legacy_before);
    assert_eq!(table_count(&fixture.db, "agent_asset_versions"), 1);
    assert_eq!(table_count(&fixture.db, "agent_asset_heads"), 1);

    let replay = fixture
        .repository
        .commit_candidate_bundle(candidate(), &production, &interactive, &version, &quality)
        .unwrap();
    assert_eq!(replay, bundle);
    assert!(fixture
        .repository
        .legacy_conversion_intent(PROJECT_ID)
        .unwrap()
        .is_none());
}

#[test]
fn stale_authorization_and_non_legacy_projects_are_rejected_without_partial_state() {
    let fixture = Fixture::new("legacy_stale_intent", true);
    authorize(&fixture.repository, "legacy_stale_intent_authorize");
    Connection::open(&fixture.db)
        .unwrap()
        .execute(
            "UPDATE legacy_agent_conversion_intents SET snapshot_revision=2 WHERE project_id=?",
            [PROJECT_ID],
        )
        .unwrap();
    let production = glb("stale-production");
    let interactive = glb("stale-interactive");
    let rebuilt_version = version();
    let quality = quality(&rebuilt_version, &production);
    let stale = fixture
        .repository
        .commit_candidate_bundle(
            candidate(),
            &production,
            &interactive,
            &rebuilt_version,
            &quality,
        )
        .unwrap_err();
    assert_eq!(stale.code(), "LEGACY_CONVERSION_AUTHORIZATION_STALE");
    assert_eq!(table_count(&fixture.db, "agent_asset_versions"), 0);
    assert_eq!(table_count(&fixture.db, "forgecad_core_objects"), 0);

    let corrupt = Fixture::new("legacy_corrupt_hash", true);
    Connection::open(&corrupt.db)
        .unwrap()
        .execute(
            "UPDATE project_versions SET spec_sha256=? WHERE version_id=?",
            params!["f".repeat(64), LEGACY_VERSION_ID],
        )
        .unwrap();
    let corrupt_source = corrupt
        .repository
        .authorize_legacy_conversion_idempotent(
            PROJECT_ID,
            SnapshotEtag(1),
            NOW,
            &format!("POST /api/v1/projects/{PROJECT_ID}/active-design:convert-legacy"),
            "legacy_corrupt_hash",
            &request_hash(1, "legacy_corrupt_hash"),
        )
        .unwrap_err();
    assert_eq!(corrupt_source.code(), "LEGACY_SEMANTIC_HASH_MISMATCH");
    assert!(corrupt.repository.snapshot(PROJECT_ID).unwrap().is_none());
    assert!(corrupt
        .repository
        .legacy_conversion_intent(PROJECT_ID)
        .unwrap()
        .is_none());

    let empty = Fixture::new("legacy_missing", false);
    let missing = empty
        .repository
        .authorize_legacy_conversion_idempotent(
            PROJECT_ID,
            SnapshotEtag(1),
            NOW,
            &format!("POST /api/v1/projects/{PROJECT_ID}/active-design:convert-legacy"),
            "legacy_missing",
            &request_hash(1, "legacy_missing"),
        )
        .unwrap_err();
    assert_eq!(missing.code(), "RESOURCE_NOT_FOUND");
    assert!(empty
        .repository
        .legacy_conversion_intent(PROJECT_ID)
        .unwrap()
        .is_none());

    let agent = Fixture::new("legacy_already_agent", false);
    agent.repository.commit_initial_asset(&version()).unwrap();
    let already_agent = agent
        .repository
        .authorize_legacy_conversion_idempotent(
            PROJECT_ID,
            SnapshotEtag(1),
            NOW,
            &format!("POST /api/v1/projects/{PROJECT_ID}/active-design:convert-legacy"),
            "legacy_already_agent",
            &request_hash(1, "legacy_already_agent"),
        )
        .unwrap_err();
    assert_eq!(already_agent.code(), "ACTIVE_DESIGN_NOT_LEGACY");
    assert!(agent
        .repository
        .legacy_conversion_intent(PROJECT_ID)
        .unwrap()
        .is_none());
}
