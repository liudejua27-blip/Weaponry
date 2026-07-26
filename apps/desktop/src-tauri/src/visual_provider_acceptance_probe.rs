//! Explicit live acceptance probe for the Forge Studio visual pipeline.
//!
//! Normal launches return before reading any visual credential or creating a
//! task. A live run requires four caller-owned environment values. The report
//! contains only Rust-owned hashes/readback facts and fixed error codes; it
//! never serializes the prompt, provider response, endpoint, API key, PNG, or
//! GLB bytes.

use std::{env, fs, path::PathBuf};

use forgecad_app_server::CancellationToken;
use forgecad_core::{PbrChannel, VisualQualityTier, VisualRemoteJobState};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, Runtime};

use super::{
    finish_visual_remote_job, generate_visual_asset_inner, stable_visual_error_code,
    GenerateVisualAssetRequest, VisualProviderState,
};

const ENABLE_FLAG: &str = "FORGESTUDIO_VISUAL_ACCEPTANCE";
const CONFIRM_FLAG: &str = "FORGESTUDIO_VISUAL_ACCEPTANCE_CONFIRM";
const RUN_ID_FLAG: &str = "FORGESTUDIO_VISUAL_ACCEPTANCE_RUN_ID";
const OUTPUT_FLAG: &str = "FORGESTUDIO_VISUAL_ACCEPTANCE_OUTPUT";
const LIVE_CONFIRMATION: &str = "I_UNDERSTAND_THIS_MAY_INCUR_VISUAL_PROVIDER_COST";
const SCHEMA_VERSION: &str = "ForgeStudioVisualProviderAcceptance@1";
const ACCEPTANCE_INTENT: &str = "设计一个非功能性的深海文明未来机械收藏道具：完整单体轮廓、层叠黑色金属与陶瓷装甲、蓝色生物发光流线、细密接缝和紧固件、轻微边缘磨损；三分之四视角，干净背景。";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbeConfig {
    run_id_sha256: String,
    output: PathBuf,
}

#[derive(Debug, Serialize)]
struct ProbeReport {
    schema_version: &'static str,
    status: &'static str,
    execution_mode: &'static str,
    run_id_sha256: String,
    provider_owner: &'static str,
    credential_source: &'static str,
    concept_provider_completed: bool,
    neural_provider_completed: bool,
    remote_job_completed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    concept_png_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    glb_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    glb_byte_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    triangle_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mesh_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    material_count: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pbr_channels: Vec<String>,
    every_primitive_has_uv0: bool,
    every_primitive_has_tangent: bool,
    no_raw_prompt_or_response: bool,
    no_key_or_provider_endpoint: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
}

impl ProbeReport {
    fn failed(config: &ProbeConfig, error_code: String) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            status: "fail",
            execution_mode: "live_explicit_opt_in",
            run_id_sha256: config.run_id_sha256.clone(),
            provider_owner: "rust_desktop",
            credential_source: "private_visual_secret_file",
            concept_provider_completed: false,
            neural_provider_completed: false,
            remote_job_completed: false,
            concept_png_sha256: None,
            glb_sha256: None,
            glb_byte_size: None,
            triangle_count: None,
            mesh_count: None,
            material_count: None,
            pbr_channels: Vec::new(),
            every_primitive_has_uv0: false,
            every_primitive_has_tangent: false,
            no_raw_prompt_or_response: true,
            no_key_or_provider_endpoint: true,
            error_code: Some(error_code),
        }
    }
}

pub(crate) fn run_if_enabled<R: Runtime>(app: AppHandle<R>) {
    let config = match parse_config(|name| env::var(name).ok()) {
        Ok(None) => return,
        Ok(Some(config)) => config,
        Err(_) => return,
    };
    tauri::async_runtime::spawn(async move {
        let report = run(app, &config)
            .await
            .unwrap_or_else(|code| ProbeReport::failed(&config, code));
        write_report(&config.output, &report);
    });
}

async fn run<R: Runtime>(app: AppHandle<R>, config: &ProbeConfig) -> Result<ProbeReport, String> {
    let state = app.state::<VisualProviderState>();
    let project = state
        .repository
        .list_projects(false, 1)
        .map_err(|_| "VISUAL_ACCEPTANCE_PROJECT_READ_FAILED".to_string())?
        .into_iter()
        .next()
        .ok_or_else(|| "VISUAL_ACCEPTANCE_ACTIVE_PROJECT_REQUIRED".to_string())?;
    let suffix = &config.run_id_sha256[..20];
    let client_request_id = format!("visual_acceptance_{suffix}");
    let input = GenerateVisualAssetRequest {
        client_request_id: client_request_id.clone(),
        project_id: project.project_id,
        turn_id: format!("visual_acceptance_turn_{suffix}"),
        user_intent: ACCEPTANCE_INTENT.into(),
        quality_tier: VisualQualityTier::StandardAsset,
        input_evidence: Vec::new(),
    };
    let response =
        match generate_visual_asset_inner(&input, &app, state.inner(), CancellationToken::new())
            .await
        {
            Ok(response) => response,
            Err(message) => {
                let code = stable_visual_error_code(&message, "VISUAL_ACCEPTANCE_FAILED");
                let _ = finish_visual_remote_job(
                    &state.repository,
                    &client_request_id,
                    VisualRemoteJobState::Failed { code: code.clone() },
                    "VISUAL_ACCEPTANCE_FAILED",
                );
                return Err(code);
            }
        };
    let completed = state
        .repository
        .visual_remote_job(&client_request_id)
        .map_err(|_| "VISUAL_ACCEPTANCE_JOB_READ_FAILED".to_string())?
        .is_some_and(|record| matches!(record.state, VisualRemoteJobState::Completed { .. }));
    if !completed {
        return Err("VISUAL_ACCEPTANCE_JOB_NOT_COMPLETED".into());
    }
    Ok(ProbeReport {
        schema_version: SCHEMA_VERSION,
        status: "pass",
        execution_mode: "live_explicit_opt_in",
        run_id_sha256: config.run_id_sha256.clone(),
        provider_owner: "rust_desktop",
        credential_source: "private_visual_secret_file",
        concept_provider_completed: true,
        neural_provider_completed: true,
        remote_job_completed: true,
        concept_png_sha256: Some(response.concept_reference.image_object_sha256),
        glb_sha256: Some(response.inspection.sha256),
        glb_byte_size: Some(response.inspection.byte_size),
        triangle_count: Some(response.inspection.triangle_count),
        mesh_count: Some(response.inspection.mesh_count),
        material_count: Some(response.inspection.material_count),
        pbr_channels: response
            .inspection
            .pbr_channels
            .into_iter()
            .map(pbr_channel_name)
            .map(str::to_string)
            .collect(),
        every_primitive_has_uv0: response.inspection.every_primitive_has_uv0,
        every_primitive_has_tangent: response.inspection.every_primitive_has_tangent,
        no_raw_prompt_or_response: true,
        no_key_or_provider_endpoint: true,
        error_code: None,
    })
}

fn pbr_channel_name(channel: PbrChannel) -> &'static str {
    match channel {
        PbrChannel::BaseColor => "base_color",
        PbrChannel::Normal => "normal",
        PbrChannel::Roughness => "roughness",
        PbrChannel::Metallic => "metallic",
        PbrChannel::AmbientOcclusion => "ambient_occlusion",
        PbrChannel::Emissive => "emissive",
    }
}

fn parse_config(
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<Option<ProbeConfig>, &'static str> {
    if lookup(ENABLE_FLAG).as_deref() != Some("1") {
        return Ok(None);
    }
    if lookup(CONFIRM_FLAG).as_deref() != Some(LIVE_CONFIRMATION) {
        return Err("VISUAL_ACCEPTANCE_CONFIRMATION_REQUIRED");
    }
    let run_id = lookup(RUN_ID_FLAG).ok_or("VISUAL_ACCEPTANCE_RUN_ID_REQUIRED")?;
    if run_id.len() < 12
        || run_id.len() > 80
        || !run_id.starts_with("live_")
        || !run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err("VISUAL_ACCEPTANCE_RUN_ID_INVALID");
    }
    let output = lookup(OUTPUT_FLAG)
        .map(PathBuf::from)
        .ok_or("VISUAL_ACCEPTANCE_OUTPUT_REQUIRED")?;
    if !output.is_absolute() || output.extension().and_then(|value| value.to_str()) != Some("json")
    {
        return Err("VISUAL_ACCEPTANCE_OUTPUT_INVALID");
    }
    Ok(Some(ProbeConfig {
        run_id_sha256: format!("{:x}", Sha256::digest(run_id.as_bytes())),
        output,
    }))
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{parse_config, CONFIRM_FLAG, ENABLE_FLAG, OUTPUT_FLAG, RUN_ID_FLAG};

    #[test]
    fn disabled_probe_reads_no_live_configuration() {
        let reads = std::cell::Cell::new(0_u8);
        let parsed = parse_config(|_| {
            reads.set(reads.get() + 1);
            None
        })
        .unwrap();
        assert!(parsed.is_none());
        assert_eq!(reads.get(), 1);
    }

    #[test]
    fn live_probe_requires_all_explicit_opt_in_values() {
        let mut values = BTreeMap::from([(ENABLE_FLAG, "1".to_string())]);
        assert_eq!(
            parse_config(|name| values.get(name).cloned()).unwrap_err(),
            "VISUAL_ACCEPTANCE_CONFIRMATION_REQUIRED"
        );
        values.insert(
            CONFIRM_FLAG,
            "I_UNDERSTAND_THIS_MAY_INCUR_VISUAL_PROVIDER_COST".into(),
        );
        values.insert(RUN_ID_FLAG, "live_visual_probe_001".into());
        values.insert(OUTPUT_FLAG, "/tmp/forge-studio-visual.json".into());
        let parsed = parse_config(|name| values.get(name).cloned())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.run_id_sha256.len(), 64);
    }
}
