#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod viewer;

#[tauri::command]
fn viewer_read_model() -> serde_json::Value {
    viewer::read_model()
}

#[tauri::command]
fn viewer_artifact_bytes(artifact_id: String, candidate_id: String) -> serde_json::Value {
    viewer::read_artifact_bytes(&artifact_id, &candidate_id)
}

fn main() {
    if std::env::args().any(|argument| argument == "--build-identity") {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "schema_version": "ForgeCADDevBuildIdentity@1",
                "component": "forgecad-viewer",
                "build_cohort_sha256": forgecad_runtime::build_cohort_sha256()
            }))
            .expect("build identity serializes")
        );
        return;
    }
    if std::env::args().any(|argument| argument == "--viewer-read-model") {
        println!(
            "{}",
            serde_json::to_string(&viewer::read_model()).expect("Viewer read model serializes")
        );
        return;
    }
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            viewer_read_model,
            viewer_artifact_bytes
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ForgeCAD Runtime Viewer");
}
