//! Opt-in packaged acceptance probe for the K003 Rust-owned product core.
//!
//! This module is not an API and is never reached by a normal desktop launch.
//! When the dedicated packaged smoke sets its exact environment contract, the
//! first launch creates a Project and runs build, segment and commit through
//! the exact production compatibility dispatcher. It then reads Snapshot,
//! Quality and both GLB profiles through the same routes. The restart phase
//! repeats the read-only route/CAS checks against caller-supplied identities.

use std::{env, sync::Arc, thread};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use forgecad_app_server::{
    compatibility::{AllowedHttpMethod, LocalAgentEndpoint, PreparedCompatHttpRequest},
    CancellationToken,
};
use forgecad_app_server_protocol::{CompatHttpResponse, ProtocolHttpBody};
use forgecad_core::{
    semantic_sha256, verify_forgecad_glb, ActiveDesign, AssetVersionStatus, ExportReference,
    ObjectReference, Project,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    app_server_bridge::AppServerBridge, append_supervisor_log, rust_core_runtime::RustCoreRuntime,
};

pub(crate) const K003_PACKAGED_PROBE_SCHEMA: &str = "ForgeCADK003PackagedCoreProbe@1";
pub(crate) const K003_PACKAGED_PROBE_MARKER: &str =
    "ForgeCAD K003 packaged Rust core probe report=";
pub(crate) const K003_EXECUTION_PATH: &str =
    "compat_project_create>compat_blockout_build>compat_blockout_segment>compat_blockout_commit>compat_snapshot_quality_glb_readback>compat_render_package_readback";

const PROBE_FLAG: &str = "FORGECAD_K003_PACKAGED_PROBE";
const PROBE_PHASE: &str = "FORGECAD_K003_PACKAGED_PROBE_PHASE";
const PROBE_ENDPOINT: &str = "http://127.0.0.1:1";
const CREATE_REQUEST_ID: &str = "create_k003_packaged_probe";
const BUILD_REQUEST_ID: &str = "build_k003_packaged_probe";
const SEGMENT_REQUEST_ID: &str = "segment_k003_packaged_probe";
const COMMIT_REQUEST_ID: &str = "commit_k003_packaged_probe";
const PLAN_ID: &str = "plan_k003_packaged_probe";
const DIRECTION_ID: &str = "direction_primary";
const DOMAIN_PACK_ID: &str = "pack_future_weapon_prop";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbePhase {
    Initial,
    Restart,
}

impl ProbePhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::Restart => "restart",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct K003ProbeExpected {
    project_id: String,
    asset_version_id: String,
    snapshot_etag: String,
    project_semantic_sha256: String,
    snapshot_semantic_sha256: String,
    glb_sha256: String,
    render_set_sha256: String,
    render_package_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct K003ProbeConfig {
    phase: ProbePhase,
    expected: Option<K003ProbeExpected>,
}

#[derive(Debug, Clone, Serialize)]
struct K003ProbeReport {
    schema_version: &'static str,
    phase: &'static str,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    asset_version_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot_etag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_semantic_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot_semantic_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    glb_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    render_set_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    render_package_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_calls: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_network_call_made: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_path: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostic: Option<ProbeDiagnostic>,
}

/// Bounded route-level evidence for a failed packaged probe. Request and
/// response bodies, messages, secrets and prompt content are never recorded.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ProbeDiagnostic {
    method: String,
    route: String,
    http_status: Option<u16>,
    stable_error_code: String,
    startup_phase: &'static str,
    correlation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct K003CoreProbeFacts {
    pub(crate) project_id: String,
    pub(crate) asset_version_id: String,
    pub(crate) snapshot_etag: String,
    pub(crate) project_semantic_sha256: String,
    pub(crate) snapshot_semantic_sha256: String,
    pub(crate) glb_sha256: String,
    pub(crate) render_set_sha256: String,
    pub(crate) render_package_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeFailure {
    code: &'static str,
    diagnostic: Option<ProbeDiagnostic>,
}

impl ProbeFailure {
    const fn new(code: &'static str) -> Self {
        Self {
            code,
            diagnostic: None,
        }
    }

    fn from_response(
        code: &'static str,
        phase: ProbePhase,
        method: AllowedHttpMethod,
        path: &str,
        correlation_id: impl Into<String>,
        response: &CompatHttpResponse,
    ) -> Self {
        Self {
            code,
            diagnostic: Some(ProbeDiagnostic {
                method: method.as_str().to_string(),
                route: normalized_route(path),
                http_status: Some(response.status),
                stable_error_code: response_error_code(response)
                    .unwrap_or_else(|| format!("HTTP_STATUS_{}", response.status)),
                startup_phase: phase.as_str(),
                correlation_id: correlation_id.into(),
            }),
        }
    }
}

/// Executes at most one explicitly enabled probe and always emits one bounded
/// report for enabled runs. A disabled normal launch performs no reads, native
/// Product Tool calls, geometry work, or product-state mutations here.
pub(crate) fn run_if_enabled(bridge: AppServerBridge, core: Arc<RustCoreRuntime>) {
    let phase_for_failure = env::var(PROBE_PHASE)
        .ok()
        .filter(|value| matches!(value.as_str(), "initial" | "restart"))
        .unwrap_or_else(|| "initial".to_string());
    let config = match parse_config(|name| env::var(name).ok()) {
        Ok(Some(config)) => config,
        Ok(None) => return,
        Err(failure) => {
            append_report(K003ProbeReport::failure(
                if phase_for_failure == "restart" {
                    ProbePhase::Restart
                } else {
                    ProbePhase::Initial
                },
                failure.code,
                failure.diagnostic,
            ));
            return;
        }
    };

    // The probe exercises the real Rust compatibility routes and may invoke
    // the disposable geometry worker. Keep it outside Tauri setup so the
    // app-server/WebView readiness marker is not held behind acceptance work.
    let phase = config.phase;
    let probe = thread::Builder::new()
        .name("forgecad-k003-packaged-probe-runner".into())
        .spawn(move || {
            crate::wait_for_k001_packaged_probe_if_enabled();
            crate::wait_for_k002_packaged_probe_if_enabled();
            let result = run_routes_on_isolated_runtime(bridge, Arc::clone(&core), &config)
                .and_then(|facts| {
                    if let Some(expected) = config.expected.as_ref() {
                        validate_expected(&facts, expected)?;
                    }
                    Ok(facts)
                });
            match result {
                Ok(facts) => append_report(K003ProbeReport::success(phase, facts)),
                Err(failure) => append_report(K003ProbeReport::failure(
                    phase,
                    failure.code,
                    failure.diagnostic,
                )),
            }
        });
    if probe.is_err() {
        append_report(K003ProbeReport::failure(
            phase,
            "NATIVE_PREVIEW_RUNTIME_FAILED",
            None,
        ));
    }
}

fn run_routes_on_isolated_runtime(
    bridge: AppServerBridge,
    core: Arc<RustCoreRuntime>,
    config: &K003ProbeConfig,
) -> Result<K003CoreProbeFacts, ProbeFailure> {
    let config = config.clone();
    thread::Builder::new()
        .name("forgecad-k003-packaged-probe".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| ProbeFailure::new("NATIVE_PREVIEW_RUNTIME_FAILED"))?;
            runtime.block_on(async move {
                match config.phase {
                    ProbePhase::Initial => run_initial_compat_flow(&bridge, core.as_ref()).await,
                    ProbePhase::Restart => {
                        run_restart_compat_flow(
                            &bridge,
                            core.as_ref(),
                            config
                                .expected
                                .as_ref()
                                .expect("restart config always has expected facts"),
                        )
                        .await
                    }
                }
            })
        })
        .map_err(|_| ProbeFailure::new("NATIVE_PREVIEW_RUNTIME_FAILED"))?
        .join()
        .map_err(|_| ProbeFailure::new("NATIVE_PREVIEW_RUNTIME_FAILED"))?
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompatReadbackFacts {
    snapshot_etag: String,
    production_glb_sha256: String,
    render_set_sha256: String,
    render_package_sha256: String,
}

async fn run_initial_compat_flow(
    bridge: &AppServerBridge,
    core: &RustCoreRuntime,
) -> Result<K003CoreProbeFacts, ProbeFailure> {
    let (_, project) = execute_json(
        ProbePhase::Initial,
        bridge,
        AllowedHttpMethod::Post,
        "/api/v1/projects",
        Some(CREATE_REQUEST_ID),
        Some(json!({
            "client_request_id": CREATE_REQUEST_ID,
            "name": "K003 packaged production concept probe",
            "profile_id": "profile_weapon_concept_v1"
        })),
        &[200, 201],
    )
    .await?;
    let project_id = required_response_id(&project, "project_id")?;

    let direction = |direction_id: &str, title: &str| {
        json!({
            "direction_id": direction_id,
            "title": title,
            "summary": "Complete non-functional exterior concept.",
            "silhouette": "compact",
            "primary_part_roles": ["primary_form", "secondary_form"],
            "material_direction": "dark metal and bounded visual coating"
        })
    };
    let plan = json!({
        "schema_version": "MechanicalConceptPlan@1",
        "plan_id": PLAN_ID,
        "domain_pack_id": DOMAIN_PACK_ID,
        "brief": "non-functional future game prop production concept",
        "generation_stage": "blockout",
        "spec": {"project_id": project_id},
        "directions": [
            direction(DIRECTION_ID, "Primary")
        ],
        "provider_id": "rust_app_server",
        "shape_program_ready": false
    });
    let (_, built) = execute_json(
        ProbePhase::Initial,
        bridge,
        AllowedHttpMethod::Post,
        "/api/v1/agent/blockouts",
        Some(BUILD_REQUEST_ID),
        Some(json!({
            "client_request_id": BUILD_REQUEST_ID,
            "plan": plan,
            "direction_id": DIRECTION_ID,
            "variation_index": 0,
            "presentation_profile": "quick_sketch"
        })),
        &[200],
    )
    .await?;
    let artifact_id = required_response_id(&built, "artifact_id")?;
    let variant_id = required_response_id(&built, "variant_id")?;
    if built.get("shape_program").is_none()
        || built.get("glb_base64").and_then(Value::as_str).is_none()
    {
        return Err(ProbeFailure::new("COMPAT_BLOCKOUT_BUILD_INVALID"));
    }

    let (_, segmented) = execute_json(
        ProbePhase::Initial,
        bridge,
        AllowedHttpMethod::Post,
        "/api/v1/agent/blockouts:segment",
        Some(SEGMENT_REQUEST_ID),
        Some(json!({
            "client_request_id": SEGMENT_REQUEST_ID,
            "plan": plan,
            "direction_id": DIRECTION_ID,
            "variant_id": variant_id,
            "variation_index": 0,
            "presentation_profile": "quick_sketch",
            "artifact_id": artifact_id
        })),
        &[200],
    )
    .await?;
    if segmented.get("segmentation_status").and_then(Value::as_str) != Some("candidate")
        || segmented
            .get("parts")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    {
        return Err(ProbeFailure::new("COMPAT_BLOCKOUT_SEGMENT_INVALID"));
    }

    let (_, committed) = execute_json(
        ProbePhase::Initial,
        bridge,
        AllowedHttpMethod::Post,
        "/api/v1/agent/blockouts:commit",
        Some(COMMIT_REQUEST_ID),
        Some(json!({
            "client_request_id": COMMIT_REQUEST_ID,
            "artifact_id": artifact_id,
            "project_id": project_id,
            "summary": "Non-functional future game prop production concept"
        })),
        &[201],
    )
    .await?;
    let asset_version_id = required_response_id(&committed, "asset_version_id")?;
    if committed.get("project_id").and_then(Value::as_str) != Some(project_id.as_str())
        || committed.get("version_no").and_then(Value::as_u64) != Some(1)
    {
        return Err(ProbeFailure::new("COMPAT_BLOCKOUT_COMMIT_INVALID"));
    }

    let route_facts =
        validate_compat_readback(ProbePhase::Initial, bridge, &project_id, &asset_version_id)
            .await?;
    let mut facts = read_facts(core, &project_id, &asset_version_id)?;
    if route_facts.snapshot_etag != facts.snapshot_etag
        || route_facts.production_glb_sha256 != facts.glb_sha256
    {
        return Err(ProbeFailure::new("COMPAT_CORE_READBACK_MISMATCH"));
    }
    facts.render_set_sha256 = route_facts.render_set_sha256;
    facts.render_package_sha256 = route_facts.render_package_sha256;
    Ok(facts)
}

async fn run_restart_compat_flow(
    bridge: &AppServerBridge,
    core: &RustCoreRuntime,
    expected: &K003ProbeExpected,
) -> Result<K003CoreProbeFacts, ProbeFailure> {
    let route_facts = validate_compat_readback(
        ProbePhase::Restart,
        bridge,
        &expected.project_id,
        &expected.asset_version_id,
    )
    .await?;
    if route_facts.snapshot_etag != expected.snapshot_etag
        || route_facts.production_glb_sha256 != expected.glb_sha256
        || route_facts.render_set_sha256 != expected.render_set_sha256
        || route_facts.render_package_sha256 != expected.render_package_sha256
    {
        return Err(ProbeFailure::new("PROBE_EXPECTATION_MISMATCH"));
    }
    let mut facts = read_facts(core, &expected.project_id, &expected.asset_version_id)?;
    if route_facts.snapshot_etag != facts.snapshot_etag
        || route_facts.production_glb_sha256 != facts.glb_sha256
    {
        return Err(ProbeFailure::new("COMPAT_CORE_READBACK_MISMATCH"));
    }
    facts.render_set_sha256 = route_facts.render_set_sha256;
    facts.render_package_sha256 = route_facts.render_package_sha256;
    Ok(facts)
}

async fn validate_compat_readback(
    phase: ProbePhase,
    bridge: &AppServerBridge,
    project_id: &str,
    asset_version_id: &str,
) -> Result<CompatReadbackFacts, ProbeFailure> {
    let (_, project) = execute_json(
        phase,
        bridge,
        AllowedHttpMethod::Get,
        &format!("/api/v1/projects/{project_id}"),
        None,
        None,
        &[200],
    )
    .await?;
    if project.get("project_id").and_then(Value::as_str) != Some(project_id) {
        return Err(ProbeFailure::new("COMPAT_PROJECT_READBACK_INVALID"));
    }

    let (_, version) = execute_json(
        phase,
        bridge,
        AllowedHttpMethod::Get,
        &format!("/api/v1/agent/asset-versions/{asset_version_id}"),
        None,
        None,
        &[200],
    )
    .await?;
    if version.get("asset_version_id").and_then(Value::as_str) != Some(asset_version_id)
        || version.get("project_id").and_then(Value::as_str) != Some(project_id)
        || version.get("status").and_then(Value::as_str) != Some("committed")
    {
        return Err(ProbeFailure::new("COMPAT_VERSION_READBACK_INVALID"));
    }

    let (snapshot_response, snapshot) = execute_json(
        phase,
        bridge,
        AllowedHttpMethod::Get,
        &format!("/api/v1/projects/{project_id}/active-design"),
        None,
        None,
        &[200],
    )
    .await?;
    let snapshot_etag = response_header(&snapshot_response, "etag")
        .map(str::to_string)
        .ok_or_else(|| ProbeFailure::new("COMPAT_SNAPSHOT_READBACK_INVALID"))?;
    let active_version = snapshot
        .pointer("/active_design/asset_version_id")
        .and_then(Value::as_str);
    let export_version = snapshot
        .pointer("/export/source_version_id")
        .and_then(Value::as_str);
    let quality_report_id = snapshot
        .pointer("/quality/quality_report_id")
        .and_then(Value::as_str)
        .filter(|value| is_stable_probe_id(value))
        .map(str::to_string)
        .ok_or_else(|| ProbeFailure::new("COMPAT_SNAPSHOT_READBACK_INVALID"))?;
    if snapshot.get("project_id").and_then(Value::as_str) != Some(project_id)
        || active_version != Some(asset_version_id)
        || export_version != Some(asset_version_id)
        || snapshot
            .pointer("/quality/asset_version_id")
            .and_then(Value::as_str)
            != Some(asset_version_id)
    {
        return Err(ProbeFailure::new("COMPAT_SNAPSHOT_READBACK_INVALID"));
    }

    let (preview_response, preview_bytes) = execute_binary(
        phase,
        bridge,
        &format!("/api/v1/agent/asset-versions/{asset_version_id}:preview.glb"),
    )
    .await?;
    let preview_readback = verify_forgecad_glb(&preview_bytes, Some("interactive_preview"))
        .map_err(|_| ProbeFailure::new("COMPAT_PREVIEW_GLB_INVALID"))?;
    validate_artifact_headers(
        &preview_response,
        &preview_readback.glb_sha256,
        "interactive_preview",
    )?;

    let (production_response, production_bytes) = execute_binary(
        phase,
        bridge,
        &format!("/api/v1/agent/asset-versions/{asset_version_id}:model.glb"),
    )
    .await?;
    let production_readback = verify_forgecad_glb(&production_bytes, Some("production_concept"))
        .map_err(|_| ProbeFailure::new("COMPAT_PRODUCTION_GLB_INVALID"))?;
    validate_artifact_headers(
        &production_response,
        &production_readback.glb_sha256,
        "production_concept",
    )?;
    if response_header(&preview_response, "x-forgecad-shape-program-sha256")
        != response_header(&production_response, "x-forgecad-shape-program-sha256")
    {
        return Err(ProbeFailure::new("COMPAT_PROFILE_IDENTITY_MISMATCH"));
    }

    let (_, quality) = execute_json(
        phase,
        bridge,
        AllowedHttpMethod::Get,
        &format!("/api/v1/agent/quality-reports/{quality_report_id}"),
        None,
        None,
        &[200],
    )
    .await?;
    if quality.get("asset_version_id").and_then(Value::as_str) != Some(asset_version_id)
        || quality.get("status").and_then(Value::as_str) != Some("passed")
        || quality
            .pointer("/compile_readback/glb_sha256")
            .and_then(Value::as_str)
            != Some(production_readback.glb_sha256.as_str())
        || quality
            .pointer("/compile_readback/artifact_profile/artifact_profile_id")
            .and_then(Value::as_str)
            != Some("production_concept")
    {
        return Err(ProbeFailure::new("COMPAT_QUALITY_READBACK_INVALID"));
    }

    let (_, render_set) = execute_json(
        phase,
        bridge,
        AllowedHttpMethod::Get,
        &format!("/api/v1/agent/asset-versions/{asset_version_id}:render?width=64&height=64"),
        None,
        None,
        &[200],
    )
    .await?;
    let render_set_sha256 = render_set
        .get("render_set_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .map(str::to_string)
        .ok_or_else(|| ProbeFailure::new("COMPAT_RENDER_READBACK_INVALID"))?;
    let views = render_set
        .get("views")
        .and_then(Value::as_array)
        .ok_or_else(|| ProbeFailure::new("COMPAT_RENDER_READBACK_INVALID"))?;
    let view_ids = views
        .iter()
        .filter_map(|view| view.get("view_id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    if render_set.get("schema_version").and_then(Value::as_str) != Some("AgentAssetRenderSet@1")
        || render_set.get("asset_version_id").and_then(Value::as_str) != Some(asset_version_id)
        || render_set.get("width").and_then(Value::as_u64) != Some(64)
        || render_set.get("height").and_then(Value::as_u64) != Some(64)
        || view_ids != ["iso", "front", "side", "top"]
        || views.iter().any(|view| {
            view.get("readback_status").and_then(Value::as_str) != Some("passed")
                || view
                    .get("sha256")
                    .and_then(Value::as_str)
                    .filter(|value| is_sha256(value))
                    .is_none()
                || view
                    .get("png_base64")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .is_none()
        })
    {
        return Err(ProbeFailure::new("COMPAT_RENDER_READBACK_INVALID"));
    }
    let (render_package_response, render_package_bytes) = execute_binary(
        phase,
        bridge,
        &format!(
            "/api/v1/agent/asset-versions/{asset_version_id}:render-package?width=64&height=64&render_set_sha256={render_set_sha256}"
        ),
    )
    .await?;
    if response_header(&render_package_response, "content-type") != Some("application/zip")
        || response_header(&render_package_response, "x-forgecad-render-set-sha256")
            != Some(render_set_sha256.as_str())
        || !render_package_bytes.starts_with(b"PK\x03\x04")
        || render_package_bytes.len() < 256
    {
        return Err(ProbeFailure::new("COMPAT_RENDER_PACKAGE_INVALID"));
    }
    let render_package_sha256 = sha256_hex(&render_package_bytes);

    Ok(CompatReadbackFacts {
        snapshot_etag,
        production_glb_sha256: production_readback.glb_sha256,
        render_set_sha256,
        render_package_sha256,
    })
}

async fn execute_json(
    phase: ProbePhase,
    bridge: &AppServerBridge,
    method: AllowedHttpMethod,
    path: &str,
    idempotency_key: Option<&str>,
    body: Option<Value>,
    expected_statuses: &[u16],
) -> Result<(CompatHttpResponse, Value), ProbeFailure> {
    let mut headers = Vec::new();
    if let Some(key) = idempotency_key {
        headers.push(("Idempotency-Key".into(), key.into()));
    }
    let body = match body {
        Some(value) => {
            headers.push(("Content-Type".into(), "application/json".into()));
            ProtocolHttpBody::Utf8 {
                data: value.to_string(),
            }
        }
        None => ProtocolHttpBody::Empty,
    };
    let response = bridge
        .execute_k003_packaged_compat(
            PreparedCompatHttpRequest {
                endpoint: LocalAgentEndpoint::parse(PROBE_ENDPOINT)
                    .map_err(|_| ProbeFailure::new("COMPAT_REQUEST_INVALID"))?,
                method,
                path: path.to_string(),
                headers,
                body,
            },
            CancellationToken::new(),
        )
        .await
        .map_err(|_| ProbeFailure::new("COMPAT_ROUTE_FAILED"))?;
    if !expected_statuses.contains(&response.status) {
        return Err(ProbeFailure::from_response(
            "COMPAT_ROUTE_REJECTED",
            phase,
            method,
            path,
            idempotency_key.unwrap_or("k003_probe_readback"),
            &response,
        ));
    }
    let ProtocolHttpBody::Utf8 { data } = &response.body else {
        return Err(ProbeFailure::new("COMPAT_JSON_RESPONSE_INVALID"));
    };
    let value = serde_json::from_str(data)
        .map_err(|_| ProbeFailure::new("COMPAT_JSON_RESPONSE_INVALID"))?;
    Ok((response, value))
}

async fn execute_binary(
    phase: ProbePhase,
    bridge: &AppServerBridge,
    path: &str,
) -> Result<(CompatHttpResponse, Vec<u8>), ProbeFailure> {
    let response = bridge
        .execute_k003_packaged_compat(
            PreparedCompatHttpRequest {
                endpoint: LocalAgentEndpoint::parse(PROBE_ENDPOINT)
                    .map_err(|_| ProbeFailure::new("COMPAT_REQUEST_INVALID"))?,
                method: AllowedHttpMethod::Get,
                path: path.to_string(),
                headers: Vec::new(),
                body: ProtocolHttpBody::Empty,
            },
            CancellationToken::new(),
        )
        .await
        .map_err(|_| ProbeFailure::new("COMPAT_ROUTE_FAILED"))?;
    if response.status != 200 {
        return Err(ProbeFailure::from_response(
            "COMPAT_ROUTE_REJECTED",
            phase,
            AllowedHttpMethod::Get,
            path,
            "k003_probe_binary_readback",
            &response,
        ));
    }
    let ProtocolHttpBody::Base64 { data } = &response.body else {
        return Err(ProbeFailure::new("COMPAT_BINARY_RESPONSE_INVALID"));
    };
    let bytes = BASE64_STANDARD
        .decode(data)
        .map_err(|_| ProbeFailure::new("COMPAT_BINARY_RESPONSE_INVALID"))?;
    Ok((response, bytes))
}

fn response_header<'a>(response: &'a CompatHttpResponse, name: &str) -> Option<&'a str> {
    response
        .headers
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn normalized_route(path: &str) -> String {
    path.split('?').next().unwrap_or(path).to_string()
}

fn response_error_code(response: &CompatHttpResponse) -> Option<String> {
    let ProtocolHttpBody::Utf8 { data } = &response.body else {
        return None;
    };
    let value = serde_json::from_str::<Value>(data).ok()?;
    let code = value.pointer("/error/code").and_then(Value::as_str)?;
    if code.is_empty()
        || code.len() > 160
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
    {
        return None;
    }
    Some(code.to_string())
}

fn validate_artifact_headers(
    response: &CompatHttpResponse,
    glb_sha256: &str,
    profile_id: &str,
) -> Result<(), ProbeFailure> {
    if response_header(response, "x-forgecad-artifact-profile") != Some(profile_id)
        || response_header(response, "x-forgecad-glb-sha256") != Some(glb_sha256)
        || response_header(response, "x-forgecad-shape-program-sha256")
            .filter(|value| is_sha256(value))
            .is_none()
    {
        return Err(ProbeFailure::new("COMPAT_ARTIFACT_HEADERS_INVALID"));
    }
    Ok(())
}

fn required_response_id(value: &Value, field: &str) -> Result<String, ProbeFailure> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_stable_probe_id(value))
        .map(str::to_string)
        .ok_or_else(|| ProbeFailure::new("COMPAT_RESPONSE_ID_INVALID"))
}

pub(crate) fn read_facts(
    core: &RustCoreRuntime,
    project_id: &str,
    asset_version_id: &str,
) -> Result<K003CoreProbeFacts, ProbeFailure> {
    let repository = core.repository();
    let project = repository
        .project(project_id)
        .map_err(|_| ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"))?
        .ok_or_else(|| ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"))?;
    let version = repository
        .version(asset_version_id)
        .map_err(|_| ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"))?
        .ok_or_else(|| ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"))?;
    let snapshot = repository
        .snapshot(project_id)
        .map_err(|_| ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"))?
        .ok_or_else(|| ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"))?;
    if version.project_id != project_id
        || version.status != AssetVersionStatus::Committed
        || snapshot.active_design.asset_version_id() != Some(asset_version_id)
        || snapshot.export.source_version_id() != asset_version_id
        || snapshot
            .quality
            .as_ref()
            .map(|quality| quality.asset_version_id.as_str())
            != Some(asset_version_id)
    {
        return Err(ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"));
    }
    let object = repository
        .object_for_reference(&ObjectReference {
            reference_kind: "asset_version".into(),
            owner_id: asset_version_id.into(),
            role: "production_glb".into(),
        })
        .map_err(|_| ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"))?
        .ok_or_else(|| ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"))?;
    let glb = repository
        .read_object(&object.sha256)
        .map_err(|_| ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"))?;
    let canonical = verify_forgecad_glb(&glb, Some("production_concept"))
        .map_err(|_| ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"))?;
    if object.extension != "glb"
        || object.byte_size != glb.len() as u64
        || canonical.glb_sha256 != object.sha256
        || semantic_sha256(&version.shape_program)
            .map_err(|_| ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"))?
            != version_shape_sha_from_quality(core, &snapshot)?
    {
        return Err(ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"));
    }

    Ok(K003CoreProbeFacts {
        project_id: project_id.into(),
        asset_version_id: asset_version_id.into(),
        snapshot_etag: snapshot.etag().to_string(),
        project_semantic_sha256: project_row_semantic_sha256(&project)?,
        snapshot_semantic_sha256: snapshot_row_semantic_sha256(&snapshot)?,
        glb_sha256: object.sha256,
        render_set_sha256: String::new(),
        render_package_sha256: String::new(),
    })
}

fn version_shape_sha_from_quality(
    core: &RustCoreRuntime,
    snapshot: &forgecad_core::ActiveDesignSnapshot,
) -> Result<String, ProbeFailure> {
    let quality = snapshot
        .quality
        .as_ref()
        .ok_or_else(|| ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"))?;
    let report = core
        .repository()
        .quality_report(&quality.quality_report_id)
        .map_err(|_| ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"))?
        .ok_or_else(|| ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"))?;
    report
        .report
        .get("compile_readback")
        .and_then(|value| value.get("shape_program_sha256"))
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .map(str::to_string)
        .ok_or_else(|| ProbeFailure::new("RUST_CORE_RECOVERY_FAILED"))
}

fn project_row_semantic_sha256(project: &Project) -> Result<String, ProbeFailure> {
    let status = serde_json::to_value(project.status)
        .map_err(|_| ProbeFailure::new("RUST_CORE_SEMANTIC_HASH_FAILED"))?;
    semantic_sha256(&json!({
        "project_id": project.project_id,
        "profile_id": project.profile_id,
        "domain_type": project.domain_type,
        "name": project.name,
        "status": status,
        "current_version_id": project.current_version_id
    }))
    .map_err(|_| ProbeFailure::new("RUST_CORE_SEMANTIC_HASH_FAILED"))
}

fn snapshot_row_semantic_sha256(
    snapshot: &forgecad_core::ActiveDesignSnapshot,
) -> Result<String, ProbeFailure> {
    let (
        source,
        active_asset_version_id,
        active_assembly_graph_id,
        legacy_version_id,
        legacy_module_graph_id,
    ) = match &snapshot.active_design {
        ActiveDesign::AgentAsset {
            asset_version_id,
            assembly_graph_id,
            ..
        } => (
            "agent_asset",
            Some(asset_version_id.clone()),
            Some(assembly_graph_id.clone()),
            None,
            None,
        ),
        ActiveDesign::LegacyConceptReadOnly {
            legacy_version_id,
            module_graph_id,
            ..
        } => (
            "legacy_concept_read_only",
            None,
            None,
            Some(legacy_version_id.clone()),
            Some(module_graph_id.clone()),
        ),
    };
    let (preview_change_set_id, preview_base_asset_version_id) = snapshot
        .preview
        .as_ref()
        .map(|preview| {
            (
                Some(preview.change_set_id.clone()),
                Some(preview.base_asset_version_id.clone()),
            )
        })
        .unwrap_or((None, None));
    let (quality_report_id, quality_asset_version_id) = snapshot
        .quality
        .as_ref()
        .map(|quality| {
            (
                Some(quality.quality_report_id.clone()),
                Some(quality.asset_version_id.clone()),
            )
        })
        .unwrap_or((None, None));
    let (export_source, export_source_version_id) = match &snapshot.export {
        ExportReference::AgentAsset {
            source_version_id, ..
        } => ("agent_asset", source_version_id.clone()),
        ExportReference::LegacyConceptReadOnly {
            source_version_id, ..
        } => ("legacy_concept_read_only", source_version_id.clone()),
    };
    semantic_sha256(&json!({
        "project_id": snapshot.project_id,
        "source": source,
        "active_asset_version_id": active_asset_version_id,
        "active_assembly_graph_id": active_assembly_graph_id,
        "legacy_version_id": legacy_version_id,
        "legacy_module_graph_id": legacy_module_graph_id,
        "selected_part_id": snapshot.selected_part_id,
        "preview_change_set_id": preview_change_set_id,
        "preview_base_asset_version_id": preview_base_asset_version_id,
        "quality_report_id": quality_report_id,
        "quality_asset_version_id": quality_asset_version_id,
        "export_source": export_source,
        "export_source_version_id": export_source_version_id,
        "revision": snapshot.revision,
        "render_preset_json": snapshot.render_preset,
        "selected_material_zone_id": snapshot.selected_material_zone_id,
        "part_display_json": snapshot.part_display
    }))
    .map_err(|_| ProbeFailure::new("RUST_CORE_SEMANTIC_HASH_FAILED"))
}

fn validate_expected(
    facts: &K003CoreProbeFacts,
    expected: &K003ProbeExpected,
) -> Result<(), ProbeFailure> {
    if facts.project_id != expected.project_id
        || facts.asset_version_id != expected.asset_version_id
        || facts.snapshot_etag != expected.snapshot_etag
        || facts.project_semantic_sha256 != expected.project_semantic_sha256
        || facts.snapshot_semantic_sha256 != expected.snapshot_semantic_sha256
        || facts.glb_sha256 != expected.glb_sha256
        || facts.render_set_sha256 != expected.render_set_sha256
        || facts.render_package_sha256 != expected.render_package_sha256
    {
        return Err(ProbeFailure::new("PROBE_EXPECTATION_MISMATCH"));
    }
    Ok(())
}

fn parse_config(
    get: impl Fn(&str) -> Option<String>,
) -> Result<Option<K003ProbeConfig>, ProbeFailure> {
    if get(PROBE_FLAG).as_deref() != Some("1") {
        return Ok(None);
    }
    let phase = match get(PROBE_PHASE).as_deref() {
        Some("initial") => ProbePhase::Initial,
        Some("restart") => ProbePhase::Restart,
        _ => return Err(ProbeFailure::new("PROBE_CONFIG_INVALID")),
    };
    let expected = if phase == ProbePhase::Restart {
        Some(K003ProbeExpected {
            project_id: required_id(&get, "FORGECAD_K003_EXPECT_PROJECT_ID")?,
            asset_version_id: required_id(&get, "FORGECAD_K003_EXPECT_ASSET_VERSION_ID")?,
            snapshot_etag: required_text(&get, "FORGECAD_K003_EXPECT_SNAPSHOT_ETAG", 256)?,
            project_semantic_sha256: required_sha(&get, "FORGECAD_K003_EXPECT_PROJECT_SHA256")?,
            snapshot_semantic_sha256: required_sha(&get, "FORGECAD_K003_EXPECT_SNAPSHOT_SHA256")?,
            glb_sha256: required_sha(&get, "FORGECAD_K003_EXPECT_GLB_SHA256")?,
            render_set_sha256: required_sha(&get, "FORGECAD_K003_EXPECT_RENDER_SET_SHA256")?,
            render_package_sha256: required_sha(
                &get,
                "FORGECAD_K003_EXPECT_RENDER_PACKAGE_SHA256",
            )?,
        })
    } else {
        None
    };
    Ok(Some(K003ProbeConfig { phase, expected }))
}

fn required_id(get: &impl Fn(&str) -> Option<String>, name: &str) -> Result<String, ProbeFailure> {
    get(name)
        .filter(|value| is_stable_probe_id(value))
        .ok_or_else(|| ProbeFailure::new("PROBE_CONFIG_INVALID"))
}

fn required_sha(get: &impl Fn(&str) -> Option<String>, name: &str) -> Result<String, ProbeFailure> {
    get(name)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| ProbeFailure::new("PROBE_CONFIG_INVALID"))
}

fn required_text(
    get: &impl Fn(&str) -> Option<String>,
    name: &str,
    maximum: usize,
) -> Result<String, ProbeFailure> {
    get(name)
        .filter(|value| value == value.trim() && !value.is_empty() && value.len() <= maximum)
        .ok_or_else(|| ProbeFailure::new("PROBE_CONFIG_INVALID"))
}

fn is_stable_probe_id(value: &str) -> bool {
    (1..=160).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn append_report(report: K003ProbeReport) {
    let encoded = serde_json::to_string(&report).unwrap_or_else(|_| {
        format!(
            "{{\"schema_version\":\"{K003_PACKAGED_PROBE_SCHEMA}\",\"phase\":\"{}\",\"ok\":false,\"error_code\":\"PROBE_REPORT_SERIALIZATION_FAILED\"}}",
            report.phase
        )
    });
    append_supervisor_log(&format!("{K003_PACKAGED_PROBE_MARKER}{encoded}"));
}

impl K003ProbeReport {
    fn success(phase: ProbePhase, facts: K003CoreProbeFacts) -> Self {
        Self {
            schema_version: K003_PACKAGED_PROBE_SCHEMA,
            phase: phase.as_str(),
            ok: true,
            project_id: Some(facts.project_id),
            asset_version_id: Some(facts.asset_version_id),
            snapshot_etag: Some(facts.snapshot_etag),
            project_semantic_sha256: Some(facts.project_semantic_sha256),
            snapshot_semantic_sha256: Some(facts.snapshot_semantic_sha256),
            glb_sha256: Some(facts.glb_sha256),
            render_set_sha256: Some(facts.render_set_sha256),
            render_package_sha256: Some(facts.render_package_sha256),
            provider_calls: Some(0),
            provider_network_call_made: Some(false),
            execution_path: Some(K003_EXECUTION_PATH),
            error_code: None,
            diagnostic: None,
        }
    }

    fn failure(
        phase: ProbePhase,
        error_code: &'static str,
        diagnostic: Option<ProbeDiagnostic>,
    ) -> Self {
        Self {
            schema_version: K003_PACKAGED_PROBE_SCHEMA,
            phase: phase.as_str(),
            ok: false,
            project_id: None,
            asset_version_id: None,
            snapshot_etag: None,
            project_semantic_sha256: None,
            snapshot_semantic_sha256: None,
            glb_sha256: None,
            render_set_sha256: None,
            render_package_sha256: None,
            provider_calls: None,
            provider_network_call_made: None,
            execution_path: None,
            error_code: Some(error_code),
            diagnostic,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn environment(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn disabled_probe_has_no_config_or_side_effect_path() {
        let values = BTreeMap::<String, String>::new();
        assert_eq!(parse_config(|name| values.get(name).cloned()), Ok(None));
        let values = environment(&[(PROBE_FLAG, "0"), (PROBE_PHASE, "initial")]);
        assert_eq!(parse_config(|name| values.get(name).cloned()), Ok(None));
    }

    #[test]
    fn initial_and_restart_environment_contracts_are_strict() {
        let initial = environment(&[(PROBE_FLAG, "1"), (PROBE_PHASE, "initial")]);
        assert_eq!(
            parse_config(|name| initial.get(name).cloned()),
            Ok(Some(K003ProbeConfig {
                phase: ProbePhase::Initial,
                expected: None,
            }))
        );

        let sha = "a".repeat(64);
        let restart = environment(&[
            (PROBE_FLAG, "1"),
            (PROBE_PHASE, "restart"),
            ("FORGECAD_K003_EXPECT_PROJECT_ID", "prj_probe"),
            ("FORGECAD_K003_EXPECT_ASSET_VERSION_ID", "assetver_probe_v1"),
            (
                "FORGECAD_K003_EXPECT_SNAPSHOT_ETAG",
                "W/\"active-design-2\"",
            ),
            ("FORGECAD_K003_EXPECT_PROJECT_SHA256", &sha),
            ("FORGECAD_K003_EXPECT_SNAPSHOT_SHA256", &sha),
            ("FORGECAD_K003_EXPECT_GLB_SHA256", &sha),
            ("FORGECAD_K003_EXPECT_RENDER_SET_SHA256", &sha),
            ("FORGECAD_K003_EXPECT_RENDER_PACKAGE_SHA256", &sha),
        ]);
        let parsed = parse_config(|name| restart.get(name).cloned())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.phase, ProbePhase::Restart);
        assert_eq!(parsed.expected.unwrap().glb_sha256, sha);

        let invalid = environment(&[(PROBE_FLAG, "1"), (PROBE_PHASE, "restart")]);
        assert_eq!(
            parse_config(|name| invalid.get(name).cloned()),
            Err(ProbeFailure::new("PROBE_CONFIG_INVALID"))
        );
    }

    #[test]
    fn restart_comparison_rejects_each_semantic_or_byte_identity_drift() {
        let sha = "b".repeat(64);
        let facts = K003CoreProbeFacts {
            project_id: "prj_probe".into(),
            asset_version_id: "assetver_probe_v1".into(),
            snapshot_etag: "W/\"active-design-2\"".into(),
            project_semantic_sha256: sha.clone(),
            snapshot_semantic_sha256: sha.clone(),
            glb_sha256: sha.clone(),
            render_set_sha256: sha.clone(),
            render_package_sha256: sha.clone(),
        };
        let expected = K003ProbeExpected {
            project_id: facts.project_id.clone(),
            asset_version_id: facts.asset_version_id.clone(),
            snapshot_etag: facts.snapshot_etag.clone(),
            project_semantic_sha256: sha.clone(),
            snapshot_semantic_sha256: sha.clone(),
            glb_sha256: sha.clone(),
            render_set_sha256: sha.clone(),
            render_package_sha256: sha,
        };
        assert_eq!(validate_expected(&facts, &expected), Ok(()));
        for field in 0..8 {
            let mut changed = expected.clone();
            match field {
                0 => changed.project_id = "prj_other".into(),
                1 => changed.asset_version_id = "assetver_other".into(),
                2 => changed.snapshot_etag = "W/\"active-design-3\"".into(),
                3 => changed.project_semantic_sha256 = "c".repeat(64),
                4 => changed.snapshot_semantic_sha256 = "c".repeat(64),
                5 => changed.glb_sha256 = "c".repeat(64),
                6 => changed.render_set_sha256 = "c".repeat(64),
                _ => changed.render_package_sha256 = "c".repeat(64),
            }
            assert_eq!(
                validate_expected(&facts, &changed),
                Err(ProbeFailure::new("PROBE_EXPECTATION_MISMATCH"))
            );
        }
    }

    #[test]
    fn success_report_is_exactly_provider_free_and_uses_required_execution_path() {
        let facts = K003CoreProbeFacts {
            project_id: "prj_probe".into(),
            asset_version_id: "assetver_probe_v1".into(),
            snapshot_etag: "W/\"active-design-2\"".into(),
            project_semantic_sha256: "a".repeat(64),
            snapshot_semantic_sha256: "b".repeat(64),
            glb_sha256: "c".repeat(64),
            render_set_sha256: "d".repeat(64),
            render_package_sha256: "e".repeat(64),
        };
        let value =
            serde_json::to_value(K003ProbeReport::success(ProbePhase::Initial, facts)).unwrap();
        assert_eq!(value["schema_version"], K003_PACKAGED_PROBE_SCHEMA);
        assert_eq!(value["provider_calls"], 0);
        assert_eq!(value["provider_network_call_made"], false);
        assert_eq!(value["execution_path"], K003_EXECUTION_PATH);
        assert!(value.get("error_code").is_none());
    }

    #[test]
    fn route_failure_diagnostic_is_bounded_and_excludes_response_body() {
        let response = CompatHttpResponse {
            schema_version: "ForgeCADHttpCompatResponse@1".into(),
            status: 503,
            headers: vec![],
            body: ProtocolHttpBody::Utf8 {
                data: r#"{"error":{"code":"SQLITE_BUSY","message":"secret prompt body must not escape"}}"#.into(),
            },
        };
        let failure = ProbeFailure::from_response(
            "COMPAT_ROUTE_REJECTED",
            ProbePhase::Initial,
            AllowedHttpMethod::Post,
            "/api/v1/projects?secret=query",
            "create_k003_packaged_probe",
            &response,
        );
        let report =
            K003ProbeReport::failure(ProbePhase::Initial, failure.code, failure.diagnostic);
        let serialized = serde_json::to_string(&report).unwrap();
        assert!(serialized.contains("POST"));
        assert!(serialized.contains("/api/v1/projects"));
        assert!(!serialized.contains("secret=query"));
        assert!(!serialized.contains("secret prompt body"));
        assert!(serialized.contains("SQLITE_BUSY"));
        assert!(serialized.contains("\"http_status\":503"));
        assert!(serialized.contains("\"startup_phase\":\"initial\""));
        assert!(serialized.contains("create_k003_packaged_probe"));
    }
}
