//! Opt-in packaged proof for the independent C110G parallel-link family.
//!
//! This is deliberately separate from the serial-chain MVP probe.  It proves
//! that the new Recipe/Connector registry survives the same Rust-owned
//! packaged lifecycle: one bounded intent, production GLB readback, a reviewed
//! AssemblyDelta attachment, confirmation, export and a fresh-process resume.
//! It never calls a network Provider and never writes credentials to the
//! report.

use std::{
    env, fs,
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
    thread,
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use forgecad_app_server::compatibility::AllowedHttpMethod;
use serde::Serialize;
use serde_json::{json, Value};

use crate::app_server_bridge::AppServerBridge;
use crate::mvp_arm_packaged_probe::{
    compat_binary, compat_json, header_u64, header_value, native, preview_decision, required_id,
    sha256_hex, wait_terminal,
};

const PROBE_FLAG: &str = "FORGECAD_C110G_PACKAGED_PROBE";
const OUTPUT_FLAG: &str = "FORGECAD_C110G_PACKAGED_PROBE_OUTPUT";
const RESUME_FLAG: &str = "FORGECAD_C110G_PACKAGED_RESUME";
const RESUME_INPUT_FLAG: &str = "FORGECAD_C110G_PACKAGED_RESUME_INPUT";
const BRIEF: &str = "双导轨并联维护机械臂，工业底座、滑台、并联连杆和环形工具座";
const ROOT_RECIPE_ID: &str = "recipe_c110g_parallel_link_root";
const ATTACHMENT_RECIPE_ID: &str = "recipe_c110g_parallel_link";
const ATTACHMENT_SLOT_ID: &str = "slot_c110g_parallel_link";
const SCHEMA_VERSION: &str = "ForgeCADC110GPackagedProtocolProof@1";
const RESUME_SCHEMA_VERSION: &str = "ForgeCADC110GPackagedResumeProof@1";

#[derive(Serialize)]
struct ProviderEvidence {
    source_kind: &'static str,
    internal_subrequests: u64,
    action_loop_steps: u64,
    product_tool_calls: u64,
    external_network_calls: u64,
    credential_reads: u64,
}

#[derive(Serialize)]
struct GlbEvidence {
    glb_sha256: String,
    triangle_count: u64,
}

#[derive(Serialize)]
struct DeltaEvidence {
    change_set_id: String,
    parent_asset_version_id: String,
    asset_version_id: String,
    parent_part_id: String,
    added_part_id: String,
    recipe_id: &'static str,
    slot_id: &'static str,
    operation_count: u64,
    preview: GlbEvidence,
}

#[derive(Serialize)]
struct ActiveEvidence {
    asset_version_id: String,
    snapshot_revision: u64,
}

#[derive(Serialize)]
struct ExportEvidence {
    asset_version_id: String,
    glb_sha256: String,
    glb_byte_size: u64,
    triangle_count: u64,
    x_forgecad_glb_sha256: String,
}

#[derive(Serialize)]
struct ProbeReport {
    schema_version: &'static str,
    status: &'static str,
    architecture: &'static str,
    brief: &'static str,
    project_id: Option<String>,
    thread_id: Option<String>,
    turn_id: Option<String>,
    root_recipe_id: Option<String>,
    initial_asset_version_id: Option<String>,
    initial_preview: Option<GlbEvidenceWithId>,
    delta: Option<DeltaEvidence>,
    active_design: Option<ActiveEvidence>,
    export: Option<ExportEvidence>,
    provider: ProviderEvidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
}

#[derive(Serialize)]
struct GlbEvidenceWithId {
    preview_id: String,
    artifact_profile_id: &'static str,
    glb_sha256: String,
    triangle_count: u64,
}

#[derive(Serialize)]
struct ResumeReport {
    schema_version: &'static str,
    status: &'static str,
    project_id: Option<String>,
    expected_asset_version_id: Option<String>,
    active_design: Option<ActiveEvidence>,
    export: Option<ExportEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
}

#[derive(Debug)]
struct ProbeFailure {
    code: String,
    project_id: Option<String>,
    thread_id: Option<String>,
    turn_id: Option<String>,
}

impl ProbeFailure {
    fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            project_id: None,
            thread_id: None,
            turn_id: None,
        }
    }

    fn with_ids(
        code: impl Into<String>,
        project_id: Option<String>,
        thread_id: Option<String>,
        turn_id: Option<String>,
    ) -> Self {
        Self {
            code: code.into(),
            project_id,
            thread_id,
            turn_id,
        }
    }
}

pub(crate) fn run_if_enabled(bridge: AppServerBridge) {
    if env::var(PROBE_FLAG).as_deref() != Ok("1") {
        return;
    }
    let Some(output) = env::var(OUTPUT_FLAG).ok().map(PathBuf::from) else {
        return;
    };
    if !output.is_absolute() {
        return;
    }
    if env::var(RESUME_FLAG).as_deref() == Ok("1") {
        let Some(input) = env::var(RESUME_INPUT_FLAG).ok().map(PathBuf::from) else {
            return;
        };
        if !input.is_absolute() {
            return;
        }
        let _ = thread::Builder::new()
            .name("forgecad-c110g-packaged-resume-probe".into())
            .spawn(move || {
                let report =
                    match catch_unwind(AssertUnwindSafe(|| run_resume_blocking(bridge, &input))) {
                        Ok(Ok(report)) => report,
                        Ok(Err(error)) => ResumeReport {
                            schema_version: RESUME_SCHEMA_VERSION,
                            status: "fail",
                            project_id: error.project_id,
                            expected_asset_version_id: None,
                            active_design: None,
                            export: None,
                            error_code: Some(error.code),
                        },
                        Err(_) => ResumeReport {
                            schema_version: RESUME_SCHEMA_VERSION,
                            status: "fail",
                            project_id: None,
                            expected_asset_version_id: None,
                            active_design: None,
                            export: None,
                            error_code: Some("C110G_RESUME_PROBE_PANIC".into()),
                        },
                    };
                write_report(&output, &report);
            });
        return;
    }
    let _ = thread::Builder::new()
        .name("forgecad-c110g-packaged-probe".into())
        .spawn(move || {
            let report = match catch_unwind(AssertUnwindSafe(|| run_blocking(bridge))) {
                Ok(Ok(report)) => report,
                Ok(Err(error)) => failure_report(error),
                Err(_) => failure_report(ProbeFailure::new("C110G_PROBE_PANIC")),
            };
            write_report(&output, &report);
        });
}

fn provider_evidence(provider_requests: u64, product_tool_calls: u64) -> ProviderEvidence {
    ProviderEvidence {
        source_kind: "offline_deterministic",
        internal_subrequests: provider_requests,
        action_loop_steps: provider_requests,
        product_tool_calls,
        external_network_calls: 0,
        credential_reads: 0,
    }
}

fn failure_report(error: ProbeFailure) -> ProbeReport {
    ProbeReport {
        schema_version: SCHEMA_VERSION,
        status: "fail",
        architecture: "parallel_link",
        brief: BRIEF,
        project_id: error.project_id,
        thread_id: error.thread_id,
        turn_id: error.turn_id,
        root_recipe_id: None,
        initial_asset_version_id: None,
        initial_preview: None,
        delta: None,
        active_design: None,
        export: None,
        provider: provider_evidence(0, 0),
        error_code: Some(error.code),
    }
}

fn write_report(output: &PathBuf, report: &impl Serialize) {
    if let Some(parent) = output.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let temporary = output.with_extension("tmp");
    if let Ok(bytes) = serde_json::to_vec(report) {
        if fs::write(&temporary, bytes).is_ok() {
            let _ = fs::rename(temporary, output);
        }
    }
}

fn run_blocking(bridge: AppServerBridge) -> Result<ProbeReport, ProbeFailure> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| ProbeFailure::new("C110G_PROBE_RUNTIME_FAILED"))?;
    runtime.block_on(run(bridge))
}

fn run_resume_blocking(
    bridge: AppServerBridge,
    input: &PathBuf,
) -> Result<ResumeReport, ProbeFailure> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| ProbeFailure::new("C110G_RESUME_RUNTIME_FAILED"))?;
    runtime.block_on(run_resume(bridge, input))
}

async fn run(bridge: AppServerBridge) -> Result<ProbeReport, ProbeFailure> {
    let project = compat_json(
        &bridge,
        AllowedHttpMethod::Post,
        "/api/v1/projects",
        Some("c110g_project_create"),
        None,
        Some(json!({
            "client_request_id": "c110g_project_create",
            "name": "C110G 并联机械臂 packaged 验证",
            "profile_id": "profile_weapon_concept_v1"
        })),
        &[200, 201],
    )
    .await
    .map_err(ProbeFailure::new)?;
    let project_id = required_id(&project, "project_id")
        .ok_or_else(|| ProbeFailure::new("C110G_PROJECT_ID_MISSING"))?;
    let thread = native(
        &bridge,
        "c110g_thread_create",
        "thread/create",
        json!({
            "schema_version": "AgentThreadCommand@1",
            "command_id": "c110g_thread_create",
            "command": {"operation":"create", "request": {
                "client_request_id":"c110g_thread_create",
                "project_id":project_id,
                "title":"C110G 并联机械臂",
                "provider_id":"deepseek"
            }}
        }),
    )
    .await
    .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), None, None))?;
    let thread_id = thread
        .pointer("/result/thread/thread_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            ProbeFailure::with_ids(
                "C110G_THREAD_ID_MISSING",
                Some(project_id.clone()),
                None,
                None,
            )
        })?;
    let started = native(
        &bridge,
        "c110g_turn_start",
        "turn/start",
        json!({
            "schema_version":"AgentTurnCommand@1",
            "command_id":"c110g_turn_start",
            "command":{"operation":"start","thread_id":thread_id,"request":{
                "client_request_id":"c110g_turn_start",
                "message":BRIEF,
                "clarification_domain_pack_id":null
            }}
        }),
    )
    .await
    .map_err(|code| {
        ProbeFailure::with_ids(
            code,
            Some(project_id.clone()),
            Some(thread_id.clone()),
            None,
        )
    })?;
    let turn_id = started
        .pointer("/result/turn/turn_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            ProbeFailure::with_ids(
                "C110G_TURN_ID_MISSING",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                None,
            )
        })?;
    let turn = wait_terminal(&bridge, &thread_id, &turn_id)
        .await
        .map_err(|code| {
            ProbeFailure::with_ids(
                code,
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            )
        })?;
    if turn.get("status").and_then(Value::as_str) != Some("completed") {
        return Err(ProbeFailure::with_ids(
            "C110G_TURN_NOT_COMPLETED",
            Some(project_id),
            Some(thread_id),
            Some(turn_id),
        ));
    }
    let usage = turn.get("usage").cloned().unwrap_or(Value::Null);
    let provider_requests = usage
        .get("provider_requests")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let product_tool_calls = usage
        .get("product_tool_calls")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    if provider_requests != 8
        || product_tool_calls != 7
        || usage.get("network_call_made").and_then(Value::as_bool) != Some(false)
    {
        return Err(ProbeFailure::with_ids(
            "C110G_PROVIDER_EVIDENCE_INVALID",
            Some(project_id),
            Some(thread_id),
            Some(turn_id),
        ));
    }
    let decision = preview_decision(&turn).ok_or_else(|| {
        ProbeFailure::with_ids(
            "C110G_PREVIEW_DECISION_MISSING",
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        )
    })?;
    let preview_id = required_id(&decision["preview"], "preview_id").ok_or_else(|| {
        ProbeFailure::with_ids(
            "C110G_PREVIEW_ID_MISSING",
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        )
    })?;
    let expected_sha = required_id(&decision["preview"], "artifact_sha256").ok_or_else(|| {
        ProbeFailure::with_ids(
            "C110G_PREVIEW_SHA_MISSING",
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        )
    })?;
    if decision["preview"]["artifact_profile_id"].as_str() != Some("production_concept") {
        return Err(ProbeFailure::with_ids(
            "C110G_PREVIEW_PROFILE_INVALID",
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        ));
    }
    let preview_path = format!("/api/v1/agent/projects/{project_id}/turns/{turn_id}/single-results/{preview_id}:preview.glb");
    let (preview_response, preview_bytes) = compat_binary(
        &bridge,
        &preview_path,
        Some(&format!("\"sha256:{expected_sha}\"")),
    )
    .await
    .map_err(|code| {
        ProbeFailure::with_ids(
            code,
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        )
    })?;
    let preview_sha = sha256_hex(&preview_bytes);
    let preview_triangles =
        header_u64(&preview_response, "X-ForgeCAD-Triangle-Count").ok_or_else(|| {
            ProbeFailure::with_ids(
                "C110G_PREVIEW_TRIANGLE_HEADER_MISSING",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            )
        })?;
    if preview_sha != expected_sha || preview_triangles == 0 || preview_triangles > 150_000 {
        return Err(ProbeFailure::with_ids(
            "C110G_PREVIEW_READBACK_INVALID",
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        ));
    }
    let confirmed = compat_json(
        &bridge,
        AllowedHttpMethod::Post,
        &format!("/api/v1/agent/projects/{project_id}/turns/{turn_id}/single-results/{preview_id}:confirm"),
        Some("c110g_preview_confirm"),
        Some(&format!("\"sha256:{expected_sha}\"")),
        Some(json!({
            "client_request_id":"c110g_preview_confirm",
            "expected_artifact_sha256":expected_sha,
            "summary":"Confirm C110G parallel-link packaged preview"
        })),
        &[201],
    )
    .await
    .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
    let initial_asset_version_id =
        required_id(&confirmed, "asset_version_id").ok_or_else(|| {
            ProbeFailure::with_ids(
                "C110G_INITIAL_VERSION_MISSING",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            )
        })?;
    let root_recipe_id = confirmed
        .pointer("/assembly_graph/component_recipe_instances")
        .and_then(Value::as_array)
        .and_then(|instances| {
            instances.iter().find_map(|instance| {
                (instance
                    .get("parent_instance_id")
                    .is_some_and(Value::is_null))
                .then(|| instance.pointer("/recipe/recipe_id"))
                .flatten()
                .and_then(Value::as_str)
                .map(str::to_string)
            })
        })
        .ok_or_else(|| {
            ProbeFailure::with_ids(
                "C110G_ROOT_RECIPE_MISSING",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            )
        })?;
    if root_recipe_id != ROOT_RECIPE_ID {
        return Err(ProbeFailure::with_ids(
            "C110G_ROOT_RECIPE_INVALID",
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        ));
    }
    let root_part_id = confirmed
        .pointer("/assembly_graph/parts")
        .and_then(Value::as_array)
        .and_then(|parts| {
            parts
                .iter()
                .find(|part| part.get("parent_part_id").is_some_and(Value::is_null))
                .and_then(|part| part.get("part_id"))
                .and_then(Value::as_str)
        })
        .map(str::to_string)
        .ok_or_else(|| {
            ProbeFailure::with_ids(
                "C110G_ROOT_PART_MISSING",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            )
        })?;
    let proposed = compat_json(
        &bridge,
        AllowedHttpMethod::Post,
        &format!("/api/v1/agent/asset-versions/{initial_asset_version_id}/change-sets"),
        Some("c110g_delta_propose"),
        None,
        Some(json!({
            "client_request_id":"c110g_delta_propose",
            "summary":"Add a reviewed parallel-link visual strut",
            "operations":[{
                "operation_id":"delta_c110g_add_parallel_link",
                "op":"add_reviewed_recipe",
                "part_id":root_part_id,
                "new_part_id":"part_c110g_added_link",
                "parent_connector_id":"connector_parallel_carriage",
                "child_connector_id":"connector_parallel_link_mount",
                "recipe_id":ATTACHMENT_RECIPE_ID,
                "slot_id":ATTACHMENT_SLOT_ID,
                "transform":{"position":[0.0,18.0,8.0],"rotation":[0.0,0.12,0.0],"scale":[1.0,1.0,1.0]}
            }]
        })),
        &[201],
    )
    .await
    .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
    let change_set_id = required_id(&proposed, "change_set_id").ok_or_else(|| {
        ProbeFailure::with_ids(
            "C110G_DELTA_ID_MISSING",
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        )
    })?;
    compat_json(
        &bridge,
        AllowedHttpMethod::Post,
        &format!("/api/v1/agent/change-sets/{change_set_id}:preview"),
        Some("c110g_delta_preview"),
        None,
        None,
        &[200],
    )
    .await
    .map_err(|code| {
        ProbeFailure::with_ids(
            code,
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        )
    })?;
    let (delta_response, delta_bytes) = compat_binary(
        &bridge,
        &format!("/api/v1/agent/change-sets/{change_set_id}:preview.glb"),
        None,
    )
    .await
    .map_err(|code| {
        ProbeFailure::with_ids(
            code,
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        )
    })?;
    let delta_sha = header_value(&delta_response, "X-ForgeCAD-GLB-SHA256").ok_or_else(|| {
        ProbeFailure::with_ids(
            "C110G_DELTA_SHA_HEADER_MISSING",
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        )
    })?;
    let delta_triangles =
        header_u64(&delta_response, "X-ForgeCAD-Triangle-Count").ok_or_else(|| {
            ProbeFailure::with_ids(
                "C110G_DELTA_TRIANGLE_HEADER_MISSING",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            )
        })?;
    // The ChangeSet preview uses the interactive profile, whereas the
    // initial V003 result uses production_concept. Their triangle budgets
    // are intentionally different; the confirmed production export below
    // is the lineage/quality comparison. Only require a real hashed delta.
    if sha256_hex(&delta_bytes) != delta_sha || delta_triangles == 0 {
        return Err(ProbeFailure::with_ids(
            "C110G_DELTA_READBACK_INVALID",
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        ));
    }
    let confirmed_delta = compat_json(
        &bridge,
        AllowedHttpMethod::Post,
        &format!("/api/v1/agent/change-sets/{change_set_id}:confirm"),
        Some("c110g_delta_confirm"),
        None,
        None,
        &[200],
    )
    .await
    .map_err(|code| {
        ProbeFailure::with_ids(
            code,
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        )
    })?;
    let delta_version = confirmed_delta.get("asset_version").ok_or_else(|| {
        ProbeFailure::with_ids(
            "C110G_DELTA_VERSION_MISSING",
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        )
    })?;
    let delta_asset_version_id =
        required_id(delta_version, "asset_version_id").ok_or_else(|| {
            ProbeFailure::with_ids(
                "C110G_DELTA_VERSION_ID_MISSING",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            )
        })?;
    if delta_version
        .get("parent_asset_version_id")
        .and_then(Value::as_str)
        != Some(initial_asset_version_id.as_str())
        || delta_version.get("version_no").and_then(Value::as_u64) != Some(2)
        || delta_version
            .get("parts")
            .and_then(Value::as_array)
            .map(Vec::len)
            != confirmed
                .get("parts")
                .and_then(Value::as_array)
                .map(|parts| parts.len() + 1)
    {
        return Err(ProbeFailure::with_ids(
            "C110G_DELTA_LINEAGE_INVALID",
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        ));
    }
    let active = compat_json(
        &bridge,
        AllowedHttpMethod::Get,
        &format!("/api/v1/projects/{project_id}/active-design"),
        None,
        None,
        None,
        &[200],
    )
    .await
    .map_err(|code| {
        ProbeFailure::with_ids(
            code,
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        )
    })?;
    let active_id = active
        .pointer("/active_design/asset_version_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            ProbeFailure::with_ids(
                "C110G_ACTIVE_DESIGN_MISSING",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            )
        })?;
    let snapshot_revision = active
        .get("revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ProbeFailure::with_ids(
                "C110G_SNAPSHOT_REVISION_MISSING",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            )
        })?;
    if active_id != delta_asset_version_id {
        return Err(ProbeFailure::with_ids(
            "C110G_ACTIVE_DESIGN_DRIFT",
            Some(project_id.clone()),
            Some(thread_id.clone()),
            Some(turn_id.clone()),
        ));
    }
    let (export, _export_bytes, export_header_sha) = export_and_readback(
        &bridge,
        &delta_asset_version_id,
        &project_id,
        &thread_id,
        &turn_id,
    )
    .await?;
    Ok(ProbeReport {
        schema_version: SCHEMA_VERSION,
        status: "pass",
        architecture: "parallel_link",
        brief: BRIEF,
        project_id: Some(project_id),
        thread_id: Some(thread_id),
        turn_id: Some(turn_id),
        root_recipe_id: Some(root_recipe_id),
        initial_asset_version_id: Some(initial_asset_version_id.clone()),
        initial_preview: Some(GlbEvidenceWithId {
            preview_id,
            artifact_profile_id: "production_concept",
            glb_sha256: expected_sha,
            triangle_count: preview_triangles,
        }),
        delta: Some(DeltaEvidence {
            change_set_id,
            parent_asset_version_id: initial_asset_version_id,
            asset_version_id: delta_asset_version_id.clone(),
            parent_part_id: root_part_id,
            added_part_id: "part_c110g_added_link".into(),
            recipe_id: ATTACHMENT_RECIPE_ID,
            slot_id: ATTACHMENT_SLOT_ID,
            operation_count: 1,
            preview: GlbEvidence {
                glb_sha256: delta_sha,
                triangle_count: delta_triangles,
            },
        }),
        active_design: Some(ActiveEvidence {
            asset_version_id: active_id,
            snapshot_revision,
        }),
        export: Some(ExportEvidence {
            asset_version_id: delta_asset_version_id,
            glb_sha256: export
                .get("glb_sha256")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            glb_byte_size: export
                .get("glb_byte_size")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            triangle_count: export
                .get("triangle_count")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            x_forgecad_glb_sha256: export_header_sha,
        }),
        provider: provider_evidence(provider_requests, product_tool_calls),
        error_code: None,
    })
}

async fn export_and_readback(
    bridge: &AppServerBridge,
    asset_version_id: &str,
    project_id: &str,
    thread_id: &str,
    turn_id: &str,
) -> Result<(Value, Vec<u8>, String), ProbeFailure> {
    let export = compat_json(
        &bridge,
        AllowedHttpMethod::Post,
        &format!("/api/v1/agent/asset-versions/{asset_version_id}:export"),
        Some("c110g_export"),
        None,
        None,
        &[200],
    )
    .await
    .map_err(|code| {
        ProbeFailure::with_ids(
            code,
            Some(project_id.into()),
            Some(thread_id.into()),
            Some(turn_id.into()),
        )
    })?;
    let export_bytes = export
        .get("glb_base64")
        .and_then(Value::as_str)
        .and_then(|value| BASE64_STANDARD.decode(value).ok())
        .ok_or_else(|| {
            ProbeFailure::with_ids(
                "C110G_EXPORT_BYTES_MISSING",
                Some(project_id.into()),
                Some(thread_id.into()),
                Some(turn_id.into()),
            )
        })?;
    let export_sha = required_id(&export, "glb_sha256").ok_or_else(|| {
        ProbeFailure::with_ids(
            "C110G_EXPORT_SHA_MISSING",
            Some(project_id.into()),
            Some(thread_id.into()),
            Some(turn_id.into()),
        )
    })?;
    if export.get("asset_version_id").and_then(Value::as_str) != Some(asset_version_id)
        || export.get("artifact_profile_id").and_then(Value::as_str) != Some("production_concept")
        || export.get("readback_status").and_then(Value::as_str) != Some("passed")
        || sha256_hex(&export_bytes) != export_sha
        || export_bytes.len() as u64
            != export
                .get("glb_byte_size")
                .and_then(Value::as_u64)
                .unwrap_or_default()
    {
        return Err(ProbeFailure::with_ids(
            "C110G_EXPORT_READBACK_INVALID",
            Some(project_id.into()),
            Some(thread_id.into()),
            Some(turn_id.into()),
        ));
    }
    let (model_response, model_bytes) = compat_binary(
        &bridge,
        &format!("/api/v1/agent/asset-versions/{asset_version_id}:model.glb"),
        None,
    )
    .await
    .map_err(|code| {
        ProbeFailure::with_ids(
            code,
            Some(project_id.into()),
            Some(thread_id.into()),
            Some(turn_id.into()),
        )
    })?;
    let header_sha = header_value(&model_response, "X-ForgeCAD-GLB-SHA256").ok_or_else(|| {
        ProbeFailure::with_ids(
            "C110G_EXPORT_HEADER_MISSING",
            Some(project_id.into()),
            Some(thread_id.into()),
            Some(turn_id.into()),
        )
    })?;
    if model_bytes != export_bytes || header_sha != export_sha {
        return Err(ProbeFailure::with_ids(
            "C110G_EXPORT_HEADER_DRIFT",
            Some(project_id.into()),
            Some(thread_id.into()),
            Some(turn_id.into()),
        ));
    }
    Ok((export, export_bytes, header_sha))
}

async fn run_resume(
    bridge: AppServerBridge,
    input: &PathBuf,
) -> Result<ResumeReport, ProbeFailure> {
    let checkpoint: Value = fs::read(input)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(|| ProbeFailure::new("C110G_RESUME_INPUT_INVALID"))?;
    let project_id = required_id(&checkpoint, "project_id")
        .ok_or_else(|| ProbeFailure::new("C110G_RESUME_PROJECT_MISSING"))?;
    let expected_asset_version_id = checkpoint
        .pointer("/delta/asset_version_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            ProbeFailure::with_ids(
                "C110G_RESUME_VERSION_MISSING",
                Some(project_id.clone()),
                None,
                None,
            )
        })?;
    let active = compat_json(
        &bridge,
        AllowedHttpMethod::Get,
        &format!("/api/v1/projects/{project_id}/active-design"),
        None,
        None,
        None,
        &[200],
    )
    .await
    .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), None, None))?;
    let active_id = active
        .pointer("/active_design/asset_version_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            ProbeFailure::with_ids(
                "C110G_RESUME_ACTIVE_MISSING",
                Some(project_id.clone()),
                None,
                None,
            )
        })?;
    let revision = active
        .get("revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ProbeFailure::with_ids(
                "C110G_RESUME_REVISION_MISSING",
                Some(project_id.clone()),
                None,
                None,
            )
        })?;
    if active_id != expected_asset_version_id {
        return Err(ProbeFailure::with_ids(
            "C110G_RESUME_HEAD_DRIFT",
            Some(project_id),
            None,
            None,
        ));
    }
    let (export, _bytes, header_sha) = export_and_readback(
        &bridge,
        &expected_asset_version_id,
        &project_id,
        "resume",
        "resume",
    )
    .await?;
    Ok(ResumeReport {
        schema_version: RESUME_SCHEMA_VERSION,
        status: "pass",
        project_id: Some(project_id),
        expected_asset_version_id: Some(expected_asset_version_id.clone()),
        active_design: Some(ActiveEvidence {
            asset_version_id: expected_asset_version_id.clone(),
            snapshot_revision: revision,
        }),
        export: Some(ExportEvidence {
            asset_version_id: expected_asset_version_id,
            glb_sha256: required_id(&export, "glb_sha256").unwrap_or_default(),
            glb_byte_size: export
                .get("glb_byte_size")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            triangle_count: export
                .get("triangle_count")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            x_forgecad_glb_sha256: header_sha,
        }),
        error_code: None,
    })
}
