#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod viewer;

#[tauri::command]
fn viewer_read_model() -> serde_json::Value {
    viewer::read_model()
}

#[tauri::command]
fn viewer_read_model_summary() -> serde_json::Value {
    viewer::read_model_summary()
}

#[tauri::command]
fn viewer_artifact_bytes(artifact_id: String, candidate_id: String) -> serde_json::Value {
    viewer::read_artifact_bytes(&artifact_id, &candidate_id)
}

#[tauri::command]
fn viewer_reference_bytes(reference_id: String, project_id: String) -> serde_json::Value {
    viewer::read_reference_bytes(&reference_id, &project_id)
}

#[tauri::command]
fn viewer_render_pass(render_set_hash: String, pass: String) -> serde_json::Value {
    viewer::read_render_pass(&render_set_hash, &pass)
}

#[tauri::command]
fn viewer_visual_evidence(candidate_id: String) -> serde_json::Value {
    viewer::read_visual_evidence(&candidate_id)
}

#[tauri::command]
fn viewer_agentic_projection(project_id: String, candidate_id: String) -> serde_json::Value {
    viewer::read_agentic_projection(&project_id, &candidate_id)
}

#[tauri::command]
fn viewer_agentic_session(project_id: String, candidate_id: String) -> serde_json::Value {
    viewer::read_agentic_session(&project_id, &candidate_id)
}

#[tauri::command]
fn viewer_mechanical_animation_inventory(
    project_id: String,
    candidate_id: String,
    artifact_id: String,
) -> serde_json::Value {
    viewer::read_mechanical_animation_inventory(&project_id, &candidate_id, &artifact_id)
}

#[tauri::command]
fn viewer_mechanical_animation_clip(
    project_id: String,
    candidate_id: String,
    artifact_id: String,
    clip_id: String,
) -> serde_json::Value {
    viewer::read_mechanical_animation_clip(&project_id, &candidate_id, &artifact_id, &clip_id)
}

#[tauri::command]
fn viewer_mechanical_animation_frame_preview(
    project_id: String,
    candidate_id: String,
    artifact_id: String,
    clip_id: String,
    sample_time_ticks: u64,
) -> serde_json::Value {
    viewer::read_mechanical_animation_frame_preview(
        &project_id,
        &candidate_id,
        &artifact_id,
        &clip_id,
        sample_time_ticks,
    )
}

#[tauri::command]
fn viewer_provenance_graph(
    project_id: String,
    candidate_id: String,
    candidate_state_sha256: String,
    artifact_id: String,
) -> serde_json::Value {
    viewer::read_provenance_graph(
        &project_id,
        &candidate_id,
        &candidate_state_sha256,
        &artifact_id,
    )
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
            viewer_read_model_summary,
            viewer_artifact_bytes,
            viewer_reference_bytes,
            viewer_render_pass,
            viewer_visual_evidence,
            viewer_agentic_projection,
            viewer_agentic_session,
            viewer_mechanical_animation_inventory,
            viewer_mechanical_animation_clip,
            viewer_mechanical_animation_frame_preview,
            viewer_provenance_graph
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ForgeCAD Runtime Viewer");
}
