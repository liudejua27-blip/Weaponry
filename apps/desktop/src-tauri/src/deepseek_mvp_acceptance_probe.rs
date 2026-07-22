//! Explicit, Rust-native DeepSeek MVP acceptance probe.
//!
//! This is deliberately separate from the deterministic mechanical-arm proof:
//! the normal application never enters this module, and a caller must opt in
//! with both a per-run confirmation and a caller-owned absolute report path.
//! The only credential source is the production Rust `ProviderCredentialStore`
//! already wired into `AppServerBridge`; this module never reads a Keychain,
//! environment API key, secret file, or Provider configuration itself.

use std::{env, fs, path::PathBuf, thread};

use forgecad_app_server::compatibility::AllowedHttpMethod;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    app_server_bridge::AppServerBridge,
    mvp_arm_packaged_probe::{compat_json, native, required_id, wait_terminal},
};

const ENABLE_FLAG: &str = "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE";
const CONFIRM_FLAG: &str = "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE_CONFIRM";
const RUN_ID_FLAG: &str = "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE_RUN_ID";
const OUTPUT_FLAG: &str = "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE_OUTPUT";
const SCHEMA_VERSION: &str = "ForgeCADDeepSeekMvpAcceptance@1";
const LIVE_CONFIRMATION: &str = "I_UNDERSTAND_THIS_MAY_INCUR_PROVIDER_COST";
const BRIEF: &str = "设计一台非功能性桌面维护机械臂概念资产：固定基座、双连杆、旋转腕部和夹爪，深色金属与蓝色点缀。";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbeConfig {
    run_id_hash: String,
    output: PathBuf,
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
    live_turn: PhaseEvidence,
    cancellation: PhaseEvidence,
    local_failure: PhaseEvidence,
    no_raw_prompt_or_response: bool,
    no_key_or_provider_endpoint: bool,
    /// A fixed phase label, never a server-supplied message or identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    error_phase: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
struct PhaseEvidence {
    status: &'static str,
    network_call_made: bool,
    asset_or_snapshot_writes: u64,
    /// Rust-owned proof that the completed plan contained a validated
    /// ArmDesignIntent@1 and that `plan_complete_concept` recorded its
    /// reviewed recipe lowering.  This is a boolean by design: the probe
    /// must never persist the provider's plan text or raw tool arguments.
    arm_intent_bound: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<&'static str>,
    /// Coarsened from the Rust-owned failure kind/code.  Never serialize the
    /// provider's message, endpoint, model, prompt, response, or trace.
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_category: Option<&'static str>,
}

impl PhaseEvidence {
    fn not_run() -> Self {
        Self {
            status: "not_run",
            network_call_made: false,
            asset_or_snapshot_writes: 0,
            arm_intent_bound: false,
            input_tokens: None,
            output_tokens: None,
            error_code: None,
            failure_category: None,
        }
    }
}

/// `run_if_enabled` must write a useful, redacted report even when one phase
/// fails.  A plain `Result<&str>` used to discard the observed terminal Turn,
/// making a real provider failure indistinguishable from "not run".
struct ProbeFailure {
    phase: &'static str,
    code: &'static str,
    evidence: PhaseEvidence,
}

impl ProbeFailure {
    fn before_terminal(phase: &'static str, code: &'static str) -> Self {
        Self {
            phase,
            code,
            evidence: PhaseEvidence {
                status: "failed_before_terminal",
                network_call_made: false,
                asset_or_snapshot_writes: 0,
                arm_intent_bound: false,
                input_tokens: None,
                output_tokens: None,
                error_code: None,
                failure_category: Some("native_protocol"),
            },
        }
    }

    fn observed(phase: &'static str, code: &'static str, evidence: PhaseEvidence) -> Self {
        Self {
            phase,
            code,
            evidence,
        }
    }
}

/// Normal launches return before reading any configuration or starting a
/// thread. Live execution requires all four values to be passed by the caller.
pub(crate) fn run_if_enabled(bridge: AppServerBridge) {
    let config = match parse_config(|name| env::var(name).ok()) {
        Ok(None) => return,
        Ok(Some(config)) => config,
        Err(_) => return,
    };
    let _ = thread::Builder::new()
        .name("forgecad-deepseek-mvp-acceptance".into())
        .spawn(move || {
            let report = run(bridge, &config).unwrap_or_else(|code| ProbeReport {
                schema_version: SCHEMA_VERSION,
                status: "fail",
                execution_mode: "live_explicit_opt_in",
                run_id_sha256: config.run_id_hash.clone(),
                provider_owner: "rust_desktop",
                credential_source: "rust_provider_credential_store",
                network_calls_made: 0,
                live_turn: PhaseEvidence::not_run(),
                cancellation: PhaseEvidence::not_run(),
                local_failure: PhaseEvidence::not_run(),
                no_raw_prompt_or_response: true,
                no_key_or_provider_endpoint: true,
                error_phase: Some("initialization"),
                error_code: Some(code),
            });
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
        return Err("LIVE_CONFIRMATION_REQUIRED");
    }
    let run_id = lookup(RUN_ID_FLAG).ok_or("LIVE_RUN_ID_REQUIRED")?;
    if !valid_run_id(&run_id) {
        return Err("LIVE_RUN_ID_INVALID");
    }
    let output = lookup(OUTPUT_FLAG)
        .map(PathBuf::from)
        .ok_or("LIVE_OUTPUT_REQUIRED")?;
    if !output.is_absolute() {
        return Err("LIVE_OUTPUT_INVALID");
    }
    Ok(Some(ProbeConfig {
        run_id_hash: sha256_hex(run_id.as_bytes()),
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

fn run(bridge: AppServerBridge, config: &ProbeConfig) -> Result<ProbeReport, &'static str> {
    if env::var("FORGECAD_MVP_OFFLINE_ARM").as_deref() == Ok("1") {
        return Err("LIVE_PROVIDER_DISABLED_BY_OFFLINE_MODE");
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "LIVE_RUNTIME_UNAVAILABLE")?;
    runtime.block_on(async move {
        let live_turn = match run_live_turn(&bridge).await {
            Ok(evidence) => evidence,
            Err(failure) => return Ok(failed_report(config, failure)),
        };
        let cancellation = match run_cancelled_turn(&bridge).await {
            Ok(evidence) => evidence,
            Err(failure) => return Ok(failed_report(config, failure)),
        };
        let local_failure = match run_local_failure(&bridge).await {
            Ok(evidence) => evidence,
            Err(failure) => return Ok(failed_report(config, failure)),
        };
        let network_calls_made = u64::from(live_turn.network_call_made)
            .saturating_add(u64::from(cancellation.network_call_made));
        Ok(ProbeReport {
            schema_version: SCHEMA_VERSION,
            status: "pass",
            execution_mode: "live_explicit_opt_in",
            run_id_sha256: config.run_id_hash.clone(),
            provider_owner: "rust_desktop",
            credential_source: "rust_provider_credential_store",
            network_calls_made,
            live_turn,
            cancellation,
            local_failure,
            no_raw_prompt_or_response: true,
            no_key_or_provider_endpoint: true,
            error_phase: None,
            error_code: None,
        })
    })
}

fn failed_report(config: &ProbeConfig, failure: ProbeFailure) -> ProbeReport {
    let mut report = ProbeReport {
        schema_version: SCHEMA_VERSION,
        status: "fail",
        execution_mode: "live_explicit_opt_in",
        run_id_sha256: config.run_id_hash.clone(),
        provider_owner: "rust_desktop",
        credential_source: "rust_provider_credential_store",
        network_calls_made: 0,
        live_turn: PhaseEvidence::not_run(),
        cancellation: PhaseEvidence::not_run(),
        local_failure: PhaseEvidence::not_run(),
        no_raw_prompt_or_response: true,
        no_key_or_provider_endpoint: true,
        error_phase: Some(failure.phase),
        error_code: Some(failure.code),
    };
    match failure.phase {
        "live_turn" => report.live_turn = failure.evidence,
        "cancellation" => report.cancellation = failure.evidence,
        "local_failure" => report.local_failure = failure.evidence,
        _ => unreachable!("ProbeFailure phase must be a fixed acceptance phase"),
    }
    report.network_calls_made = u64::from(report.live_turn.network_call_made)
        .saturating_add(u64::from(report.cancellation.network_call_made));
    report
}

async fn create_project(
    bridge: &AppServerBridge,
    request_id: &str,
) -> Result<String, &'static str> {
    let value = compat_json(
        bridge,
        AllowedHttpMethod::Post,
        "/api/v1/projects",
        Some(request_id),
        None,
        Some(json!({
            "client_request_id": request_id,
            "name": "DeepSeek acceptance transient project",
            "profile_id": "profile_weapon_concept_v1"
        })),
        &[200, 201],
    )
    .await
    .map_err(|_| "LIVE_PROJECT_CREATE_REJECTED")?;
    required_id(&value, "project_id").ok_or("LIVE_PROJECT_ID_MISSING")
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
                "title": "DeepSeek acceptance",
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
                "message": BRIEF,
                "clarification_domain_pack_id": null
            }}
        }),
    )
    .await
    .map_err(|_| "LIVE_TURN_START_REJECTED")?;
    let result = value
        .get("result")
        .ok_or("LIVE_TURN_START_RESULT_MISSING")?;
    let turn_id = result
        .pointer("/turn/turn_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or("LIVE_TURN_ID_MISSING")?;
    let cancellation_id = result
        .get("cancellation_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or("LIVE_CANCELLATION_ID_MISSING")?;
    let cancellation_token = result
        .get("cancellation_token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or("LIVE_CANCELLATION_TOKEN_MISSING")?;
    Ok((turn_id, cancellation_id, cancellation_token))
}

/// Read the project head without serialising it into the acceptance evidence.
/// A transient live Turn must not create either an active asset *or* an
/// ActiveDesignSnapshot revision before the user explicitly confirms V003.
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
    .map_err(|_| "LIVE_ACTIVE_DESIGN_READ_REJECTED")?;
    empty_snapshot_revision_from_value(&value)
}

fn empty_snapshot_revision_from_value(value: &Value) -> Result<u64, &'static str> {
    // Q002 deliberately represents a new project's zero-state as the stable
    // ACTIVE_DESIGN_NOT_FOUND response. That is the strongest possible proof
    // that no Snapshot or asset exists yet; only this exact 404 payload maps
    // to revision zero. Every other error remains fail-closed.
    if value.pointer("/error/code").and_then(Value::as_str) == Some("ACTIVE_DESIGN_NOT_FOUND") {
        return Ok(0);
    }
    if value.get("error").is_some() {
        return Err("LIVE_ACTIVE_DESIGN_READ_REJECTED");
    }
    if !value
        .pointer("/active_design/asset_version_id")
        .is_none_or(Value::is_null)
    {
        return Err("LIVE_ACTIVE_ASSET_SIDE_EFFECT");
    }
    value
        .get("revision")
        .and_then(Value::as_u64)
        .ok_or("LIVE_ACTIVE_DESIGN_REVISION_MISSING")
}

async fn run_live_turn(bridge: &AppServerBridge) -> Result<PhaseEvidence, ProbeFailure> {
    let project_id = create_project(bridge, "live_accept_project")
        .await
        .map_err(|code| ProbeFailure::before_terminal("live_turn", code))?;
    let thread_id = create_thread(bridge, &project_id, "live_accept_thread")
        .await
        .map_err(|code| ProbeFailure::before_terminal("live_turn", code))?;
    let revision_before = empty_snapshot_revision(bridge, &project_id)
        .await
        .map_err(|code| ProbeFailure::before_terminal("live_turn", code))?;
    let (turn_id, _, _) = start_turn(bridge, &thread_id, "live_accept_turn")
        .await
        .map_err(|code| ProbeFailure::before_terminal("live_turn", code))?;
    let turn = wait_terminal(bridge, &thread_id, &turn_id)
        .await
        .map_err(|_| ProbeFailure::before_terminal("live_turn", "LIVE_TURN_TIMEOUT"))?;
    let revision_after = empty_snapshot_revision(bridge, &project_id)
        .await
        .map_err(|code| ProbeFailure::before_terminal("live_turn", code))?;
    let evidence = observed_turn_evidence(&turn, revision_after != revision_before);
    if turn.get("status").and_then(Value::as_str) != Some("completed") {
        return Err(ProbeFailure::observed(
            "live_turn",
            "LIVE_TURN_NOT_EPHEMERAL_COMPLETION",
            evidence,
        ));
    }
    if revision_after != revision_before {
        return Err(ProbeFailure::observed(
            "live_turn",
            "LIVE_TURN_SNAPSHOT_SIDE_EFFECT",
            evidence,
        ));
    }
    let usage = turn.get("usage").unwrap_or(&Value::Null);
    if usage.get("network_call_made").and_then(Value::as_bool) != Some(true) {
        return Err(ProbeFailure::observed(
            "live_turn",
            "LIVE_TURN_NETWORK_ATTEMPT_MISSING",
            evidence,
        ));
    }
    if !evidence.arm_intent_bound {
        return Err(ProbeFailure::observed(
            "live_turn",
            "LIVE_TURN_ARM_INTENT_MISSING",
            evidence,
        ));
    }
    Ok(evidence)
}

async fn run_cancelled_turn(bridge: &AppServerBridge) -> Result<PhaseEvidence, ProbeFailure> {
    let project_id = create_project(bridge, "live_cancel_project")
        .await
        .map_err(|code| ProbeFailure::before_terminal("cancellation", code))?;
    let thread_id = create_thread(bridge, &project_id, "live_cancel_thread")
        .await
        .map_err(|code| ProbeFailure::before_terminal("cancellation", code))?;
    let revision_before = empty_snapshot_revision(bridge, &project_id)
        .await
        .map_err(|code| ProbeFailure::before_terminal("cancellation", code))?;
    let (turn_id, cancellation_id, cancellation_token) =
        start_turn(bridge, &thread_id, "live_cancel_turn")
            .await
            .map_err(|code| ProbeFailure::before_terminal("cancellation", code))?;
    let cancelled = native(
        bridge,
        "live_cancel_command",
        "turn/cancel",
        json!({
            "schema_version": "AgentTurnCommand@1",
            "command_id": "live_cancel_command",
            "command": {"operation":"cancel","thread_id":thread_id,"turn_id":turn_id,
                "cancellation_id":cancellation_id,"cancellation_token":cancellation_token}
        }),
    )
    .await
    .map_err(|_| ProbeFailure::before_terminal("cancellation", "LIVE_CANCEL_REJECTED"))?;
    if cancelled
        .pointer("/result/accepted")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err(ProbeFailure::before_terminal(
            "cancellation",
            "LIVE_CANCEL_NOT_ACCEPTED",
        ));
    }
    let turn = wait_terminal(bridge, &thread_id, &turn_id)
        .await
        .map_err(|_| ProbeFailure::before_terminal("cancellation", "LIVE_CANCEL_TIMEOUT"))?;
    let revision_after = empty_snapshot_revision(bridge, &project_id)
        .await
        .map_err(|code| ProbeFailure::before_terminal("cancellation", code))?;
    let evidence = observed_turn_evidence(&turn, revision_after != revision_before);
    if turn.get("status").and_then(Value::as_str) != Some("cancelled") {
        return Err(ProbeFailure::observed(
            "cancellation",
            "LIVE_CANCEL_TERMINAL_DRIFT",
            evidence,
        ));
    }
    if revision_after != revision_before {
        return Err(ProbeFailure::observed(
            "cancellation",
            "LIVE_CANCEL_SNAPSHOT_SIDE_EFFECT",
            evidence,
        ));
    }
    Ok(evidence)
}

async fn run_local_failure(bridge: &AppServerBridge) -> Result<PhaseEvidence, ProbeFailure> {
    let value = native(
        bridge,
        "live_failure_preflight",
        "provider/preflight",
        json!({
            "schema_version": "ProviderPreflightCommand@1",
            "execution_id": "live_failure_preflight",
            "requested_provider_id": "unsupported_provider"
        }),
    )
    .await
    .map_err(|_| ProbeFailure::before_terminal("local_failure", "LIVE_FAILURE_PROBE_REJECTED"))?;
    if value.get("status").and_then(Value::as_str) != Some("unconfigured")
        || value.get("network_call_made").and_then(Value::as_bool) != Some(false)
    {
        return Err(ProbeFailure::before_terminal(
            "local_failure",
            "LIVE_FAILURE_NOT_FAIL_CLOSED",
        ));
    }
    Ok(PhaseEvidence {
        status: "failed_closed",
        network_call_made: false,
        asset_or_snapshot_writes: 0,
        arm_intent_bound: false,
        input_tokens: None,
        output_tokens: None,
        error_code: Some("UNSUPPORTED_PROVIDER_REJECTED"),
        failure_category: Some("provider_configuration"),
    })
}

fn observed_turn_evidence(turn: &Value, snapshot_changed: bool) -> PhaseEvidence {
    let usage = turn.get("usage").unwrap_or(&Value::Null);
    let status = match turn.get("status").and_then(Value::as_str) {
        Some("completed") => "completed",
        Some("failed") => "failed",
        Some("cancelled") => "cancelled",
        _ => "terminal_unknown",
    };
    PhaseEvidence {
        status,
        network_call_made: usage
            .get("network_call_made")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        // The report deliberately exposes no IDs or mutable revisions.  Any
        // change from the confirmed zero-state is a write boundary, and one
        // is a conservative lower bound for this acceptance diagnostic.
        asset_or_snapshot_writes: u64::from(snapshot_changed),
        arm_intent_bound: turn_contains_bound_arm_intent(turn),
        input_tokens: usage.get("input_tokens").and_then(Value::as_u64),
        output_tokens: usage.get("output_tokens").and_then(Value::as_u64),
        error_code: safe_phase_error_code(turn),
        failure_category: failure_category(turn),
    }
}

/// Detect only the fixed, Rust-normalized plan evidence.  The provider's
/// original JSON is intentionally not copied into the report.  A plan counts
/// only when the persisted Plan item contains both ArmDesignIntent@1 and the
/// `lowered` result emitted by the Rust Product Tool executor.
fn turn_contains_bound_arm_intent(turn: &Value) -> bool {
    turn.get("items")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                let is_plan = item.get("item_type").and_then(Value::as_str) == Some("plan")
                    || item.pointer("/payload/tool_name").and_then(Value::as_str)
                        == Some("plan_complete_concept");
                if !is_plan {
                    return false;
                }
                let plan = item.pointer("/payload/result/plan");
                plan.and_then(|value| value.get("arm_design_intent"))
                    .and_then(Value::as_object)
                    .and_then(|intent| intent.get("schema_version"))
                    .and_then(Value::as_str)
                    == Some("ArmDesignIntent@1")
                    && plan
                        .and_then(|value| value.pointer("/arm_recipe_lowering/status"))
                        .and_then(Value::as_str)
                        == Some("lowered")
            })
        })
}

/// Project only reviewed Rust-owned Provider codes into the acceptance report.
/// Do not trust a prefix: Turn JSON is a protocol boundary and unknown values
/// must remain redacted even when they look like a Provider code.
fn safe_phase_error_code(turn: &Value) -> Option<&'static str> {
    let category = failure_category(turn);
    if category == Some("product_tool") || category == Some("product_tool_schema") {
        return safe_product_tool_code(
            turn.get("error_code")
                .and_then(Value::as_str)
                .or_else(|| turn.pointer("/usage/error_code").and_then(Value::as_str)),
        );
    }
    if category != Some("provider_execution") {
        return None;
    }
    match turn.get("error_code").and_then(Value::as_str) {
        Some("PROVIDER_INVALID_REQUEST") => Some("PROVIDER_INVALID_REQUEST"),
        Some("PROVIDER_AUTHENTICATION_FAILED") => Some("PROVIDER_AUTHENTICATION_FAILED"),
        Some("PROVIDER_BALANCE_REQUIRED") => Some("PROVIDER_BALANCE_REQUIRED"),
        Some("PROVIDER_RATE_LIMITED") => Some("PROVIDER_RATE_LIMITED"),
        Some("PROVIDER_SERVER_UNAVAILABLE") => Some("PROVIDER_SERVER_UNAVAILABLE"),
        Some("PROVIDER_TIMEOUT") => Some("PROVIDER_TIMEOUT"),
        Some("PROVIDER_TRANSPORT_FAILED") => Some("PROVIDER_TRANSPORT_FAILED"),
        Some("PROVIDER_EMPTY_CONTENT") => Some("PROVIDER_EMPTY_CONTENT"),
        Some("PROVIDER_EMPTY_JSON") => Some("PROVIDER_EMPTY_JSON"),
        Some("PROVIDER_INVALID_JSON") => Some("PROVIDER_INVALID_JSON"),
        Some("PROVIDER_SCHEMA_MISMATCH") => turn
            .get("error_message")
            .and_then(Value::as_str)
            .and_then(provider_schema_diagnostic_code)
            .or(Some("PROVIDER_SCHEMA_MISMATCH")),
        Some("PROVIDER_SCHEMA_RESPONSE_NOT_SSE") => Some("PROVIDER_SCHEMA_RESPONSE_NOT_SSE"),
        Some("PROVIDER_SCHEMA_RESPONSE_TOO_LARGE") => Some("PROVIDER_SCHEMA_RESPONSE_TOO_LARGE"),
        Some("PROVIDER_SCHEMA_SSE_LINE_TOO_LARGE") => Some("PROVIDER_SCHEMA_SSE_LINE_TOO_LARGE"),
        Some("PROVIDER_SCHEMA_SSE_EVENT_TOO_LARGE") => Some("PROVIDER_SCHEMA_SSE_EVENT_TOO_LARGE"),
        Some("PROVIDER_SCHEMA_SSE_FIELD_INVALID") => Some("PROVIDER_SCHEMA_SSE_FIELD_INVALID"),
        Some("PROVIDER_SCHEMA_SSE_DUPLICATE_DONE") => Some("PROVIDER_SCHEMA_SSE_DUPLICATE_DONE"),
        Some("PROVIDER_SCHEMA_SSE_DATA_AFTER_DONE") => Some("PROVIDER_SCHEMA_SSE_DATA_AFTER_DONE"),
        Some("PROVIDER_SCHEMA_SSE_OBJECT_INVALID") => Some("PROVIDER_SCHEMA_SSE_OBJECT_INVALID"),
        Some("PROVIDER_SCHEMA_MISSING_CHOICES") => Some("PROVIDER_SCHEMA_MISSING_CHOICES"),
        Some("PROVIDER_SCHEMA_MULTI_CHOICE") => Some("PROVIDER_SCHEMA_MULTI_CHOICE"),
        Some("PROVIDER_SCHEMA_USAGE_ORDER") => Some("PROVIDER_SCHEMA_USAGE_ORDER"),
        Some("PROVIDER_SCHEMA_DATA_AFTER_USAGE") => Some("PROVIDER_SCHEMA_DATA_AFTER_USAGE"),
        Some("PROVIDER_SCHEMA_CHOICE_INVALID") => Some("PROVIDER_SCHEMA_CHOICE_INVALID"),
        Some("PROVIDER_SCHEMA_FINISH_TYPE") => Some("PROVIDER_SCHEMA_FINISH_TYPE"),
        Some("PROVIDER_SCHEMA_FINISH_UNSUPPORTED") => Some("PROVIDER_SCHEMA_FINISH_UNSUPPORTED"),
        Some("PROVIDER_SCHEMA_FINISH_CONFLICT") => Some("PROVIDER_SCHEMA_FINISH_CONFLICT"),
        Some("PROVIDER_SCHEMA_MISSING_DELTA") => Some("PROVIDER_SCHEMA_MISSING_DELTA"),
        Some("PROVIDER_SCHEMA_CONTENT_TOO_LARGE") => Some("PROVIDER_SCHEMA_CONTENT_TOO_LARGE"),
        Some("PROVIDER_SCHEMA_REASONING_TOO_LARGE") => Some("PROVIDER_SCHEMA_REASONING_TOO_LARGE"),
        Some("PROVIDER_SCHEMA_TOOL_DELTA_ARRAY") => Some("PROVIDER_SCHEMA_TOOL_DELTA_ARRAY"),
        Some("PROVIDER_SCHEMA_TOOL_DELTA_TOO_MANY") => Some("PROVIDER_SCHEMA_TOOL_DELTA_TOO_MANY"),
        Some("PROVIDER_SCHEMA_TOOL_DELTA_INVALID") => Some("PROVIDER_SCHEMA_TOOL_DELTA_INVALID"),
        Some("PROVIDER_SCHEMA_TOOL_INDEX_INVALID") => Some("PROVIDER_SCHEMA_TOOL_INDEX_INVALID"),
        Some("PROVIDER_SCHEMA_TOOL_TYPE") => Some("PROVIDER_SCHEMA_TOOL_TYPE"),
        Some("PROVIDER_SCHEMA_TOOL_ID_TOO_LARGE") => Some("PROVIDER_SCHEMA_TOOL_ID_TOO_LARGE"),
        Some("PROVIDER_SCHEMA_TOOL_FUNCTION_INVALID") => {
            Some("PROVIDER_SCHEMA_TOOL_FUNCTION_INVALID")
        }
        Some("PROVIDER_SCHEMA_TOOL_NAME_TOO_LARGE") => Some("PROVIDER_SCHEMA_TOOL_NAME_TOO_LARGE"),
        Some("PROVIDER_SCHEMA_TOOL_ARGUMENTS_TOO_LARGE") => {
            Some("PROVIDER_SCHEMA_TOOL_ARGUMENTS_TOO_LARGE")
        }
        Some("PROVIDER_SCHEMA_TOOL_REQUIRED_FIELD") => Some("PROVIDER_SCHEMA_TOOL_REQUIRED_FIELD"),
        Some("PROVIDER_SCHEMA_TOOL_ARGUMENTS_OBJECT") => {
            Some("PROVIDER_SCHEMA_TOOL_ARGUMENTS_OBJECT")
        }
        Some("PROVIDER_SCHEMA_TOOL_ARGUMENTS_INVALID_JSON") => {
            Some("PROVIDER_SCHEMA_TOOL_ARGUMENTS_INVALID_JSON")
        }
        Some("PROVIDER_SCHEMA_TOOL_TOO_MANY") => Some("PROVIDER_SCHEMA_TOOL_TOO_MANY"),
        Some("PROVIDER_SCHEMA_REASONING_MISSING") => Some("PROVIDER_SCHEMA_REASONING_MISSING"),
        Some("PROVIDER_SCHEMA_DONE_MISSING") => Some("PROVIDER_SCHEMA_DONE_MISSING"),
        Some("PROVIDER_SCHEMA_USAGE_MISSING") => Some("PROVIDER_SCHEMA_USAGE_MISSING"),
        Some("PROVIDER_SCHEMA_FINISH_MISSING") => Some("PROVIDER_SCHEMA_FINISH_MISSING"),
        Some("PROVIDER_SCHEMA_USAGE_OBJECT") => Some("PROVIDER_SCHEMA_USAGE_OBJECT"),
        Some("PROVIDER_SCHEMA_USAGE_PROMPT_MISSING") => {
            Some("PROVIDER_SCHEMA_USAGE_PROMPT_MISSING")
        }
        Some("PROVIDER_SCHEMA_USAGE_COMPLETION_MISSING") => {
            Some("PROVIDER_SCHEMA_USAGE_COMPLETION_MISSING")
        }
        Some("PROVIDER_SCHEMA_USAGE_TOO_LARGE") => Some("PROVIDER_SCHEMA_USAGE_TOO_LARGE"),
        Some("PROVIDER_SCHEMA_USAGE_TOTAL_MISMATCH") => {
            Some("PROVIDER_SCHEMA_USAGE_TOTAL_MISMATCH")
        }
        Some("PROVIDER_SCHEMA_USAGE_CACHE_MISMATCH") => {
            Some("PROVIDER_SCHEMA_USAGE_CACHE_MISMATCH")
        }
        Some("PROVIDER_SCHEMA_USAGE_TYPE") => Some("PROVIDER_SCHEMA_USAGE_TYPE"),
        Some("PROVIDER_OUTPUT_TRUNCATED") => Some("PROVIDER_OUTPUT_TRUNCATED"),
        Some("PROVIDER_CONTENT_FILTERED") => Some("PROVIDER_CONTENT_FILTERED"),
        Some("PROVIDER_SYSTEM_RESOURCE_UNAVAILABLE") => {
            Some("PROVIDER_SYSTEM_RESOURCE_UNAVAILABLE")
        }
        Some("PROVIDER_CANCELLED") => Some("PROVIDER_CANCELLED"),
        _ => None,
    }
}

/// Product Tool failures are projected only from this fixed Rust-owned list.
/// This gives the acceptance probe actionable diagnostics without ever
/// copying an untrusted provider message or tool argument into its report.
fn safe_product_tool_code(code: Option<&str>) -> Option<&'static str> {
    match code {
        Some("ACTION_LOOP_DOMAIN_UNRESOLVED") => Some("ACTION_LOOP_DOMAIN_UNRESOLVED"),
        Some("ACTION_LOOP_DOMAIN_CONFLICT") => Some("ACTION_LOOP_DOMAIN_CONFLICT"),
        Some("CONCEPT_PLAN_MISSING") => Some("CONCEPT_PLAN_MISSING"),
        Some("CONCEPT_PLAN_DOMAIN_MISSING") => Some("CONCEPT_PLAN_DOMAIN_MISSING"),
        Some("CONCEPT_PLAN_DOMAIN_UNSUPPORTED") => Some("CONCEPT_PLAN_DOMAIN_UNSUPPORTED"),
        Some("CONCEPT_PLAN_BRIEF_MISSING") => Some("CONCEPT_PLAN_BRIEF_MISSING"),
        Some("CONCEPT_PLAN_GEOMETRY_STRATEGY_INVALID") => {
            Some("CONCEPT_PLAN_GEOMETRY_STRATEGY_INVALID")
        }
        Some("ARM_DESIGN_INTENT_INVALID") => Some("ARM_DESIGN_INTENT_INVALID"),
        Some("ARM_DESIGN_INTENT_REQUIRED") => Some("ARM_DESIGN_INTENT_REQUIRED"),
        Some("ARM_INTENT_ARCHITECTURE_UNSUPPORTED") => Some("ARM_INTENT_ARCHITECTURE_UNSUPPORTED"),
        Some("ARM_RECIPE_LOWERING_SERIALIZATION_FAILED") => {
            Some("ARM_RECIPE_LOWERING_SERIALIZATION_FAILED")
        }
        Some("ASSEMBLY_DELTA_INVALID") => Some("ASSEMBLY_DELTA_INVALID"),
        Some("ASSEMBLY_DELTA_NOT_ALLOWED_ON_INITIAL_SYNTHESIS") => {
            Some("ASSEMBLY_DELTA_NOT_ALLOWED_ON_INITIAL_SYNTHESIS")
        }
        Some("ASSEMBLY_DELTA_BASE_STALE") => Some("ASSEMBLY_DELTA_BASE_STALE"),
        Some("NATIVE_PRODUCT_TOOL_UNSUPPORTED") => Some("NATIVE_PRODUCT_TOOL_UNSUPPORTED"),
        Some("NATIVE_PRODUCT_TOOL_RESULT_INVALID") => Some("NATIVE_PRODUCT_TOOL_RESULT_INVALID"),
        Some("NATIVE_PRODUCT_TOOL_ARGUMENT_SCHEMA_INVALID") => {
            Some("NATIVE_PRODUCT_TOOL_ARGUMENT_SCHEMA_INVALID")
        }
        Some("NATIVE_PRODUCT_TOOL_CALL_LIMIT") => Some("NATIVE_PRODUCT_TOOL_CALL_LIMIT"),
        Some("PRODUCT_TOOL_EXECUTION_FAILED") => Some("PRODUCT_TOOL_EXECUTION_FAILED"),
        Some("PRODUCT_TOOL_OUTPUT_SERIALIZATION_FAILED") => {
            Some("PRODUCT_TOOL_OUTPUT_SERIALIZATION_FAILED")
        }
        Some("REVIEWED_CATALOG_DOMAIN_UNAVAILABLE") => Some("REVIEWED_CATALOG_DOMAIN_UNAVAILABLE"),
        Some("REVIEWED_RECIPE_EXPANSION_FAILED") => Some("REVIEWED_RECIPE_EXPANSION_FAILED"),
        Some("ARM_GEOMETRY_FAMILY_INVALID") => Some("ARM_GEOMETRY_FAMILY_INVALID"),
        _ => None,
    }
}

/// Map only Rust-owned fixed Provider messages to stable, redacted diagnostic
/// codes.  The message itself is never written to the acceptance report; an
/// unknown message deliberately falls back to the generic code.
fn provider_schema_diagnostic_code(message: &str) -> Option<&'static str> {
    match message {
        "Provider response was not a bounded SSE stream." => {
            Some("PROVIDER_SCHEMA_RESPONSE_NOT_SSE")
        }
        "Provider SSE chunk is missing choices." => Some("PROVIDER_SCHEMA_MISSING_CHOICES"),
        "Provider completion choice was invalid." => Some("PROVIDER_SCHEMA_CHOICE_INVALID"),
        "Provider completion choice is missing delta." => Some("PROVIDER_SCHEMA_MISSING_DELTA"),
        "Provider Tool Call delta was not an array." => Some("PROVIDER_SCHEMA_TOOL_DELTA_ARRAY"),
        "Provider Tool Call delta was invalid." => Some("PROVIDER_SCHEMA_TOOL_DELTA_INVALID"),
        "Provider Tool Call index was invalid." => Some("PROVIDER_SCHEMA_TOOL_INDEX_INVALID"),
        "Provider Tool Call function was invalid." => Some("PROVIDER_SCHEMA_TOOL_FUNCTION_INVALID"),
        "Provider Tool Call was missing a required field." => {
            Some("PROVIDER_SCHEMA_TOOL_REQUIRED_FIELD")
        }
        "Provider Tool Call arguments were not a JSON object." => {
            Some("PROVIDER_SCHEMA_TOOL_ARGUMENTS_OBJECT")
        }
        "Provider Tool Call arguments were not valid JSON." => {
            Some("PROVIDER_SCHEMA_TOOL_ARGUMENTS_INVALID_JSON")
        }
        "Provider SSE stream did not include final usage." => Some("PROVIDER_SCHEMA_USAGE_MISSING"),
        "Provider SSE stream did not include a finish reason." => {
            Some("PROVIDER_SCHEMA_FINISH_MISSING")
        }
        "Thinking-mode Provider Tool Calls omitted the required reasoning continuation." => {
            Some("PROVIDER_SCHEMA_REASONING_MISSING")
        }
        "Provider stop response unexpectedly included tool calls." => {
            Some("PROVIDER_SCHEMA_STOP_WITH_TOOLS")
        }
        _ => None,
    }
}

fn failure_category(turn: &Value) -> Option<&'static str> {
    let failure_kind = turn.pointer("/usage/failure_kind").and_then(Value::as_str);
    let error_code = turn.get("error_code").and_then(Value::as_str);
    match failure_kind {
        Some("provider") => Some("provider_execution"),
        Some("product_tool") => Some("product_tool"),
        Some("product_tool_schema") => Some("product_tool_schema"),
        Some("product_tool_budget") => Some("product_tool_budget"),
        Some("token_budget") => Some("token_budget"),
        Some("cost_budget") => Some("cost_budget"),
        Some("wall_time_budget") => Some("wall_time_budget"),
        Some("cancelled") => Some("cancelled"),
        Some("duplicate_tool_call") => Some("duplicate_tool_call"),
        Some("permanent_write_rejected") => Some("permanent_write_rejected"),
        Some("item_event_persistence") => Some("item_event_persistence"),
        Some("runtime") => Some("native_runtime"),
        _ if error_code.is_some_and(|code| code.starts_with("PROVIDER_")) => {
            Some("provider_preflight")
        }
        _ if error_code.is_some_and(|code| code.starts_with("ACTION_LOOP_PROVIDER_")) => {
            Some("provider_configuration")
        }
        _ if error_code == Some("GENERATION_SOURCE_BINDING_FAILED") => Some("native_runtime"),
        _ if turn.get("status").and_then(Value::as_str) == Some("failed") => {
            Some("unclassified_terminal_failure")
        }
        _ => None,
    }
}

fn sha256_hex(value: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_or_incomplete_live_probe_never_becomes_callable() {
        assert_eq!(parse_config(|_| None).unwrap(), None);
        let enabled = |name: &str| match name {
            ENABLE_FLAG => Some("1".into()),
            _ => None,
        };
        assert_eq!(parse_config(enabled), Err("LIVE_CONFIRMATION_REQUIRED"));
    }

    #[test]
    fn explicit_live_probe_hashes_run_id_and_requires_absolute_output() {
        let config = parse_config(|name| match name {
            ENABLE_FLAG => Some("1".into()),
            CONFIRM_FLAG => Some(LIVE_CONFIRMATION.into()),
            RUN_ID_FLAG => Some("live_acceptance_20260719".into()),
            OUTPUT_FLAG => Some("/tmp/forgecad-live-report.json".into()),
            _ => None,
        })
        .unwrap()
        .unwrap();
        assert_eq!(config.run_id_hash.len(), 64);
        assert!(!config.run_id_hash.contains("live_acceptance"));
        assert!(config.output.is_absolute());
        assert!(!valid_run_id("live_short"));
    }

    #[test]
    fn report_schema_has_no_secret_or_prompt_fields() {
        let report = ProbeReport {
            schema_version: SCHEMA_VERSION,
            status: "pass",
            execution_mode: "live_explicit_opt_in",
            run_id_sha256: "a".repeat(64),
            provider_owner: "rust_desktop",
            credential_source: "rust_provider_credential_store",
            network_calls_made: 0,
            live_turn: PhaseEvidence::not_run(),
            cancellation: PhaseEvidence::not_run(),
            local_failure: PhaseEvidence::not_run(),
            no_raw_prompt_or_response: true,
            no_key_or_provider_endpoint: true,
            error_phase: None,
            error_code: None,
        };
        let encoded = serde_json::to_string(&report).unwrap();
        for forbidden in ["api_key", "secret", "base_url", "model", BRIEF] {
            assert!(!encoded.contains(forbidden));
        }
    }

    #[test]
    fn failed_live_turn_preserves_only_safe_terminal_diagnostics() {
        let evidence = observed_turn_evidence(
            &json!({
                "status": "failed",
                "error_code": "PROVIDER_TRANSPORT_FAILURE",
                "error_message": "https://provider.example/v1/chat?key=secret-value",
                "usage": {
                    "network_call_made": true,
                    "input_tokens": 12,
                    "output_tokens": 0,
                    "failure_kind": "provider"
                },
                "request_text": BRIEF,
                "items": [{"payload": {"content": "untrusted response"}}]
            }),
            false,
        );
        assert_eq!(evidence.status, "failed");
        assert!(evidence.network_call_made);
        assert_eq!(evidence.failure_category, Some("provider_execution"));
        assert_eq!(evidence.error_code, None);
        assert_eq!(evidence.asset_or_snapshot_writes, 0);

        let config = ProbeConfig {
            run_id_hash: "a".repeat(64),
            output: PathBuf::from("/tmp/unused.json"),
        };
        let report = failed_report(
            &config,
            ProbeFailure::observed("live_turn", "LIVE_TURN_NOT_EPHEMERAL_COMPLETION", evidence),
        );
        let encoded = serde_json::to_string(&report).unwrap();
        assert!(encoded.contains("provider_execution"));
        for forbidden in [
            "PROVIDER_TRANSPORT_FAILURE",
            "provider.example",
            "secret-value",
            BRIEF,
            "untrusted response",
            "error_message",
            "request_text",
        ] {
            assert!(!encoded.contains(forbidden));
        }
    }

    #[test]
    fn failed_live_turn_projects_only_reviewed_provider_code() {
        let evidence = observed_turn_evidence(
            &json!({
                "status": "failed",
                "error_code": "PROVIDER_AUTHENTICATION_FAILED",
                "error_message": "https://provider.example/v1/chat?key=secret-value",
                "usage": {
                    "network_call_made": true,
                    "input_tokens": 0,
                    "output_tokens": 0,
                    "failure_kind": "provider"
                },
                "request_text": BRIEF,
                "items": [{"payload": {"content": "untrusted response"}}]
            }),
            false,
        );
        assert_eq!(evidence.error_code, Some("PROVIDER_AUTHENTICATION_FAILED"));
        let encoded = serde_json::to_string(&evidence).unwrap();
        assert!(encoded.contains("PROVIDER_AUTHENTICATION_FAILED"));
        for forbidden in [
            "provider.example",
            "secret-value",
            BRIEF,
            "untrusted response",
            "error_message",
            "request_text",
        ] {
            assert!(!encoded.contains(forbidden));
        }

        let non_provider = observed_turn_evidence(
            &json!({
                "status": "failed",
                "error_code": "PROVIDER_AUTHENTICATION_FAILED",
                "usage": {"failure_kind": "runtime", "network_call_made": false}
            }),
            false,
        );
        assert_eq!(non_provider.error_code, None);
    }

    #[test]
    fn schema_diagnostic_codes_are_fixed_and_messages_are_not_projected() {
        assert_eq!(
            provider_schema_diagnostic_code(
                "Thinking-mode Provider Tool Calls omitted the required reasoning continuation."
            ),
            Some("PROVIDER_SCHEMA_REASONING_MISSING")
        );
        assert_eq!(
            provider_schema_diagnostic_code("untrusted provider body"),
            None
        );
        let evidence = observed_turn_evidence(
            &json!({
                "status": "failed",
                "error_code": "PROVIDER_SCHEMA_MISMATCH",
                "error_message": "Provider SSE stream did not include final usage.",
                "usage": {"network_call_made": true, "failure_kind": "provider"}
            }),
            false,
        );
        assert_eq!(evidence.error_code, Some("PROVIDER_SCHEMA_USAGE_MISSING"));
        let encoded = serde_json::to_string(&evidence).unwrap();
        assert!(!encoded.contains("final usage"));
    }

    #[test]
    fn live_evidence_only_marks_rust_lowered_arm_intent() {
        let completed = json!({
            "status": "completed",
            "usage": {"network_call_made": true},
            "items": [{
                "item_type": "plan",
                "payload": {
                    "tool_name": "plan_complete_concept",
                    "result": {"accepted": true, "plan": {
                        "arm_design_intent": {
                            "schema_version": "ArmDesignIntent@1"
                        },
                        "arm_recipe_lowering": {"status": "lowered"}
                    }}
                }
            }]
        });
        assert!(turn_contains_bound_arm_intent(&completed));
        assert!(observed_turn_evidence(&completed, false).arm_intent_bound);

        let unlowered = json!({
            "status": "completed",
            "usage": {"network_call_made": true},
            "items": [{
                "item_type": "plan",
                "payload": {"result": {"plan": {
                    "arm_design_intent": {"schema_version": "ArmDesignIntent@1"}
                }}}
            }]
        });
        assert!(!turn_contains_bound_arm_intent(&unlowered));
        assert!(!observed_turn_evidence(&unlowered, false).arm_intent_bound);
    }

    #[test]
    fn empty_project_uses_stable_q002_zero_state_without_accepting_other_errors() {
        assert_eq!(
            empty_snapshot_revision_from_value(&json!({
                "error": {"code": "ACTIVE_DESIGN_NOT_FOUND", "message": "stable"}
            })),
            Ok(0),
        );
        assert_eq!(
            empty_snapshot_revision_from_value(&json!({
                "error": {"code": "PROJECT_NOT_FOUND", "message": "stable"}
            })),
            Err("LIVE_ACTIVE_DESIGN_READ_REJECTED"),
        );
        assert_eq!(
            empty_snapshot_revision_from_value(&json!({
                "revision": 0,
                "active_design": {"asset_version_id": null}
            })),
            Ok(0),
        );
        assert_eq!(
            empty_snapshot_revision_from_value(&json!({
                "revision": 1,
                "active_design": {"asset_version_id": "assetver_unexpected"}
            })),
            Err("LIVE_ACTIVE_ASSET_SIDE_EFFECT"),
        );
    }
}
