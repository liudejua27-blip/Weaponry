//! Local, deterministic mechanical-arm MVP Provider.
//!
//! This is a production runtime boundary for the opt-in local MVP, not a test
//! fake and not a frontend asset injection.  It emits the same restricted
//! Product Tool protocol as a network Provider, so the Rust Action Loop,
//! NativeProductToolExecutor, V003 gate and Rust persistence remain the only
//! route to a preview.

use std::{
    env,
    sync::atomic::{AtomicU64, Ordering},
};

use forgecad_app_server::{
    CancellationToken, ProviderClient, ProviderError, ProviderEventSink, ProviderFinishReason,
    ProviderFuture, ProviderHealthCheck, ProviderMessage, ProviderPreflight, ProviderRequest,
    ProviderRequestBudgetPolicy, ProviderResponse, ProviderRole, ProviderStreamEvent,
    ProviderToolCall, ProviderUsage,
};
use serde_json::json;

pub const MVP_PROVIDER_ID: &str = "deepseek";
pub const MVP_MODEL: &str = "本机机械臂 MVP";
const MVP_SOURCE_LABEL: &str = "offline_deterministic";
const MAX_BRIEF_BYTES: usize = 8_000;
const ARCHITECTURE_FLAG: &str = "FORGECAD_MVP_ARM_ARCHITECTURE";

/// A code-owned local provider used only when `FORGECAD_MVP_OFFLINE_ARM=1`.
/// It never loads credentials or opens a network transport.
#[derive(Default)]
pub struct LocalRoboticArmMvpProvider {
    calls: AtomicU64,
}

impl LocalRoboticArmMvpProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn calls(&self) -> u64 {
        self.calls.load(Ordering::Relaxed)
    }
}

impl ProviderClient for LocalRoboticArmMvpProvider {
    fn preflight(&self, cancellation: CancellationToken) -> ProviderFuture<ProviderPreflight> {
        Box::pin(async move {
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled(false));
            }
            Ok(ProviderPreflight {
                provider_id: MVP_PROVIDER_ID.into(),
                model: MVP_MODEL.into(),
                configured: true,
                streaming: true,
                tool_calls: true,
                network_call_made: false,
            })
        })
    }

    fn request_budget_policy(
        &self,
        request: &ProviderRequest,
    ) -> Result<ProviderRequestBudgetPolicy, ProviderError> {
        validate_request_identity(request)?;
        // This remains an explicit, non-zero accounting contract so the
        // Action Loop exercises its regular reservation/release path. It is
        // not an external billing estimate.
        ProviderRequestBudgetPolicy {
            input_tokens_upper_bound: 64,
            input_cost_ceiling_microusd: 1,
            output_microusd_per_million_tokens: 1,
        }
        .validate()
    }

    fn stream(
        &self,
        request: ProviderRequest,
        cancellation: CancellationToken,
        mut events: ProviderEventSink,
    ) -> ProviderFuture<ProviderResponse> {
        let response = (|| {
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled(false));
            }
            validate_request_identity(&request)?;
            let step = completed_product_tool_calls(&request.messages);
            let result = match step {
                0 => tool_response(
                    "mvp_arm_01_plan",
                    "plan_complete_concept",
                    json!({
                        "plan": arm_plan(extract_arm_brief(&request.messages)?)
                    }),
                ),
                1 => tool_response(
                    "mvp_arm_02_style",
                    "select_style_recipe",
                    json!({
                        "domain_pack_id": "pack_robotic_arm_concept",
                        "intent": "流线工业维护机械臂"
                    }),
                ),
                2 => tool_response(
                    "mvp_arm_03_build",
                    "build_candidate_geometry",
                    json!({
                        "direction_id": "direction_mvp_robotic_arm",
                        // `showcase` is the code-owned request vocabulary;
                        // the native catalog maps it to the verified
                        // `production_concept` artifact profile.
                        "presentation_profile": "showcase"
                    }),
                ),
                3 => tool_response(
                    "mvp_arm_04_compile",
                    "compile_readback_candidate",
                    json!({}),
                ),
                4 => tool_response("mvp_arm_05_render", "render_candidate_views", json!({})),
                5 => tool_response("mvp_arm_06_evaluate", "evaluate_candidate", json!({})),
                6 => tool_response("mvp_arm_07_preview", "prepare_candidate_preview", json!({})),
                7 => final_response(),
                _ => Err(ProviderError::schema_mismatch(
                    "本机机械臂 MVP 收到超出已审核工具序列的请求。",
                    false,
                )),
            }?;
            Ok(result)
        })();

        self.calls.fetch_add(1, Ordering::Relaxed);
        Box::pin(async move {
            let response = response?;
            for call in &response.tool_calls {
                events(ProviderStreamEvent::ToolCallReady(call.clone()));
            }
            if let Some(content) = response.content.as_ref() {
                events(ProviderStreamEvent::ContentDelta(content.clone()));
            }
            response.validate()
        })
    }

    fn check(
        &self,
        provider_id: String,
        timeout_ms: u32,
        cancellation: CancellationToken,
    ) -> ProviderFuture<ProviderHealthCheck> {
        Box::pin(async move {
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled(false));
            }
            if provider_id != MVP_PROVIDER_ID || timeout_ms == 0 || timeout_ms > 120_000 {
                return Err(ProviderError::schema_mismatch(
                    "本机机械臂 MVP 检查请求不在受限合同内。",
                    false,
                ));
            }
            Ok(ProviderHealthCheck {
                provider_id: MVP_PROVIDER_ID.into(),
                network_call_made: false,
                usage: None,
            })
        })
    }

    fn cancel(
        &self,
        _cancellation_id: String,
        _cancellation_token: String,
    ) -> ProviderFuture<bool> {
        Box::pin(async { Ok(true) })
    }
}

fn validate_request_identity(request: &ProviderRequest) -> Result<(), ProviderError> {
    if request.provider_id != MVP_PROVIDER_ID || request.context_digest.len() != 64 {
        return Err(ProviderError::schema_mismatch(
            "本机机械臂 MVP 请求不符合受限 Provider 合同。",
            false,
        ));
    }
    Ok(())
}

fn completed_product_tool_calls(messages: &[ProviderMessage]) -> usize {
    messages
        .iter()
        .filter(|message| message.role == ProviderRole::Assistant)
        .map(|message| message.tool_calls.len())
        .sum()
}

fn extract_arm_brief(messages: &[ProviderMessage]) -> Result<String, ProviderError> {
    let brief = messages
        .iter()
        .rev()
        .find(|message| message.role == ProviderRole::User)
        .map(|message| message.content.trim())
        .filter(|brief| !brief.is_empty() && brief.len() <= MAX_BRIEF_BYTES)
        .ok_or_else(|| {
            ProviderError::schema_mismatch("本机机械臂 MVP 缺少有效的用户描述。", false)
        })?;
    let normalized = brief.to_ascii_lowercase();
    let is_arm = [
        "机械臂",
        "机械手臂",
        "robotic arm",
        "robot arm",
        "robotic-arm",
    ]
    .iter()
    .any(|term| normalized.contains(term));
    if !is_arm {
        return Err(ProviderError::schema_mismatch(
            "本机机械臂 MVP 仅支持机械臂描述；请描述一个非功能展示用机械臂。",
            false,
        ));
    }
    Ok(brief.to_string())
}

fn arm_plan(brief: String) -> serde_json::Value {
    let mut plan = json!({
        "schema_version": "MechanicalConceptPlan@1",
        "plan_id": "plan_mvp_robotic_arm",
        "domain_pack_id": "pack_robotic_arm_concept",
        "brief": brief,
        "generation_stage": "blockout",
        "spec": {},
        "directions": [{
            "direction_id": "direction_mvp_robotic_arm",
            "title": "本机机械臂 MVP",
            "summary": "非功能展示用桌面机械臂，包含连续外壳、可见关节、表面饰条与生产级 PBR 材质分区。",
            "silhouette": "industrial",
            "primary_part_roles": ["link_armor", "surface_trim"],
            "material_direction": "阳极金属连杆外壳、深色关节对比、克制蓝色饰条与生产级 PBR 纹理分区"
        }],
        "provider_id": MVP_SOURCE_LABEL,
        "shape_program_ready": false
    });

    // The deterministic packaged probe can exercise a reviewed alternative
    // architecture without pretending that the local Provider is DeepSeek.
    // The switch is opt-in and still emits the same bounded ArmDesignIntent
    // that the real Provider contract requires.
    if env::var(ARCHITECTURE_FLAG).as_deref() == Ok("parallel_link") {
        plan["arm_design_intent"] = json!({
            "schema_version": "ArmDesignIntent@1",
            "domain_pack_id": "pack_robotic_arm_concept",
            "architecture": "parallel_link",
            "joint_language": "armored_bearing",
            "link_language": "twin_rail",
            "base_language": "industrial_deck",
            "wrist_language": "fork_wrist",
            "end_effector_language": "parallel_gripper",
            "cable_language": "armored_harness",
            "surface_language": ["panel_seams", "flowline"],
            "material_palette": "graphite_blue",
            "detail_density": "dense",
            "pose": "grounded",
            "proportion_profile": "balanced",
            "style_keywords": ["parallel", "industrial"],
            "source": "user_brief",
            "visual_only": true
        });
    }
    plan
}

fn tool_response(
    call_id: &str,
    name: &str,
    arguments: serde_json::Value,
) -> Result<ProviderResponse, ProviderError> {
    Ok(ProviderResponse {
        content: None,
        tool_calls: vec![ProviderToolCall {
            call_id: call_id.into(),
            name: name.into(),
            arguments,
        }],
        ephemeral_reasoning: None,
        usage: ProviderUsage {
            input_tokens: 1,
            output_tokens: 1,
            prompt_cache_hit_tokens: 0,
            prompt_cache_miss_tokens: 0,
            estimated_cost_microusd: 1,
        },
        finish_reason: ProviderFinishReason::ToolCalls,
        network_call_made: false,
    })
}

fn final_response() -> Result<ProviderResponse, ProviderError> {
    Ok(ProviderResponse {
        content: Some(
            "本机机械臂 MVP 已完成唯一结果：C106 机械臂 Recipe、受限几何 GLB readback 与 V003 硬门均已通过；请确认后保存。".into(),
        ),
        tool_calls: Vec::new(),
        ephemeral_reasoning: None,
        usage: ProviderUsage {
            input_tokens: 1,
            output_tokens: 1,
            prompt_cache_hit_tokens: 0,
            prompt_cache_miss_tokens: 0,
            estimated_cost_microusd: 1,
        },
        finish_reason: ProviderFinishReason::Stop,
        network_call_made: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    fn request(messages: Vec<ProviderMessage>) -> ProviderRequest {
        ProviderRequest {
            provider_id: MVP_PROVIDER_ID.into(),
            model: MVP_MODEL.into(),
            context_digest: "a".repeat(64),
            messages,
            tools: Vec::new(),
            max_output_tokens: 128,
        }
    }

    fn user_message(content: &str) -> ProviderMessage {
        ProviderMessage {
            role: ProviderRole::User,
            content: content.into(),
            tool_call_id: None,
            tool_calls: Vec::new(),
            ephemeral_reasoning: None,
        }
    }

    #[test]
    fn emits_one_restricted_arm_sequence_without_network() {
        block_on(async {
            let provider = LocalRoboticArmMvpProvider::new();
            let cancellation = CancellationToken::new();
            let registry = forgecad_app_server::ProductToolRegistry::default();
            let mut messages = vec![user_message("设计一台蓝黑色桌面机械臂，带有精细金属饰条")];
            let expected = [
                "plan_complete_concept",
                "select_style_recipe",
                "build_candidate_geometry",
                "compile_readback_candidate",
                "render_candidate_views",
                "evaluate_candidate",
                "prepare_candidate_preview",
            ];
            for name in expected {
                let response = provider
                    .stream(
                        request(messages.clone()),
                        cancellation.clone(),
                        Box::new(|_| {}),
                    )
                    .await
                    .unwrap();
                assert!(!response.network_call_made);
                assert_eq!(response.tool_calls.len(), 1);
                assert_eq!(response.tool_calls[0].name, name);
                // The local Provider must remain a normal Product Tool client:
                // every emitted call is schema-validated by the same registry
                // that production ActionLoop execution uses.
                registry
                    .build_execution_request(
                        "turn_mvp_arm",
                        &response.tool_calls[0],
                        "execution_mvp_arm",
                        "cancel_mvp_arm",
                        "token_mvp_arm",
                    )
                    .unwrap();
                messages.push(ProviderMessage {
                    role: ProviderRole::Assistant,
                    content: String::new(),
                    tool_call_id: None,
                    tool_calls: response.tool_calls,
                    ephemeral_reasoning: None,
                });
            }
            let final_response = provider
                .stream(request(messages), cancellation, Box::new(|_| {}))
                .await
                .unwrap();
            assert_eq!(final_response.finish_reason, ProviderFinishReason::Stop);
            assert!(!final_response.network_call_made);
            assert_eq!(provider.calls(), 8);
        });
    }

    #[test]
    fn fails_closed_for_non_arm_briefs() {
        block_on(async {
            let provider = LocalRoboticArmMvpProvider::new();
            let error = provider
                .stream(
                    request(vec![user_message("设计一辆概念车")]),
                    CancellationToken::new(),
                    Box::new(|_| {}),
                )
                .await
                .unwrap_err();
            assert_eq!(error.code, "PROVIDER_SCHEMA_MISMATCH");
            assert!(!error.network_call_made);
        });
    }
}
