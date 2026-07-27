//! Explicit, live DeepSeek acceptance for the ForgeVisualProgram path.
//!
//! This probe is intentionally not an extension of either the deterministic
//! C111 packaged proof or the legacy arm-intent acceptance.  It proves the
//! current empty-project text route: a real Provider authors one bounded
//! `ForgeVisualProgram@1`, then Rust owns compilation, readback, eight-view
//! rendering, evaluation, single-result preparation, confirmation, Snapshot
//! creation, and byte-identical export.
//!
//! It is dormant unless a caller supplies *all* explicit opt-in values.  The
//! probe never opens a credential store or reads Provider configuration; that
//! remains exclusively inside the production bridge.

use std::{env, fs, path::PathBuf, thread};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use forgecad_app_server::compatibility::AllowedHttpMethod;
use forgecad_app_server_protocol::CompatHttpResponse;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    app_server_bridge::AppServerBridge,
    deepseek_mvp_acceptance_probe::{
        failure_category, safe_failed_tool_stage, safe_phase_error_code,
    },
    mvp_arm_packaged_probe::{
        compat_binary, compat_json, compat_json_response, native, preview_decision, required_id,
        wait_terminal,
    },
};

const ENABLE_FLAG: &str = "FORGECAD_DEEPSEEK_FORGE_VISUAL_ACCEPTANCE";
const CONFIRM_FLAG: &str = "FORGECAD_DEEPSEEK_FORGE_VISUAL_ACCEPTANCE_CONFIRM";
const RUN_ID_FLAG: &str = "FORGECAD_DEEPSEEK_FORGE_VISUAL_ACCEPTANCE_RUN_ID";
const OUTPUT_FLAG: &str = "FORGECAD_DEEPSEEK_FORGE_VISUAL_ACCEPTANCE_OUTPUT";
const SCHEMA_VERSION: &str = "ForgeCADDeepSeekForgeVisualAcceptance@1";
const LIVE_CONFIRMATION: &str = "I_UNDERSTAND_THIS_MAY_INCUR_PROVIDER_COST";
const BRIEF: &str = "设计一台非功能展示用未来维护机械臂概念资产：完整固定基座、两段装甲连杆、可见旋转关节、腕部与夹爪；深石墨金属、陶瓷嵌件、克制蓝色流线和表面警示细节。";

const REQUIRED_TOOL_STAGES: [&str; 6] = [
    "author_forge_visual_program",
    "build_candidate_geometry",
    "compile_readback_candidate",
    "render_candidate_views",
    "evaluate_candidate",
    "prepare_candidate_preview",
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbeConfig {
    run_id_sha256: String,
    output: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct VisualEvidence {
    status: &'static str,
    network_call_made: bool,
    author_forge_visual_program_completed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    author_source_mode: Option<&'static str>,
    rust_compile_readback_completed: bool,
    rust_eight_view_render_completed: bool,
    rust_evaluate_completed: bool,
    single_result_ready: bool,
    preview_hash_matches_bytes_and_header: bool,
    confirmed_asset_created: bool,
    snapshot_advanced: bool,
    export_hash_matches_bytes_json_and_header: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn_error_code: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_category: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failed_tool_stage: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_error_code_sha256: Option<String>,
    /// A finite, code-owned label extracted only from the loopback geometry
    /// boundary. It can never contain an arbitrary Provider operation id or
    /// any part of the authored program.
    #[serde(skip_serializing_if = "Option::is_none")]
    unsupported_runtime_operation: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    completed_tool_stages: Vec<&'static str>,
}

impl VisualEvidence {
    fn not_run() -> Self {
        Self {
            status: "not_run",
            network_call_made: false,
            author_forge_visual_program_completed: false,
            author_source_mode: None,
            rust_compile_readback_completed: false,
            rust_eight_view_render_completed: false,
            rust_evaluate_completed: false,
            single_result_ready: false,
            preview_hash_matches_bytes_and_header: false,
            confirmed_asset_created: false,
            snapshot_advanced: false,
            export_hash_matches_bytes_json_and_header: false,
            input_tokens: None,
            output_tokens: None,
            turn_error_code: None,
            failure_category: None,
            failed_tool_stage: None,
            tool_error_code_sha256: None,
            unsupported_runtime_operation: None,
            completed_tool_stages: Vec::new(),
        }
    }
}

#[derive(Serialize)]
struct ProbeReport {
    schema_version: &'static str,
    status: &'static str,
    execution_mode: &'static str,
    run_id_sha256: String,
    provider_owner: &'static str,
    credential_source: &'static str,
    network_calls_made: u64,
    visual_program_turn: VisualEvidence,
    no_raw_prompt_or_response: bool,
    no_key_or_provider_endpoint: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_phase: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<&'static str>,
}

struct ProbeFailure {
    code: &'static str,
    evidence: VisualEvidence,
}

impl ProbeFailure {
    fn before_terminal(code: &'static str) -> Self {
        Self {
            code,
            evidence: VisualEvidence {
                status: "failed_before_terminal",
                ..VisualEvidence::not_run()
            },
        }
    }

    fn observed(code: &'static str, evidence: VisualEvidence) -> Self {
        Self { code, evidence }
    }
}

/// Normal launches do not inspect any environment beyond this opt-in flag.
pub(crate) fn run_if_enabled(bridge: AppServerBridge) {
    let config = match parse_config(|name| env::var(name).ok()) {
        Ok(None) => return,
        Ok(Some(config)) => config,
        Err(_) => return,
    };
    let _ = thread::Builder::new()
        .name("forgecad-deepseek-forge-visual-acceptance".into())
        .spawn(move || {
            let report = run(bridge, &config)
                .unwrap_or_else(|code| failed_report(&config, ProbeFailure::before_terminal(code)));
            write_report(&config.output, &report);
        });
}

fn parse_config(
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<Option<ProbeConfig>, &'static str> {
    if lookup(ENABLE_FLAG).as_deref() != Some("1") {
        return Ok(None);
    }
    if lookup(CONFIRM_FLAG).as_deref() != Some(LIVE_CONFIRMATION) {
        return Err("FORGE_VISUAL_LIVE_CONFIRMATION_REQUIRED");
    }
    let run_id = lookup(RUN_ID_FLAG).ok_or("FORGE_VISUAL_LIVE_RUN_ID_REQUIRED")?;
    if !valid_run_id(&run_id) {
        return Err("FORGE_VISUAL_LIVE_RUN_ID_INVALID");
    }
    let output = lookup(OUTPUT_FLAG)
        .map(PathBuf::from)
        .ok_or("FORGE_VISUAL_LIVE_OUTPUT_REQUIRED")?;
    if !output.is_absolute() {
        return Err("FORGE_VISUAL_LIVE_OUTPUT_INVALID");
    }
    Ok(Some(ProbeConfig {
        run_id_sha256: sha256_hex(run_id.as_bytes()),
        output,
    }))
}

fn valid_run_id(value: &str) -> bool {
    value.len() >= 12
        && value.len() <= 80
        && value.starts_with("live_")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn write_report(output: &PathBuf, report: &ProbeReport) {
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

fn failed_report(config: &ProbeConfig, failure: ProbeFailure) -> ProbeReport {
    ProbeReport {
        schema_version: SCHEMA_VERSION,
        status: "fail",
        execution_mode: "live_explicit_opt_in",
        run_id_sha256: config.run_id_sha256.clone(),
        provider_owner: "rust_desktop",
        credential_source: "rust_provider_credential_store",
        network_calls_made: u64::from(failure.evidence.network_call_made),
        visual_program_turn: failure.evidence,
        no_raw_prompt_or_response: true,
        no_key_or_provider_endpoint: true,
        error_phase: Some("visual_program_turn"),
        error_code: Some(failure.code),
    }
}

fn run(bridge: AppServerBridge, config: &ProbeConfig) -> Result<ProbeReport, &'static str> {
    if env::var("FORGECAD_MVP_OFFLINE_ARM").as_deref() == Ok("1") {
        return Err("FORGE_VISUAL_LIVE_PROVIDER_DISABLED_BY_OFFLINE_MODE");
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "FORGE_VISUAL_LIVE_RUNTIME_UNAVAILABLE")?;
    runtime.block_on(async move {
        match run_visual_program_turn(&bridge).await {
            Ok(evidence) => Ok(ProbeReport {
                schema_version: SCHEMA_VERSION,
                status: "pass",
                execution_mode: "live_explicit_opt_in",
                run_id_sha256: config.run_id_sha256.clone(),
                provider_owner: "rust_desktop",
                credential_source: "rust_provider_credential_store",
                network_calls_made: u64::from(evidence.network_call_made),
                visual_program_turn: evidence,
                no_raw_prompt_or_response: true,
                no_key_or_provider_endpoint: true,
                error_phase: None,
                error_code: None,
            }),
            Err(failure) => Ok(failed_report(config, failure)),
        }
    })
}

async fn create_project(bridge: &AppServerBridge) -> Result<String, &'static str> {
    let value = compat_json(
        bridge,
        AllowedHttpMethod::Post,
        "/api/v1/projects",
        Some("forge_visual_live_project"),
        None,
        Some(json!({
            "client_request_id": "forge_visual_live_project",
            "name": "Live ForgeVisual acceptance transient project",
            "profile_id": "profile_weapon_concept_v1"
        })),
        &[200, 201],
    )
    .await
    .map_err(|_| "FORGE_VISUAL_LIVE_PROJECT_CREATE_REJECTED")?;
    required_id(&value, "project_id").ok_or("FORGE_VISUAL_LIVE_PROJECT_ID_MISSING")
}

async fn create_thread(bridge: &AppServerBridge, project_id: &str) -> Result<String, &'static str> {
    let value = native(
        bridge,
        "forge_visual_live_thread",
        "thread/create",
        json!({
            "schema_version": "AgentThreadCommand@1",
            "command_id": "forge_visual_live_thread",
            "command": {"operation":"create","request":{
                "client_request_id": "forge_visual_live_thread",
                "project_id": project_id,
                "title": "Live ForgeVisual acceptance",
                "provider_id": "deepseek"
            }}
        }),
    )
    .await
    .map_err(|_| "FORGE_VISUAL_LIVE_THREAD_CREATE_REJECTED")?;
    value
        .pointer("/result/thread/thread_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or("FORGE_VISUAL_LIVE_THREAD_ID_MISSING")
}

async fn start_turn(
    bridge: &AppServerBridge,
    thread_id: &str,
) -> Result<(String, String, String), &'static str> {
    let value = native(
        bridge,
        "forge_visual_live_turn",
        "turn/start",
        json!({
            "schema_version": "AgentTurnCommand@1",
            "command_id": "forge_visual_live_turn",
            "command": {"operation":"start","thread_id":thread_id,"request":{
                "client_request_id": "forge_visual_live_turn",
                "message": BRIEF,
                "clarification_domain_pack_id": null
            }}
        }),
    )
    .await
    .map_err(|_| "FORGE_VISUAL_LIVE_TURN_START_REJECTED")?;
    let result = value
        .get("result")
        .ok_or("FORGE_VISUAL_LIVE_TURN_RESULT_MISSING")?;
    let turn_id = result
        .pointer("/turn/turn_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or("FORGE_VISUAL_LIVE_TURN_ID_MISSING")?;
    let cancellation_id = result
        .get("cancellation_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or("FORGE_VISUAL_LIVE_CANCELLATION_ID_MISSING")?;
    let cancellation_token = result
        .get("cancellation_token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or("FORGE_VISUAL_LIVE_CANCELLATION_TOKEN_MISSING")?;
    Ok((turn_id, cancellation_id, cancellation_token))
}

async fn empty_snapshot_revision(
    bridge: &AppServerBridge,
    project_id: &str,
) -> Result<u64, &'static str> {
    let value = compat_json(
        bridge,
        AllowedHttpMethod::Get,
        &format!("/api/v1/projects/{project_id}/active-design"),
        None,
        None,
        None,
        &[200, 404],
    )
    .await
    .map_err(|_| "FORGE_VISUAL_LIVE_ACTIVE_DESIGN_READ_REJECTED")?;
    if value.pointer("/error/code").and_then(Value::as_str) == Some("ACTIVE_DESIGN_NOT_FOUND") {
        return Ok(0);
    }
    if value.get("error").is_some()
        || !value
            .pointer("/active_design/asset_version_id")
            .is_none_or(Value::is_null)
    {
        return Err("FORGE_VISUAL_LIVE_ACTIVE_DESIGN_NOT_EMPTY");
    }
    value
        .get("revision")
        .and_then(Value::as_u64)
        .ok_or("FORGE_VISUAL_LIVE_ACTIVE_DESIGN_REVISION_MISSING")
}

async fn run_visual_program_turn(bridge: &AppServerBridge) -> Result<VisualEvidence, ProbeFailure> {
    let project_id = create_project(bridge)
        .await
        .map_err(ProbeFailure::before_terminal)?;
    let thread_id = create_thread(bridge, &project_id)
        .await
        .map_err(ProbeFailure::before_terminal)?;
    let snapshot_before = empty_snapshot_revision(bridge, &project_id)
        .await
        .map_err(ProbeFailure::before_terminal)?;
    let (turn_id, _, _) = start_turn(bridge, &thread_id)
        .await
        .map_err(ProbeFailure::before_terminal)?;
    let turn = wait_terminal(bridge, &thread_id, &turn_id)
        .await
        .map_err(|_| ProbeFailure::before_terminal("FORGE_VISUAL_LIVE_TURN_TIMEOUT"))?;
    let mut evidence = observed_turn_evidence(&turn);
    if turn.get("status").and_then(Value::as_str) != Some("completed") {
        return Err(ProbeFailure::observed(
            "FORGE_VISUAL_LIVE_TURN_NOT_COMPLETED",
            evidence,
        ));
    }
    if !evidence.network_call_made {
        return Err(ProbeFailure::observed(
            "FORGE_VISUAL_LIVE_NETWORK_EVIDENCE_MISSING",
            evidence,
        ));
    }
    if !evidence.author_forge_visual_program_completed {
        return Err(ProbeFailure::observed(
            "FORGE_VISUAL_LIVE_AUTHOR_MISSING",
            evidence,
        ));
    }
    if !(evidence.rust_compile_readback_completed
        && evidence.rust_eight_view_render_completed
        && evidence.rust_evaluate_completed
        && evidence.single_result_ready)
    {
        return Err(ProbeFailure::observed(
            "FORGE_VISUAL_LIVE_RUST_COMPLETION_MISSING",
            evidence,
        ));
    }
    if empty_snapshot_revision(bridge, &project_id)
        .await
        .map_err(ProbeFailure::before_terminal)?
        != snapshot_before
    {
        return Err(ProbeFailure::observed(
            "FORGE_VISUAL_LIVE_PREVIEW_SIDE_EFFECT",
            evidence,
        ));
    }

    let decision = preview_decision(&turn).ok_or_else(|| {
        ProbeFailure::observed("FORGE_VISUAL_LIVE_SINGLE_RESULT_MISSING", evidence.clone())
    })?;
    let preview_id = decision
        .pointer("/preview/preview_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ProbeFailure::observed("FORGE_VISUAL_LIVE_PREVIEW_ID_MISSING", evidence.clone())
        })?;
    let artifact_sha = decision
        .pointer("/preview/artifact_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            ProbeFailure::observed("FORGE_VISUAL_LIVE_PREVIEW_SHA_MISSING", evidence.clone())
        })?;
    let etag = format!("\"sha256:{artifact_sha}\"");
    let base_path =
        format!("/api/v1/agent/projects/{project_id}/turns/{turn_id}/single-results/{preview_id}");
    let (preview_response, preview_bytes) =
        compat_binary(bridge, &format!("{base_path}:preview.glb"), Some(&etag))
            .await
            .map_err(|_| {
                ProbeFailure::observed("FORGE_VISUAL_LIVE_PREVIEW_READ_REJECTED", evidence.clone())
            })?;
    let preview_ok = sha256_hex(&preview_bytes) == artifact_sha
        && header_value(&preview_response, "X-ForgeCAD-GLB-SHA256") == Some(artifact_sha);
    evidence.preview_hash_matches_bytes_and_header = preview_ok;
    if !preview_ok {
        return Err(ProbeFailure::observed(
            "FORGE_VISUAL_LIVE_PREVIEW_HASH_INVALID",
            evidence,
        ));
    }

    let confirmed = compat_json(
        bridge,
        AllowedHttpMethod::Post,
        &format!("{base_path}:confirm"),
        Some("forge_visual_live_confirm"),
        Some(&etag),
        Some(json!({
            "client_request_id": "forge_visual_live_confirm",
            "expected_artifact_sha256": artifact_sha,
            "summary": "Explicit live ForgeVisual acceptance confirmation"
        })),
        &[201],
    )
    .await
    .map_err(|_| ProbeFailure::observed("FORGE_VISUAL_LIVE_CONFIRM_REJECTED", evidence.clone()))?;
    let asset_version_id = required_id(&confirmed, "asset_version_id").ok_or_else(|| {
        ProbeFailure::observed(
            "FORGE_VISUAL_LIVE_CONFIRM_VERSION_MISSING",
            evidence.clone(),
        )
    })?;
    evidence.confirmed_asset_created = true;

    let active = compat_json(
        bridge,
        AllowedHttpMethod::Get,
        &format!("/api/v1/projects/{project_id}/active-design"),
        None,
        None,
        None,
        &[200],
    )
    .await
    .map_err(|_| {
        ProbeFailure::observed("FORGE_VISUAL_LIVE_SNAPSHOT_READ_REJECTED", evidence.clone())
    })?;
    let snapshot_revision = active
        .get("revision")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    evidence.snapshot_advanced = snapshot_revision > snapshot_before
        && active
            .pointer("/active_design/asset_version_id")
            .and_then(Value::as_str)
            == Some(asset_version_id.as_str());
    if !evidence.snapshot_advanced {
        return Err(ProbeFailure::observed(
            "FORGE_VISUAL_LIVE_SNAPSHOT_DRIFT",
            evidence,
        ));
    }

    let (export_response, export) = compat_json_response(
        bridge,
        AllowedHttpMethod::Post,
        &format!("/api/v1/agent/asset-versions/{asset_version_id}:export"),
        Some("forge_visual_live_export"),
        None,
        None,
        &[200],
    )
    .await
    .map_err(|_| ProbeFailure::observed("FORGE_VISUAL_LIVE_EXPORT_REJECTED", evidence.clone()))?;
    let export_bytes = export
        .get("glb_base64")
        .and_then(Value::as_str)
        .and_then(|value| BASE64_STANDARD.decode(value).ok())
        .ok_or_else(|| {
            ProbeFailure::observed("FORGE_VISUAL_LIVE_EXPORT_BYTES_MISSING", evidence.clone())
        })?;
    let export_sha = required_id(&export, "glb_sha256")
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            ProbeFailure::observed("FORGE_VISUAL_LIVE_EXPORT_SHA_MISSING", evidence.clone())
        })?;
    evidence.export_hash_matches_bytes_json_and_header = sha256_hex(&export_bytes) == export_sha
        && export_sha == artifact_sha
        && header_value(&export_response, "X-ForgeCAD-GLB-SHA256") == Some(export_sha.as_str())
        && header_value(&export_response, "X-ForgeCAD-GLB-Byte-Size")
            .and_then(|value| value.parse::<usize>().ok())
            == Some(export_bytes.len());
    if !evidence.export_hash_matches_bytes_json_and_header {
        return Err(ProbeFailure::observed(
            "FORGE_VISUAL_LIVE_EXPORT_HASH_INVALID",
            evidence,
        ));
    }
    evidence.status = "completed";
    Ok(evidence)
}

fn observed_turn_evidence(turn: &Value) -> VisualEvidence {
    let completed_tool_stages = completed_tool_stages(turn);
    let has_stage = |stage| {
        completed_tool_stages
            .iter()
            .any(|candidate| candidate == &stage)
    };
    let usage = turn.get("usage").unwrap_or(&Value::Null);
    VisualEvidence {
        status: match turn.get("status").and_then(Value::as_str) {
            Some("completed") => "completed",
            Some("failed") => "failed",
            Some("cancelled") => "cancelled",
            _ => "terminal_unknown",
        },
        network_call_made: usage
            .get("network_call_made")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        author_forge_visual_program_completed: has_stage("author_forge_visual_program"),
        author_source_mode: author_source_mode(turn),
        rust_compile_readback_completed: has_stage("compile_readback_candidate"),
        rust_eight_view_render_completed: has_stage("render_candidate_views"),
        rust_evaluate_completed: has_stage("evaluate_candidate"),
        single_result_ready: has_stage("prepare_candidate_preview")
            && preview_decision(turn)
                .as_ref()
                .and_then(|value| value.get("schema_version"))
                .and_then(Value::as_str)
                == Some("SingleResultDecision@1"),
        preview_hash_matches_bytes_and_header: false,
        confirmed_asset_created: false,
        snapshot_advanced: false,
        export_hash_matches_bytes_json_and_header: false,
        input_tokens: usage.get("input_tokens").and_then(Value::as_u64),
        output_tokens: usage.get("output_tokens").and_then(Value::as_u64),
        turn_error_code: safe_phase_error_code(turn),
        failure_category: failure_category(turn),
        failed_tool_stage: safe_failed_tool_stage(turn),
        tool_error_code_sha256: failed_tool_error_code_sha256(turn),
        unsupported_runtime_operation: safe_unsupported_runtime_operation(turn),
        completed_tool_stages,
    }
}

fn author_source_mode(turn: &Value) -> Option<&'static str> {
    let program_id = turn
        .get("items")?
        .as_array()?
        .iter()
        .find(|item| {
            item.get("item_type").and_then(Value::as_str) == Some("tool_result")
                && item.get("status").and_then(Value::as_str) == Some("completed")
                && item.pointer("/payload/tool_name").and_then(Value::as_str)
                    == Some("author_forge_visual_program")
        })?
        .pointer("/payload/tool_result/validated_output/value/program_id")?
        .as_str()?;
    if program_id == "visualprog_multimodal_c111_fallback" {
        Some("reviewed_fallback")
    } else if program_id.starts_with("visualprog_provider_ir_") {
        Some("provider_authoring_ir")
    } else if !program_id.is_empty()
        && program_id.len() <= 128
        && program_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
    {
        Some("provider_program")
    } else {
        None
    }
}

fn failed_tool_error_code(turn: &Value) -> Option<&str> {
    turn.get("items")?
        .as_array()?
        .iter()
        .rev()
        .find_map(|item| {
            (item.get("status").and_then(Value::as_str) == Some("failed"))
                .then(|| {
                    item.pointer("/payload/error_code")
                        .and_then(Value::as_str)
                        .or_else(|| {
                            item.pointer("/payload/tool_result/error_code")
                                .and_then(Value::as_str)
                        })
                })
                .flatten()
        })
}

fn failed_tool_error_message(turn: &Value) -> Option<&str> {
    turn.get("items")?
        .as_array()?
        .iter()
        .rev()
        .find_map(|item| {
            (item.get("status").and_then(Value::as_str) == Some("failed"))
                .then(|| {
                    item.pointer("/payload/message")
                        .and_then(Value::as_str)
                        .or_else(|| {
                            item.pointer("/payload/tool_result/message")
                                .and_then(Value::as_str)
                        })
                })
                .flatten()
        })
}

/// The Provider and its authored JSON are untrusted at this boundary.  The
/// app bridge substitutes these exact, code-owned sentences only after
/// validating a finite loopback detail label.  Do not loosen this to parse an
/// operation token from a free-form message.
fn safe_unsupported_runtime_operation(turn: &Value) -> Option<&'static str> {
    if failed_tool_error_code(turn) != Some("UNSUPPORTED_RUNTIME_OPERATION") {
        return None;
    }
    match failed_tool_error_message(turn) {
        Some("The restricted geometry executor rejected the unsupported bevel operation.") => {
            Some("bevel")
        }
        Some("The restricted geometry executor rejected the unsupported boolean operation.") => {
            Some("boolean")
        }
        Some("The restricted geometry executor rejected the unsupported chamfer operation.") => {
            Some("chamfer")
        }
        Some("The restricted geometry executor rejected the unsupported cone operation.") => {
            Some("cone")
        }
        Some("The restricted geometry executor rejected the unsupported difference operation.") => {
            Some("difference")
        }
        Some("The restricted geometry executor rejected the unsupported fillet operation.") => {
            Some("fillet")
        }
        Some("The restricted geometry executor rejected the unsupported intersect operation.") => {
            Some("intersect")
        }
        Some(
            "The restricted geometry executor rejected the unsupported intersection operation.",
        ) => Some("intersection"),
        Some("The restricted geometry executor rejected the unsupported offset operation.") => {
            Some("offset")
        }
        Some("The restricted geometry executor rejected the unsupported plane operation.") => {
            Some("plane")
        }
        Some(
            "The restricted geometry executor rejected the unsupported rounded_box operation.",
        ) => Some("rounded_box"),
        Some("The restricted geometry executor rejected the unsupported shell operation.") => {
            Some("shell")
        }
        Some("The restricted geometry executor rejected the unsupported sphere operation.") => {
            Some("sphere")
        }
        Some("The restricted geometry executor rejected the unsupported torus operation.") => {
            Some("torus")
        }
        Some("The restricted geometry executor rejected the unsupported tube operation.") => {
            Some("tube")
        }
        _ => None,
    }
}

fn failed_tool_error_code_sha256(turn: &Value) -> Option<String> {
    let code = failed_tool_error_code(turn)?;
    if code.is_empty()
        || code.len() > 128
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return None;
    }
    Some(sha256_hex(code.as_bytes()))
}

fn completed_tool_stages(turn: &Value) -> Vec<&'static str> {
    REQUIRED_TOOL_STAGES
        .iter()
        .copied()
        .filter(|stage| {
            turn.get("items")
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    items.iter().any(|item| {
                        item.get("item_type").and_then(Value::as_str) == Some("tool_result")
                            && item.get("status").and_then(Value::as_str) == Some("completed")
                            && item.pointer("/payload/tool_name").and_then(Value::as_str)
                                == Some(*stage)
                    })
                })
        })
        .collect()
}

fn header_value<'a>(response: &'a CompatHttpResponse, name: &str) -> Option<&'a str> {
    response
        .headers
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opt_in_configuration_is_fail_closed_and_never_uses_a_relative_report() {
        assert_eq!(parse_config(|_| None).unwrap(), None);
        assert_eq!(
            parse_config(|name| match name {
                ENABLE_FLAG => Some("1".into()),
                _ => None,
            })
            .unwrap_err(),
            "FORGE_VISUAL_LIVE_CONFIRMATION_REQUIRED"
        );
        assert_eq!(
            parse_config(|name| match name {
                ENABLE_FLAG => Some("1".into()),
                CONFIRM_FLAG => Some(LIVE_CONFIRMATION.into()),
                RUN_ID_FLAG => Some("live_forge_visual_001".into()),
                OUTPUT_FLAG => Some("relative.json".into()),
                _ => None,
            })
            .unwrap_err(),
            "FORGE_VISUAL_LIVE_OUTPUT_INVALID"
        );
    }

    #[test]
    fn completion_evidence_accepts_only_rust_owned_visual_stages() {
        let turn = json!({
            "status":"completed",
            "usage":{"network_call_made":true,"input_tokens":12,"output_tokens":34},
            "items": REQUIRED_TOOL_STAGES.iter().map(|tool_name| json!({
                "item_type":"tool_result", "status":"completed", "payload":{"tool_name":tool_name}
            })).collect::<Vec<_>>()
        });
        let evidence = observed_turn_evidence(&turn);
        assert!(evidence.author_forge_visual_program_completed);
        assert!(evidence.rust_compile_readback_completed);
        assert!(evidence.rust_eight_view_render_completed);
        assert!(evidence.rust_evaluate_completed);
        assert!(
            !evidence.single_result_ready,
            "a tool name alone cannot forge a decision"
        );
    }

    #[test]
    fn author_source_mode_distinguishes_reviewed_fallback_without_exposing_program_content() {
        let turn = json!({
            "status":"completed",
            "usage":{"network_call_made":true},
            "items":[{
                "item_type":"tool_result",
                "status":"completed",
                "payload":{
                    "tool_name":"author_forge_visual_program",
                    "tool_result":{"validated_output":{"value":{
                        "program_id":"visualprog_multimodal_c111_fallback"
                    }}}
                }
            }]
        });
        assert_eq!(
            observed_turn_evidence(&turn).author_source_mode,
            Some("reviewed_fallback")
        );
        let mut provider_turn = turn;
        provider_turn["items"][0]["payload"]["tool_result"]["validated_output"]["value"]
            ["program_id"] = json!("visualprog_provider_original");
        assert_eq!(
            observed_turn_evidence(&provider_turn).author_source_mode,
            Some("provider_program")
        );
        provider_turn["items"][0]["payload"]["tool_result"]["validated_output"]["value"]
            ["program_id"] = json!("visualprog_provider_ir_0123456789abcdef01234567");
        assert_eq!(
            observed_turn_evidence(&provider_turn).author_source_mode,
            Some("provider_authoring_ir")
        );
    }

    #[test]
    fn report_serialization_has_no_prompt_or_provider_configuration_fields() {
        let report = failed_report(
            &ProbeConfig {
                run_id_sha256: "a".repeat(64),
                output: PathBuf::from("/tmp/report.json"),
            },
            ProbeFailure::before_terminal("FORGE_VISUAL_LIVE_TURN_TIMEOUT"),
        );
        let encoded = serde_json::to_string(&report).unwrap();
        for forbidden in ["prompt", "response", "api_key", "base_url", "model"] {
            assert!(!encoded.contains(&format!("\"{forbidden}\"")));
        }
    }

    #[test]
    fn nested_product_tool_failure_code_is_projected_through_the_fixed_allowlist() {
        let turn = json!({
            "status":"failed",
            "usage":{"network_call_made":true,"failure_kind":"product_tool"},
            "items":[{
                "item_type":"tool_result",
                "status":"failed",
                "payload":{
                    "tool_name":"author_forge_visual_program",
                    "tool_result":{"error_code":"FORGE_VISUAL_PROGRAM_INVALID"}
                }
            }]
        });
        let evidence = observed_turn_evidence(&turn);
        assert_eq!(
            evidence.turn_error_code,
            Some("FORGE_VISUAL_PROGRAM_INVALID")
        );
        assert_eq!(
            evidence.failed_tool_stage,
            Some("author_forge_visual_program")
        );
        assert_eq!(
            evidence.tool_error_code_sha256,
            Some(sha256_hex(b"FORGE_VISUAL_PROGRAM_INVALID"))
        );
    }

    #[test]
    fn unsupported_runtime_operation_is_projected_only_from_an_exact_bridge_message() {
        let turn = json!({
            "status":"failed",
            "usage":{"network_call_made":true,"failure_kind":"product_tool"},
            "items":[{
                "item_type":"tool_result",
                "status":"failed",
                "payload":{
                    "tool_name":"author_forge_visual_program",
                    "tool_result":{
                        "error_code":"UNSUPPORTED_RUNTIME_OPERATION",
                        "message":"The restricted geometry executor rejected the unsupported fillet operation."
                    }
                }
            }]
        });
        assert_eq!(
            observed_turn_evidence(&turn).unsupported_runtime_operation,
            Some("fillet")
        );

        let malicious = json!({
            "status":"failed",
            "usage":{"network_call_made":true,"failure_kind":"product_tool"},
            "items":[{
                "item_type":"tool_result",
                "status":"failed",
                "payload":{
                    "tool_name":"author_forge_visual_program",
                    "tool_result":{
                        "error_code":"UNSUPPORTED_RUNTIME_OPERATION",
                        "message":"The restricted geometry executor rejected the unsupported user_secret operation."
                    }
                }
            }]
        });
        assert_eq!(
            observed_turn_evidence(&malicious).unsupported_runtime_operation,
            None
        );
    }
}
