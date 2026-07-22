//! Explicit live DeepSeek continuation acceptance for an already confirmed arm.
//!
//! The deterministic C110D packaged probe proves that the ChangeSet/GLB path
//! works.  This probe proves the missing boundary: DeepSeek must emit a
//! validated `AssemblyDeltaProgram@1`, Rust must bind it to the current head,
//! and the resulting preview/confirm/export must be a real immutable child.
//! It is opt-in, uses only the Rust Provider credential store, and writes a
//! redacted report.  The caller supplies a temporary library seeded by the
//! offline C110D probe; no user project is touched.

use std::{env, fs, path::PathBuf, thread};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use forgecad_app_server::{
    compatibility::{AllowedHttpMethod, LocalAgentEndpoint, PreparedCompatHttpRequest},
    CancellationToken,
};
use forgecad_app_server_protocol::{CompatHttpResponse, ProtocolHttpBody};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    app_server_bridge::AppServerBridge,
    mvp_arm_packaged_probe::{compat_json, native, required_id, wait_terminal, PROBE_ENDPOINT},
};

const ENABLE_FLAG: &str = "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE";
const CONFIRM_FLAG: &str = "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_CONFIRM";
const RUN_ID_FLAG: &str = "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_RUN_ID";
const OUTPUT_FLAG: &str = "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_OUTPUT";
const INPUT_FLAG: &str = "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_INPUT";
const RESUME_FLAG: &str = "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_RESUME";
const SCHEMA_VERSION: &str = "ForgeCADDeepSeekDeltaAcceptance@1";
const CONFIRMATION: &str = "I_UNDERSTAND_THIS_MAY_INCUR_PROVIDER_COST";
const CONTINUATION_BRIEF: &str = "在当前已确认的机械臂上继续设计：增加一个可见传感器舱和一条线缆导向，保留现有蓝黑金属语言；只做非功能展示概念。必须调用 plan_complete_concept，并在 plan.assembly_delta 中输出 AssemblyDeltaProgram@1 增量方案，base_asset_version_id 必须读取当前 ActiveDesignSnapshot；只使用工具合同列出的 reviewed recipe、slot、Part/Connector 和 bounded transform，不要重新生成完整机械臂，不要输出 dimensions、ShapeProgram、代码或未知字段，最后等待工作台预览确认。";

#[derive(Debug, Clone)]
struct ProbeConfig {
    run_id_sha256: String,
    input: PathBuf,
    output: PathBuf,
    resume: bool,
}

#[derive(Debug, Clone)]
struct Seed {
    project_id: String,
    base_asset_version_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct PhaseEvidence {
    status: &'static str,
    network_call_made: bool,
    asset_or_snapshot_writes: u64,
    delta_bound: bool,
    preview_glb_sha256: Option<String>,
    preview_triangle_count: Option<u64>,
    confirmed: bool,
    restarted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn_status: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn_error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<&'static str>,
}

impl PhaseEvidence {
    fn not_run() -> Self {
        Self {
            status: "not_run",
            network_call_made: false,
            asset_or_snapshot_writes: 0,
            delta_bound: false,
            preview_glb_sha256: None,
            preview_triangle_count: None,
            confirmed: false,
            restarted: false,
            turn_status: None,
            turn_error_code: None,
            error_code: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct DeltaReport {
    schema_version: &'static str,
    status: &'static str,
    execution_mode: &'static str,
    run_id_sha256: String,
    provider_owner: &'static str,
    credential_source: &'static str,
    network_calls_made: u64,
    project_id: Option<String>,
    base_asset_version_id: Option<String>,
    new_asset_version_id: Option<String>,
    change_set_id: Option<String>,
    delta: PhaseEvidence,
    restart: PhaseEvidence,
    no_raw_prompt_or_response: bool,
    no_key_or_provider_endpoint: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_phase: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<&'static str>,
}

struct Failure {
    phase: &'static str,
    code: &'static str,
    evidence: PhaseEvidence,
}

impl Failure {
    fn before(phase: &'static str, code: &'static str) -> Self {
        Self {
            phase,
            code,
            evidence: PhaseEvidence {
                status: "failed_before_terminal",
                error_code: Some(code),
                ..PhaseEvidence::not_run()
            },
        }
    }

    fn observed(phase: &'static str, code: &'static str, mut evidence: PhaseEvidence) -> Self {
        evidence.status = "failed";
        evidence.error_code = Some(code);
        Self {
            phase,
            code,
            evidence,
        }
    }
}

pub(crate) fn run_if_enabled(bridge: AppServerBridge) {
    let config = match parse_config(|name| env::var(name).ok()) {
        Ok(Some(config)) => config,
        Ok(None) | Err(_) => return,
    };
    let _ = thread::Builder::new()
        .name("forgecad-deepseek-delta-acceptance".into())
        .spawn(move || {
            let report =
                run(bridge, &config).unwrap_or_else(|failure| failed_report(&config, failure));
            write_report(&config.output, &report);
        });
}

fn parse_config(
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<Option<ProbeConfig>, &'static str> {
    if lookup(ENABLE_FLAG).as_deref() != Some("1") {
        return Ok(None);
    }
    if lookup(CONFIRM_FLAG).as_deref() != Some(CONFIRMATION) {
        return Err("LIVE_CONFIRMATION_REQUIRED");
    }
    let run_id = lookup(RUN_ID_FLAG).ok_or("LIVE_RUN_ID_REQUIRED")?;
    if !valid_run_id(&run_id) {
        return Err("LIVE_RUN_ID_INVALID");
    }
    let input = PathBuf::from(lookup(INPUT_FLAG).ok_or("LIVE_INPUT_REQUIRED")?);
    let output = PathBuf::from(lookup(OUTPUT_FLAG).ok_or("LIVE_OUTPUT_REQUIRED")?);
    if !input.is_absolute() || !output.is_absolute() {
        return Err("LIVE_OUTPUT_INVALID");
    }
    Ok(Some(ProbeConfig {
        run_id_sha256: sha256_hex(run_id.as_bytes()),
        input,
        output,
        resume: lookup(RESUME_FLAG).as_deref() == Some("1"),
    }))
}

fn valid_run_id(value: &str) -> bool {
    (12..=80).contains(&value.len())
        && value.starts_with("live_")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn write_report(path: &PathBuf, report: &DeltaReport) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let temporary = path.with_extension("tmp");
    if let Ok(bytes) = serde_json::to_vec(report) {
        if fs::write(&temporary, bytes).is_ok() {
            let _ = fs::rename(temporary, path);
        }
    }
}

fn failed_report(config: &ProbeConfig, failure: Failure) -> DeltaReport {
    let mut report = DeltaReport {
        schema_version: SCHEMA_VERSION,
        status: "fail",
        execution_mode: "live_explicit_opt_in",
        run_id_sha256: config.run_id_sha256.clone(),
        provider_owner: "rust_desktop",
        credential_source: "rust_provider_credential_store",
        network_calls_made: 0,
        project_id: None,
        base_asset_version_id: None,
        new_asset_version_id: None,
        change_set_id: None,
        delta: PhaseEvidence::not_run(),
        restart: PhaseEvidence::not_run(),
        no_raw_prompt_or_response: true,
        no_key_or_provider_endpoint: true,
        error_phase: Some(failure.phase),
        error_code: Some(failure.code),
    };
    match failure.phase {
        "delta" => report.delta = failure.evidence,
        "restart" => report.restart = failure.evidence,
        _ => {}
    }
    report.network_calls_made = u64::from(report.delta.network_call_made);
    report
}

fn read_seed(path: &PathBuf) -> Result<Seed, &'static str> {
    let value: Value = serde_json::from_slice(&fs::read(path).map_err(|_| "LIVE_INPUT_INVALID")?)
        .map_err(|_| "LIVE_INPUT_INVALID")?;
    let project_id = required_id(&value, "project_id").ok_or("LIVE_SEED_PROJECT_MISSING")?;
    let base_asset_version_id = value
        .pointer("/c110d/v4_asset_version_id")
        .and_then(Value::as_str)
        .or_else(|| value.get("base_asset_version_id").and_then(Value::as_str))
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .ok_or("LIVE_SEED_ASSET_MISSING")?;
    Ok(Seed {
        project_id,
        base_asset_version_id,
    })
}

fn run(bridge: AppServerBridge, config: &ProbeConfig) -> Result<DeltaReport, Failure> {
    let seed = read_seed(&config.input).map_err(|code| Failure::before("delta", code))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Failure::before("delta", "LIVE_RUNTIME_UNAVAILABLE"))?;
    if config.resume {
        return runtime.block_on(run_resume(&bridge, config, &seed));
    }
    runtime.block_on(run_delta(&bridge, config, &seed))
}

async fn run_delta(
    bridge: &AppServerBridge,
    config: &ProbeConfig,
    seed: &Seed,
) -> Result<DeltaReport, Failure> {
    let active = compat_json(
        bridge,
        AllowedHttpMethod::Get,
        &format!("/api/v1/projects/{}/active-design", seed.project_id),
        None,
        None,
        None,
        &[200],
    )
    .await
    .map_err(|_| Failure::before("delta", "LIVE_ACTIVE_DESIGN_READ_REJECTED"))?;
    if active
        .pointer("/active_design/asset_version_id")
        .and_then(Value::as_str)
        != Some(seed.base_asset_version_id.as_str())
    {
        return Err(Failure::before("delta", "LIVE_ACTIVE_ASSET_HEAD_DRIFT"));
    }
    let thread_id = create_thread(bridge, &seed.project_id, "live_delta_thread")
        .await
        .map_err(|code| Failure::before("delta", code))?;
    let (turn_id, _cancellation_id, _cancellation_token) =
        start_turn(bridge, &thread_id, "live_delta_turn")
            .await
            .map_err(|code| Failure::before("delta", code))?;
    let turn = wait_terminal(bridge, &thread_id, &turn_id)
        .await
        .map_err(|_| Failure::before("delta", "LIVE_DELTA_TURN_TIMEOUT"))?;
    let network_call_made = turn
        .pointer("/usage/network_call_made")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut evidence = PhaseEvidence {
        status: match turn.get("status").and_then(Value::as_str) {
            Some("completed") => "completed",
            Some("cancelled") => "cancelled",
            _ => "failed",
        },
        network_call_made,
        asset_or_snapshot_writes: 0,
        delta_bound: false,
        preview_glb_sha256: None,
        preview_triangle_count: None,
        confirmed: false,
        restarted: false,
        turn_status: terminal_turn_status(&turn),
        turn_error_code: terminal_turn_error_code(&turn),
        error_code: None,
    };
    if turn.get("status").and_then(Value::as_str) != Some("completed") {
        return Err(Failure::observed(
            "delta",
            "LIVE_DELTA_TURN_NOT_COMPLETED",
            evidence,
        ));
    }
    let delta = extract_delta(&turn)
        .ok_or_else(|| Failure::observed("delta", "LIVE_DELTA_NOT_BOUND", evidence.clone()))?;
    if delta.get("base_asset_version_id").and_then(Value::as_str)
        != Some(seed.base_asset_version_id.as_str())
    {
        return Err(Failure::observed(
            "delta",
            "LIVE_DELTA_BASE_DRIFT",
            evidence,
        ));
    }
    let operations = delta
        .get("operations")
        .and_then(Value::as_array)
        .filter(|operations| !operations.is_empty() && operations.len() <= 8)
        .ok_or_else(|| {
            Failure::observed("delta", "LIVE_DELTA_OPERATIONS_INVALID", evidence.clone())
        })?;
    if !operations.iter().all(reviewed_delta_operation) {
        return Err(Failure::observed(
            "delta",
            "LIVE_DELTA_REVIEWED_ALLOWLIST_FAILED",
            evidence,
        ));
    }
    evidence.delta_bound = true;
    let proposed = compat_json(
        bridge,
        AllowedHttpMethod::Post,
        &format!("/api/v1/agent/asset-versions/{}/change-sets", seed.base_asset_version_id),
        Some("live_delta_propose"),
        None,
        Some(json!({
            "client_request_id": "live_delta_propose",
            "summary": delta.get("summary").and_then(Value::as_str).unwrap_or("DeepSeek reviewed arm continuation"),
            "operations": operations,
        })),
        &[201],
    )
    .await
    .map_err(|_| Failure::observed("delta", "LIVE_DELTA_CHANGE_SET_REJECTED", evidence.clone()))?;
    let change_set_id = required_id(&proposed, "change_set_id").ok_or_else(|| {
        Failure::observed(
            "delta",
            "LIVE_DELTA_CHANGE_SET_ID_MISSING",
            evidence.clone(),
        )
    })?;
    compat_json(
        bridge,
        AllowedHttpMethod::Post,
        &format!("/api/v1/agent/change-sets/{change_set_id}:preview"),
        Some("live_delta_preview"),
        None,
        None,
        &[200],
    )
    .await
    .map_err(|_| Failure::observed("delta", "LIVE_DELTA_PREVIEW_REJECTED", evidence.clone()))?;
    let (response, bytes) = preview_binary(
        bridge,
        &format!("/api/v1/agent/change-sets/{change_set_id}:preview.glb"),
    )
    .await
    .map_err(|_| Failure::observed("delta", "LIVE_DELTA_PREVIEW_GLB_REJECTED", evidence.clone()))?;
    let preview_sha = header_value(&response, "X-ForgeCAD-GLB-SHA256").ok_or_else(|| {
        Failure::observed("delta", "LIVE_DELTA_PREVIEW_SHA_MISSING", evidence.clone())
    })?;
    let preview_triangles =
        header_u64(&response, "X-ForgeCAD-Triangle-Count").ok_or_else(|| {
            Failure::observed(
                "delta",
                "LIVE_DELTA_PREVIEW_TRIANGLES_MISSING",
                evidence.clone(),
            )
        })?;
    if sha256_hex(&bytes) != preview_sha || preview_triangles == 0 {
        return Err(Failure::observed(
            "delta",
            "LIVE_DELTA_PREVIEW_READBACK_INVALID",
            evidence,
        ));
    }
    evidence.preview_glb_sha256 = Some(preview_sha);
    evidence.preview_triangle_count = Some(preview_triangles);
    let confirmed = compat_json(
        bridge,
        AllowedHttpMethod::Post,
        &format!("/api/v1/agent/change-sets/{change_set_id}:confirm"),
        Some("live_delta_confirm"),
        None,
        None,
        &[200],
    )
    .await
    .map_err(|_| Failure::observed("delta", "LIVE_DELTA_CONFIRM_REJECTED", evidence.clone()))?;
    let asset_version = confirmed.get("asset_version").ok_or_else(|| {
        Failure::observed("delta", "LIVE_DELTA_VERSION_MISSING", evidence.clone())
    })?;
    let new_asset_version_id = required_id(asset_version, "asset_version_id").ok_or_else(|| {
        Failure::observed("delta", "LIVE_DELTA_VERSION_ID_MISSING", evidence.clone())
    })?;
    if asset_version
        .get("parent_asset_version_id")
        .and_then(Value::as_str)
        != Some(seed.base_asset_version_id.as_str())
    {
        return Err(Failure::observed(
            "delta",
            "LIVE_DELTA_LINEAGE_INVALID",
            evidence,
        ));
    }
    let active_after = compat_json(
        bridge,
        AllowedHttpMethod::Get,
        &format!("/api/v1/projects/{}/active-design", seed.project_id),
        None,
        None,
        None,
        &[200],
    )
    .await
    .map_err(|_| Failure::observed("delta", "LIVE_DELTA_ACTIVE_READ_REJECTED", evidence.clone()))?;
    if active_after
        .pointer("/active_design/asset_version_id")
        .and_then(Value::as_str)
        != Some(new_asset_version_id.as_str())
    {
        return Err(Failure::observed(
            "delta",
            "LIVE_DELTA_ACTIVE_HEAD_INVALID",
            evidence,
        ));
    }
    let export = compat_json(
        bridge,
        AllowedHttpMethod::Post,
        &format!("/api/v1/agent/asset-versions/{new_asset_version_id}:export"),
        Some("live_delta_export"),
        None,
        None,
        &[200],
    )
    .await
    .map_err(|_| Failure::observed("delta", "LIVE_DELTA_EXPORT_REJECTED", evidence.clone()))?;
    if export.get("asset_version_id").and_then(Value::as_str) != Some(new_asset_version_id.as_str())
        || export.get("glb_sha256").and_then(Value::as_str).is_none()
    {
        return Err(Failure::observed(
            "delta",
            "LIVE_DELTA_EXPORT_INVALID",
            evidence,
        ));
    }
    evidence.confirmed = true;
    evidence.asset_or_snapshot_writes = 1;
    Ok(DeltaReport {
        schema_version: SCHEMA_VERSION,
        status: "pass",
        execution_mode: "live_explicit_opt_in",
        run_id_sha256: config.run_id_sha256.clone(),
        provider_owner: "rust_desktop",
        credential_source: "rust_provider_credential_store",
        network_calls_made: u64::from(network_call_made),
        project_id: Some(seed.project_id.clone()),
        base_asset_version_id: Some(seed.base_asset_version_id.clone()),
        new_asset_version_id: Some(new_asset_version_id),
        change_set_id: Some(change_set_id),
        delta: evidence,
        restart: PhaseEvidence::not_run(),
        no_raw_prompt_or_response: true,
        no_key_or_provider_endpoint: true,
        error_phase: None,
        error_code: None,
    })
}

async fn run_resume(
    bridge: &AppServerBridge,
    config: &ProbeConfig,
    seed: &Seed,
) -> Result<DeltaReport, Failure> {
    let checkpoint: Value = serde_json::from_slice(
        &fs::read(&config.input).map_err(|_| Failure::before("restart", "LIVE_INPUT_INVALID"))?,
    )
    .map_err(|_| Failure::before("restart", "LIVE_INPUT_INVALID"))?;
    let expected = checkpoint
        .get("new_asset_version_id")
        .and_then(Value::as_str)
        .ok_or_else(|| Failure::before("restart", "LIVE_RESUME_ASSET_MISSING"))?;
    if expected == seed.base_asset_version_id {
        return Err(Failure::before("restart", "LIVE_RESUME_ASSET_NOT_ADVANCED"));
    }
    let active = compat_json(
        bridge,
        AllowedHttpMethod::Get,
        &format!("/api/v1/projects/{}/active-design", seed.project_id),
        None,
        None,
        None,
        &[200],
    )
    .await
    .map_err(|_| Failure::before("restart", "LIVE_RESUME_ACTIVE_READ_REJECTED"))?;
    if active
        .pointer("/active_design/asset_version_id")
        .and_then(Value::as_str)
        != Some(expected)
    {
        return Err(Failure::before("restart", "LIVE_RESUME_HEAD_DRIFT"));
    }
    let export = compat_json(
        bridge,
        AllowedHttpMethod::Post,
        &format!("/api/v1/agent/asset-versions/{expected}:export"),
        Some("live_delta_resume_export"),
        None,
        None,
        &[200],
    )
    .await
    .map_err(|_| Failure::before("restart", "LIVE_RESUME_EXPORT_REJECTED"))?;
    if export.get("asset_version_id").and_then(Value::as_str) != Some(expected)
        || export.get("glb_sha256").and_then(Value::as_str).is_none()
    {
        return Err(Failure::before("restart", "LIVE_RESUME_EXPORT_INVALID"));
    }
    let restart = PhaseEvidence {
        status: "pass",
        network_call_made: false,
        asset_or_snapshot_writes: 0,
        delta_bound: true,
        preview_glb_sha256: None,
        preview_triangle_count: export.get("triangle_count").and_then(Value::as_u64),
        confirmed: true,
        restarted: true,
        turn_status: None,
        turn_error_code: None,
        error_code: None,
    };
    Ok(DeltaReport {
        schema_version: SCHEMA_VERSION,
        status: "pass",
        execution_mode: "live_explicit_opt_in",
        run_id_sha256: config.run_id_sha256.clone(),
        provider_owner: "rust_desktop",
        credential_source: "rust_provider_credential_store",
        network_calls_made: 0,
        project_id: Some(seed.project_id.clone()),
        base_asset_version_id: Some(seed.base_asset_version_id.clone()),
        new_asset_version_id: Some(expected.to_string()),
        change_set_id: checkpoint
            .get("change_set_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        delta: PhaseEvidence::not_run(),
        restart,
        no_raw_prompt_or_response: true,
        no_key_or_provider_endpoint: true,
        error_phase: None,
        error_code: None,
    })
}

async fn create_thread(
    bridge: &AppServerBridge,
    project_id: &str,
    request_id: &str,
) -> Result<String, &'static str> {
    let value = native(
        bridge,
        request_id,
        "thread/create",
        json!({
            "schema_version": "AgentThreadCommand@1",
            "command_id": request_id,
            "command": {"operation":"create","request":{
                "client_request_id": request_id,
                "project_id": project_id,
                "title": "DeepSeek arm continuation",
                "provider_id": "deepseek"
            }}
        }),
    )
    .await
    .map_err(|_| "LIVE_THREAD_CREATE_REJECTED")?;
    value
        .pointer("/result/thread/thread_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or("LIVE_THREAD_ID_MISSING")
}

async fn start_turn(
    bridge: &AppServerBridge,
    thread_id: &str,
    request_id: &str,
) -> Result<(String, String, String), &'static str> {
    let value = native(
        bridge,
        request_id,
        "turn/start",
        json!({
            "schema_version": "AgentTurnCommand@1",
            "command_id": request_id,
            "command": {"operation":"start","thread_id":thread_id,"request":{
                "client_request_id": request_id,
                "message": CONTINUATION_BRIEF,
                "clarification_domain_pack_id": null
            }}
        }),
    )
    .await
    .map_err(|_| "LIVE_TURN_START_REJECTED")?;
    let result = value
        .get("result")
        .ok_or("LIVE_TURN_START_RESULT_MISSING")?;
    Ok((
        result
            .pointer("/turn/turn_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or("LIVE_TURN_ID_MISSING")?,
        result
            .get("cancellation_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or("LIVE_CANCELLATION_ID_MISSING")?,
        result
            .get("cancellation_token")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or("LIVE_CANCELLATION_TOKEN_MISSING")?,
    ))
}

fn extract_delta(turn: &Value) -> Option<Value> {
    turn.get("items")?.as_array()?.iter().find_map(|item| {
        let is_plan = item.get("item_type").and_then(Value::as_str) == Some("plan")
            || item.pointer("/payload/tool_name").and_then(Value::as_str)
                == Some("plan_complete_concept");
        if !is_plan {
            return None;
        }
        item.pointer("/payload/result/plan/assembly_delta")
            .cloned()
            .filter(|value| value.is_object())
    })
}

fn reviewed_delta_operation(operation: &Value) -> bool {
    let Some(op) = operation.get("op").and_then(Value::as_str) else {
        return false;
    };
    match op {
        "add_reviewed_recipe" => {
            matches!(
                operation.get("recipe_id").and_then(Value::as_str),
                Some(
                    "recipe_c106_arm_turntable"
                        | "recipe_c106_arm_joint_housing"
                        | "recipe_c106_arm_link_armor"
                        | "recipe_c106_arm_cable_harness"
                        | "recipe_c106_arm_gripper"
                        | "recipe_c106_arm_surface_trim"
                        | "recipe_c110c_arm_sensor_pod"
                        | "recipe_c110d_arm_actuator_cover"
                        | "recipe_c110d_arm_cable_guide"
                        | "recipe_c110d_arm_wrist_tool_mount"
                        | "recipe_c110g_parallel_rail"
                        | "recipe_c110g_parallel_carriage"
                        | "recipe_c110g_parallel_link"
                        | "recipe_c110g_parallel_end_effector"
                )
            ) && matches!(
                operation.get("slot_id").and_then(Value::as_str),
                Some(
                    "slot_arm_sensor_pod"
                        | "slot_arm_guard_rail"
                        | "slot_arm_tool_changer"
                        | "slot_arm_camera_boom"
                        | "slot_c110g_parallel_rail"
                        | "slot_c110g_parallel_carriage"
                        | "slot_c110g_parallel_link"
                        | "slot_c110g_parallel_tool"
                )
            )
        }
        "replace_reviewed_recipe" => {
            matches!(
                operation.get("recipe_id").and_then(Value::as_str),
                Some(
                    "recipe_c106_arm_turntable"
                        | "recipe_c106_arm_joint_housing"
                        | "recipe_c106_arm_link_armor"
                        | "recipe_c106_arm_cable_harness"
                        | "recipe_c106_arm_gripper"
                        | "recipe_c106_arm_surface_trim"
                        | "recipe_c110c_arm_sensor_pod"
                        | "recipe_c110d_arm_actuator_cover"
                        | "recipe_c110d_arm_cable_guide"
                        | "recipe_c110d_arm_wrist_tool_mount"
                        | "recipe_c110g_parallel_rail"
                        | "recipe_c110g_parallel_carriage"
                        | "recipe_c110g_parallel_link"
                        | "recipe_c110g_parallel_end_effector"
                )
            )
        }
        "set_part_transform" | "set_joint_pose" | "snap_part_to_connector" => true,
        _ => false,
    }
}

fn terminal_turn_status(turn: &Value) -> Option<&'static str> {
    match turn.get("status").and_then(Value::as_str) {
        Some("completed") => Some("completed"),
        Some("failed") => Some("failed"),
        Some("cancelled") => Some("cancelled"),
        _ => None,
    }
}

/// Project only stable Rust-owned terminal categories into the redacted
/// acceptance report. Provider response text, prompts and endpoint details
/// must never become diagnostic evidence.
fn terminal_turn_error_code(turn: &Value) -> Option<String> {
    let code = turn.get("error_code").and_then(Value::as_str)?;
    let stable = match code {
        "ACTION_LOOP_WALL_TIME_EXCEEDED" => "ACTION_LOOP_WALL_TIME_EXCEEDED",
        "ACTION_LOOP_TOTAL_TOKEN_BUDGET_EXCEEDED" => "ACTION_LOOP_TOTAL_TOKEN_BUDGET_EXCEEDED",
        "ACTION_LOOP_PRODUCT_TOOL_BUDGET_EXCEEDED" => "ACTION_LOOP_PRODUCT_TOOL_BUDGET_EXCEEDED",
        "ACTION_LOOP_PROVIDER_CAPABILITY_MISMATCH" => "ACTION_LOOP_PROVIDER_CAPABILITY_MISMATCH",
        "ACTION_LOOP_PROVIDER_FAILED" => "ACTION_LOOP_PROVIDER_FAILED",
        "ACTION_LOOP_CANCELLED" => "ACTION_LOOP_CANCELLED",
        "PRODUCT_TOOL_UNKNOWN" => "PRODUCT_TOOL_UNKNOWN",
        "PRODUCT_TOOL_INPUT_SCHEMA_INVALID" => "PRODUCT_TOOL_INPUT_SCHEMA_INVALID",
        "ARM_INTENT_ARCHITECTURE_UNSUPPORTED" => "ARM_INTENT_ARCHITECTURE_UNSUPPORTED",
        "ASSEMBLY_DELTA_INVALID" => "ASSEMBLY_DELTA_INVALID",
        _ if code.len() <= 96
            && code
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_') =>
        {
            code
        }
        _ => "TURN_ERROR_UNCLASSIFIED",
    };
    Some(stable.to_string())
}

async fn preview_binary(
    bridge: &AppServerBridge,
    path: &str,
) -> Result<(CompatHttpResponse, Vec<u8>), &'static str> {
    let response = bridge
        .execute_k003_packaged_compat(
            PreparedCompatHttpRequest {
                endpoint: LocalAgentEndpoint::parse(PROBE_ENDPOINT)
                    .map_err(|_| "LIVE_ENDPOINT_INVALID")?,
                method: AllowedHttpMethod::Get,
                path: path.into(),
                headers: Vec::new(),
                body: ProtocolHttpBody::Empty,
            },
            CancellationToken::new(),
        )
        .await
        .map_err(|_| "LIVE_PREVIEW_ROUTE_FAILED")?;
    if response.status != 200 {
        return Err("LIVE_PREVIEW_ROUTE_REJECTED");
    }
    let ProtocolHttpBody::Base64 { data } = &response.body else {
        return Err("LIVE_PREVIEW_BODY_INVALID");
    };
    let bytes = BASE64_STANDARD
        .decode(data)
        .map_err(|_| "LIVE_PREVIEW_BODY_INVALID")?;
    Ok((response, bytes))
}

fn header_u64(response: &CompatHttpResponse, name: &str) -> Option<u64> {
    response
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .and_then(|(_, value)| value.parse().ok())
}

fn header_value(response: &CompatHttpResponse, name: &str) -> Option<String> {
    response
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.clone())
}

fn sha256_hex(value: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value);
    format!("{:x}", hasher.finalize())
}
