//! Opt-in, no-GUI packaged proof for the local mechanical-arm MVP.
//!
//! The probe drives the release-shaped Rust app-server protocol, not a browser
//! fixture or frontend model. It is enabled only by the dedicated verification
//! command and writes one bounded evidence report to its caller-owned path.

use std::{
    env, fs,
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
    thread,
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use forgecad_app_server::{
    compatibility::{AllowedHttpMethod, LocalAgentEndpoint, PreparedCompatHttpRequest},
    CancellationToken,
};
use forgecad_app_server_protocol::{CompatHttpResponse, ProtocolHttpBody};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::app_server_bridge::AppServerBridge;

const PROBE_FLAG: &str = "FORGECAD_MVP_ARM_PACKAGED_PROBE";
const OUTPUT_FLAG: &str = "FORGECAD_MVP_ARM_PACKAGED_PROBE_OUTPUT";
const RESUME_FLAG: &str = "FORGECAD_MVP_ARM_PACKAGED_RESUME";
const RESUME_INPUT_FLAG: &str = "FORGECAD_MVP_ARM_PACKAGED_RESUME_INPUT";
pub(crate) const PROBE_ENDPOINT: &str = "http://127.0.0.1:1";
const BRIEF: &str = "流线三关节维护机械臂，固定基座、双连杆、旋转腕部和夹爪";
const SCHEMA_VERSION: &str = "ForgeCADArmMvpPackagedProtocolProof@3";
// C108 separates the lightweight interactive preview from the reviewed
// production readback envelope. Keep both preview delivery and export bound
// to that production contract rather than the obsolete 14,392-triangle
// mechanism fixture.
const C106_PRODUCTION_TRIANGLE_MIN: u64 = 80_000;
const C106_PRODUCTION_TRIANGLE_MAX: u64 = 150_000;
const EXPECTED_ROOT_RECIPE_ID: &str = "recipe_c106_arm_service_display";

#[derive(Serialize)]
struct ProbeReport {
    schema_version: &'static str,
    status: &'static str,
    brief: &'static str,
    project_id: Option<String>,
    thread_id: Option<String>,
    turn_id: Option<String>,
    preview: Option<PreviewEvidence>,
    root_recipe_id: Option<String>,
    v1_asset_version_id: Option<String>,
    a005: Option<A005Evidence>,
    c110c: Option<C110CEvidence>,
    c110d: Option<C110DEvidence>,
    active_design: Option<ActiveDesignEvidence>,
    export: Option<ExportEvidence>,
    provider: ProviderEvidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
}

#[derive(Serialize)]
struct PreviewEvidence {
    preview_id: String,
    artifact_profile_id: String,
    glb_sha256: String,
    triangle_count: u64,
}

#[derive(Serialize)]
struct A005Evidence {
    change_set_id: String,
    parent_asset_version_id: String,
    v2_asset_version_id: String,
    part_id: String,
    material_zone_id: String,
    surface_adornment_count: u64,
}

#[derive(Serialize)]
struct C110CEvidence {
    change_set_id: String,
    parent_asset_version_id: String,
    v3_asset_version_id: String,
    parent_part_id: String,
    added_part_id: String,
    operation_count: u64,
    preview_glb_sha256: String,
    preview_triangle_count: u64,
}

#[derive(Serialize)]
struct C110DEvidence {
    change_set_id: String,
    parent_asset_version_id: String,
    v4_asset_version_id: String,
    parent_part_id: String,
    added_part_ids: Vec<String>,
    recipe_ids: Vec<String>,
    operation_count: u64,
    preview_glb_sha256: String,
    preview_triangle_count: u64,
}

#[derive(Serialize)]
struct ActiveDesignEvidence {
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
struct ResumeReport {
    schema_version: &'static str,
    status: &'static str,
    project_id: Option<String>,
    expected_asset_version_id: Option<String>,
    active_design: Option<ActiveDesignEvidence>,
    export: Option<ExportEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
}

#[derive(Serialize)]
struct ProviderEvidence {
    source_kind: &'static str,
    internal_subrequests: u64,
    action_loop_steps: u64,
    product_tool_calls: u64,
    external_network_calls: u64,
    credential_reads: u64,
}

pub(crate) fn run_if_enabled(bridge: AppServerBridge) {
    if env::var(PROBE_FLAG).as_deref() != Ok("1") {
        return;
    }
    let output = match env::var(OUTPUT_FLAG).ok().map(PathBuf::from) {
        Some(path) if path.is_absolute() => path,
        _ => return,
    };
    if env::var(RESUME_FLAG).as_deref() == Ok("1") {
        let input = match env::var(RESUME_INPUT_FLAG).ok().map(PathBuf::from) {
            Some(path) if path.is_absolute() => path,
            _ => return,
        };
        let _ = thread::Builder::new()
            .name("forgecad-mvp-arm-packaged-resume-probe".into())
            .spawn(move || {
                let report = match catch_unwind(AssertUnwindSafe(|| run_resume(bridge, &input))) {
                    Ok(result) => result.unwrap_or_else(|failure| ResumeReport {
                        schema_version: "ForgeCADArmMvpPackagedResumeProof@3",
                        status: "fail",
                        project_id: failure.project_id,
                        expected_asset_version_id: None,
                        active_design: None,
                        export: None,
                        error_code: Some(failure.code),
                    }),
                    Err(_) => ResumeReport {
                        schema_version: "ForgeCADArmMvpPackagedResumeProof@3",
                        status: "fail",
                        project_id: None,
                        expected_asset_version_id: None,
                        active_design: None,
                        export: None,
                        error_code: Some("MVP_ARM_RESUME_PROBE_PANIC".into()),
                    },
                };
                write_report(&output, &report);
            });
        return;
    }
    let _ = thread::Builder::new()
        .name("forgecad-mvp-arm-packaged-probe".into())
        .spawn(move || {
            let report = match catch_unwind(AssertUnwindSafe(|| run(bridge))) {
                Ok(result) => result.unwrap_or_else(|failure| probe_failure_report(failure)),
                Err(_) => probe_failure_report(ProbeFailure::new("MVP_ARM_PROBE_PANIC")),
            };
            write_report(&output, &report);
        });
}

fn probe_failure_report(failure: ProbeFailure) -> ProbeReport {
    ProbeReport {
        schema_version: SCHEMA_VERSION,
        status: "fail",
        brief: BRIEF,
        project_id: failure.project_id,
        thread_id: failure.thread_id,
        turn_id: failure.turn_id,
        preview: None,
        root_recipe_id: None,
        v1_asset_version_id: None,
        a005: None,
        c110c: None,
        c110d: None,
        active_design: None,
        export: None,
        provider: ProviderEvidence {
            source_kind: "offline_deterministic",
            internal_subrequests: 0,
            action_loop_steps: 0,
            product_tool_calls: 0,
            external_network_calls: 0,
            credential_reads: 0,
        },
        error_code: Some(failure.code),
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

fn run(bridge: AppServerBridge) -> Result<ProbeReport, ProbeFailure> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| ProbeFailure::new("MVP_ARM_PROBE_RUNTIME_FAILED"))?;
    runtime.block_on(async move {
        let project = compat_json(
            &bridge,
            AllowedHttpMethod::Post,
            "/api/v1/projects",
            Some("mvp_arm_project_create"),
            None,
            Some(json!({
                "client_request_id": "mvp_arm_project_create",
                "name": "本机机械臂 MVP 协议验证",
                "profile_id": "profile_weapon_concept_v1"
            })),
            &[200, 201],
        )
        .await
        .map_err(ProbeFailure::new)?;
        let project_id = required_id(&project, "project_id").ok_or_else(|| {
            ProbeFailure::new("MVP_ARM_PROJECT_ID_MISSING")
        })?;

        let thread = native(
            &bridge,
            "mvp_arm_thread_create",
            "thread/create",
            json!({
                "schema_version": "AgentThreadCommand@1",
                "command_id": "mvp_arm_thread_create",
                "command": {
                    "operation": "create",
                    "request": {
                        "client_request_id": "mvp_arm_thread_create",
                        "project_id": project_id,
                        "title": "本机机械臂 MVP",
                        "provider_id": "deepseek"
                    }
                }
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
                    "MVP_ARM_THREAD_ID_MISSING",
                    Some(project_id.clone()),
                    None,
                    None,
                )
            })?;

        let started = native(
            &bridge,
            "mvp_arm_turn_start",
            "turn/start",
            json!({
                "schema_version": "AgentTurnCommand@1",
                "command_id": "mvp_arm_turn_start",
                "command": {
                    "operation": "start",
                    "thread_id": thread_id,
                    "request": {
                        "client_request_id": "mvp_arm_turn_start",
                        "message": BRIEF,
                        "clarification_domain_pack_id": null
                    }
                }
            }),
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), None))?;
        let turn_id = started
            .pointer("/result/turn/turn_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| {
                ProbeFailure::with_ids(
                    "MVP_ARM_TURN_ID_MISSING",
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
                "MVP_ARM_TURN_NOT_COMPLETED",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
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
                "MVP_ARM_PROVIDER_EVIDENCE_INVALID",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            ));
        }
        let decision = preview_decision(&turn).ok_or_else(|| {
            ProbeFailure::with_ids(
                "MVP_ARM_PREVIEW_DECISION_MISSING",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            )
        })?;
        let preview_id = required_id(&decision["preview"], "preview_id").ok_or_else(|| {
            ProbeFailure::with_ids(
                "MVP_ARM_PREVIEW_ID_MISSING",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            )
        })?;
        let expected_sha = required_id(&decision["preview"], "artifact_sha256").ok_or_else(|| {
            ProbeFailure::with_ids(
                "MVP_ARM_PREVIEW_SHA_MISSING",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            )
        })?;
        if decision["preview"]["artifact_profile_id"].as_str() != Some("production_concept") {
            return Err(ProbeFailure::with_ids(
                "MVP_ARM_PREVIEW_PROFILE_INVALID",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            ));
        }
        let preview_path = format!(
            "/api/v1/agent/projects/{project_id}/turns/{turn_id}/single-results/{preview_id}:preview.glb"
        );
        let (preview_response, bytes) = compat_binary(
            &bridge,
            &preview_path,
            Some(&format!("\"sha256:{expected_sha}\"")),
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let actual_sha = sha256_hex(&bytes);
        let triangle_count = header_u64(&preview_response, "X-ForgeCAD-Triangle-Count").ok_or_else(|| {
            ProbeFailure::with_ids("MVP_ARM_TRIANGLE_HEADER_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone()))
        })?;
        if actual_sha != expected_sha {
            return Err(ProbeFailure::with_ids(
                "MVP_ARM_PREVIEW_HASH_MISMATCH",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            ));
        }
        if !within_c106_production_triangle_budget(triangle_count) {
            return Err(ProbeFailure::with_ids(
                "MVP_ARM_PREVIEW_TRIANGLE_BUDGET_INVALID",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            ));
        }

        let preview_etag = format!("\"sha256:{expected_sha}\"");
        let confirmed = compat_json(
            &bridge,
            AllowedHttpMethod::Post,
            &format!(
                "/api/v1/agent/projects/{project_id}/turns/{turn_id}/single-results/{preview_id}:confirm"
            ),
            Some("mvp_arm_preview_confirm"),
            Some(&preview_etag),
            Some(json!({
                "client_request_id": "mvp_arm_preview_confirm",
                "expected_artifact_sha256": expected_sha,
                "summary": "Confirm local mechanical-arm MVP packaged protocol proof"
            })),
            &[201],
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let v1_asset_version_id = required_id(&confirmed, "asset_version_id").ok_or_else(|| {
            ProbeFailure::with_ids("MVP_ARM_V1_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone()))
        })?;
        let root_recipe_id = confirmed
            .pointer("/assembly_graph/component_recipe_instances")
            .and_then(Value::as_array)
            .and_then(|instances| {
                instances.iter().find_map(|instance| {
                    instance
                        .get("parent_instance_id")
                        .filter(|parent| parent.is_null())
                        .and_then(|_| instance.pointer("/recipe/recipe_id"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
            })
            .ok_or_else(|| {
                ProbeFailure::with_ids(
                    "MVP_ARM_ROOT_RECIPE_MISSING",
                    Some(project_id.clone()),
                    Some(thread_id.clone()),
                    Some(turn_id.clone()),
                )
            })?;
        if root_recipe_id != EXPECTED_ROOT_RECIPE_ID {
            return Err(ProbeFailure::with_ids(
                "MVP_ARM_ROOT_RECIPE_INVALID",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            ));
        }
        let (part_id, material_zone_id) = first_part_zone(&confirmed).ok_or_else(|| {
            ProbeFailure::with_ids(
                "MVP_ARM_A005_TARGET_MISSING",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            )
        })?;
        compat_json(
            &bridge,
            AllowedHttpMethod::Post,
            "/api/v1/agent/skills/surface-adornment:enable",
            Some("mvp_arm_enable_surface_adornment"),
            None,
            Some(json!({
                "schema_version": "EnableSurfaceAdornmentSkillRequest@1",
                "client_request_id": "mvp_arm_enable_surface_adornment",
                "confirm_enable": true
            })),
            &[200],
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let proposed = compat_json(
            &bridge,
            AllowedHttpMethod::Post,
            &format!(
                "/api/v1/agent/asset-versions/{v1_asset_version_id}/surface-adornments:preview"
            ),
            Some("mvp_arm_a005_propose"),
            None,
            Some(json!({
                "schema_version": "SurfaceAdornmentPreviewRequest@1",
                "client_request_id": "mvp_arm_a005_propose",
                "part_id": part_id,
                "material_zone_id": material_zone_id,
                "kind": "flowline",
                "motif": "double_flowline",
                "intensity": "subtle",
                "coverage": "center_band"
            })),
            &[201],
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let change_set_id = required_id(&proposed, "change_set_id").ok_or_else(|| {
            ProbeFailure::with_ids("MVP_ARM_A005_CHANGE_SET_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone()))
        })?;
        compat_json(
            &bridge,
            AllowedHttpMethod::Post,
            &format!("/api/v1/agent/change-sets/{change_set_id}:preview"),
            Some("mvp_arm_a005_preview"),
            None,
            None,
            &[200],
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let confirmed_a005 = compat_json(
            &bridge,
            AllowedHttpMethod::Post,
            &format!("/api/v1/agent/change-sets/{change_set_id}:confirm"),
            Some("mvp_arm_a005_confirm"),
            None,
            None,
            &[200],
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let confirmed_v2 = confirmed_a005
            .get("asset_version")
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_V2_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let confirmed_v2_asset_version_id = required_id(confirmed_v2, "asset_version_id").ok_or_else(|| {
            ProbeFailure::with_ids("MVP_ARM_V2_ID_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone()))
        })?;
        if confirmed_v2.get("parent_asset_version_id").and_then(Value::as_str)
            != Some(v1_asset_version_id.as_str())
            || confirmed_v2.get("version_no").and_then(Value::as_u64) != Some(2)
        {
            return Err(ProbeFailure::with_ids(
                "MVP_ARM_V2_LINEAGE_INVALID",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            ));
        }
        // The confirm envelope is intentionally compact. Read the immutable
        // Rust-owned version before selecting a parent for the next edit;
        // never infer parts from the envelope or from the preview JSON.
        let v2 = compat_json(
            &bridge,
            AllowedHttpMethod::Get,
            &format!("/api/v1/agent/asset-versions/{confirmed_v2_asset_version_id}"),
            Some("mvp_arm_a005_asset_readback"),
            None,
            None,
            &[200],
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let v2_asset_version_id = required_id(&v2, "asset_version_id").ok_or_else(|| {
            ProbeFailure::with_ids("MVP_ARM_V2_READBACK_ID_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone()))
        })?;
        if v2_asset_version_id != confirmed_v2_asset_version_id {
            return Err(ProbeFailure::with_ids(
                "MVP_ARM_V2_READBACK_LINEAGE_INVALID",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            ));
        }
        let surface_adornment_count = v2
            .pointer("/assembly_graph/surface_adornments")
            .and_then(Value::as_array)
            .map(|items| items.len() as u64)
            .unwrap_or_default();
        if surface_adornment_count < 1 {
            return Err(ProbeFailure::with_ids(
                "MVP_ARM_A005_PROVENANCE_MISSING",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            ));
        }
        // AgentAssetVersion.parts is the compact asset inventory. Connector
        // geometry belongs to the AssemblyGraph contract, so select the
        // attachment parent from the graph readback rather than inventing a
        // second connector representation on the part inventory.
        let graph_parts = v2
            .pointer("/assembly_graph/parts")
            .and_then(Value::as_array)
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110C_GRAPH_PARTS_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110c_parent = graph_parts
            .iter()
            .find(|part| {
                    part.get("role").and_then(Value::as_str) == Some("joint_housing")
                        && part
                            .get("connectors")
                            .and_then(Value::as_array)
                            .is_some_and(|connectors| !connectors.is_empty())
            })
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110C_PARENT_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110c_parent_part_id = c110c_parent
            .get("part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110C_PARENT_ID_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110c_parent_connector_id = c110c_parent
            .pointer("/connectors/0/connector_id")
            .and_then(Value::as_str)
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110C_PARENT_CONNECTOR_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110c_root = graph_parts
            .iter()
            .find(|part| part.get("parent_part_id").is_some_and(Value::is_null))
            .or_else(|| {
                v2.pointer("/assembly_graph/root_part_id")
                    .and_then(Value::as_str)
                    .and_then(|root_id| graph_parts.iter().find(|part| part.get("part_id").and_then(Value::as_str) == Some(root_id)))
            })
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110C_ROOT_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110c_root_part_id = c110c_root
            .get("part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110C_ROOT_ID_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110c_root_connector_id = c110c_root
            .pointer("/connectors/0/connector_id")
            .and_then(Value::as_str)
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110C_ROOT_CONNECTOR_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110c_proposed = compat_json(
            &bridge,
            AllowedHttpMethod::Post,
            &format!("/api/v1/agent/asset-versions/{v2_asset_version_id}/change-sets"),
            Some("mvp_arm_c110c_propose"),
            None,
            Some(json!({
                "client_request_id": "mvp_arm_c110c_propose",
                "summary": "Add a sensor pod, pose the joint, and snap the reviewed assembly.",
                "operations": [
                    {
                        "operation_id": "delta_c110c_add_sensor",
                        "op": "add_reviewed_recipe",
                        "part_id": c110c_parent_part_id,
                        "new_part_id": "part_c110c_sensor_pod",
                        "parent_connector_id": c110c_parent_connector_id,
                        "child_connector_id": "connector_sensor_pod_mount",
                        "recipe_id": "recipe_c110c_arm_sensor_pod",
                        "slot_id": "slot_arm_sensor_pod",
                        "transform": {"position":[0.0,12.0,0.0],"rotation":[0.0,0.2,0.0],"scale":[1.0,1.0,1.0]}
                    },
                    {
                        "operation_id": "delta_c110c_pose_joint",
                        "op": "set_joint_pose",
                        "part_id": c110c_parent_part_id,
                        "joint_id": "joint_c110c_visual_wrist",
                        "pose": {"rotation":[0.0,0.12,0.0],"translation":[4.0,0.0,0.0]}
                    },
                    {
                        "operation_id": "delta_c110c_snap_joint",
                        "op": "snap_part_to_connector",
                        "part_id": c110c_parent_part_id,
                        "target_part_id": c110c_root_part_id,
                        "target_connector_id": c110c_root_connector_id,
                        "connector_id": c110c_parent_connector_id
                    }
                ]
            })),
            &[201],
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110c_change_set_id = required_id(&c110c_proposed, "change_set_id")
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110C_CHANGE_SET_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        compat_json(
            &bridge,
            AllowedHttpMethod::Post,
            &format!("/api/v1/agent/change-sets/{c110c_change_set_id}:preview"),
            Some("mvp_arm_c110c_preview"),
            None,
            None,
            &[200],
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let (c110c_preview_response, c110c_preview_bytes) = compat_binary(
            &bridge,
            &format!("/api/v1/agent/change-sets/{c110c_change_set_id}:preview.glb"),
            None,
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110c_preview_sha = header_value(&c110c_preview_response, "X-ForgeCAD-GLB-SHA256")
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110C_PREVIEW_HEADER_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110c_preview_triangle_count = header_u64(&c110c_preview_response, "X-ForgeCAD-Triangle-Count")
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110C_TRIANGLE_HEADER_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        if sha256_hex(&c110c_preview_bytes) != c110c_preview_sha || c110c_preview_triangle_count == 0 {
            return Err(ProbeFailure::with_ids("MVP_ARM_C110C_PREVIEW_READBACK_INVALID", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())));
        }
        let c110c_confirmed = compat_json(
            &bridge,
            AllowedHttpMethod::Post,
            &format!("/api/v1/agent/change-sets/{c110c_change_set_id}:confirm"),
            Some("mvp_arm_c110c_confirm"),
            None,
            None,
            &[200],
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110c_version = c110c_confirmed
            .get("asset_version")
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110C_VERSION_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110c_asset_version_id = required_id(c110c_version, "asset_version_id")
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110C_VERSION_ID_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        if c110c_version.get("parent_asset_version_id").and_then(Value::as_str) != Some(v2_asset_version_id.as_str())
            || c110c_version.get("version_no").and_then(Value::as_u64) != Some(3)
            || c110c_version.get("parts").and_then(Value::as_array).map(Vec::len)
                != v2.get("parts").and_then(Value::as_array).map(|parts| parts.len() + 1)
        {
            return Err(ProbeFailure::with_ids("MVP_ARM_C110C_LINEAGE_INVALID", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())));
        }
        // C110D proves that the same confirmed arm remains editable: add two
        // different reviewed visual Recipes to the existing V3 parent, then
        // compile/read back and confirm one atomic V4 snapshot.
        let c110d_proposed = compat_json(
            &bridge,
            AllowedHttpMethod::Post,
            &format!("/api/v1/agent/asset-versions/{c110c_asset_version_id}/change-sets"),
            Some("mvp_arm_c110d_propose"),
            None,
            Some(json!({
                "client_request_id": "mvp_arm_c110d_propose",
                "summary": "Add an actuator cover and cable guide to the confirmed arm.",
                "operations": [
                    {
                        "operation_id": "delta_c110d_add_actuator_cover",
                        "op": "add_reviewed_recipe",
                        "part_id": c110c_parent_part_id,
                        "new_part_id": "part_c110d_actuator_cover",
                        "parent_connector_id": c110c_parent_connector_id,
                        "child_connector_id": "connector_actuator_cover_mount",
                        "recipe_id": "recipe_c110d_arm_actuator_cover",
                        "slot_id": "slot_arm_guard_rail",
                        "transform": {"position":[0.0,24.0,0.0],"rotation":[0.0,0.18,0.0],"scale":[1.0,1.0,1.0]}
                    },
                    {
                        "operation_id": "delta_c110d_add_cable_guide",
                        "op": "add_reviewed_recipe",
                        "part_id": c110c_parent_part_id,
                        "new_part_id": "part_c110d_cable_guide",
                        "parent_connector_id": c110c_parent_connector_id,
                        "child_connector_id": "connector_cable_guide_mount",
                        "recipe_id": "recipe_c110d_arm_cable_guide",
                        "slot_id": "slot_arm_camera_boom",
                        "transform": {"position":[0.0,-30.0,18.0],"rotation":[0.0,-0.12,0.0],"scale":[1.0,1.0,1.0]}
                    }
                ]
            })),
            &[201],
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110d_change_set_id = required_id(&c110d_proposed, "change_set_id")
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110D_CHANGE_SET_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        compat_json(
            &bridge,
            AllowedHttpMethod::Post,
            &format!("/api/v1/agent/change-sets/{c110d_change_set_id}:preview"),
            Some("mvp_arm_c110d_preview"),
            None,
            None,
            &[200],
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let (c110d_preview_response, c110d_preview_bytes) = compat_binary(
            &bridge,
            &format!("/api/v1/agent/change-sets/{c110d_change_set_id}:preview.glb"),
            None,
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110d_preview_sha = header_value(&c110d_preview_response, "X-ForgeCAD-GLB-SHA256")
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110D_PREVIEW_HEADER_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110d_preview_triangle_count = header_u64(&c110d_preview_response, "X-ForgeCAD-Triangle-Count")
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110D_TRIANGLE_HEADER_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        if sha256_hex(&c110d_preview_bytes) != c110d_preview_sha || c110d_preview_triangle_count <= c110c_preview_triangle_count {
            return Err(ProbeFailure::with_ids("MVP_ARM_C110D_PREVIEW_READBACK_INVALID", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())));
        }
        let c110d_confirmed = compat_json(
            &bridge,
            AllowedHttpMethod::Post,
            &format!("/api/v1/agent/change-sets/{c110d_change_set_id}:confirm"),
            Some("mvp_arm_c110d_confirm"),
            None,
            None,
            &[200],
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110d_version = c110d_confirmed
            .get("asset_version")
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110D_VERSION_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let c110d_asset_version_id = required_id(c110d_version, "asset_version_id")
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_C110D_VERSION_ID_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        if c110d_version.get("parent_asset_version_id").and_then(Value::as_str) != Some(c110c_asset_version_id.as_str())
            || c110d_version.get("version_no").and_then(Value::as_u64) != Some(4)
            || c110d_version.get("parts").and_then(Value::as_array).map(Vec::len)
                != c110c_version.get("parts").and_then(Value::as_array).map(|parts| parts.len() + 2)
        {
            return Err(ProbeFailure::with_ids("MVP_ARM_C110D_LINEAGE_INVALID", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())));
        }
        let active_design = compat_json(
            &bridge,
            AllowedHttpMethod::Get,
            &format!("/api/v1/projects/{project_id}/active-design"),
            None,
            None,
            None,
            &[200],
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let active_asset_version_id = active_design
            .pointer("/active_design/asset_version_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_ACTIVE_DESIGN_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let snapshot_revision = active_design
            .get("revision")
            .and_then(Value::as_u64)
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_SNAPSHOT_REVISION_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        if active_asset_version_id != c110d_asset_version_id {
            return Err(ProbeFailure::with_ids(
                "MVP_ARM_ACTIVE_DESIGN_C110D_DRIFT",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            ));
        }
        let export = compat_json(
            &bridge,
            AllowedHttpMethod::Post,
            &format!("/api/v1/agent/asset-versions/{c110d_asset_version_id}:export"),
            Some("mvp_arm_c110d_export"),
            None,
            None,
            &[200],
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let export_sha = required_id(&export, "glb_sha256").ok_or_else(|| {
            ProbeFailure::with_ids("MVP_ARM_EXPORT_SHA_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone()))
        })?;
        let export_bytes = export
            .get("glb_base64")
            .and_then(Value::as_str)
            .and_then(|value| BASE64_STANDARD.decode(value).ok())
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_EXPORT_BYTES_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let export_byte_size = export.get("glb_byte_size").and_then(Value::as_u64).unwrap_or_default();
        let export_triangle_count = export.get("triangle_count").and_then(Value::as_u64).unwrap_or_default();
        if !within_c106_production_triangle_budget(export_triangle_count) {
            return Err(ProbeFailure::with_ids(
                "MVP_ARM_EXPORT_TRIANGLE_BUDGET_INVALID",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            ));
        }
        if export.get("asset_version_id").and_then(Value::as_str) != Some(c110d_asset_version_id.as_str())
            || export.get("artifact_profile_id").and_then(Value::as_str) != Some("production_concept")
            || export.get("readback_status").and_then(Value::as_str) != Some("passed")
            || sha256_hex(&export_bytes) != export_sha
            || export_bytes.len() as u64 != export_byte_size
        {
            return Err(ProbeFailure::with_ids(
                "MVP_ARM_EXPORT_READBACK_INVALID",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            ));
        }
        let (model_response, model_bytes) = compat_binary(
            &bridge,
            &format!("/api/v1/agent/asset-versions/{c110d_asset_version_id}:model.glb"),
            None,
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        let export_header_sha = header_value(&model_response, "X-ForgeCAD-GLB-SHA256")
            .ok_or_else(|| ProbeFailure::with_ids("MVP_ARM_EXPORT_HEADER_MISSING", Some(project_id.clone()), Some(thread_id.clone()), Some(turn_id.clone())))?;
        if model_bytes != export_bytes || export_header_sha != export_sha {
            return Err(ProbeFailure::with_ids(
                "MVP_ARM_EXPORT_HEADER_DRIFT",
                Some(project_id.clone()),
                Some(thread_id.clone()),
                Some(turn_id.clone()),
            ));
        }

        Ok(ProbeReport {
            schema_version: SCHEMA_VERSION,
            status: "pass",
            brief: BRIEF,
            project_id: Some(project_id),
            thread_id: Some(thread_id),
            turn_id: Some(turn_id),
            preview: Some(PreviewEvidence {
                preview_id,
                artifact_profile_id: "production_concept".into(),
                glb_sha256: expected_sha,
                triangle_count,
            }),
            root_recipe_id: Some(root_recipe_id),
            v1_asset_version_id: Some(v1_asset_version_id.clone()),
            a005: Some(A005Evidence {
                change_set_id,
                parent_asset_version_id: v1_asset_version_id,
                v2_asset_version_id: v2_asset_version_id.clone(),
                part_id,
                material_zone_id,
                surface_adornment_count,
            }),
            c110c: Some(C110CEvidence {
                change_set_id: c110c_change_set_id,
                parent_asset_version_id: v2_asset_version_id,
                v3_asset_version_id: c110c_asset_version_id.clone(),
                parent_part_id: c110c_parent_part_id.to_owned(),
                added_part_id: "part_c110c_sensor_pod".into(),
                operation_count: 3,
                preview_glb_sha256: c110c_preview_sha,
                preview_triangle_count: c110c_preview_triangle_count,
            }),
            c110d: Some(C110DEvidence {
                change_set_id: c110d_change_set_id,
                parent_asset_version_id: c110c_asset_version_id,
                v4_asset_version_id: c110d_asset_version_id.clone(),
                parent_part_id: c110c_parent_part_id.to_owned(),
                added_part_ids: vec!["part_c110d_actuator_cover".into(), "part_c110d_cable_guide".into()],
                recipe_ids: vec!["recipe_c110d_arm_actuator_cover".into(), "recipe_c110d_arm_cable_guide".into()],
                operation_count: 2,
                preview_glb_sha256: c110d_preview_sha,
                preview_triangle_count: c110d_preview_triangle_count,
            }),
            active_design: Some(ActiveDesignEvidence {
                asset_version_id: active_asset_version_id,
                snapshot_revision,
            }),
            export: Some(ExportEvidence {
                asset_version_id: c110d_asset_version_id,
                glb_sha256: export_sha,
                glb_byte_size: export_byte_size,
                triangle_count: export_triangle_count,
                x_forgecad_glb_sha256: export_header_sha,
            }),
            provider: ProviderEvidence {
                source_kind: "offline_deterministic",
                internal_subrequests: provider_requests,
                action_loop_steps: provider_requests,
                product_tool_calls,
                external_network_calls: 0,
                credential_reads: 0,
            },
            error_code: None,
        })
    })
}

fn first_part_zone(version: &Value) -> Option<(String, String)> {
    version.get("parts")?.as_array()?.iter().find_map(|part| {
        let part_id = part.get("part_id")?.as_str()?;
        let zone_id = part
            .get("material_zone_ids")?
            .as_array()?
            .first()?
            .as_str()?;
        Some((part_id.to_string(), zone_id.to_string()))
    })
}

fn run_resume(bridge: AppServerBridge, input: &PathBuf) -> Result<ResumeReport, ProbeFailure> {
    let checkpoint: Value = fs::read(input)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(|| ProbeFailure::new("MVP_ARM_RESUME_INPUT_INVALID"))?;
    let project_id = required_id(&checkpoint, "project_id")
        .ok_or_else(|| ProbeFailure::new("MVP_ARM_RESUME_PROJECT_MISSING"))?;
    let expected_asset_version_id = checkpoint
        .pointer("/c110d/v4_asset_version_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            ProbeFailure::with_ids(
                "MVP_ARM_RESUME_C110D_MISSING",
                Some(project_id.clone()),
                None,
                None,
            )
        })?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| ProbeFailure::new("MVP_ARM_RESUME_RUNTIME_FAILED"))?;
    runtime.block_on(async move {
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
        let active_asset_version_id = active
            .pointer("/active_design/asset_version_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| {
                ProbeFailure::with_ids(
                    "MVP_ARM_RESUME_ACTIVE_MISSING",
                    Some(project_id.clone()),
                    None,
                    None,
                )
            })?;
        let snapshot_revision =
            active
                .get("revision")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    ProbeFailure::with_ids(
                        "MVP_ARM_RESUME_REVISION_MISSING",
                        Some(project_id.clone()),
                        None,
                        None,
                    )
                })?;
        if active_asset_version_id != expected_asset_version_id {
            return Err(ProbeFailure::with_ids(
                "MVP_ARM_RESUME_HEAD_DRIFT",
                Some(project_id.clone()),
                None,
                None,
            ));
        }
        let export = compat_json(
            &bridge,
            AllowedHttpMethod::Post,
            &format!("/api/v1/agent/asset-versions/{expected_asset_version_id}:export"),
            Some("mvp_arm_resume_export"),
            None,
            None,
            &[200],
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), None, None))?;
        let export_sha = required_id(&export, "glb_sha256").ok_or_else(|| {
            ProbeFailure::with_ids(
                "MVP_ARM_RESUME_EXPORT_SHA_MISSING",
                Some(project_id.clone()),
                None,
                None,
            )
        })?;
        let export_bytes = export
            .get("glb_base64")
            .and_then(Value::as_str)
            .and_then(|value| BASE64_STANDARD.decode(value).ok())
            .ok_or_else(|| {
                ProbeFailure::with_ids(
                    "MVP_ARM_RESUME_EXPORT_BYTES_MISSING",
                    Some(project_id.clone()),
                    None,
                    None,
                )
            })?;
        let export_byte_size = export
            .get("glb_byte_size")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let export_triangle_count = export
            .get("triangle_count")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let (model_response, model_bytes) = compat_binary(
            &bridge,
            &format!("/api/v1/agent/asset-versions/{expected_asset_version_id}:model.glb"),
            None,
        )
        .await
        .map_err(|code| ProbeFailure::with_ids(code, Some(project_id.clone()), None, None))?;
        let header_sha =
            header_value(&model_response, "X-ForgeCAD-GLB-SHA256").ok_or_else(|| {
                ProbeFailure::with_ids(
                    "MVP_ARM_RESUME_EXPORT_HEADER_MISSING",
                    Some(project_id.clone()),
                    None,
                    None,
                )
            })?;
        if !within_c106_production_triangle_budget(export_triangle_count) {
            return Err(ProbeFailure::with_ids(
                "MVP_ARM_RESUME_EXPORT_TRIANGLE_BUDGET_INVALID",
                Some(project_id.clone()),
                None,
                None,
            ));
        }
        if export.get("asset_version_id").and_then(Value::as_str)
            != Some(expected_asset_version_id.as_str())
            || sha256_hex(&export_bytes) != export_sha
            || export_bytes.len() as u64 != export_byte_size
            || model_bytes != export_bytes
            || header_sha != export_sha
        {
            return Err(ProbeFailure::with_ids(
                "MVP_ARM_RESUME_EXPORT_INVALID",
                Some(project_id.clone()),
                None,
                None,
            ));
        }
        Ok(ResumeReport {
            schema_version: "ForgeCADArmMvpPackagedResumeProof@3",
            status: "pass",
            project_id: Some(project_id),
            expected_asset_version_id: Some(expected_asset_version_id.clone()),
            active_design: Some(ActiveDesignEvidence {
                asset_version_id: active_asset_version_id,
                snapshot_revision,
            }),
            export: Some(ExportEvidence {
                asset_version_id: expected_asset_version_id,
                glb_sha256: export_sha,
                glb_byte_size: export_byte_size,
                triangle_count: export_triangle_count,
                x_forgecad_glb_sha256: header_sha,
            }),
            error_code: None,
        })
    })
}

pub(crate) async fn native(
    bridge: &AppServerBridge,
    request_id: &str,
    method: &str,
    params: Value,
) -> Result<Value, &'static str> {
    bridge
        .execute_mvp_packaged_native(request_id, method, params)
        .await
        .map_err(|_| "MVP_ARM_NATIVE_PROTOCOL_REJECTED")
}

pub(crate) async fn wait_terminal(
    bridge: &AppServerBridge,
    thread_id: &str,
    turn_id: &str,
) -> Result<Value, &'static str> {
    // C108 keeps the 101k-triangle production GLB and its deterministic audit
    // thumbnails inside one bounded Turn. The probe must outlive the Turn's
    // 280-second ceiling so it reports the terminal Rust truth instead of
    // inventing an earlier probe timeout; the shell retains the outer bound.
    for attempt in 0..3_000 {
        let value = native(
            bridge,
            &format!("mvp_arm_turn_read_{attempt}"),
            "turn/read",
            json!({
                "schema_version": "AgentTurnCommand@1",
                "command_id": format!("mvp_arm_turn_read_{attempt}"),
                "command": {"operation": "read", "thread_id": thread_id, "turn_id": turn_id}
            }),
        )
        .await?;
        let turn = value
            .pointer("/result/turn")
            .cloned()
            .ok_or("MVP_ARM_TURN_READ_INVALID")?;
        if matches!(
            turn.get("status").and_then(Value::as_str),
            Some("completed" | "failed" | "cancelled")
        ) {
            return Ok(turn);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err("MVP_ARM_TURN_TIMEOUT")
}

pub(crate) fn preview_decision(turn: &Value) -> Option<Value> {
    turn.get("items")?.as_array()?.iter().find_map(|item| {
        (item.pointer("/payload/tool_name").and_then(Value::as_str)
            == Some("prepare_candidate_preview"))
        .then(|| {
            item.pointer("/payload/tool_result/validated_output/value")
                .cloned()
        })
        .flatten()
        .and_then(|value| {
            // Native ToolResult persistence stores the validated
            // SingleResultDecision directly. Keep the narrow nested fallback
            // only for older compatibility envelopes.
            if value.get("schema_version").and_then(Value::as_str) == Some("SingleResultDecision@1")
            {
                Some(value)
            } else {
                value.get("single_result_decision").cloned()
            }
        })
    })
}

pub(crate) async fn compat_json(
    bridge: &AppServerBridge,
    method: AllowedHttpMethod,
    path: &str,
    idempotency_key: Option<&str>,
    if_match: Option<&str>,
    body: Option<Value>,
    accepted: &[u16],
) -> Result<Value, String> {
    let mut headers = Vec::new();
    if let Some(key) = idempotency_key {
        headers.push(("Idempotency-Key".into(), key.into()));
    }
    if let Some(value) = if_match {
        headers.push(("If-Match".into(), value.into()));
    }
    let body = body.map_or(ProtocolHttpBody::Empty, |value| {
        headers.push(("Content-Type".into(), "application/json".into()));
        ProtocolHttpBody::Utf8 {
            data: value.to_string(),
        }
    });
    let response = bridge
        .execute_k003_packaged_compat(
            PreparedCompatHttpRequest {
                endpoint: LocalAgentEndpoint::parse(PROBE_ENDPOINT)
                    .map_err(|_| "MVP_ARM_COMPAT_ENDPOINT_INVALID".to_string())?,
                method,
                path: path.into(),
                headers,
                body,
            },
            CancellationToken::new(),
        )
        .await
        .map_err(|_| "MVP_ARM_COMPAT_ROUTE_FAILED".to_string())?;
    if !accepted.contains(&response.status) {
        let boundary_code = match &response.body {
            ProtocolHttpBody::Utf8 { data } => {
                serde_json::from_str::<Value>(data).ok().and_then(|value| {
                    value
                        .pointer("/error/code")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
            }
            _ => None,
        };
        return Err(boundary_code.map_or_else(
            || format!("MVP_ARM_COMPAT_STATUS_{}", response.status),
            |code| format!("MVP_ARM_BOUNDARY_{code}"),
        ));
    }
    let ProtocolHttpBody::Utf8 { data } = response.body else {
        return Err("MVP_ARM_COMPAT_JSON_MISSING".to_string());
    };
    serde_json::from_str(&data).map_err(|_| "MVP_ARM_COMPAT_JSON_INVALID".to_string())
}

pub(crate) async fn compat_binary(
    bridge: &AppServerBridge,
    path: &str,
    if_match: Option<&str>,
) -> Result<(CompatHttpResponse, Vec<u8>), &'static str> {
    let headers = if_match
        .map(|value| vec![("If-Match".into(), value.into())])
        .unwrap_or_default();
    let response = bridge
        .execute_k003_packaged_compat(
            PreparedCompatHttpRequest {
                endpoint: LocalAgentEndpoint::parse(PROBE_ENDPOINT)
                    .map_err(|_| "MVP_ARM_COMPAT_ENDPOINT_INVALID")?,
                method: AllowedHttpMethod::Get,
                path: path.into(),
                headers,
                body: ProtocolHttpBody::Empty,
            },
            CancellationToken::new(),
        )
        .await
        .map_err(|_| "MVP_ARM_PREVIEW_ROUTE_FAILED")?;
    if response.status != 200 {
        return Err("MVP_ARM_PREVIEW_ROUTE_REJECTED");
    }
    let ProtocolHttpBody::Base64 { data } = &response.body else {
        return Err("MVP_ARM_PREVIEW_BODY_INVALID");
    };
    let bytes = BASE64_STANDARD
        .decode(data)
        .map_err(|_| "MVP_ARM_PREVIEW_BODY_INVALID")?;
    Ok((response, bytes))
}

pub(crate) fn required_id(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn header_u64(response: &CompatHttpResponse, name: &str) -> Option<u64> {
    response
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .and_then(|(_, value)| value.parse().ok())
}

pub(crate) fn header_value(response: &CompatHttpResponse, name: &str) -> Option<String> {
    response
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.clone())
}

fn within_c106_production_triangle_budget(triangle_count: u64) -> bool {
    (C106_PRODUCTION_TRIANGLE_MIN..=C106_PRODUCTION_TRIANGLE_MAX).contains(&triangle_count)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
