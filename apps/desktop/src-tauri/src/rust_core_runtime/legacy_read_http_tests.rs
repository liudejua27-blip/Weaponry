use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use base64::Engine as _;
use forgecad_app_server::compatibility::LocalAgentEndpoint;
use forgecad_app_server_protocol::ProtocolHttpBody;
use forgecad_core::{semantic_sha256, Project, ProjectStatus};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::*;

static NEXT_ROOT: AtomicU64 = AtomicU64::new(1);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let serial = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "forgecad-k003-legacy-http-{}-{serial}",
            std::process::id()
        ));
        if path.exists() {
            fs::remove_dir_all(&path).unwrap();
        }
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn legacy_read_only_http_adapters_are_rust_owned_bounded_and_zero_write() {
    let root = TestRoot::new();
    let runtime = RustCoreRuntime::open(root.path(), "legacy-http-first").unwrap();
    let fixture = seed_fixture(&runtime);
    let semantic_before = runtime
        .repository()
        .legacy_read_only_hash(&fixture.project_id)
        .unwrap()
        .unwrap();
    let sentinel = Connection::open(runtime.repository().db_path()).unwrap();
    let data_version_before: i64 = sentinel
        .query_row("PRAGMA data_version", [], |row| row.get(0))
        .unwrap();

    let project = handled(
        &runtime,
        &get(format!("/api/v1/projects/{}", fixture.project_id)),
    );
    assert_eq!(project.status, 200);
    let project_json = response_json(&project);
    assert_eq!(project_json["current_spec"], fixture.spec);
    assert_eq!(
        project_json["versions"][0]["version_id"],
        fixture.version_id
    );
    assert_no_storage_leak(&project_json, root.path());

    let version = handled(
        &runtime,
        &get(format!("/api/v1/versions/{}", fixture.version_id)),
    );
    assert_eq!(version.status, 200);
    let version_json = response_json(&version);
    assert_eq!(version_json["spec"], fixture.spec);
    assert_no_storage_leak(&version_json, root.path());

    let graph = handled(
        &runtime,
        &get(format!("/api/v1/module-graphs/{}", fixture.graph_id)),
    );
    assert_eq!(graph.status, 200);
    let graph_json = response_json(&graph);
    assert_eq!(graph_json["graph"], fixture.graph);
    assert_no_storage_leak(&graph_json, root.path());

    let catalog = handled(
        &runtime,
        &get("/api/v1/module-assets?pack_id=pack_weapon_concept_v1&limit=1"),
    );
    assert_eq!(catalog.status, 200);
    assert_eq!(header(&catalog, "Cache-Control"), Some("no-store"));
    let catalog_json = response_json(&catalog);
    assert_eq!(catalog_json["items"].as_array().unwrap().len(), 1);
    let first_module_id = catalog_json["items"][0]["manifest"]["module_id"]
        .as_str()
        .unwrap();
    let next_cursor = catalog_json["next_cursor"].as_str().unwrap();
    assert_eq!(first_module_id, next_cursor);
    assert_no_storage_leak(&catalog_json, root.path());
    let second_page = handled(
        &runtime,
        &get(format!(
            "/api/v1/module-assets?pack_id=pack_weapon_concept_v1&limit=1&cursor={next_cursor}"
        )),
    );
    let second_page_json = response_json(&second_page);
    assert_eq!(second_page_json["items"].as_array().unwrap().len(), 1);
    assert!(second_page_json["next_cursor"].is_null());
    assert_no_storage_leak(&second_page_json, root.path());

    let active = handled(
        &runtime,
        &get(format!(
            "/api/v1/projects/{}/active-design",
            fixture.project_id
        )),
    );
    assert_eq!(active.status, 200);
    assert_eq!(header(&active, "ETag"), Some("W/\"active-design-1\""));
    let active_json = response_json(&active);
    assert_eq!(
        active_json["active_design"]["source"],
        "legacy_concept_read_only"
    );
    assert_eq!(
        active_json["active_design"]["legacy_version_id"],
        fixture.version_id
    );
    assert!(active_json["active_design"]
        .get("asset_version_id")
        .is_none());

    let module = handled(
        &runtime,
        &get(format!("/api/v1/module-assets/{first_module_id}/file")),
    );
    assert_eq!(module.status, 200);
    assert_eq!(header(&module, "Content-Type"), Some("model/gltf-binary"));
    assert_eq!(
        header(&module, "X-ForgeCAD-Object-SHA256"),
        Some(fixture.glb_sha256.as_str())
    );
    let ProtocolHttpBody::Base64 { data } = &module.body else {
        panic!("legacy module GLB must use the binary protocol body");
    };
    assert_eq!(BASE64_STANDARD.decode(data).unwrap(), fixture.glb);
    assert!(module.headers.iter().all(|(name, value)| {
        !name.eq_ignore_ascii_case("X-ForgeCAD-Object-Path")
            && !value.contains(root.path().to_string_lossy().as_ref())
    }));

    for route in [
        "/api/v1/module-assets".to_string(),
        "/api/v1/module-assets?pack_id=pack_weapon_concept_v1&limit=101".to_string(),
        "/api/v1/module-assets?pack_id=pack_weapon_concept_v1&unexpected=true".to_string(),
    ] {
        let invalid = handled(&runtime, &get(route));
        assert_eq!(invalid.status, 400);
    }

    for route in [
        format!("/api/v1/projects/{}/variants", fixture.project_id),
        format!("/api/v1/projects/{}/change-sets", fixture.project_id),
        format!(
            "/api/v1/projects/{}/change-set-audit-exports",
            fixture.project_id
        ),
        format!("/api/v1/module-assets/{}/thumbnail", fixture.module_id),
        format!("/api/v1/versions/{}/exports", fixture.version_id),
    ] {
        let retired = handled(&runtime, &get(route));
        assert_eq!(retired.status, 410);
        assert_eq!(
            response_json(&retired)["error"]["code"],
            "LEGACY_CONCEPT_ROUTE_RETIRED"
        );
    }

    assert_eq!(
        sentinel
            .query_row(
                "SELECT COUNT(*) FROM active_design_snapshots WHERE project_id=?",
                [&fixture.project_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0,
        "legacy Snapshot recovery through GET must remain a pure read"
    );
    let data_version_after: i64 = sentinel
        .query_row("PRAGMA data_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(data_version_after, data_version_before);
    assert_eq!(
        runtime
            .repository()
            .legacy_read_only_hash(&fixture.project_id)
            .unwrap()
            .unwrap(),
        semantic_before
    );
    let project_before_restart = project_json;
    let active_before_restart = active_json;
    drop(sentinel);
    drop(runtime);

    let restarted = RustCoreRuntime::open(root.path(), "legacy-http-restart").unwrap();
    let project_after_restart = handled(
        &restarted,
        &get(format!("/api/v1/projects/{}", fixture.project_id)),
    );
    assert_eq!(
        response_json(&project_after_restart),
        project_before_restart
    );
    let active_after_restart = handled(
        &restarted,
        &get(format!(
            "/api/v1/projects/{}/active-design",
            fixture.project_id
        )),
    );
    assert_eq!(response_json(&active_after_restart), active_before_restart);
    assert_eq!(
        restarted
            .repository()
            .legacy_read_only_hash(&fixture.project_id)
            .unwrap()
            .unwrap(),
        semantic_before
    );
    let connection = Connection::open(restarted.repository().db_path()).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM active_design_snapshots WHERE project_id=?",
                [&fixture.project_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
}

fn get(path: impl Into<String>) -> PreparedCompatHttpRequest {
    PreparedCompatHttpRequest {
        endpoint: LocalAgentEndpoint::parse("http://127.0.0.1:8000").unwrap(),
        method: AllowedHttpMethod::Get,
        path: path.into(),
        headers: Vec::new(),
        body: ProtocolHttpBody::Empty,
    }
}

fn handled(runtime: &RustCoreRuntime, request: &PreparedCompatHttpRequest) -> CompatHttpResponse {
    runtime
        .handle_compat_http(request)
        .expect("legacy route must never fall back to Python")
        .unwrap()
}

fn response_json(response: &CompatHttpResponse) -> Value {
    let ProtocolHttpBody::Utf8 { data } = &response.body else {
        panic!("expected UTF-8 JSON response");
    };
    serde_json::from_str(data).unwrap()
}

fn header<'a>(response: &'a CompatHttpResponse, wanted: &str) -> Option<&'a str> {
    response
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
        .map(|(_, value)| value.as_str())
}

fn assert_no_storage_leak(value: &Value, root: &Path) {
    let encoded = serde_json::to_string(value).unwrap();
    assert!(!encoded.contains("object_path"));
    assert!(!encoded.contains("logical_path"));
    assert!(!encoded.contains(root.to_string_lossy().as_ref()));
    assert!(!encoded.contains("glb_base64"));
}

struct Fixture {
    project_id: String,
    version_id: String,
    graph_id: String,
    module_id: String,
    spec: Value,
    graph: Value,
    glb: Vec<u8>,
    glb_sha256: String,
}

fn seed_fixture(runtime: &RustCoreRuntime) -> Fixture {
    let project_id = "prj_legacy_http".to_string();
    let version_id = "ver_legacy_http_v1".to_string();
    let graph_id = "mg_legacy_http_v1".to_string();
    let module_id = "module_legacy_http_shell".to_string();
    runtime
        .repository()
        .create_project(&Project {
            project_id: project_id.clone(),
            profile_id: "profile_weapon_concept_v1".into(),
            domain_type: "weapon_concept".into(),
            name: "Legacy HTTP concept".into(),
            status: ProjectStatus::Active,
            current_version_id: None,
            created_at: "2026-07-17T00:00:00Z".into(),
            updated_at: "2026-07-17T00:00:01Z".into(),
        })
        .unwrap();
    let spec = json!({
        "schema_version": "WeaponConceptSpec@1",
        "project_id": project_id,
        "profile_id": "profile_weapon_concept_v1",
        "name": "Legacy HTTP concept",
        "archetype": "future_modular_sidearm",
        "intended_uses": ["game_asset", "film_prop", "non_functional_display"],
        "style": {"keywords": ["future", "non-functional"], "palette": ["graphite"], "detail_density": 0.72},
        "proportions": {"overall_length_mm": 230.0, "body_height_mm": 54.0, "grip_angle_deg": 15.0},
        "required_slots": ["core"],
        "optional_slots": [],
        "constraints": {"symmetry": "mostly_symmetric", "max_triangle_count": 180000},
        "assumptions": ["Fictional non-functional visual concept only"]
    });
    let graph = json!({
        "schema_version": "ModuleGraph@1",
        "graph_id": graph_id,
        "project_id": project_id,
        "root_node_id": "node_legacy_http_shell",
        "nodes": [{
            "node_id": "node_legacy_http_shell",
            "module_id": module_id,
            "transform": {"position": [0.0, 0.0, 0.0], "rotation": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0]},
            "mirror_axis": "none",
            "locked": false,
            "visible": true
        }],
        "edges": []
    });
    let spec_sha256 = semantic_sha256(&spec).unwrap();
    let graph_sha256 = semantic_sha256(&graph).unwrap();
    let glb = minimal_glb();
    let glb_sha256 = format!("{:x}", Sha256::digest(&glb));
    let object_path = format!(
        "objects/sha256/{}/{}/{}.glb",
        &glb_sha256[..2],
        &glb_sha256[2..4],
        glb_sha256
    );
    let full_path = runtime.library_root().join(&object_path);
    fs::create_dir_all(full_path.parent().unwrap()).unwrap();
    fs::write(&full_path, &glb).unwrap();
    let manifest = json!({
        "schema_version": "ModuleAssetManifest@1",
        "module_id": module_id,
        "pack_id": "pack_weapon_concept_v1",
        "category": "core_shell",
        "asset_id": "asset_legacy_http_glb",
        "sha256": glb_sha256,
        "bounds_mm": [230.0, 54.0, 42.0],
        "triangle_count": 12,
        "material_slots": ["body"],
        "connectors": []
    });
    let manifest_sha256 = semantic_sha256(&manifest).unwrap();
    let auxiliary_module_id = "module_legacy_http_aux";
    let auxiliary_manifest = json!({
        "schema_version": "ModuleAssetManifest@1",
        "module_id": auxiliary_module_id,
        "pack_id": "pack_weapon_concept_v1",
        "category": "side_accessory",
        "asset_id": "asset_legacy_http_glb",
        "sha256": glb_sha256,
        "bounds_mm": [30.0, 12.0, 8.0],
        "triangle_count": 12,
        "material_slots": ["body"],
        "connectors": []
    });
    let auxiliary_manifest_sha256 = semantic_sha256(&auxiliary_manifest).unwrap();
    let connection = Connection::open(runtime.repository().db_path()).unwrap();
    connection
        .execute(
            "INSERT INTO project_versions(version_id, project_id, parent_version_id, version_no, status, summary, spec_schema_version, spec_json, spec_sha256, module_graph_id, change_set_id, created_at) VALUES (?, ?, NULL, 1, 'committed', 'Legacy immutable HTTP concept', 'WeaponConceptSpec@1', ?, ?, ?, NULL, '2026-07-17T00:00:02Z')",
            params![version_id, project_id, spec.to_string(), spec_sha256, graph_id],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO concept_assets(asset_id, project_id, version_id, role, logical_path, object_path, sha256, byte_size, mime_type, metadata_json, created_at, soft_deleted_at) VALUES ('asset_legacy_http_glb', ?, ?, 'module_glb', 'modules/legacy_http.glb', ?, ?, ?, 'model/gltf-binary', '{}', '2026-07-17T00:00:02Z', NULL)",
            params![project_id, version_id, object_path, glb_sha256, glb.len() as i64],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO module_assets(module_id, pack_id, category, asset_id, schema_version, manifest_json, manifest_sha256, status, created_at, updated_at) VALUES (?, 'pack_weapon_concept_v1', 'core_shell', 'asset_legacy_http_glb', 'ModuleAssetManifest@1', ?, ?, 'active', '2026-07-17T00:00:02Z', '2026-07-17T00:00:02Z')",
            params![module_id, manifest.to_string(), manifest_sha256],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO module_assets(module_id, pack_id, category, asset_id, schema_version, manifest_json, manifest_sha256, status, created_at, updated_at) VALUES (?, 'pack_weapon_concept_v1', 'side_accessory', 'asset_legacy_http_glb', 'ModuleAssetManifest@1', ?, ?, 'active', '2026-07-17T00:00:02Z', '2026-07-17T00:00:02Z')",
            params![
                auxiliary_module_id,
                auxiliary_manifest.to_string(),
                auxiliary_manifest_sha256
            ],
        )
        .unwrap();
    for (catalog_module_id, display_name, catalog_path) in [
        (module_id.as_str(), "Legacy shell", "core_shell"),
        (auxiliary_module_id, "Legacy side detail", "side_accessory"),
    ] {
        connection
            .execute(
                "INSERT INTO module_asset_catalog_metadata(module_id, display_name, description, tags_json, catalog_path, origin_claim, creator_name, review_status, reviewer_name, reviewed_at, review_note, updated_at) VALUES (?, ?, 'Reviewed historical display-only module.', '[\"legacy\",\"reviewed\"]', ?, 'self_declared_original', 'ForgeCAD Author', 'approved', 'Independent Reviewer', '2026-07-17T00:00:02Z', 'Read-only compatibility catalog.', '2026-07-17T00:00:02Z')",
                params![catalog_module_id, display_name, catalog_path],
            )
            .unwrap();
    }
    connection
        .execute(
            "INSERT INTO module_graphs(graph_id, project_id, version_id, root_node_id, schema_version, graph_json, graph_sha256, validation_status, created_at, updated_at) VALUES (?, ?, ?, 'node_legacy_http_shell', 'ModuleGraph@1', ?, ?, 'valid', '2026-07-17T00:00:03Z', '2026-07-17T00:00:03Z')",
            params![graph_id, project_id, version_id, graph.to_string(), graph_sha256],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE projects SET current_version_id=? WHERE project_id=?",
            params![version_id, project_id],
        )
        .unwrap();
    Fixture {
        project_id,
        version_id,
        graph_id,
        module_id,
        spec,
        graph,
        glb,
        glb_sha256,
    }
}

fn minimal_glb() -> Vec<u8> {
    let mut document = serde_json::to_vec(&json!({"asset":{"version":"2.0"}})).unwrap();
    while document.len() % 4 != 0 {
        document.push(b' ');
    }
    let total_length = 12 + 8 + document.len();
    let mut glb = Vec::with_capacity(total_length);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&(total_length as u32).to_le_bytes());
    glb.extend_from_slice(&(document.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&document);
    glb
}
