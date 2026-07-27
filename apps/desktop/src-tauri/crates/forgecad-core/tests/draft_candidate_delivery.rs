use std::{collections::BTreeMap, path::PathBuf};

use forgecad_core::{
    semantic_sha256, verify_forgecad_glb, AgentAssetVersion, AssetStage, AssetVersionStatus,
    CoreRepository, DraftArtifactReference, DraftCandidate, DraftCandidateStatus, ObjectReference,
    Project, ProjectStatus, QualityReport, QualityStatus,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const PROJECT_ID: &str = "project_draft_delivery";

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
            .ensure_default_domain_profile("2026-07-27T08:00:00Z")
            .unwrap();
        repository
            .create_project(&Project {
                project_id: PROJECT_ID.into(),
                profile_id: "profile_weapon_concept_v1".into(),
                domain_type: "weapon_concept".into(),
                name: "Draft delivery boundary".into(),
                status: ProjectStatus::Active,
                current_version_id: None,
                created_at: "2026-07-27T08:00:01Z".into(),
                updated_at: "2026-07-27T08:00:01Z".into(),
            })
            .unwrap();
        Self {
            root,
            db,
            repository,
        }
    }
}

fn glb_container(label: &str) -> Vec<u8> {
    let mut json_chunk = serde_json::to_vec(&json!({
        "asset": {"version": "2.0"},
        "extras": {"creative_label": label},
    }))
    .unwrap();
    while json_chunk.len() % 4 != 0 {
        json_chunk.push(b' ');
    }
    let total = 12 + 8 + json_chunk.len();
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(b"glTF");
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&(total as u32).to_le_bytes());
    bytes.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&0x4e4f534a_u32.to_le_bytes());
    bytes.extend_from_slice(&json_chunk);
    bytes
}

fn draft(base: Option<&str>) -> DraftCandidate {
    DraftCandidate {
        schema_version: "DraftCandidate@1".into(),
        candidate_id: "draft_delivery_1".into(),
        project_id: PROJECT_ID.into(),
        base_asset_version_id: base.map(str::to_string),
        summary: "Creative draft only".into(),
        plan_id: "plan_draft_delivery".into(),
        direction_id: "direction_auto".into(),
        domain_pack_id: "pack_weapon_concept_v1".into(),
        artifact_id: "artifact_draft_delivery".into(),
        parts: vec![json!({"part_id": "part_shell", "role": "core_shell"})],
        shape_program: json!({
            "schema_version": "ShapeProgram@1",
            "program_id": "shape_draft_delivery",
        }),
        assembly_graph: json!({
            "schema_version": "AssemblyGraph@1",
            "graph_id": "graph_draft_delivery",
            "parts": [{"part_id": "part_shell", "material_zone_ids": ["zone_shell"]}],
        }),
        material_bindings: BTreeMap::new(),
        interactive_preview: DraftArtifactReference {
            sha256: "0".repeat(64),
            byte_size: 1,
            extension: "glb".into(),
        },
        idempotency_key: "draft_delivery_idempotency".into(),
        request_hash: "1".repeat(64),
        status: DraftCandidateStatus::Draft,
        confirmed_asset_version_id: None,
        quality_report_id: None,
        failure_code: None,
        created_at: "2026-07-27T08:00:02Z".into(),
        updated_at: "2026-07-27T08:00:02Z".into(),
    }
}

fn base_version() -> AgentAssetVersion {
    AgentAssetVersion {
        asset_version_id: "asset_delivery_base".into(),
        project_id: PROJECT_ID.into(),
        parent_asset_version_id: None,
        version_no: 1,
        status: AssetVersionStatus::Committed,
        summary: "Confirmed base".into(),
        stage: AssetStage::EditableAsset,
        plan_id: "plan_draft_delivery".into(),
        direction_id: "direction_auto".into(),
        domain_pack_id: "pack_weapon_concept_v1".into(),
        artifact_id: "artifact_delivery_base".into(),
        parts: draft(None).parts,
        shape_program: draft(None).shape_program,
        assembly_graph: draft(None).assembly_graph,
        material_bindings: BTreeMap::new(),
        created_at: "2026-07-27T08:00:03Z".into(),
    }
}

fn strict_glb(profile_id: &str) -> Vec<u8> {
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
        "asset":{"version":"2.0","generator":"ForgeCAD draft delivery contract test"},
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

fn strict_quality(glb: &[u8], version: &AgentAssetVersion) -> QualityReport {
    let facts = verify_forgecad_glb(glb, Some("production_concept")).unwrap();
    QualityReport {
        quality_report_id: "quality_draft_delivery_success".into(),
        project_id: version.project_id.clone(),
        asset_version_id: version.asset_version_id.clone(),
        report: json!({
            "schema_version":"AgentAssetQualityReport@1",
            "quality_report_id":"quality_draft_delivery_success",
            "asset_version_id":version.asset_version_id,
            "status":"passed",
            "evidence_source":"geometry_compile_readback",
            "triangle_count":facts.triangle_count,
            "bounds_mm":facts.bounds_mm,
            "compile_readback":{
                "schema_version":"GeometryCompileReadback@2",
                "runtime_manifest_version":facts.runtime_manifest_version,
                "artifact_profile":{"artifact_profile_id":"production_concept","profile_sha256":facts.artifact_profile_sha256},
                "shape_program_sha256":semantic_sha256(&version.shape_program).unwrap(),
                "glb_sha256":facts.glb_sha256,
                "glb_byte_size":facts.glb_byte_size,
                "triangle_count":facts.triangle_count,
                "bounds_mm":facts.bounds_mm,
                "mesh_count":facts.mesh_count,
                "primitive_count":facts.primitive_count,
                "material_count":facts.material_count,
                "closed_manifold":facts.closed_manifold,
                "surface_provenance_present":facts.surface_provenance_present
            }
        }),
        status: QualityStatus::Passed,
        created_at: "2026-07-27T08:00:06Z".into(),
    }
}

#[test]
fn draft_preview_is_restartable_and_has_zero_permanent_version_or_snapshot_side_effects() {
    let fixture = Fixture::new("draft_delivery_preview");
    let interactive = glb_container("creative-inner-loop");
    let draft = fixture
        .repository
        .stage_draft_candidate(draft(None), &interactive)
        .unwrap();

    assert_eq!(draft.draft.status, DraftCandidateStatus::Draft);
    assert!(fixture.repository.snapshot(PROJECT_ID).unwrap().is_none());
    let connection = Connection::open(&fixture.db).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM agent_asset_versions", [], |row| row
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        0
    );
    drop(connection);
    drop(fixture.repository);

    let restarted = CoreRepository::open(
        &fixture.db,
        fixture.root.path(),
        "draft_delivery_preview_restart",
    )
    .unwrap();
    let readback = restarted
        .read_draft_candidate_bundle(&draft.draft.candidate_id)
        .unwrap()
        .unwrap();
    assert_eq!(readback.draft, draft.draft);
    assert_eq!(
        restarted
            .read_object(&readback.interactive_preview_glb.sha256)
            .unwrap(),
        interactive
    );

    let cancelled = restarted
        .cancel_draft_candidate(&draft.draft.candidate_id, "2026-07-27T08:00:04Z")
        .unwrap();
    assert_eq!(cancelled.status, DraftCandidateStatus::Cancelled);
    assert!(restarted
        .read_draft_candidate_bundle(&draft.draft.candidate_id)
        .unwrap()
        .is_none());
    assert!(restarted.snapshot(PROJECT_ID).unwrap().is_none());
}

#[test]
fn confirmation_atomically_creates_strict_asset_snapshot_and_is_idempotent() {
    let fixture = Fixture::new("draft_delivery_confirm_success");
    let interactive = strict_glb("interactive_preview");
    let production = strict_glb("production_concept");
    fixture
        .repository
        .stage_draft_candidate(draft(None), &interactive)
        .unwrap();
    let mut resulting = base_version();
    resulting.asset_version_id = "asset_delivery_confirmed".into();
    resulting.artifact_id = "artifact_draft_delivery".into();
    resulting.summary = "Confirmed delivery".into();
    resulting.created_at = "2026-07-27T08:00:05Z".into();
    let quality = strict_quality(&production, &resulting);

    let confirmed = fixture
        .repository
        .confirm_draft_candidate(
            "draft_delivery_1",
            &resulting,
            &production,
            &quality,
            None,
            "2026-07-27T08:00:07Z",
        )
        .unwrap();
    assert_eq!(confirmed.version, resulting);
    assert_eq!(
        confirmed.production_glb.sha256,
        verify_forgecad_glb(&production, Some("production_concept"))
            .unwrap()
            .glb_sha256
    );
    assert!(confirmed.snapshot.quality.is_some());
    assert_eq!(
        fixture.repository.head(PROJECT_ID).unwrap().as_deref(),
        Some("asset_delivery_confirmed")
    );
    assert!(fixture
        .repository
        .read_draft_candidate_bundle("draft_delivery_1")
        .unwrap()
        .is_none());

    let replay = fixture
        .repository
        .confirm_draft_candidate(
            "draft_delivery_1",
            &resulting,
            &production,
            &quality,
            None,
            "2026-07-27T08:00:08Z",
        )
        .unwrap();
    assert_eq!(replay, confirmed);
    let connection = Connection::open(&fixture.db).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM agent_asset_versions", [], |row| row
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        1
    );
}

#[test]
fn confirmation_starts_strict_readback_and_rejects_without_snapshot_or_version_write() {
    let fixture = Fixture::new("draft_delivery_strict");
    let interactive = glb_container("creative-inner-loop");
    fixture
        .repository
        .stage_draft_candidate(draft(None), &interactive)
        .unwrap();
    let resulting = AgentAssetVersion {
        asset_version_id: "asset_delivery_confirmed".into(),
        project_id: PROJECT_ID.into(),
        parent_asset_version_id: None,
        version_no: 1,
        status: AssetVersionStatus::Committed,
        summary: "Confirmed delivery".into(),
        stage: AssetStage::EditableAsset,
        plan_id: "plan_draft_delivery".into(),
        direction_id: "direction_auto".into(),
        domain_pack_id: "pack_weapon_concept_v1".into(),
        artifact_id: "artifact_draft_delivery".into(),
        parts: draft(None).parts,
        shape_program: draft(None).shape_program,
        assembly_graph: draft(None).assembly_graph,
        material_bindings: BTreeMap::new(),
        created_at: "2026-07-27T08:00:05Z".into(),
    };
    let quality = QualityReport {
        quality_report_id: "quality_delivery_confirmed".into(),
        project_id: PROJECT_ID.into(),
        asset_version_id: resulting.asset_version_id.clone(),
        report: json!({}),
        status: QualityStatus::Unavailable,
        created_at: "2026-07-27T08:00:05Z".into(),
    };
    let error = fixture
        .repository
        .confirm_draft_candidate(
            "draft_delivery_1",
            &resulting,
            &glb_container("not-production-pbr"),
            &quality,
            None,
            "2026-07-27T08:00:06Z",
        )
        .unwrap_err();
    assert!(matches!(
        error.code(),
        "QUALITY_STATUS_INVALID" | "FORGECAD_GLB_INVALID"
    ));
    assert!(fixture.repository.snapshot(PROJECT_ID).unwrap().is_none());
    let connection = Connection::open(&fixture.db).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM agent_asset_versions", [], |row| row
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        0
    );
}

#[test]
fn failed_draft_keeps_terminal_diagnostic_without_product_state() {
    let fixture = Fixture::new("draft_delivery_failed");
    fixture
        .repository
        .stage_draft_candidate(draft(None), &glb_container("creative-inner-loop"))
        .unwrap();
    let failed = fixture
        .repository
        .fail_draft_candidate(
            "draft_delivery_1",
            "STRICT_PRODUCTION_READBACK_FAILED",
            "2026-07-27T08:00:06Z",
        )
        .unwrap();
    assert_eq!(failed.status, DraftCandidateStatus::Failed);
    assert_eq!(
        failed.failure_code.as_deref(),
        Some("STRICT_PRODUCTION_READBACK_FAILED")
    );
    assert!(fixture
        .repository
        .read_draft_candidate_bundle("draft_delivery_1")
        .unwrap()
        .is_none());
    assert!(fixture.repository.snapshot(PROJECT_ID).unwrap().is_none());
    let connection = Connection::open(&fixture.db).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM agent_asset_versions", [], |row| row
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        0
    );
}

#[test]
fn strict_confirmed_export_rejects_bytes_that_do_not_match_confirmed_production_object() {
    let fixture = Fixture::new("draft_delivery_export");
    let version = base_version();
    fixture.repository.commit_initial_asset(&version).unwrap();
    let production = glb_container("confirmed-production");
    // This compatibility setup only establishes an existing Rust production
    // object; the strict export assertion below is independent of GLB quality.
    let object = fixture
        .repository
        .attach_object_bytes(
            &ObjectReference {
                reference_kind: "asset_version".into(),
                owner_id: version.asset_version_id.clone(),
                role: "production_glb".into(),
            },
            &production,
            "glb",
            "2026-07-27T08:00:06Z",
        )
        .unwrap();
    let snapshot = fixture.repository.snapshot(PROJECT_ID).unwrap().unwrap();
    let error = fixture
        .repository
        .attach_confirmed_export_bytes(
            PROJECT_ID,
            snapshot.etag(),
            "production_glb",
            b"different-bytes",
            "glb",
            "2026-07-27T08:00:07Z",
        )
        .unwrap_err();
    assert_eq!(error.code(), "CONFIRMED_EXPORT_HASH_MISMATCH");
    assert_eq!(object.byte_size, production.len() as u64);

    let (exported, _) = fixture
        .repository
        .attach_confirmed_export_bytes(
            PROJECT_ID,
            snapshot.etag(),
            "production_glb",
            &production,
            "glb",
            "2026-07-27T08:00:08Z",
        )
        .unwrap();
    assert_eq!(exported.sha256, object.sha256);
}
