use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use forgecad_core::{
    CoreRepository, CreateReferenceEvidenceRequest, ImportExternalGlbRequest, Project,
    ProjectStatus, ReferenceClass, ReferenceEvidenceKind, EXTERNAL_GLB_REFERENCE_ROLE,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use tempfile::TempDir;

const PROJECT_ID: &str = "prj_external_atomic";

struct Fixture {
    root: TempDir,
    db: PathBuf,
    repository: CoreRepository,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let db = root.path().join("library.db");
        let repository = CoreRepository::open(&db, root.path(), "external-import-first").unwrap();
        repository
            .ensure_default_domain_profile("2026-07-17T10:00:00Z")
            .unwrap();
        repository
            .create_project(&Project {
                project_id: PROJECT_ID.into(),
                profile_id: "profile_weapon_concept_v1".into(),
                domain_type: "weapon_concept".into(),
                name: "External GLB atomic import".into(),
                status: ProjectStatus::Active,
                current_version_id: None,
                created_at: "2026-07-17T10:00:01Z".into(),
                updated_at: "2026-07-17T10:00:01Z".into(),
            })
            .unwrap();
        Self {
            root,
            db,
            repository,
        }
    }
}

#[test]
fn import_is_atomic_idempotent_cas_backed_and_restart_readable() {
    let Fixture {
        root,
        db,
        repository,
    } = Fixture::new();
    let glb = test_glb("atomic");
    let request = request(PROJECT_ID, &glb);
    let first = repository
        .import_external_glb(&request, "external-import-key", "2026-07-17T10:00:02Z")
        .unwrap();
    assert_eq!(first.response.asset_version.version_no, 1);
    assert_eq!(
        first.response.asset_version.shape_program["schema_version"],
        "ExternalGLBReference@1"
    );
    assert_eq!(
        first.response.asset_version.shape_program["editable"],
        false
    );
    assert_eq!(first.response.inspection.triangle_count, 1);
    assert_eq!(first.imported_glb.file_name, "reference.glb");
    assert_eq!(first.object.extension, "glb");
    assert_eq!(first.object.ref_count, 1);
    let snapshot = first.snapshot.as_ref().unwrap();
    assert_eq!(snapshot.revision, 1);
    assert!(snapshot.quality.is_none());
    assert!(snapshot.preview.is_none());
    assert_eq!(
        repository
            .object_for_reference(&forgecad_core::ObjectReference {
                reference_kind: "asset_version".into(),
                owner_id: first.response.asset_version.asset_version_id.clone(),
                role: EXTERNAL_GLB_REFERENCE_ROLE.into(),
            })
            .unwrap()
            .unwrap(),
        first.object
    );
    assert_eq!(repository.read_object(&first.object.sha256).unwrap(), glb);

    let replay = repository
        .import_external_glb(&request, "external-import-key", "2026-07-17T10:00:03Z")
        .unwrap();
    assert_eq!(replay, first);
    assert_counts(&db, (1, 1, 1, 1, 1));

    let mut changed = request.clone();
    changed.summary = "different semantic request".into();
    assert_eq!(
        repository
            .import_external_glb(&changed, "external-import-key", "2026-07-17T10:00:04Z")
            .unwrap_err()
            .code(),
        "IDEMPOTENCY_CONFLICT"
    );
    assert_counts(&db, (1, 1, 1, 1, 1));

    let version_id = first.response.asset_version.asset_version_id.clone();
    drop(repository);
    let restarted = CoreRepository::open(&db, root.path(), "external-import-restart").unwrap();
    let readback = restarted
        .external_glb_import_bundle(&version_id)
        .unwrap()
        .unwrap();
    restarted
        .validate_external_glb_import_bundle(&readback)
        .unwrap();
    assert_eq!(readback, first);
    assert_eq!(restarted.read_object(&readback.object.sha256).unwrap(), glb);
}

#[test]
fn rejected_or_unowned_import_leaves_no_database_or_cas_state() {
    let fixture = Fixture::new();
    let invalid = ImportExternalGlbRequest {
        glb_base64: BASE64_STANDARD.encode(b"not-a-real-glb-content"),
        ..request(PROJECT_ID, &test_glb("unused"))
    };
    assert_eq!(
        fixture
            .repository
            .import_external_glb(&invalid, "invalid-glb", "2026-07-17T10:01:00Z")
            .unwrap_err()
            .code(),
        "GLB_IMPORT_REJECTED"
    );

    let external_uri = mutate_document(test_glb("external-uri"), |document| {
        document["buffers"][0]["uri"] = json!("outside.bin");
    });
    assert_eq!(
        fixture
            .repository
            .import_external_glb(
                &request(PROJECT_ID, &external_uri),
                "external-uri",
                "2026-07-17T10:01:01Z",
            )
            .unwrap_err()
            .code(),
        "GLB_IMPORT_REJECTED"
    );

    let unknown_project_glb = test_glb("unknown-project");
    assert_eq!(
        fixture
            .repository
            .import_external_glb(
                &request("prj_missing", &unknown_project_glb),
                "unknown-project",
                "2026-07-17T10:01:02Z",
            )
            .unwrap_err()
            .code(),
        "RESOURCE_NOT_FOUND"
    );
    assert_counts(&fixture.db, (0, 0, 0, 0, 0));
    assert_eq!(
        regular_file_count(&fixture.root.path().join("objects/sha256")),
        0
    );
    assert_eq!(
        regular_file_count(&fixture.root.path().join("objects/.pending")),
        0
    );
}

#[test]
fn historical_idempotency_replay_survives_a_later_imported_head() {
    let fixture = Fixture::new();
    let first_request = request(PROJECT_ID, &test_glb("first-head"));
    let first = fixture
        .repository
        .import_external_glb(&first_request, "first-head-key", "2026-07-17T10:02:00Z")
        .unwrap();
    let second_request = request(PROJECT_ID, &test_glb("second-head"));
    let second = fixture
        .repository
        .import_external_glb(&second_request, "second-head-key", "2026-07-17T10:02:01Z")
        .unwrap();
    assert_ne!(
        first.response.asset_version.asset_version_id,
        second.response.asset_version.asset_version_id
    );
    assert_eq!(second.response.asset_version.version_no, 2);

    let historical = fixture
        .repository
        .import_external_glb(&first_request, "first-head-key", "2026-07-17T10:02:02Z")
        .unwrap();
    assert_eq!(historical.response, first.response);
    assert!(historical.snapshot.is_none());
    fixture
        .repository
        .validate_external_glb_import_bundle(&historical)
        .unwrap();
    assert_counts(&fixture.db, (2, 2, 1, 1, 2));
}

#[test]
fn r007_glb_evidence_supports_sealed_import_and_direct_bytes_without_moving_design() {
    let Fixture {
        root,
        db,
        repository,
    } = Fixture::new();
    let glb = test_glb("r007-reference");
    let imported = repository
        .import_external_glb(
            &request(PROJECT_ID, &glb),
            "r007-source-import",
            "2026-07-18T15:00:00Z",
        )
        .unwrap();
    let imported_asset_version_id = imported.response.asset_version.asset_version_id.clone();
    let head_before = repository.head(PROJECT_ID).unwrap();
    let snapshot_before = repository.snapshot(PROJECT_ID).unwrap();

    repository
        .create_project(&Project {
            project_id: "prj_reference_other".into(),
            profile_id: "profile_weapon_concept_v1".into(),
            domain_type: "weapon_concept".into(),
            name: "Cross-project reference rejection".into(),
            status: ProjectStatus::Active,
            current_version_id: None,
            created_at: "2026-07-18T15:00:00Z".into(),
            updated_at: "2026-07-18T15:00:00Z".into(),
        })
        .unwrap();
    let cross_project = CreateReferenceEvidenceRequest {
        schema_version: "ReferenceEvidenceCreateRequest@1".into(),
        client_request_id: "r007-cross-project-import".into(),
        project_id: "prj_reference_other".into(),
        kind: ReferenceEvidenceKind::Glb,
        reference_class: Some(ReferenceClass::GlbReadback),
        file_name: None,
        media_type: None,
        content_base64: None,
        imported_asset_version_id: Some(imported_asset_version_id.clone()),
        source_statement: "User selected a sealed GLB as visual evidence.".into(),
        license_statement: "User declares local reference rights.".into(),
        missing_views: vec!["detail".into()],
        user_notes: "Cross-project use must be rejected.".into(),
        domain_pack_id: None,
    };
    assert_eq!(
        repository
            .create_reference_evidence(&cross_project, "2026-07-18T15:00:00Z")
            .unwrap_err()
            .code(),
        "REFERENCE_EVIDENCE_PROJECT_MISMATCH"
    );
    assert!(repository.head("prj_reference_other").unwrap().is_none());
    assert!(repository
        .snapshot("prj_reference_other")
        .unwrap()
        .is_none());

    let imported_evidence = repository
        .create_reference_evidence(
            &CreateReferenceEvidenceRequest {
                schema_version: "ReferenceEvidenceCreateRequest@1".into(),
                client_request_id: "r007-imported-glb-evidence".into(),
                project_id: PROJECT_ID.into(),
                kind: ReferenceEvidenceKind::Glb,
                reference_class: Some(ReferenceClass::GlbReadback),
                file_name: None,
                media_type: None,
                content_base64: None,
                imported_asset_version_id: Some(imported_asset_version_id.clone()),
                source_statement: "User selected the sealed project GLB as visual evidence.".into(),
                license_statement: "User declares local reference rights.".into(),
                missing_views: vec!["detail".into()],
                user_notes: "Visible vehicle shell and material zones only.".into(),
                domain_pack_id: None,
            },
            "2026-07-18T15:00:01Z",
        )
        .unwrap();
    assert_eq!(
        imported_evidence
            .source_imported_asset_version_id
            .as_deref(),
        Some(imported_asset_version_id.as_str())
    );
    assert_eq!(
        imported_evidence.source_object_sha256,
        imported.object.sha256
    );
    assert_eq!(
        imported_evidence.glb_inspection,
        Some(imported.response.inspection)
    );
    assert_eq!(repository.head(PROJECT_ID).unwrap(), head_before);
    assert_eq!(repository.snapshot(PROJECT_ID).unwrap(), snapshot_before);

    let direct_evidence = repository
        .create_reference_evidence(
            &CreateReferenceEvidenceRequest {
                schema_version: "ReferenceEvidenceCreateRequest@1".into(),
                client_request_id: "r007-direct-glb-evidence".into(),
                project_id: PROJECT_ID.into(),
                kind: ReferenceEvidenceKind::Glb,
                reference_class: Some(ReferenceClass::GlbReadback),
                file_name: Some("authorized-reference.glb".into()),
                media_type: Some("model/gltf-binary".into()),
                content_base64: Some(BASE64_STANDARD.encode(&glb)),
                imported_asset_version_id: None,
                source_statement: "User supplied this GLB directly as visual evidence.".into(),
                license_statement: "User declares local reference rights.".into(),
                missing_views: vec!["detail".into()],
                user_notes: "Visible vehicle shell and material zones only.".into(),
                domain_pack_id: Some("pack_vehicle_concept".into()),
            },
            "2026-07-18T15:00:02Z",
        )
        .unwrap();
    assert!(direct_evidence.source_imported_asset_version_id.is_none());
    assert_eq!(direct_evidence.source_object_sha256, imported.object.sha256);
    assert!(direct_evidence.glb_inspection.is_some());
    assert_eq!(repository.head(PROJECT_ID).unwrap(), head_before);
    assert_eq!(repository.snapshot(PROJECT_ID).unwrap(), snapshot_before);

    let imported_evidence_id = imported_evidence.evidence_id.clone();
    let direct_evidence_id = direct_evidence.evidence_id.clone();
    drop(repository);
    let restarted = CoreRepository::open(&db, root.path(), "r007-glb-evidence-restart").unwrap();
    assert_eq!(
        restarted
            .reference_evidence(&imported_evidence_id)
            .unwrap()
            .unwrap(),
        imported_evidence
    );
    assert_eq!(
        restarted
            .reference_evidence(&direct_evidence_id)
            .unwrap()
            .unwrap(),
        direct_evidence
    );
    assert_eq!(restarted.head(PROJECT_ID).unwrap(), head_before);
    assert_eq!(restarted.snapshot(PROJECT_ID).unwrap(), snapshot_before);
}

fn request(project_id: &str, glb: &[u8]) -> ImportExternalGlbRequest {
    ImportExternalGlbRequest {
        client_request_id: "external-import-request".into(),
        project_id: project_id.into(),
        domain_pack_id: "pack_vehicle_concept".into(),
        file_name: "../unsafe\\reference.glb".into(),
        glb_base64: BASE64_STANDARD.encode(glb),
        summary: "Verified external vehicle reference".into(),
    }
}

fn assert_counts(db: &Path, expected: (i64, i64, i64, i64, i64)) {
    let connection = Connection::open(db).unwrap();
    let actual = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM agent_asset_versions), (SELECT COUNT(*) FROM agent_imported_glbs), (SELECT COUNT(*) FROM agent_asset_heads), (SELECT COUNT(*) FROM active_design_snapshots), (SELECT COUNT(*) FROM forgecad_core_object_references WHERE role='external_reference_glb')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .unwrap();
    assert_eq!(actual, expected);
}

fn regular_file_count(path: &Path) -> usize {
    if !path.exists() {
        return 0;
    }
    std::fs::read_dir(path)
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

fn test_glb(label: &str) -> Vec<u8> {
    let positions = [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    let indices = [0_u16, 1, 2];
    let mut binary = Vec::new();
    for value in positions {
        binary.extend_from_slice(&value.to_le_bytes());
    }
    let index_offset = binary.len();
    for value in indices {
        binary.extend_from_slice(&value.to_le_bytes());
    }
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let document = json!({
        "asset":{"version":"2.0","generator":label},
        "scene":0,
        "scenes":[{"nodes":[0]}],
        "nodes":[{"mesh":0}],
        "meshes":[{"primitives":[{
            "attributes":{"POSITION":0},"indices":1,"material":0,"mode":4
        }]}],
        "materials":[{}],
        "buffers":[{"byteLength":binary.len()}],
        "bufferViews":[
            {"buffer":0,"byteOffset":0,"byteLength":index_offset},
            {"buffer":0,"byteOffset":index_offset,"byteLength":6,"target":34963}
        ],
        "accessors":[
            {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]},
            {"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}
        ]
    });
    encode_glb(document, binary)
}

fn mutate_document(glb: Vec<u8>, mutate: impl FnOnce(&mut Value)) -> Vec<u8> {
    let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let mut document: Value =
        serde_json::from_slice(glb[20..20 + json_length].trim_ascii_end()).unwrap();
    mutate(&mut document);
    let binary_offset = 20 + json_length;
    let binary_length =
        u32::from_le_bytes(glb[binary_offset..binary_offset + 4].try_into().unwrap()) as usize;
    let binary = glb[binary_offset + 8..binary_offset + 8 + binary_length].to_vec();
    encode_glb(document, binary)
}

fn encode_glb(document: Value, mut binary: Vec<u8>) -> Vec<u8> {
    let mut json_chunk = serde_json::to_vec(&document).unwrap();
    while json_chunk.len() % 4 != 0 {
        json_chunk.push(b' ');
    }
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let total = 12 + 8 + json_chunk.len() + 8 + binary.len();
    let mut glb = Vec::with_capacity(total);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json_chunk);
    glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    glb.extend_from_slice(&binary);
    glb
}
