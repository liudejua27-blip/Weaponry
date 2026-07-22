use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Barrier},
};

use forgecad_core::{
    semantic_sha256, verify_forgecad_glb, AgentAssetChangeSet, AgentAssetVersion, AssetStage,
    AssetVersionStatus, ChangeSetStatus, CoreRepository, ObjectReference, Project, ProjectStatus,
    QualityReport, QualityStatus, SnapshotEtag,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const PROJECT_ID: &str = "project_change_bundle";
const BASE_ID: &str = "asset_change_bundle_v1";
const PREVIEW_ID: &str = "preview_change_bundle_v2";
const RESULT_ID: &str = "asset_change_bundle_v2";
const CHANGE_ID: &str = "change_bundle_v2";
const QUALITY_ID: &str = "quality_change_bundle_v2";

struct Fixture {
    root: TempDir,
    db: PathBuf,
    repository: CoreRepository,
}

impl Fixture {
    fn new(instance: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let db = root.path().join("library.db");
        let repository = CoreRepository::open(&db, root.path(), instance).unwrap();
        repository
            .ensure_default_domain_profile("2026-07-17T10:00:00Z")
            .unwrap();
        repository
            .create_project(&Project {
                project_id: PROJECT_ID.into(),
                profile_id: "profile_weapon_concept_v1".into(),
                domain_type: "weapon_concept".into(),
                name: "ChangeSet bundle".into(),
                status: ProjectStatus::Active,
                current_version_id: None,
                created_at: "2026-07-17T10:00:01Z".into(),
                updated_at: "2026-07-17T10:00:01Z".into(),
            })
            .unwrap();
        repository.commit_initial_asset(&base_version()).unwrap();
        repository.create_change_set(&change_set()).unwrap();
        Self {
            root,
            db,
            repository,
        }
    }
}

fn base_version() -> AgentAssetVersion {
    asset(
        BASE_ID,
        None,
        1,
        "base",
        "artifact_base",
        "2026-07-17T10:00:02Z",
    )
}

fn preview_version() -> AgentAssetVersion {
    asset(
        PREVIEW_ID,
        Some(BASE_ID),
        2,
        "refined",
        "artifact_refined",
        "2026-07-17T10:00:04Z",
    )
}

fn resulting_version() -> AgentAssetVersion {
    asset(
        RESULT_ID,
        Some(BASE_ID),
        2,
        "refined",
        "artifact_refined",
        "2026-07-17T10:00:05Z",
    )
}

fn asset(
    id: &str,
    parent: Option<&str>,
    version_no: u64,
    shell: &str,
    artifact_id: &str,
    created_at: &str,
) -> AgentAssetVersion {
    AgentAssetVersion {
        asset_version_id: id.into(),
        project_id: PROJECT_ID.into(),
        parent_asset_version_id: parent.map(str::to_string),
        version_no,
        status: AssetVersionStatus::Committed,
        summary: format!("{shell} production concept"),
        stage: AssetStage::EditableAsset,
        plan_id: "plan_change_bundle".into(),
        direction_id: "direction_best".into(),
        domain_pack_id: "pack_weapon_concept_v1".into(),
        artifact_id: artifact_id.into(),
        parts: vec![json!({"part_id":"part_shell","role":"core_shell"})],
        shape_program: json!({
            "schema_version":"ShapeProgram@1",
            "program_id":format!("shape_{shell}"),
            "shell":shell,
        }),
        assembly_graph: json!({
            "schema_version":"AssemblyGraph@1",
            "graph_id":format!("graph_{shell}"),
            "parts":[{"part_id":"part_shell","material_zone_ids":["zone_shell"]}],
        }),
        material_bindings: BTreeMap::new(),
        created_at: created_at.into(),
    }
}

fn change_set() -> AgentAssetChangeSet {
    AgentAssetChangeSet {
        change_set_id: CHANGE_ID.into(),
        project_id: PROJECT_ID.into(),
        base_asset_version_id: BASE_ID.into(),
        summary: "Refine shell silhouette".into(),
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
        protected_part_ids: vec![],
        preview: None,
        status: ChangeSetStatus::Proposed,
        resulting_asset_version_id: None,
        created_at: "2026-07-17T10:00:03Z".into(),
        updated_at: "2026-07-17T10:00:03Z".into(),
    }
}

fn interactive_metadata(glb: &[u8], preview: &AgentAssetVersion) -> Value {
    let facts = verify_forgecad_glb(glb, Some("interactive_preview")).unwrap();
    json!({
        "runtime_manifest_version":facts.runtime_manifest_version,
        "artifact_profile_id":"interactive_preview",
        "shape_program_sha256":semantic_sha256(&preview.shape_program).unwrap(),
        "glb_sha256":facts.glb_sha256,
        "glb_byte_size":facts.glb_byte_size,
        "triangle_count":facts.triangle_count,
        "bounds_mm":facts.bounds_mm,
        "mesh_count":facts.mesh_count,
        "primitive_count":facts.primitive_count,
        "material_count":facts.material_count,
        "closed_manifold":facts.closed_manifold,
        "surface_provenance_present":facts.surface_provenance_present,
    })
}

fn quality(glb: &[u8], version: &AgentAssetVersion) -> QualityReport {
    let facts = verify_forgecad_glb(glb, Some("production_concept")).unwrap();
    QualityReport {
        quality_report_id: QUALITY_ID.into(),
        project_id: PROJECT_ID.into(),
        asset_version_id: RESULT_ID.into(),
        report: json!({
            "schema_version":"AgentAssetQualityReport@1",
            "quality_report_id":QUALITY_ID,
            "asset_version_id":RESULT_ID,
            "status":"passed",
            "evidence_source":"geometry_compile_readback",
            "triangle_count":facts.triangle_count,
            "bounds_mm":facts.bounds_mm,
            "compile_readback":{
                "schema_version":"GeometryCompileReadback@2",
                "runtime_manifest_version":facts.runtime_manifest_version,
                "artifact_profile":{
                    "artifact_profile_id":"production_concept",
                    "profile_sha256":facts.artifact_profile_sha256,
                },
                "shape_program_sha256":semantic_sha256(&version.shape_program).unwrap(),
                "glb_sha256":facts.glb_sha256,
                "glb_byte_size":facts.glb_byte_size,
                "triangle_count":facts.triangle_count,
                "bounds_mm":facts.bounds_mm,
                "mesh_count":facts.mesh_count,
                "primitive_count":facts.primitive_count,
                "material_count":facts.material_count,
                "closed_manifold":facts.closed_manifold,
                "surface_provenance_present":facts.surface_provenance_present,
            }
        }),
        status: QualityStatus::Passed,
        created_at: "2026-07-17T10:00:06Z".into(),
    }
}

#[test]
fn preview_and_confirm_bundles_are_atomic_replayable_and_restart_readable() {
    let Fixture {
        root,
        db,
        repository,
    } = Fixture::new("change_bundle_first");
    let interactive = profile_glb("interactive_preview");
    let production = profile_glb("production_concept");
    let preview = preview_version();
    let resulting = resulting_version();
    let readback = interactive_metadata(&interactive, &preview);
    let quality = quality(&production, &resulting);

    let preview_bundle = repository
        .preview_change_set_bundle(
            CHANGE_ID,
            &preview,
            &interactive,
            &readback,
            SnapshotEtag(1),
            "2026-07-17T10:00:04Z",
        )
        .unwrap();
    assert_eq!(preview_bundle.snapshot.revision, 2);
    assert_eq!(preview_bundle.change_set.status, ChangeSetStatus::Previewed);
    repository
        .validate_change_set_preview_bundle(&preview_bundle)
        .unwrap();
    assert_eq!(
        repository
            .preview_change_set_bundle(
                CHANGE_ID,
                &preview,
                &interactive,
                &readback,
                SnapshotEtag(1),
                "2026-07-17T10:00:04Z",
            )
            .unwrap(),
        preview_bundle
    );

    let confirmed = repository
        .confirm_change_set_bundle(
            CHANGE_ID,
            &preview,
            &resulting,
            &interactive,
            &production,
            &quality,
            SnapshotEtag(2),
        )
        .unwrap();
    assert_eq!(confirmed.change_set.status, ChangeSetStatus::Confirmed);
    assert_eq!(confirmed.snapshot.revision, 3);
    assert!(confirmed.snapshot.preview.is_none());
    assert_eq!(
        confirmed
            .snapshot
            .quality
            .as_ref()
            .map(|value| value.quality_report_id.as_str()),
        Some(QUALITY_ID)
    );
    assert!(repository
        .object_for_reference(&ObjectReference {
            reference_kind: "preview".into(),
            owner_id: CHANGE_ID.into(),
            role: "interactive_preview_glb".into(),
        })
        .unwrap()
        .is_none());
    repository
        .validate_change_set_confirm_bundle(&confirmed)
        .unwrap();
    assert_eq!(
        repository
            .confirm_change_set_bundle(
                CHANGE_ID,
                &preview,
                &resulting,
                &interactive,
                &production,
                &quality,
                SnapshotEtag(2),
            )
            .unwrap(),
        confirmed
    );

    drop(repository);
    let restarted = CoreRepository::open(&db, root.path(), "change_bundle_restart").unwrap();
    let readback = restarted
        .read_change_set_confirm_bundle(CHANGE_ID, RESULT_ID, QUALITY_ID)
        .unwrap()
        .unwrap();
    assert_eq!(readback, confirmed);
    restarted
        .validate_change_set_confirm_bundle(&readback)
        .unwrap();
}

#[test]
fn confirm_failure_at_quality_insert_keeps_complete_preview_and_no_partial_result() {
    let fixture = Fixture::new("change_bundle_failure");
    let interactive = profile_glb("interactive_preview");
    let production = profile_glb("production_concept");
    let preview = preview_version();
    let resulting = resulting_version();
    let readback = interactive_metadata(&interactive, &preview);
    let quality = quality(&production, &resulting);
    let preview_bundle = fixture
        .repository
        .preview_change_set_bundle(
            CHANGE_ID,
            &preview,
            &interactive,
            &readback,
            SnapshotEtag(1),
            "2026-07-17T10:00:04Z",
        )
        .unwrap();
    let connection = Connection::open(&fixture.db).unwrap();
    connection
        .execute_batch(&format!(
            "CREATE TRIGGER fail_change_bundle_quality BEFORE INSERT ON agent_asset_quality_reports WHEN NEW.quality_report_id='{QUALITY_ID}' BEGIN SELECT RAISE(ABORT, 'injected confirm failure'); END;"
        ))
        .unwrap();
    drop(connection);

    let error = fixture
        .repository
        .confirm_change_set_bundle(
            CHANGE_ID,
            &preview,
            &resulting,
            &interactive,
            &production,
            &quality,
            SnapshotEtag(2),
        )
        .unwrap_err();
    assert_eq!(error.code(), "SQLITE_OPERATION_FAILED");
    assert!(fixture.repository.version(RESULT_ID).unwrap().is_none());
    assert!(fixture
        .repository
        .quality_report(QUALITY_ID)
        .unwrap()
        .is_none());
    assert_eq!(
        fixture.repository.head(PROJECT_ID).unwrap().as_deref(),
        Some(BASE_ID)
    );
    assert_eq!(
        fixture.repository.snapshot(PROJECT_ID).unwrap().unwrap(),
        preview_bundle.snapshot
    );
    assert_eq!(
        fixture
            .repository
            .read_change_set_preview_bundle(CHANGE_ID)
            .unwrap()
            .unwrap(),
        preview_bundle
    );
    let production_sha = verify_forgecad_glb(&production, None).unwrap().glb_sha256;
    assert!(fixture
        .repository
        .object(&production_sha)
        .unwrap()
        .is_none());
}

#[test]
fn missing_preview_reference_is_an_explicit_partial_conflict() {
    let fixture = Fixture::new("change_bundle_partial");
    let interactive = profile_glb("interactive_preview");
    let preview = preview_version();
    let readback = interactive_metadata(&interactive, &preview);
    fixture
        .repository
        .preview_change_set_bundle(
            CHANGE_ID,
            &preview,
            &interactive,
            &readback,
            SnapshotEtag(1),
            "2026-07-17T10:00:04Z",
        )
        .unwrap();
    let connection = Connection::open(&fixture.db).unwrap();
    connection
        .execute(
            "DELETE FROM forgecad_core_object_references WHERE reference_kind='preview' AND owner_id=?",
            [CHANGE_ID],
        )
        .unwrap();
    drop(connection);
    let error = fixture
        .repository
        .read_change_set_preview_bundle(CHANGE_ID)
        .unwrap_err();
    assert_eq!(error.code(), "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE");
}

#[test]
fn locking_after_proposal_blocks_preview_without_any_preview_side_effect() {
    let fixture = Fixture::new("change_bundle_lock_before_preview");
    let interactive = profile_glb("interactive_preview");
    let preview = preview_version();
    let readback = interactive_metadata(&interactive, &preview);
    let locked = fixture
        .repository
        .set_part_display_idempotent(
            PROJECT_ID,
            SnapshotEtag(1),
            "lock",
            Some("part_shell"),
            "2026-07-17T10:00:04Z",
            "test:lock-before-preview",
            "lock-before-preview",
            &"b".repeat(64),
        )
        .unwrap();
    let before_change = fixture.repository.change_set(CHANGE_ID).unwrap().unwrap();
    let error = fixture
        .repository
        .preview_change_set_bundle(
            CHANGE_ID,
            &preview,
            &interactive,
            &readback,
            locked.etag(),
            "2026-07-17T10:00:05Z",
        )
        .unwrap_err();
    assert_eq!(error.code(), "PART_PROTECTED");
    assert_eq!(
        fixture.repository.snapshot(PROJECT_ID).unwrap().unwrap(),
        locked
    );
    assert_eq!(
        fixture.repository.change_set(CHANGE_ID).unwrap().unwrap(),
        before_change
    );
    assert!(fixture
        .repository
        .object_for_reference(&ObjectReference {
            reference_kind: "preview".into(),
            owner_id: CHANGE_ID.into(),
            role: "interactive_preview_glb".into(),
        })
        .unwrap()
        .is_none());
    let sha = verify_forgecad_glb(&interactive, None).unwrap().glb_sha256;
    assert!(fixture.repository.object(&sha).unwrap().is_none());
}

#[test]
fn a_lock_appearing_after_preview_blocks_confirm_and_preserves_preview_bundle() {
    let fixture = Fixture::new("change_bundle_lock_before_confirm");
    let interactive = profile_glb("interactive_preview");
    let production = profile_glb("production_concept");
    let preview = preview_version();
    let resulting = resulting_version();
    let readback = interactive_metadata(&interactive, &preview);
    let quality = quality(&production, &resulting);
    let preview_bundle = fixture
        .repository
        .preview_change_set_bundle(
            CHANGE_ID,
            &preview,
            &interactive,
            &readback,
            SnapshotEtag(1),
            "2026-07-17T10:00:04Z",
        )
        .unwrap();

    // Simulate a later authoritative Snapshot write racing in from an older
    // runtime. The confirm transaction must re-check locks rather than trust
    // the earlier proposal/preview validation.
    let display = json!({
        "schema_version":"ActiveDesignPartDisplay@1",
        "project_id":PROJECT_ID,
        "asset_version_id":BASE_ID,
        "locked_part_ids":["part_shell"],
        "hidden_part_ids":[],
        "isolated_part_id":null,
    });
    let connection = Connection::open(&fixture.db).unwrap();
    connection
        .execute(
            "UPDATE active_design_snapshots SET part_display_json=?, revision=revision+1, updated_at='2026-07-17T10:00:05Z' WHERE project_id=? AND revision=2",
            rusqlite::params![serde_json::to_string(&display).unwrap(), PROJECT_ID],
        )
        .unwrap();
    drop(connection);
    let locked_snapshot = fixture.repository.snapshot(PROJECT_ID).unwrap().unwrap();
    assert_eq!(locked_snapshot.revision, 3);

    let error = fixture
        .repository
        .confirm_change_set_bundle(
            CHANGE_ID,
            &preview,
            &resulting,
            &interactive,
            &production,
            &quality,
            locked_snapshot.etag(),
        )
        .unwrap_err();
    assert_eq!(error.code(), "PART_PROTECTED");
    assert!(fixture.repository.version(RESULT_ID).unwrap().is_none());
    assert!(fixture
        .repository
        .quality_report(QUALITY_ID)
        .unwrap()
        .is_none());
    assert_eq!(
        fixture.repository.head(PROJECT_ID).unwrap().as_deref(),
        Some(BASE_ID)
    );
    assert_eq!(
        fixture.repository.snapshot(PROJECT_ID).unwrap().unwrap(),
        locked_snapshot
    );
    let still_previewed = fixture
        .repository
        .read_change_set_preview_bundle(CHANGE_ID)
        .unwrap()
        .unwrap();
    assert_eq!(still_previewed.change_set, preview_bundle.change_set);
    assert_eq!(
        still_previewed.sealed_preview,
        preview_bundle.sealed_preview
    );
    let production_sha = verify_forgecad_glb(&production, None).unwrap().glb_sha256;
    assert!(fixture
        .repository
        .object(&production_sha)
        .unwrap()
        .is_none());
}

#[test]
fn concurrent_confirm_callers_receive_one_identical_authoritative_bundle() {
    let fixture = Fixture::new("change_bundle_concurrent_confirm");
    let interactive = Arc::new(profile_glb("interactive_preview"));
    let production = Arc::new(profile_glb("production_concept"));
    let preview = preview_version();
    let resulting = resulting_version();
    let readback = interactive_metadata(&interactive, &preview);
    let quality = quality(&production, &resulting);
    fixture
        .repository
        .preview_change_set_bundle(
            CHANGE_ID,
            &preview,
            &interactive,
            &readback,
            SnapshotEtag(1),
            "2026-07-17T10:00:04Z",
        )
        .unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let attempts = (0..2)
        .map(|_| {
            let repository = fixture.repository.clone();
            let interactive = interactive.clone();
            let production = production.clone();
            let preview = preview.clone();
            let resulting = resulting.clone();
            let quality = quality.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                repository.confirm_change_set_bundle(
                    CHANGE_ID,
                    &preview,
                    &resulting,
                    &interactive,
                    &production,
                    &quality,
                    SnapshotEtag(2),
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
    assert_eq!(
        fixture
            .repository
            .read_change_set_confirm_bundle(CHANGE_ID, RESULT_ID, QUALITY_ID)
            .unwrap(),
        Some(results[0].clone())
    );
}

fn profile_glb(profile_id: &str) -> Vec<u8> {
    let production = profile_id == "production_concept";
    let mut profile = json!({
        "schema_version":"GeometryArtifactProfile@1",
        "artifact_profile_id":profile_id,
        "radial_segments":if production { 64 } else { 24 },
        "capsule_hemisphere_segments":if production { 14 } else { 5 },
        "smooth_loft_normals":production,
        "texture_width":if production { 1024 } else { 128 },
        "texture_height":if production { 1024 } else { 128 },
        "texture_mime_type":"image/png",
        "texture_compression":"png_deflate",
        "delivery":if production { "on_demand" } else { "interactive" },
        "triangle_budget_multiplier":if production { 6 } else { 1 },
        "max_triangle_count":if production { 250_000 } else { 100_000 },
    });
    profile["profile_sha256"] = Value::String(semantic_sha256(&profile).unwrap());
    let dimension = if production { 1024_u32 } else { 128_u32 };
    let texture_version = if production { "v4" } else { "v3" };
    let indices = [0_u16, 1, 2, 0, 3, 1, 0, 2, 3, 1, 3, 2];
    let positions = [0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    let normals = [0_f32, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
    let tangents = [
        1_f32, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0,
    ];
    let uvs = [0_f32, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    let mut binary = Vec::new();
    let mut views = Vec::<Value>::new();
    let mut append_view = |payload: &[u8], target: Option<u64>| {
        let offset = binary.len();
        binary.extend_from_slice(payload);
        let index = views.len();
        let mut view = json!({"buffer":0,"byteOffset":offset,"byteLength":payload.len()});
        if let Some(target) = target {
            view["target"] = json!(target);
        }
        views.push(view);
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        index
    };
    let index_view = append_view(
        &indices
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
        Some(34963),
    );
    let position_view = append_view(
        &positions
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
        Some(34962),
    );
    let normal_view = append_view(
        &normals
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
        Some(34962),
    );
    let tangent_view = append_view(
        &tangents
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
        Some(34962),
    );
    let uv_view = append_view(
        &uvs.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>(),
        Some(34962),
    );
    let mut images = Vec::new();
    let mut textures = Vec::new();
    for (index, role) in [
        "base_color",
        "metallic_roughness",
        "normal",
        "occlusion",
        "emissive",
    ]
    .into_iter()
    .enumerate()
    {
        let mut png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR".to_vec();
        png.extend_from_slice(&dimension.to_be_bytes());
        png.extend_from_slice(&dimension.to_be_bytes());
        let view = append_view(&png, None);
        images.push(json!({
            "name":format!("vtex_test_{role}_{texture_version}"),
            "bufferView":view,
            "mimeType":"image/png",
            "extras":{"forgecad_visual_texture":{
                "texture_id":format!("vtex_test_{role}_{texture_version}"),
                "texture_role":role,
                "mime_type":"image/png",
                "byte_size":png.len(),
                "sha256":format!("{:x}", Sha256::digest(&png)),
                "color_space":if matches!(role, "base_color" | "emissive") { "srgb" } else { "linear" },
                "width":dimension,"height":dimension,"source":"forgecad_builtin",
                "license":"not_applicable","fallback":"none","visual_only":true
            }}
        }));
        textures.push(json!({"name":format!("vtex_test_{role}_{texture_version}"),"source":index}));
    }
    drop(append_view);
    let document = json!({
        "asset":{"version":"2.0","generator":"ForgeCAD test"},
        "scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],
        "meshes":[{"primitives":[{
            "attributes":{"POSITION":1,"NORMAL":2,"TANGENT":3,"TEXCOORD_0":4},
            "indices":0,"material":0,"mode":4,
            "extras":{"forgecad_feature_node_id":"op_shell","forgecad_material_zone_id":"zone_shell",
                "forgecad_surface_ranges":[{"surface_role":"surface","first_triangle":0,"triangle_count":4}],
                "forgecad_source_face_ids":[0,1,2,3]}
        }]}],
        "materials":[{
            "pbrMetallicRoughness":{"baseColorFactor":[1,1,1,1],"metallicFactor":1,"roughnessFactor":1,
                "baseColorTexture":{"index":0},"metallicRoughnessTexture":{"index":1}},
            "normalTexture":{"index":2},"occlusionTexture":{"index":3},"emissiveTexture":{"index":4},
            "emissiveFactor":[1,1,1],"extras":{"forgecad_visual_texture_set_id":format!("vtexset_primary_builtin_{texture_version}"),
                "forgecad_texture_material_id":"mat_primary","forgecad_visual_only":true}
        }],
        "images":images,"textures":textures,"buffers":[{"byteLength":binary.len()}],"bufferViews":views,
        "accessors":[
            {"bufferView":index_view,"componentType":5123,"count":12,"type":"SCALAR"},
            {"bufferView":position_view,"componentType":5126,"count":4,"type":"VEC3","min":[0,0,0],"max":[1,1,1]},
            {"bufferView":normal_view,"componentType":5126,"count":4,"type":"VEC3"},
            {"bufferView":tangent_view,"componentType":5126,"count":4,"type":"VEC4"},
            {"bufferView":uv_view,"componentType":5126,"count":4,"type":"VEC2"}
        ],
        "extras":{"forgecad_geometry_artifact_profile":profile,"forgecad_feature_history":[{
            "node_id":"op_shell","runtime_manifest_version":"ShapeProgramRuntimeManifest@1","result_sha256":"a".repeat(64)
        }]}
    });
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
    glb.extend_from_slice(&0x4e4f534a_u32.to_le_bytes());
    glb.extend_from_slice(&json_chunk);
    glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004e4942_u32.to_le_bytes());
    glb.extend_from_slice(&binary);
    glb
}
