use std::{collections::BTreeMap, path::PathBuf};

use forgecad_core::{
    resolve_semantic_proportions, semantic_sha256, verify_forgecad_glb, AgentAssetVersion,
    AssetStage, AssetVersionStatus, BlockoutCandidate, CandidateStatus, CoreRepository, Project,
    ProjectStatus, QualityReport, QualityStatus,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const PROJECT_ID: &str = "project_semantic_proportions";
const ASSET_ID: &str = "assetver_semantic_proportions_v1";
const ARTIFACT_ID: &str = "artifact_semantic_proportions";
const QUALITY_ID: &str = "quality_semantic_proportions_v1";
const PART_ID: &str = "part_primary_form";

struct Fixture {
    _root: TempDir,
    db_path: PathBuf,
    repository: CoreRepository,
    snapshot_before_read: forgecad_core::ActiveDesignSnapshot,
}

impl Fixture {
    fn production(with_bindings: bool, glb_part_role: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let db_path = root.path().join("library.db");
        let repository =
            CoreRepository::open(&db_path, root.path(), "semantic-proportions-test").unwrap();
        repository
            .ensure_default_domain_profile("2026-07-17T12:00:00Z")
            .unwrap();
        repository.create_project(&project()).unwrap();
        let version = version(with_bindings, false);
        let production = profile_glb("production_concept", glb_part_role, "zone_primary_form");
        let interactive = profile_glb("interactive_preview", glb_part_role, "zone_primary_form");
        let facts = verify_forgecad_glb(&production, Some("production_concept")).unwrap();
        let quality = quality(&version, &facts);
        repository
            .commit_candidate_bundle(
                candidate(&version, facts.glb_sha256.clone()),
                &production,
                &interactive,
                &version,
                &quality,
            )
            .unwrap();
        let snapshot_before_read = repository.snapshot(PROJECT_ID).unwrap().unwrap();
        Self {
            _root: root,
            db_path,
            repository,
            snapshot_before_read,
        }
    }

    fn external() -> Self {
        let root = tempfile::tempdir().unwrap();
        let db_path = root.path().join("library.db");
        let repository =
            CoreRepository::open(&db_path, root.path(), "semantic-external-test").unwrap();
        repository
            .ensure_default_domain_profile("2026-07-17T12:00:00Z")
            .unwrap();
        repository.create_project(&project()).unwrap();
        let version = version(true, true);
        let snapshot_before_read = repository.commit_initial_asset(&version).unwrap();
        Self {
            _root: root,
            db_path,
            repository,
            snapshot_before_read,
        }
    }
}

#[test]
fn eligible_active_part_uses_persisted_scale_and_exact_cas_readback_without_writes() {
    let fixture = Fixture::production(true, "primary_form");
    let version_hash_before = fixture
        .repository
        .version(ASSET_ID)
        .unwrap()
        .unwrap()
        .semantic_hash()
        .unwrap();

    let resolved = resolve_semantic_proportions(&fixture.repository, ASSET_ID, PART_ID).unwrap();

    assert_eq!(
        resolved.schema_version,
        "ResolvedSemanticProportionOptions@1"
    );
    assert_eq!(
        resolved.runtime_manifest_version,
        "ShapeProgramRuntimeManifest@1"
    );
    assert_eq!(resolved.options.len(), 3);
    assert_eq!(resolved.unavailable_message, None);
    assert_eq!(resolved.options[0].recipe_id, "proportion_prop_compact");
    assert_eq!(resolved.options[0].path, "transform.scale.x");
    assert_eq!(resolved.options[0].current_value, 1.0);
    assert_eq!(resolved.options[0].target_value, 0.9);
    assert_eq!(resolved.options[0].min, 0.6);
    assert_eq!(resolved.options[0].max, 1.4);
    assert_eq!(resolved.options[0].step, 0.1);
    assert_eq!(resolved.options[0].unit, "ratio");
    assert_eq!(resolved.options[0].source_operation_ids, ["op_shell"]);
    assert_eq!(resolved.options[0].style_token.visual_only, true);
    assert_eq!(resolved.options[0].style_token.allowed_domains.len(), 4);
    assert_eq!(
        resolved.shape_program_sha256,
        semantic_sha256(&version(true, false).shape_program).unwrap()
    );
    assert_eq!(
        fixture.repository.snapshot(PROJECT_ID).unwrap().unwrap(),
        fixture.snapshot_before_read
    );
    assert_eq!(
        fixture
            .repository
            .version(ASSET_ID)
            .unwrap()
            .unwrap()
            .semantic_hash()
            .unwrap(),
        version_hash_before
    );
}

#[test]
fn no_binding_and_unmatched_surface_provenance_are_explicitly_unavailable() {
    let no_binding = Fixture::production(false, "primary_form");
    let resolved = resolve_semantic_proportions(&no_binding.repository, ASSET_ID, PART_ID).unwrap();
    assert!(resolved.options.is_empty());
    assert!(resolved
        .unavailable_message
        .as_deref()
        .unwrap()
        .contains("受限比例参数"));

    let wrong_surface = Fixture::production(true, "secondary_form");
    let resolved =
        resolve_semantic_proportions(&wrong_surface.repository, ASSET_ID, PART_ID).unwrap();
    assert!(resolved.options.is_empty());
    assert!(resolved
        .unavailable_message
        .as_deref()
        .unwrap()
        .contains("稳定表面来源"));
}

#[test]
fn stale_asset_and_external_reference_fail_before_returning_controls() {
    let stale = Fixture::production(true, "primary_form");
    let connection = Connection::open(&stale.db_path).unwrap();
    connection
        .execute(
            "INSERT INTO agent_asset_versions(asset_version_id, project_id, parent_asset_version_id, version_no, status, summary, stage, plan_id, direction_id, domain_pack_id, artifact_id, parts_json, shape_program_json, assembly_graph_json, material_bindings_json, created_at) SELECT 'assetver_other', project_id, asset_version_id, 2, 'committed', summary, stage, plan_id, direction_id, domain_pack_id, 'artifact_other', parts_json, shape_program_json, assembly_graph_json, material_bindings_json, '2026-07-17T12:00:05Z' FROM agent_asset_versions WHERE asset_version_id=?",
            [ASSET_ID],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE agent_asset_heads SET asset_version_id='assetver_other' WHERE project_id=?",
            [PROJECT_ID],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE active_design_snapshots SET active_asset_version_id='assetver_other', export_source_version_id='assetver_other', quality_report_id=NULL, quality_asset_version_id=NULL WHERE project_id=?",
            [PROJECT_ID],
        )
        .unwrap();
    let error = resolve_semantic_proportions(&stale.repository, ASSET_ID, PART_ID).unwrap_err();
    assert_eq!(error.code(), "ACTIVE_DESIGN_STALE");

    let external = Fixture::external();
    let error = resolve_semantic_proportions(&external.repository, ASSET_ID, PART_ID).unwrap_err();
    assert_eq!(error.code(), "EXTERNAL_REFERENCE_NOT_EDITABLE");
    assert_eq!(
        external.repository.snapshot(PROJECT_ID).unwrap().unwrap(),
        external.snapshot_before_read
    );
}

#[test]
fn stale_q003_readback_fails_closed_instead_of_trusting_glb_appearance() {
    let fixture = Fixture::production(true, "primary_form");
    let connection = Connection::open(&fixture.db_path).unwrap();
    let encoded: String = connection
        .query_row(
            "SELECT report_json FROM agent_asset_quality_reports WHERE quality_report_id=?",
            [QUALITY_ID],
            |row| row.get(0),
        )
        .unwrap();
    let mut report: Value = serde_json::from_str(&encoded).unwrap();
    report["compile_readback"]["glb_sha256"] = Value::String("0".repeat(64));
    connection
        .execute(
            "UPDATE agent_asset_quality_reports SET report_json=? WHERE quality_report_id=?",
            params![serde_json::to_string(&report).unwrap(), QUALITY_ID],
        )
        .unwrap();

    let error = resolve_semantic_proportions(&fixture.repository, ASSET_ID, PART_ID).unwrap_err();
    assert_eq!(error.code(), "GEOMETRY_READBACK_FAILED");
    assert_eq!(
        fixture.repository.snapshot(PROJECT_ID).unwrap().unwrap(),
        fixture.snapshot_before_read
    );
}

fn project() -> Project {
    Project {
        project_id: PROJECT_ID.into(),
        profile_id: "profile_weapon_concept_v1".into(),
        domain_type: "weapon_concept".into(),
        name: "Semantic proportions".into(),
        status: ProjectStatus::Active,
        current_version_id: None,
        created_at: "2026-07-17T12:00:01Z".into(),
        updated_at: "2026-07-17T12:00:01Z".into(),
    }
}

fn version(with_bindings: bool, external: bool) -> AgentAssetVersion {
    let bindings = with_bindings.then_some(json!([
        binding("x", "长度比例"),
        binding("y", "高度比例"),
        binding("z", "宽度比例")
    ]));
    let mut part = json!({
        "part_id":PART_ID,
        "role":"primary_form",
        "material_zone_ids":["zone_primary_form"],
        "editable_parameters":[],
        "editable_parameter_bindings":[],
        "locked":false,
        "provenance":"agent_generated"
    });
    if let Some(bindings) = bindings {
        part["editable_parameters"] = json!([
            "transform.scale.x",
            "transform.scale.y",
            "transform.scale.z"
        ]);
        part["editable_parameter_bindings"] = bindings;
    }
    let shape_program = if external {
        json!({
            "schema_version":"ExternalGLBReference@1",
            "reference_id":"reference_external_semantic"
        })
    } else {
        shape_program()
    };
    AgentAssetVersion {
        asset_version_id: ASSET_ID.into(),
        project_id: PROJECT_ID.into(),
        parent_asset_version_id: None,
        version_no: 1,
        status: AssetVersionStatus::Committed,
        summary: "Production semantic proportion fixture".into(),
        stage: AssetStage::EditableAsset,
        plan_id: "plan_semantic_proportions".into(),
        direction_id: "direction_best".into(),
        domain_pack_id: "pack_future_weapon_prop".into(),
        artifact_id: ARTIFACT_ID.into(),
        parts: vec![part],
        shape_program,
        assembly_graph: assembly_graph(),
        material_bindings: BTreeMap::new(),
        created_at: "2026-07-17T12:00:03Z".into(),
    }
}

fn binding(axis: &str, display_name: &str) -> Value {
    json!({
        "schema_version":"EditableParameterBinding@1",
        "parameter_id":format!("editparam_semantic_scale_{axis}"),
        "path":format!("transform.scale.{axis}"),
        "display_name":display_name,
        "unit":"ratio",
        "default":1.0,
        "min":0.6,
        "max":1.4,
        "step":0.1
    })
}

fn shape_program() -> Value {
    json!({
        "schema_version":"ShapeProgram@1",
        "program_id":"shape_semantic_proportions",
        "operations":[{
            "operation_id":"op_shell",
            "op":"box",
            "args":{
                "part_role":"primary_form",
                "zone_id":"zone_primary_form",
                "size":[1.0,1.0,1.0]
            }
        }],
        "outputs":[{
            "output_id":"output_shell",
            "operation_id":"op_shell",
            "part_role":"primary_form"
        }]
    })
}

fn assembly_graph() -> Value {
    json!({
        "schema_version":"AssemblyGraph@1",
        "graph_id":"graph_semantic_proportions",
        "root_part_id":PART_ID,
        "parts":[{
            "part_id":PART_ID,
            "role":"primary_form",
            "parent_part_id":null,
            "operation_id":"op_shell",
            "transform":{
                "position":[0.0,0.0,0.0],
                "rotation":[0.0,0.0,0.0],
                "scale":[1.0,1.0,1.0]
            },
            "material_zones":["zone_primary_form"],
            "material_zone_ids":["zone_primary_form"],
            "locked":false
        }]
    })
}

fn candidate(version: &AgentAssetVersion, glb_sha256: String) -> BlockoutCandidate {
    BlockoutCandidate {
        artifact_id: ARTIFACT_ID.into(),
        project_id: Some(PROJECT_ID.into()),
        plan_id: version.plan_id.clone(),
        direction_id: version.direction_id.clone(),
        domain_pack_id: version.domain_pack_id.clone(),
        status: CandidateStatus::Candidate,
        candidate: json!({"selection":"internal_best","visual_only":true}),
        shape_program: version.shape_program.clone(),
        assembly_graph: version.assembly_graph.clone(),
        material_bindings: version.material_bindings.clone(),
        glb_sha256,
        created_at: "2026-07-17T12:00:02Z".into(),
        updated_at: "2026-07-17T12:00:02Z".into(),
    }
}

fn quality(
    version: &AgentAssetVersion,
    facts: &forgecad_core::ForgeCadGlbReadback,
) -> QualityReport {
    QualityReport {
        quality_report_id: QUALITY_ID.into(),
        project_id: PROJECT_ID.into(),
        asset_version_id: ASSET_ID.into(),
        report: json!({
            "schema_version":"AgentAssetQualityReport@1",
            "quality_report_id":QUALITY_ID,
            "asset_version_id":ASSET_ID,
            "status":"passed",
            "evidence_source":"geometry_compile_readback",
            "triangle_count":facts.triangle_count,
            "bounds_mm":facts.bounds_mm,
            "compile_readback":{
                "schema_version":"GeometryCompileReadback@2",
                "runtime_manifest_version":facts.runtime_manifest_version,
                "artifact_profile":facts.artifact_profile,
                "shape_program_sha256":semantic_sha256(&version.shape_program).unwrap(),
                "glb_sha256":facts.glb_sha256,
                "glb_byte_size":facts.glb_byte_size,
                "triangle_count":facts.triangle_count,
                "bounds_mm":facts.bounds_mm,
                "mesh_count":facts.mesh_count,
                "primitive_count":facts.primitive_count,
                "material_count":facts.material_count,
                "uv0_primitive_count":facts.uv0_primitive_count,
                "normal_primitive_count":facts.normal_primitive_count,
                "tangent_primitive_count":facts.tangent_primitive_count,
                "closed_manifold":facts.closed_manifold,
                "surface_provenance_present":facts.surface_provenance_present,
                "visual_texture_set_count":facts.visual_texture_set_count,
                "visual_texture_map_count":facts.visual_texture_map_count
            }
        }),
        status: QualityStatus::Passed,
        created_at: "2026-07-17T12:00:04Z".into(),
    }
}

fn profile_glb(profile_id: &str, part_role: &str, zone_id: &str) -> Vec<u8> {
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
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>(),
        Some(34963),
    );
    let position_view = append_view(
        &positions
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>(),
        Some(34962),
    );
    let normal_view = append_view(
        &normals
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>(),
        Some(34962),
    );
    let tangent_view = append_view(
        &tangents
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>(),
        Some(34962),
    );
    let uv_view = append_view(
        &uvs.iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>(),
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
            "name":format!("vtex_semantic_{role}_{texture_version}"),
            "bufferView":view,
            "mimeType":"image/png",
            "extras":{"forgecad_visual_texture":{
                "texture_id":format!("vtex_semantic_{role}_{texture_version}"),
                "texture_role":role,
                "mime_type":"image/png",
                "byte_size":png.len(),
                "sha256":format!("{:x}", Sha256::digest(&png)),
                "color_space":if matches!(role, "base_color" | "emissive") { "srgb" } else { "linear" },
                "width":dimension,
                "height":dimension,
                "source":"forgecad_builtin",
                "license":"not_applicable",
                "fallback":"none",
                "visual_only":true
            }}
        }));
        textures.push(json!({
            "name":format!("vtex_semantic_{role}_{texture_version}"),
            "source":index
        }));
    }
    drop(append_view);
    let document = json!({
        "asset":{"version":"2.0","generator":"ForgeCAD semantic-proportions test"},
        "scene":0,
        "scenes":[{"nodes":[0]}],
        "nodes":[{"mesh":0}],
        "meshes":[{"primitives":[{
            "attributes":{"POSITION":1,"NORMAL":2,"TANGENT":3,"TEXCOORD_0":4},
            "indices":0,
            "material":0,
            "mode":4,
            "extras":{
                "forgecad_part_role":part_role,
                "forgecad_feature_node_id":"op_shell",
                "forgecad_material_zone_id":zone_id,
                "forgecad_surface_ranges":[{"surface_role":"surface","first_triangle":0,"triangle_count":4}],
                "forgecad_source_face_ids":[0,1,2,3]
            }
        }]}],
        "materials":[{
            "pbrMetallicRoughness":{
                "baseColorFactor":[1,1,1,1],
                "metallicFactor":1,
                "roughnessFactor":1,
                "baseColorTexture":{"index":0},
                "metallicRoughnessTexture":{"index":1}
            },
            "normalTexture":{"index":2},
            "occlusionTexture":{"index":3},
            "emissiveTexture":{"index":4},
            "emissiveFactor":[1,1,1],
            "extras":{
                "forgecad_visual_texture_set_id":format!("vtexset_primary_builtin_{texture_version}"),
                "forgecad_texture_material_id":"mat_primary",
                "forgecad_visual_only":true
            }
        }],
        "images":images,
        "textures":textures,
        "buffers":[{"byteLength":binary.len()}],
        "bufferViews":views,
        "accessors":[
            {"bufferView":index_view,"componentType":5123,"count":12,"type":"SCALAR"},
            {"bufferView":position_view,"componentType":5126,"count":4,"type":"VEC3","min":[0,0,0],"max":[1,1,1]},
            {"bufferView":normal_view,"componentType":5126,"count":4,"type":"VEC3"},
            {"bufferView":tangent_view,"componentType":5126,"count":4,"type":"VEC4"},
            {"bufferView":uv_view,"componentType":5126,"count":4,"type":"VEC2"}
        ],
        "extras":{
            "forgecad_geometry_artifact_profile":profile,
            "forgecad_feature_history":[{
                "node_id":"op_shell",
                "runtime_manifest_version":"ShapeProgramRuntimeManifest@1",
                "result_sha256":"a".repeat(64)
            }]
        }
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
    glb.extend_from_slice(&0x4e4f_534a_u32.to_le_bytes());
    glb.extend_from_slice(&json_chunk);
    glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004e_4942_u32.to_le_bytes());
    glb.extend_from_slice(&binary);
    glb
}
