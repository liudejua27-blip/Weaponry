//! Development universal visual author used for local ForgeCAD acceptance.
//!
//! This is deliberately a development-only ProviderClient implementation. It
//! produces the same typed `author_universal_asset` call as a remote model,
//! but never opens a socket, reads a key, or injects a mesh into the UI. Rust
//! still validates the sealed request, lowers the bounded visual program,
//! executes the restricted worker, performs GLB readback, and owns preview
//! persistence. Its category labels are only a deterministic local planning
//! aid; they are not a product allowlist or a substitute for DeepSeek/Qwen
//! visual understanding.

use std::collections::BTreeMap;

use forgecad_app_server::{
    CancellationToken, ProviderClient, ProviderError, ProviderEventSink, ProviderFinishReason,
    ProviderFuture, ProviderHealthCheck, ProviderMessage, ProviderPreflight, ProviderRequest,
    ProviderRequestBudgetPolicy, ProviderResponse, ProviderRole, ProviderStreamEvent,
    ProviderToolCall, ProviderUsage,
};
use forgecad_core::semantic_sha256;
use serde_json::{json, Value};

pub const LOCAL_UNIVERSAL_PROVIDER_ID: &str = "deepseek";
pub const LOCAL_UNIVERSAL_MODEL: &str = "本机通用视觉作者";
pub const LOCAL_UNIVERSAL_ENV: &str = "FORGECAD_LOCAL_VISUAL_AUTHOR";

const MAX_BRIEF_BYTES: usize = 16_000;
const AUTHOR_TOOL: &str = "author_universal_asset";

/// Enabled only by `FORGECAD_LOCAL_VISUAL_AUTHOR=1` in the desktop process.
/// It is not a third network Provider and must not be presented as a
/// production DeepSeek or Qwen capability.
#[derive(Default)]
pub struct LocalUniversalVisualAuthorProvider;

impl LocalUniversalVisualAuthorProvider {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ProviderClient for LocalUniversalVisualAuthorProvider {
    fn preflight(&self, cancellation: CancellationToken) -> ProviderFuture<ProviderPreflight> {
        Box::pin(async move {
            if cancellation.is_cancelled() {
                return Err(ProviderError::cancelled(false));
            }
            Ok(ProviderPreflight {
                provider_id: LOCAL_UNIVERSAL_PROVIDER_ID.into(),
                model: LOCAL_UNIVERSAL_MODEL.into(),
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
        ProviderRequestBudgetPolicy {
            input_tokens_upper_bound: 256,
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
            let sealed_request = extract_sealed_request(&request.messages)?;
            let brief = extract_brief(&request.messages)?;
            let outcome = if is_supported_humanoid_brief(&brief) {
                executable_humanoid_outcome(sealed_request)?
            } else {
                executable_generic_visual_outcome(sealed_request, &brief)?
            };
            let call_id = format!(
                "local_universal_author_{}_{}",
                request.messages.len(),
                &request.context_digest[..8]
            );
            tool_response(outcome, &call_id)
        })();

        Box::pin(async move {
            let response = response?;
            for call in &response.tool_calls {
                events(ProviderStreamEvent::ToolCallReady(call.clone()));
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
            if provider_id != LOCAL_UNIVERSAL_PROVIDER_ID || timeout_ms == 0 || timeout_ms > 120_000
            {
                return Err(ProviderError::schema_mismatch(
                    "本机通用视觉作者检查请求不在受限合同内。",
                    false,
                ));
            }
            Ok(ProviderHealthCheck {
                provider_id: LOCAL_UNIVERSAL_PROVIDER_ID.into(),
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
    if request.provider_id != LOCAL_UNIVERSAL_PROVIDER_ID
        || request.context_digest.len() != 64
        || request.model != LOCAL_UNIVERSAL_MODEL
    {
        return Err(ProviderError::schema_mismatch(
            "本机通用视觉作者请求不符合受限 Provider 合同。",
            false,
        ));
    }
    Ok(())
}

fn extract_sealed_request(messages: &[ProviderMessage]) -> Result<Value, ProviderError> {
    let attachment = messages
        .iter()
        .find(|message| {
            message.role == ProviderRole::System
                && message.content.contains("Rust 封存的通用创作请求")
        })
        .ok_or_else(|| {
            ProviderError::schema_mismatch(
                "本机通用视觉作者没有收到 Rust 封存的 UniversalAuthorRequest。",
                false,
            )
        })?;
    let start = attachment
        .content
        .find('{')
        .ok_or_else(|| ProviderError::schema_mismatch("Rust 通用创作请求投影不是 JSON。", false))?;
    let projection: Value = serde_json::from_str(&attachment.content[start..])
        .map_err(|_| ProviderError::schema_mismatch("Rust 通用创作请求投影无法解析。", false))?;
    projection
        .get("request")
        .cloned()
        .ok_or_else(|| ProviderError::schema_mismatch("Rust 通用创作请求投影缺少 request。", false))
}

fn extract_brief(messages: &[ProviderMessage]) -> Result<String, ProviderError> {
    let brief = messages
        .iter()
        .rev()
        .find(|message| message.role == ProviderRole::User)
        .map(|message| message.content.trim())
        .filter(|brief| !brief.is_empty() && brief.len() <= MAX_BRIEF_BYTES)
        .ok_or_else(|| ProviderError::schema_mismatch("本机通用视觉作者缺少用户描述。", false))?;
    Ok(brief.to_string())
}

fn normalize_universal_request(request: Value) -> Result<Value, ProviderError> {
    let typed: forgecad_core::UniversalAuthorRequest =
        serde_json::from_value(request).map_err(|_| {
            ProviderError::schema_mismatch("Rust 封存的 UniversalAuthorRequest 无法归一化。", false)
        })?;
    serde_json::to_value(typed).map_err(|_| {
        ProviderError::schema_mismatch("本机通用视觉作者无法归一化 UniversalAuthorRequest。", false)
    })
}

fn is_supported_humanoid_brief(brief: &str) -> bool {
    let lower = brief.to_ascii_lowercase();
    ["人形机器人", "仿生机器人", "humanoid", "科幻装甲"]
        .iter()
        .any(|term| brief.contains(term) || lower.contains(term))
}

#[derive(Clone, Copy)]
enum LocalVisualArchetype {
    Mechanical,
    Vehicle,
    Furniture,
    Building,
    Plant,
    Animal,
    Character,
    WeaponProp,
    Generic,
}

impl LocalVisualArchetype {
    fn tag(self) -> &'static str {
        match self {
            Self::Mechanical => "mechanical_device",
            Self::Vehicle => "vehicle",
            Self::Furniture => "furniture",
            Self::Building => "building",
            Self::Plant => "plant",
            Self::Animal => "animal",
            Self::Character => "character",
            Self::WeaponProp => "fictional_game_prop",
            Self::Generic => "open_visual_object",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Mechanical => "机械设备外观",
            Self::Vehicle => "载具外观",
            Self::Furniture => "家具外观",
            Self::Building => "建筑外观",
            Self::Plant => "植物外观",
            Self::Animal => "动物外观代理",
            Self::Character => "角色外观代理",
            Self::WeaponProp => "虚构游戏道具外观",
            Self::Generic => "开放对象外观代理",
        }
    }
}

#[derive(Clone, Copy)]
enum LocalPrimitive {
    Box {
        size: [f64; 3],
        position: [f64; 3],
        rotation: [f64; 3],
        bevel: Option<f64>,
    },
    Cylinder {
        radius: f64,
        height: f64,
        axis: &'static str,
        position: [f64; 3],
        rotation: [f64; 3],
    },
    Capsule {
        radius: f64,
        height: f64,
        axis: &'static str,
        position: [f64; 3],
        rotation: [f64; 3],
    },
    Wedge {
        size: [f64; 3],
        position: [f64; 3],
        rotation: [f64; 3],
    },
}

#[derive(Clone, Copy)]
struct LocalPartSpec {
    part_id: &'static str,
    label: &'static str,
    role: &'static str,
    traits: &'static [&'static str],
    material_id: &'static str,
    material_label: &'static str,
    material_base: &'static str,
    material_traits: &'static [&'static str],
    primitive: LocalPrimitive,
}

#[derive(Clone, Copy)]
struct LocalMaterialChoice {
    material_id: &'static str,
    label: &'static str,
    base_material_id: &'static str,
    traits: &'static [&'static str],
}

fn local_material_choice(
    archetype: LocalVisualArchetype,
    spec: LocalPartSpec,
    brief: &str,
) -> LocalMaterialChoice {
    let lower = brief.to_ascii_lowercase();
    let contains = |terms: &[&str]| {
        terms
            .iter()
            .any(|term| brief.contains(term) || lower.contains(term))
    };

    // Preserve semantic special zones first. A light/emitter must remain an
    // emissive reviewed material, and wheels/tires must not become silver just
    // because the body is silver.
    if spec.material_id == "mat_glow" {
        return LocalMaterialChoice {
            material_id: "mat_emissive_blue",
            label: "受限发光饰条",
            base_material_id: "mat_emissive_blue",
            traits: &["emissive", "accent"],
        };
    }
    if spec.role.contains("wheel") || spec.role.contains("tire") {
        return LocalMaterialChoice {
            material_id: "mat_rubber",
            label: "橡胶外观",
            base_material_id: "mat_rubber",
            traits: &["rubber", "soft"],
        };
    }
    if spec.role.contains("window") || spec.role.contains("cabin") {
        return LocalMaterialChoice {
            material_id: "mat_dark_glass",
            label: "玻璃外观",
            base_material_id: "mat_dark_glass",
            traits: &["glass", "reflective"],
        };
    }

    if contains(&["银", "银白", "铝", "金属", "silver", "aluminum", "metal"]) {
        return LocalMaterialChoice {
            material_id: "mat_aluminum",
            label: "银白金属外观",
            base_material_id: "mat_aluminum",
            traits: &["metallic", "brushed"],
        };
    }
    if contains(&["红", "橙", "赤", "red", "orange", "amber"]) {
        return LocalMaterialChoice {
            material_id: "mat_signal_red",
            label: "暖色涂层外观",
            base_material_id: "mat_signal_red",
            traits: &["painted", "accent"],
        };
    }
    if contains(&["蓝", "青", "blue", "cyan"])
        && !matches!(
            archetype,
            LocalVisualArchetype::Plant
                | LocalVisualArchetype::Animal
                | LocalVisualArchetype::Character
                | LocalVisualArchetype::Furniture
                | LocalVisualArchetype::Building
        )
    {
        return LocalMaterialChoice {
            material_id: "mat_automotive_paint",
            label: "蓝色涂层外观",
            base_material_id: "mat_automotive_paint",
            traits: &["painted", "coated"],
        };
    }
    if contains(&["黑", "石墨", "碳纤", "black", "graphite", "carbon"]) {
        return LocalMaterialChoice {
            material_id: "mat_graphite",
            label: "深色复合外观",
            base_material_id: "mat_graphite",
            traits: &["graphite", "recessed"],
        };
    }

    // The development provider is intentionally deterministic, but it must
    // still preserve the subject's material semantics. Previously plant
    // foliage inherited `mat_emissive_blue`, animals inherited rubber, and
    // character clothing inherited painted steel. That made a category-open
    // request look like a robotic outline even when the geometry route was
    // generic_visual_exterior. Keep the same bounded eight-slot PBR catalog,
    // and express the actual non-mechanical intent through Rust-owned traits
    // that the Appearance Compiler lowers to closed color/finish tokens.
    match archetype {
        LocalVisualArchetype::Plant if spec.role.contains("stem") || spec.role.contains("trunk") => {
            return LocalMaterialChoice {
                material_id: "mat_composite",
                label: "树皮与木质外观",
                base_material_id: "mat_composite",
                traits: &["bark", "wood", "natural", "matte"],
            };
        }
        LocalVisualArchetype::Plant if spec.role.contains("canopy") || spec.role.contains("branch") => {
            return LocalMaterialChoice {
                material_id: "mat_composite",
                label: "叶片与植物外观",
                base_material_id: "mat_composite",
                traits: &["foliage", "leaf", "organic", "matte"],
            };
        }
        LocalVisualArchetype::Plant if spec.role.contains("base") || spec.role.contains("pot") => {
            return LocalMaterialChoice {
                material_id: "mat_composite",
                label: "陶土基座外观",
                base_material_id: "mat_composite",
                traits: &["clay", "matte", "natural"],
            };
        }
        LocalVisualArchetype::Animal
            if spec.role.contains("body")
                || spec.role.contains("head")
                || spec.role.contains("limb")
                || spec.role.contains("tail") =>
        {
            return LocalMaterialChoice {
                material_id: "mat_abs_matte",
                label: "毛发与柔性外观",
                base_material_id: "mat_abs_matte",
                traits: &["fur", "soft", "matte", "natural"],
            };
        }
        LocalVisualArchetype::Character if spec.role.contains("head") => {
            return LocalMaterialChoice {
                material_id: "mat_abs_matte",
                label: "皮肤哑光外观",
                base_material_id: "mat_abs_matte",
                traits: &["skin", "matte", "soft", "natural"],
            };
        }
        LocalVisualArchetype::Character
            if spec.role.contains("torso")
                || spec.role.contains("arm")
                || spec.role.contains("leg") =>
        {
            return LocalMaterialChoice {
                material_id: "mat_composite",
                label: "织物服装外观",
                base_material_id: "mat_composite",
                traits: &["fabric", "cloth", "matte", "soft"],
            };
        }
        LocalVisualArchetype::Furniture if spec.role.contains("surface") || spec.role.contains("top") => {
            return LocalMaterialChoice {
                material_id: "mat_composite",
                label: "木质家具外观",
                base_material_id: "mat_composite",
                traits: &["wood", "wood_grain", "natural", "matte"],
            };
        }
        LocalVisualArchetype::Furniture if spec.role.contains("cushion") || spec.role.contains("soft") => {
            return LocalMaterialChoice {
                material_id: "mat_abs_matte",
                label: "织物软垫外观",
                base_material_id: "mat_abs_matte",
                traits: &["fabric", "cloth", "soft", "matte"],
            };
        }
        LocalVisualArchetype::Building
            if spec.role.contains("primary")
                || spec.role.contains("roof")
                || spec.role.contains("entry") =>
        {
            return LocalMaterialChoice {
                material_id: "mat_composite",
                label: "石材与混凝土外观",
                base_material_id: "mat_composite",
                traits: &["concrete", "stone", "matte", "architectural"],
            };
        }
        _ => {}
    }

    if contains(&["木", "石", "自然", "wood", "stone", "natural"]) {
        return LocalMaterialChoice {
            material_id: "mat_composite",
            label: "哑光复合外观",
            base_material_id: "mat_composite",
            traits: &["natural", "matte"],
        };
    }
    if contains(&[
        "白", "米色", "布", "皮革", "白色", "white", "fabric", "leather",
    ]) || matches!(
        archetype,
        LocalVisualArchetype::Animal
            | LocalVisualArchetype::Character
            | LocalVisualArchetype::Plant
    ) && spec.material_id == "mat_soft"
    {
        return LocalMaterialChoice {
            material_id: "mat_abs_matte",
            label: "哑光软性外观",
            base_material_id: "mat_abs_matte",
            traits: &["matte", "soft"],
        };
    }

    LocalMaterialChoice {
        material_id: spec.material_id,
        label: spec.material_label,
        base_material_id: spec.material_base,
        traits: spec.material_traits,
    }
}

fn classify_local_visual_archetype(brief: &str) -> LocalVisualArchetype {
    let lower = brief.to_ascii_lowercase();
    let contains = |terms: &[&str]| {
        terms
            .iter()
            .any(|term| brief.contains(term) || lower.contains(term))
    };
    if contains(&[
        "机械臂",
        "机器人",
        "机械",
        "设备",
        "机器",
        "机甲",
        "引擎",
        "robot",
        "robotic",
        "machine",
        "mech",
        "device",
        "engine",
    ]) {
        LocalVisualArchetype::Mechanical
    } else if contains(&[
        "汽车",
        "车辆",
        "轿车",
        "卡车",
        "摩托",
        "无人机",
        "飞船",
        "飞机",
        "vehicle",
        "car",
        "drone",
        "aircraft",
    ]) {
        LocalVisualArchetype::Vehicle
    } else if contains(&[
        "桌",
        "椅",
        "沙发",
        "家具",
        "床",
        "柜",
        "table",
        "chair",
        "sofa",
        "furniture",
    ]) {
        LocalVisualArchetype::Furniture
    } else if contains(&[
        "建筑",
        "房子",
        "楼",
        "塔",
        "桥",
        "建筑物",
        "building",
        "house",
        "tower",
        "bridge",
    ]) {
        LocalVisualArchetype::Building
    } else if contains(&[
        "树", "花", "植物", "森林", "草", "叶", "tree", "plant", "flower", "forest",
    ]) {
        LocalVisualArchetype::Plant
    } else if contains(&[
        "猫", "狗", "马", "鸟", "鱼", "动物", "兔", "鹿", "cat", "dog", "animal", "bird", "fish",
    ]) {
        LocalVisualArchetype::Animal
    } else if contains(&[
        "人物",
        "角色",
        "人类",
        "女孩",
        "男孩",
        "骑士",
        "character",
        "person",
        "human",
    ]) {
        LocalVisualArchetype::Character
    } else if contains(&[
        "武器", "枪", "刀", "剑", "炮", "道具", "weapon", "sword", "rifle", "prop",
    ]) {
        LocalVisualArchetype::WeaponProp
    } else {
        LocalVisualArchetype::Generic
    }
}

fn local_visual_parts(archetype: LocalVisualArchetype) -> Vec<LocalPartSpec> {
    let primary = &["visual_exterior", "primary_mass"];
    let secondary = &["visual_exterior", "secondary_mass"];
    let detail = &["visual_exterior", "appearance_detail"];
    let soft = &["visual_exterior", "soft_mass"];
    let organic = &["visual_exterior", "organic_proxy"];
    let metal = &["metallic", "painted"];
    let dark = &["graphite", "recessed"];
    let paint = &["painted", "accent"];
    let rubber = &["rubber", "soft"];
    let natural = &["natural", "matte"];
    match archetype {
        LocalVisualArchetype::Mechanical => vec![
            LocalPartSpec {
                part_id: "part_mechanical_frame",
                label: "设备主体",
                role: "primary_machine_body",
                traits: primary,
                material_id: "mat_primary",
                material_label: "设备涂层",
                material_base: "mat_painted_steel",
                material_traits: metal,
                primitive: LocalPrimitive::Wedge {
                    size: [520.0, 320.0, 300.0],
                    position: [0.0, 0.0, 240.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_mechanical_joint",
                label: "关节组件",
                role: "articulated_joint",
                traits: secondary,
                material_id: "mat_secondary",
                material_label: "关节材质",
                material_base: "mat_graphite",
                material_traits: dark,
                primitive: LocalPrimitive::Cylinder {
                    radius: 105.0,
                    height: 120.0,
                    axis: "y",
                    position: [0.0, 0.0, 470.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_mechanical_link",
                label: "连杆外壳",
                role: "link_shell",
                traits: secondary,
                material_id: "mat_primary",
                material_label: "设备涂层",
                material_base: "mat_painted_steel",
                material_traits: metal,
                primitive: LocalPrimitive::Box {
                    size: [180.0, 180.0, 520.0],
                    position: [0.0, 0.0, 760.0],
                    rotation: [0.0, 0.0, 0.14],
                    bevel: Some(32.0),
                },
            },
            LocalPartSpec {
                part_id: "part_mechanical_end",
                label: "末端外观",
                role: "end_effector_proxy",
                traits: detail,
                material_id: "mat_accent",
                material_label: "末端饰面",
                material_base: "mat_aluminum",
                material_traits: paint,
                primitive: LocalPrimitive::Capsule {
                    radius: 90.0,
                    height: 260.0,
                    axis: "z",
                    position: [70.0, 0.0, 1080.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_mechanical_signal",
                label: "状态灯带",
                role: "visual_status_accent",
                traits: detail,
                material_id: "mat_glow",
                material_label: "状态发光",
                material_base: "mat_emissive_blue",
                material_traits: &["emissive", "accent"],
                primitive: LocalPrimitive::Box {
                    size: [140.0, 18.0, 40.0],
                    position: [120.0, -165.0, 330.0],
                    rotation: [0.0, 0.0, 0.0],
                    bevel: Some(8.0),
                },
            },
        ],
        LocalVisualArchetype::Vehicle => vec![
            LocalPartSpec {
                part_id: "part_vehicle_body",
                label: "主体车身",
                role: "primary_body",
                traits: primary,
                material_id: "mat_primary",
                material_label: "主体涂层",
                material_base: "mat_painted_steel",
                material_traits: metal,
                primitive: LocalPrimitive::Wedge {
                    size: [760.0, 280.0, 190.0],
                    position: [0.0, 0.0, 0.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_vehicle_cabin",
                label: "上层舱体",
                role: "upper_cabin",
                traits: secondary,
                material_id: "mat_primary",
                material_label: "主体涂层",
                material_base: "mat_painted_steel",
                material_traits: metal,
                primitive: LocalPrimitive::Box {
                    size: [410.0, 220.0, 190.0],
                    position: [60.0, 0.0, 180.0],
                    rotation: [0.0, 0.0, 0.0],
                    bevel: Some(28.0),
                },
            },
            LocalPartSpec {
                part_id: "part_vehicle_wheels",
                label: "轮组",
                role: "wheel_set",
                traits: secondary,
                material_id: "mat_soft",
                material_label: "轮胎",
                material_base: "mat_rubber",
                material_traits: rubber,
                primitive: LocalPrimitive::Cylinder {
                    radius: 105.0,
                    height: 54.0,
                    axis: "y",
                    position: [-250.0, -155.0, -90.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_vehicle_trim",
                label: "前后饰面",
                role: "accent_trim",
                traits: detail,
                material_id: "mat_accent",
                material_label: "外观饰面",
                material_base: "mat_aluminum",
                material_traits: paint,
                primitive: LocalPrimitive::Wedge {
                    size: [520.0, 24.0, 42.0],
                    position: [100.0, -145.0, 50.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
        ],
        LocalVisualArchetype::Furniture => vec![
            LocalPartSpec {
                part_id: "part_furniture_top",
                label: "主体台面",
                role: "primary_surface",
                traits: primary,
                material_id: "mat_primary",
                material_label: "主体材质",
                material_base: "mat_painted_steel",
                material_traits: paint,
                primitive: LocalPrimitive::Box {
                    size: [620.0, 360.0, 64.0],
                    position: [0.0, 0.0, 240.0],
                    rotation: [0.0, 0.0, 0.0],
                    bevel: Some(24.0),
                },
            },
            LocalPartSpec {
                part_id: "part_furniture_frame",
                label: "支撑框架",
                role: "support_frame",
                traits: secondary,
                material_id: "mat_secondary",
                material_label: "支撑材质",
                material_base: "mat_graphite",
                material_traits: dark,
                primitive: LocalPrimitive::Box {
                    size: [540.0, 280.0, 48.0],
                    position: [0.0, 0.0, 160.0],
                    rotation: [0.0, 0.0, 0.0],
                    bevel: Some(12.0),
                },
            },
            LocalPartSpec {
                part_id: "part_furniture_legs",
                label: "支撑腿",
                role: "support_legs",
                traits: secondary,
                material_id: "mat_secondary",
                material_label: "支撑材质",
                material_base: "mat_graphite",
                material_traits: dark,
                primitive: LocalPrimitive::Box {
                    size: [48.0, 48.0, 210.0],
                    position: [-250.0, -120.0, 75.0],
                    rotation: [0.0, 0.0, 0.0],
                    bevel: Some(12.0),
                },
            },
            LocalPartSpec {
                part_id: "part_furniture_cushion",
                label: "软性部件",
                role: "soft_surface",
                traits: soft,
                material_id: "mat_soft",
                material_label: "软性表面",
                material_base: "mat_rubber",
                material_traits: rubber,
                primitive: LocalPrimitive::Capsule {
                    radius: 115.0,
                    height: 250.0,
                    axis: "x",
                    position: [0.0, 0.0, 300.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
        ],
        LocalVisualArchetype::Building => vec![
            LocalPartSpec {
                part_id: "part_building_base",
                label: "建筑主体",
                role: "primary_mass",
                traits: primary,
                material_id: "mat_primary",
                material_label: "建筑表面",
                material_base: "mat_painted_steel",
                material_traits: paint,
                primitive: LocalPrimitive::Box {
                    size: [520.0, 420.0, 720.0],
                    position: [0.0, 0.0, 360.0],
                    rotation: [0.0, 0.0, 0.0],
                    bevel: Some(18.0),
                },
            },
            LocalPartSpec {
                part_id: "part_building_roof",
                label: "屋顶轮廓",
                role: "roof_form",
                traits: secondary,
                material_id: "mat_secondary",
                material_label: "屋顶材质",
                material_base: "mat_graphite",
                material_traits: dark,
                primitive: LocalPrimitive::Wedge {
                    size: [600.0, 500.0, 170.0],
                    position: [0.0, 0.0, 790.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_building_windows",
                label: "窗面节奏",
                role: "window_band",
                traits: detail,
                material_id: "mat_accent",
                material_label: "窗面饰材",
                material_base: "mat_emissive_blue",
                material_traits: &["reflective", "accent"],
                primitive: LocalPrimitive::Box {
                    size: [400.0, 18.0, 86.0],
                    position: [0.0, -215.0, 470.0],
                    rotation: [0.0, 0.0, 0.0],
                    bevel: Some(8.0),
                },
            },
            LocalPartSpec {
                part_id: "part_building_entry",
                label: "入口体块",
                role: "entry_form",
                traits: detail,
                material_id: "mat_secondary",
                material_label: "入口材质",
                material_base: "mat_graphite",
                material_traits: dark,
                primitive: LocalPrimitive::Box {
                    size: [180.0, 90.0, 300.0],
                    position: [0.0, -240.0, 150.0],
                    rotation: [0.0, 0.0, 0.0],
                    bevel: Some(16.0),
                },
            },
        ],
        LocalVisualArchetype::Plant => vec![
            LocalPartSpec {
                part_id: "part_plant_trunk",
                label: "主干",
                role: "primary_stem",
                traits: organic,
                material_id: "mat_primary",
                material_label: "树干",
                material_base: "mat_painted_steel",
                material_traits: natural,
                primitive: LocalPrimitive::Cylinder {
                    radius: 80.0,
                    height: 680.0,
                    axis: "z",
                    position: [0.0, 0.0, 340.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_plant_canopy",
                label: "冠层",
                role: "canopy_mass",
                traits: organic,
                material_id: "mat_accent",
                material_label: "叶片",
                material_base: "mat_emissive_blue",
                material_traits: &["organic", "matte"],
                primitive: LocalPrimitive::Capsule {
                    radius: 230.0,
                    height: 440.0,
                    axis: "z",
                    position: [0.0, 0.0, 760.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_plant_branch",
                label: "枝叶",
                role: "branch_cluster",
                traits: organic,
                material_id: "mat_accent",
                material_label: "叶片",
                material_base: "mat_emissive_blue",
                material_traits: &["organic", "matte"],
                primitive: LocalPrimitive::Capsule {
                    radius: 42.0,
                    height: 330.0,
                    axis: "x",
                    position: [160.0, 0.0, 620.0],
                    rotation: [0.0, 0.25, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_plant_pot",
                label: "基座",
                role: "plant_base",
                traits: secondary,
                material_id: "mat_secondary",
                material_label: "基座材质",
                material_base: "mat_graphite",
                material_traits: dark,
                primitive: LocalPrimitive::Cylinder {
                    radius: 190.0,
                    height: 180.0,
                    axis: "z",
                    position: [0.0, 0.0, -90.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
        ],
        LocalVisualArchetype::Animal => vec![
            LocalPartSpec {
                part_id: "part_animal_body",
                label: "身体",
                role: "primary_body",
                traits: organic,
                material_id: "mat_soft",
                material_label: "身体表面",
                material_base: "mat_rubber",
                material_traits: &["soft", "matte"],
                primitive: LocalPrimitive::Capsule {
                    radius: 180.0,
                    height: 560.0,
                    axis: "x",
                    position: [0.0, 0.0, 260.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_animal_head",
                label: "头部",
                role: "head_mass",
                traits: organic,
                material_id: "mat_soft",
                material_label: "身体表面",
                material_base: "mat_rubber",
                material_traits: &["soft", "matte"],
                primitive: LocalPrimitive::Capsule {
                    radius: 145.0,
                    height: 280.0,
                    axis: "x",
                    position: [330.0, 0.0, 360.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_animal_legs",
                label: "四肢",
                role: "limb_set",
                traits: organic,
                material_id: "mat_soft",
                material_label: "身体表面",
                material_base: "mat_rubber",
                material_traits: &["soft", "matte"],
                primitive: LocalPrimitive::Capsule {
                    radius: 58.0,
                    height: 260.0,
                    axis: "z",
                    position: [-180.0, -105.0, 70.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_animal_tail",
                label: "尾部",
                role: "tail_form",
                traits: detail,
                material_id: "mat_soft",
                material_label: "身体表面",
                material_base: "mat_rubber",
                material_traits: &["soft", "matte"],
                primitive: LocalPrimitive::Capsule {
                    radius: 42.0,
                    height: 300.0,
                    axis: "x",
                    position: [-320.0, 0.0, 360.0],
                    rotation: [0.0, 0.35, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_animal_accent",
                label: "面部特征",
                role: "facial_accent",
                traits: detail,
                material_id: "mat_accent",
                material_label: "面部细节",
                material_base: "mat_emissive_blue",
                material_traits: &["accent", "high_contrast"],
                primitive: LocalPrimitive::Wedge {
                    size: [64.0, 32.0, 52.0],
                    position: [440.0, -90.0, 405.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
        ],
        LocalVisualArchetype::Character => vec![
            LocalPartSpec {
                part_id: "part_character_torso",
                label: "躯干",
                role: "primary_torso",
                traits: organic,
                material_id: "mat_primary",
                material_label: "服装主体",
                material_base: "mat_painted_steel",
                material_traits: paint,
                primitive: LocalPrimitive::Capsule {
                    radius: 165.0,
                    height: 390.0,
                    axis: "z",
                    position: [0.0, 0.0, 430.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_character_head",
                label: "头部",
                role: "head_mass",
                traits: organic,
                material_id: "mat_soft",
                material_label: "角色表面",
                material_base: "mat_rubber",
                material_traits: &["soft", "matte"],
                primitive: LocalPrimitive::Capsule {
                    radius: 120.0,
                    height: 220.0,
                    axis: "z",
                    position: [0.0, 0.0, 760.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_character_arms",
                label: "双臂",
                role: "arm_set",
                traits: organic,
                material_id: "mat_primary",
                material_label: "服装主体",
                material_base: "mat_painted_steel",
                material_traits: paint,
                primitive: LocalPrimitive::Capsule {
                    radius: 55.0,
                    height: 360.0,
                    axis: "z",
                    position: [-230.0, 0.0, 430.0],
                    rotation: [0.0, 0.0, -0.12],
                },
            },
            LocalPartSpec {
                part_id: "part_character_legs",
                label: "双腿",
                role: "leg_set",
                traits: organic,
                material_id: "mat_primary",
                material_label: "服装主体",
                material_base: "mat_painted_steel",
                material_traits: paint,
                primitive: LocalPrimitive::Capsule {
                    radius: 68.0,
                    height: 470.0,
                    axis: "z",
                    position: [-95.0, 0.0, 70.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_character_accent",
                label: "服装饰面",
                role: "costume_accent",
                traits: detail,
                material_id: "mat_accent",
                material_label: "服装饰面",
                material_base: "mat_emissive_blue",
                material_traits: &["accent", "high_contrast"],
                primitive: LocalPrimitive::Wedge {
                    size: [220.0, 32.0, 90.0],
                    position: [0.0, -150.0, 480.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
        ],
        LocalVisualArchetype::WeaponProp => vec![
            LocalPartSpec {
                part_id: "part_prop_body",
                label: "主体道具",
                role: "primary_prop_body",
                traits: primary,
                material_id: "mat_primary",
                material_label: "道具主体",
                material_base: "mat_painted_steel",
                material_traits: metal,
                primitive: LocalPrimitive::Wedge {
                    size: [720.0, 160.0, 170.0],
                    position: [0.0, 0.0, 220.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_prop_grip",
                label: "握持外观",
                role: "grip_prop",
                traits: detail,
                material_id: "mat_secondary",
                material_label: "握持材质",
                material_base: "mat_graphite",
                material_traits: dark,
                primitive: LocalPrimitive::Box {
                    size: [180.0, 130.0, 300.0],
                    position: [-220.0, 0.0, 10.0],
                    rotation: [0.0, 0.0, -0.18],
                    bevel: Some(22.0),
                },
            },
            LocalPartSpec {
                part_id: "part_prop_emitter",
                label: "发光饰件",
                role: "visual_emitter",
                traits: detail,
                material_id: "mat_glow",
                material_label: "发光饰件",
                material_base: "mat_emissive_blue",
                material_traits: &["emissive", "accent"],
                primitive: LocalPrimitive::Cylinder {
                    radius: 48.0,
                    height: 260.0,
                    axis: "x",
                    position: [300.0, 0.0, 220.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_prop_trim",
                label: "轮廓饰条",
                role: "surface_trim",
                traits: detail,
                material_id: "mat_accent",
                material_label: "轮廓饰条",
                material_base: "mat_aluminum",
                material_traits: paint,
                primitive: LocalPrimitive::Wedge {
                    size: [440.0, 20.0, 30.0],
                    position: [90.0, -90.0, 275.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
        ],
        LocalVisualArchetype::Generic => vec![
            LocalPartSpec {
                part_id: "part_object_core",
                label: "对象主体",
                role: "primary_object_mass",
                traits: primary,
                material_id: "mat_primary",
                material_label: "主体外观",
                material_base: "mat_painted_steel",
                material_traits: paint,
                primitive: LocalPrimitive::Capsule {
                    radius: 190.0,
                    height: 520.0,
                    axis: "z",
                    position: [0.0, 0.0, 250.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
            LocalPartSpec {
                part_id: "part_object_secondary",
                label: "对象附属体",
                role: "secondary_object_mass",
                traits: secondary,
                material_id: "mat_secondary",
                material_label: "附属外观",
                material_base: "mat_graphite",
                material_traits: dark,
                primitive: LocalPrimitive::Box {
                    size: [280.0, 220.0, 180.0],
                    position: [260.0, 0.0, 300.0],
                    rotation: [0.0, 0.0, 0.0],
                    bevel: Some(24.0),
                },
            },
            LocalPartSpec {
                part_id: "part_object_detail",
                label: "显著外观细节",
                role: "appearance_detail",
                traits: detail,
                material_id: "mat_accent",
                material_label: "外观细节",
                material_base: "mat_aluminum",
                material_traits: paint,
                primitive: LocalPrimitive::Wedge {
                    size: [260.0, 24.0, 70.0],
                    position: [0.0, -200.0, 300.0],
                    rotation: [0.0, 0.0, 0.0],
                },
            },
        ],
    }
}

fn generic_visual_geometry_program(
    archetype: LocalVisualArchetype,
    specs: &[LocalPartSpec],
    brief: &str,
) -> Value {
    let seed = semantic_sha256(&json!({"brief":brief,"archetype":archetype.tag()}))
        .ok()
        .and_then(|hash| u32::from_str_radix(&hash[..8], 16).ok())
        .unwrap_or(42)
        & 0x7fff_fffd;
    let mut nodes = Vec::new();
    let mut outputs = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        let slug = spec.part_id.strip_prefix("part_").unwrap_or(spec.part_id);
        // The local provider must not collapse every input in one category to
        // one cached outline. Keep semantic roles stable for the contract, but
        // derive the actual dimensions, placement, and rotation from the full
        // brief. This is intentionally bounded and deterministic: it gives
        // the workbench a visibly different candidate for different prompts
        // without pretending to perform image understanding locally.
        let varied = vary_local_primitive(spec.primitive, seed, index);
        let source_id = append_local_primitive(&mut nodes, slug, varied);
        let source_id = append_local_pattern(&mut nodes, slug, source_id, spec.part_id, brief);
        let material = local_material_choice(archetype, *spec, brief);
        let part_id = format!("node_{slug}_part");
        let zone_id = format!("node_{slug}_zone");
        nodes.push(json!({"kind":"part","node_id":part_id,"input_node_id":source_id,"part_id":spec.part_id,"role":spec.role}));
        nodes.push(json!({"kind":"material_zone","node_id":zone_id,"input_node_id":part_id,"zone_id":format!("zone_{slug}"),"material_id":material.material_id}));
        outputs.push(json!({"output_id":format!("output_{slug}"),"node_id":zone_id}));
    }
    let mut material_bases = BTreeMap::new();
    for spec in specs {
        let material = local_material_choice(archetype, *spec, brief);
        material_bases
            .entry(material.material_id)
            .or_insert(material.base_material_id);
    }
    let materials = material_bases
        .into_iter()
        .map(|(material_id, base_material_id)| json!({"material_id":material_id,"base_material_id":base_material_id}))
        .collect::<Vec<_>>();
    json!({
        "schema_version":"ForgeVisualGeometryProgram@2",
        "program_id":format!("visual_local_{}_{}", archetype.tag(), &format!("{:08x}", seed)),
        "domain":"generic_visual_exterior",
        "units":"millimeter",
        "seed":seed,
        "materials":materials,
        "profiles":[],"section_sets":[],"nodes":nodes,"outputs":outputs,
        "budgets":{"schema_version":"GeometryProgramBudget@1","max_profiles":1,"max_section_sets":0,"max_nodes":96,"max_parts":8,"max_materials":6,"max_outputs":8,"max_operations":128,"triangle_budget":100000}
    })
}

fn append_local_pattern(
    nodes: &mut Vec<Value>,
    slug: &str,
    source_id: String,
    part_id: &str,
    brief: &str,
) -> String {
    let lower = brief.to_ascii_lowercase();
    let contains = |terms: &[&str]| {
        terms
            .iter()
            .any(|term| brief.contains(term) || lower.contains(term))
    };
    let pattern = match part_id {
        "part_vehicle_wheels" => Some(("array", "x", 2_u16, 500.0, 0.0)),
        "part_furniture_legs" => Some(("array", "x", 2_u16, 500.0, 0.0)),
        "part_character_arms" => Some(("array", "x", 2_u16, 460.0, 0.0)),
        "part_character_legs" => Some(("array", "x", 2_u16, 190.0, 0.0)),
        "part_animal_legs" if contains(&["四足", "四条腿", "quadruped"]) => {
            Some(("radial_array", "z", 4_u16, 210.0, std::f64::consts::TAU))
        }
        "part_animal_legs" => Some(("array", "x", 2_u16, 360.0, 0.0)),
        "part_building_windows" => Some(("array", "z", 3_u16, 150.0, 0.0)),
        "part_mechanical_signal" if contains(&["灯带", "灯光", "signal", "light"]) => {
            Some(("array", "x", 2_u16, 220.0, 0.0))
        }
        "part_plant_branch" => Some(("radial_array", "z", 4_u16, 170.0, std::f64::consts::TAU)),
        _ => None,
    };
    let Some((kind, axis, count, distance, angle)) = pattern else {
        return source_id;
    };
    let pattern_id = format!("node_{slug}_{kind}");
    if kind == "array" {
        nodes.push(json!({
            "kind": "array",
            "node_id": pattern_id,
            "input_node_id": source_id,
            "axis": axis,
            "count": count,
            "spacing": distance
        }));
    } else {
        nodes.push(json!({
            "kind": "radial_array",
            "node_id": pattern_id,
            "input_node_id": source_id,
            "axis": axis,
            "count": count,
            "radius": distance,
            "angle": angle
        }));
    }
    pattern_id
}

fn local_mix(seed: u32, index: usize, channel: u32) -> u32 {
    let mut value = seed
        .wrapping_add((index as u32).wrapping_mul(0x9e37_79b9))
        .wrapping_add(channel.wrapping_mul(0x85eb_ca6b));
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

fn local_factor(seed: u32, index: usize, channel: u32, minimum: f64, maximum: f64) -> f64 {
    let unit = f64::from(local_mix(seed, index, channel) % 10_001) / 10_000.0;
    minimum + ((maximum - minimum) * unit)
}

fn local_offset(seed: u32, index: usize, channel: u32, span: f64) -> f64 {
    (local_factor(seed, index, channel, -1.0, 1.0)) * span
}

fn vary_local_primitive(primitive: LocalPrimitive, seed: u32, index: usize) -> LocalPrimitive {
    match primitive {
        LocalPrimitive::Box {
            size,
            position,
            rotation,
            bevel,
        } => LocalPrimitive::Box {
            size: [
                size[0] * local_factor(seed, index, 1, 0.82, 1.18),
                size[1] * local_factor(seed, index, 2, 0.82, 1.18),
                size[2] * local_factor(seed, index, 3, 0.82, 1.22),
            ],
            position: [
                position[0] + local_offset(seed, index, 4, 72.0),
                position[1] + local_offset(seed, index, 5, 48.0),
                position[2] + local_offset(seed, index, 6, 68.0),
            ],
            rotation: [
                rotation[0] + local_offset(seed, index, 7, 0.06),
                rotation[1] + local_offset(seed, index, 8, 0.06),
                rotation[2] + local_offset(seed, index, 9, 0.10),
            ],
            bevel: bevel.map(|radius| radius * local_factor(seed, index, 10, 0.75, 1.20)),
        },
        LocalPrimitive::Cylinder {
            radius,
            height,
            axis,
            position,
            rotation,
        } => LocalPrimitive::Cylinder {
            radius: radius * local_factor(seed, index, 11, 0.84, 1.18),
            height: height * local_factor(seed, index, 12, 0.82, 1.22),
            axis,
            position: [
                position[0] + local_offset(seed, index, 13, 70.0),
                position[1] + local_offset(seed, index, 14, 48.0),
                position[2] + local_offset(seed, index, 15, 70.0),
            ],
            rotation: [
                rotation[0] + local_offset(seed, index, 16, 0.06),
                rotation[1] + local_offset(seed, index, 17, 0.06),
                rotation[2] + local_offset(seed, index, 18, 0.10),
            ],
        },
        LocalPrimitive::Capsule {
            radius,
            height,
            axis,
            position,
            rotation,
        } => LocalPrimitive::Capsule {
            radius: radius * local_factor(seed, index, 19, 0.82, 1.20),
            height: height * local_factor(seed, index, 20, 0.82, 1.24),
            axis,
            position: [
                position[0] + local_offset(seed, index, 21, 74.0),
                position[1] + local_offset(seed, index, 22, 52.0),
                position[2] + local_offset(seed, index, 23, 72.0),
            ],
            rotation: [
                rotation[0] + local_offset(seed, index, 24, 0.06),
                rotation[1] + local_offset(seed, index, 25, 0.08),
                rotation[2] + local_offset(seed, index, 26, 0.12),
            ],
        },
        LocalPrimitive::Wedge {
            size,
            position,
            rotation,
        } => LocalPrimitive::Wedge {
            size: [
                size[0] * local_factor(seed, index, 27, 0.80, 1.20),
                size[1] * local_factor(seed, index, 28, 0.80, 1.20),
                size[2] * local_factor(seed, index, 29, 0.80, 1.24),
            ],
            position: [
                position[0] + local_offset(seed, index, 30, 80.0),
                position[1] + local_offset(seed, index, 31, 56.0),
                position[2] + local_offset(seed, index, 32, 76.0),
            ],
            rotation: [
                rotation[0] + local_offset(seed, index, 33, 0.06),
                rotation[1] + local_offset(seed, index, 34, 0.08),
                rotation[2] + local_offset(seed, index, 35, 0.12),
            ],
        },
    }
}

fn append_local_primitive(nodes: &mut Vec<Value>, slug: &str, primitive: LocalPrimitive) -> String {
    let base_id = format!("node_{slug}_base");
    match primitive {
        LocalPrimitive::Box {
            size,
            position,
            rotation,
            bevel,
        } => {
            nodes.push(json!({"kind":"box","node_id":base_id,"size":size,"position":position,"rotation":rotation}));
            if let Some(radius) = bevel {
                let bevel_id = format!("node_{slug}_bevel");
                nodes.push(json!({"kind":"bevel_approx","node_id":bevel_id,"input_node_id":base_id,"radius":radius,"segments":2}));
                bevel_id
            } else {
                base_id
            }
        }
        LocalPrimitive::Cylinder {
            radius,
            height,
            axis,
            position,
            rotation,
        } => {
            nodes.push(json!({"kind":"cylinder","node_id":base_id,"radius":radius,"height":height,"axis":axis,"position":position,"rotation":rotation}));
            base_id
        }
        LocalPrimitive::Capsule {
            radius,
            height,
            axis,
            position,
            rotation,
        } => {
            nodes.push(json!({"kind":"capsule","node_id":base_id,"radius":radius,"height":height,"axis":axis,"position":position,"rotation":rotation}));
            base_id
        }
        LocalPrimitive::Wedge {
            size,
            position,
            rotation,
        } => {
            nodes.push(json!({"kind":"wedge","node_id":base_id,"size":size,"position":position,"rotation":rotation}));
            base_id
        }
    }
}

fn executable_generic_visual_outcome(request: Value, brief: &str) -> Result<Value, ProviderError> {
    let request = normalize_universal_request(request)?;
    let request_sha256 = semantic_sha256(&request).map_err(|_| {
        ProviderError::schema_mismatch("本机通用视觉作者无法计算 request hash。", false)
    })?;
    let archetype = classify_local_visual_archetype(brief);
    let specs = local_visual_parts(archetype);
    let mut category_tags = vec!["visual_exterior".to_string(), archetype.tag().to_string()];
    if matches!(
        archetype,
        LocalVisualArchetype::Mechanical
            | LocalVisualArchetype::Vehicle
            | LocalVisualArchetype::Furniture
            | LocalVisualArchetype::Building
            | LocalVisualArchetype::WeaponProp
    ) {
        category_tags.push("hard_surface".into());
    }
    let mut materials = Vec::new();
    let mut seen_materials = BTreeMap::new();
    for spec in &specs {
        let material = local_material_choice(archetype, *spec, brief);
        seen_materials
            .entry(material.material_id)
            .or_insert_with(|| (material, Vec::<&str>::new()))
            .1
            .push(spec.part_id);
    }
    for (material_id, (material, part_ids)) in seen_materials {
        materials.push(json!({
            "material_id":material_id,
            "label":material.label,
            "part_ids":part_ids,
            "appearance_traits":material.traits
        }));
    }
    let profile = json!({
        "schema_version":"SubjectProfile@1",
        "profile_id":format!("subject_local_{}", archetype.tag()),
        "request_sha256":request_sha256,
        "identity_label":brief,
        "category":format!("{} / visual exterior proxy", archetype.label()),
        "category_tags":category_tags,
        "silhouette":format!("{}的独立外观轮廓与主要体块", archetype.label()),
        "negative_space":"由主体、附属体和显著外观部件之间的间隙组成；内部结构未观测",
        "pose":if matches!(archetype, LocalVisualArchetype::Animal | LocalVisualArchetype::Character) { "展示姿态" } else { "静态展示姿态" },
        "visible_views":[],
        "occlusions":["背面、内部和遮挡区域按外观代理推断"],
        "uncertainties":["本机作者未调用视觉网络模型；未提供的细节保持 inferred，不标记为 observed"],
        "parts":specs.iter().map(|spec| json!({"part_id":spec.part_id,"label":spec.label,"semantic_role":spec.role,"traits":spec.traits,"uncertainty_bps":4500})).collect::<Vec<_>>(),
        "features":specs.iter().flat_map(|spec| [
            json!({"feature_id":format!("feature_{}_macro",spec.part_id.strip_prefix("part_").unwrap_or(spec.part_id)),"part_id":spec.part_id,"level":"macro","description":format!("{}整体轮廓与比例",spec.label)}),
            json!({"feature_id":format!("feature_{}_meso",spec.part_id.strip_prefix("part_").unwrap_or(spec.part_id)),"part_id":spec.part_id,"level":"meso","description":format!("{}分件关系与结构节奏",spec.label)}),
            json!({"feature_id":format!("feature_{}_micro",spec.part_id.strip_prefix("part_").unwrap_or(spec.part_id)),"part_id":spec.part_id,"level":"micro","description":format!("{}表面材质与边缘处理",spec.label)})
        ]).collect::<Vec<_>>(),
        "materials":materials
    });
    let profile_sha256 = semantic_sha256(&profile).map_err(|_| {
        ProviderError::schema_mismatch("本机通用视觉作者无法计算 SubjectProfile hash。", false)
    })?;
    let requirements = specs.iter().flat_map(|spec| {
        let slug = spec.part_id.strip_prefix("part_").unwrap_or(spec.part_id);
        [
            json!({"feature_id":format!("feature_{slug}_macro"),"level":"macro","description":format!("{}整体轮廓与比例",spec.label),"salience_bps":9800,"evidence_status":"inferred","evidence_regions":[],"affected_part_ids":[spec.part_id],"channels":["geometry","base_color"],"minimum_acceptance_views":["front","iso","side"]}),
            json!({"feature_id":format!("feature_{slug}_meso"),"level":"meso","description":format!("{}分件关系与结构节奏",spec.label),"salience_bps":8700,"evidence_status":"inferred","evidence_regions":[],"affected_part_ids":[spec.part_id],"channels":["geometry","normal","roughness"],"minimum_acceptance_views":["front","iso"]}),
            json!({"feature_id":format!("feature_{slug}_micro"),"level":"micro","description":format!("{}表面材质与边缘处理",spec.label),"salience_bps":7600,"evidence_status":"inferred","evidence_regions":[],"affected_part_ids":[spec.part_id],"channels":["base_color","normal","roughness"],"minimum_acceptance_views":["iso"]})
        ]
    }).collect::<Vec<_>>();
    let feature_contract = json!({"schema_version":"VisualFeatureContract@1","contract_id":format!("vfcontract_local_{}",archetype.tag()),"request_sha256":request_sha256,"subject_profile_sha256":profile_sha256,"requirements":requirements});
    let feature_contract_sha256 = semantic_sha256(&feature_contract).map_err(|_| {
        ProviderError::schema_mismatch(
            "本机通用视觉作者无法计算 VisualFeatureContract hash。",
            false,
        )
    })?;
    let capability_manifest_sha256 = request
        .get("capability_manifest_sha256")
        .cloned()
        .unwrap_or(Value::Null);
    let plan = json!({
        "schema_version":"RepresentationPlan@1","plan_id":format!("repplan_local_{}",archetype.tag()),"request_sha256":request_sha256,"subject_profile_sha256":profile_sha256,"visual_feature_contract_sha256":feature_contract_sha256,"capability_manifest_sha256":capability_manifest_sha256,
        "parts":specs.iter().map(|spec| { let slug=spec.part_id.strip_prefix("part_").unwrap_or(spec.part_id); json!({"part_id":spec.part_id,"representation":"procedural","capability_id":"procedural.generic_visual_exterior_v1","covered_feature_ids":[format!("feature_{slug}_macro"),format!("feature_{slug}_meso"),format!("feature_{slug}_micro")],"rationale":"使用 Rust 代码所有的通用外观组合语言；不执行任意代码。"}) }).collect::<Vec<_>>()
    });
    Ok(
        json!({"outcome":"executable","schema_version":"UniversalAuthorOutcome@1","request":request,"subject_profile":profile,"visual_feature_contract":feature_contract,"representation_plan":plan,"executable_payload":generic_visual_geometry_program(archetype,&specs,brief)}),
    )
}

fn executable_humanoid_outcome(request: Value) -> Result<Value, ProviderError> {
    let request = normalize_universal_request(request)?;
    let request_sha256 = semantic_sha256(&request).map_err(|_| {
        ProviderError::schema_mismatch("本机通用视觉作者无法计算 request hash。", false)
    })?;
    let profile = humanoid_profile(&request_sha256);
    let profile_sha256 = semantic_sha256(&profile).map_err(|_| {
        ProviderError::schema_mismatch("本机通用视觉作者无法计算 SubjectProfile hash。", false)
    })?;
    let feature_contract = humanoid_feature_contract(&request_sha256, &profile_sha256);
    let feature_contract_sha256 = semantic_sha256(&feature_contract).map_err(|_| {
        ProviderError::schema_mismatch(
            "本机通用视觉作者无法计算 VisualFeatureContract hash。",
            false,
        )
    })?;
    let capability_manifest_sha256 = request
        .get("capability_manifest_sha256")
        .cloned()
        .unwrap_or(Value::Null);
    let representation_plan = humanoid_representation_plan(
        &request_sha256,
        &profile_sha256,
        &feature_contract_sha256,
        capability_manifest_sha256,
    );
    Ok(json!({
        "outcome":"executable",
        "schema_version":"UniversalAuthorOutcome@1",
        "request":request,
        "subject_profile":profile,
        "visual_feature_contract":feature_contract,
        "representation_plan":representation_plan,
        "executable_payload":humanoid_geometry_program()
    }))
}

#[allow(dead_code)]
fn limitation_outcome(request: Value, brief: String) -> Result<Value, ProviderError> {
    let request = normalize_universal_request(request)?;
    let request_sha256 = semantic_sha256(&request).map_err(|_| {
        ProviderError::schema_mismatch("本机通用视觉作者无法计算 request hash。", false)
    })?;
    let profile = json!({
        "schema_version":"SubjectProfile@1",
        "profile_id":"subject_local_limited",
        "request_sha256":request_sha256,
        "identity_label":brief,
        "category":"user-described visual object",
        "category_tags":["local_author_preview"],
        "silhouette":"根据用户描述待确认的对象轮廓",
        "negative_space":"尚未可靠确定",
        "pose":"未确定",
        "visible_views":[],
        "occlusions":[],
        "uncertainties":["本机过渡作者当前只执行银白科幻人形机器人验收样例"],
        "parts":[{"part_id":"part_subject","label":"主体","semantic_role":"primary_subject","traits":["user_described"],"uncertainty_bps":7000}],
        "features":[
            {"feature_id":"feature_subject_macro","part_id":"part_subject","level":"macro","description":"对象整体轮廓"},
            {"feature_id":"feature_subject_meso","part_id":"part_subject","level":"meso","description":"主要部件与结构"},
            {"feature_id":"feature_subject_micro","part_id":"part_subject","level":"micro","description":"外观材质与表面细节"}
        ],
        "materials":[{"material_id":"material_subject","label":"待确认材质","part_ids":["part_subject"],"appearance_traits":["unresolved"]}]
    });
    let profile_sha256 = semantic_sha256(&profile).map_err(|_| {
        ProviderError::schema_mismatch("本机通用视觉作者无法计算限制 profile hash。", false)
    })?;
    let contract = json!({
        "schema_version":"VisualFeatureContract@1",
        "contract_id":"vfcontract_local_limited",
        "request_sha256":request_sha256,
        "subject_profile_sha256":profile_sha256,
        "requirements":[
            {"feature_id":"feature_subject_macro","level":"macro","description":"对象整体轮廓","salience_bps":10000,"evidence_status":"inferred","evidence_regions":[],"affected_part_ids":["part_subject"],"channels":["geometry"],"minimum_acceptance_views":["front","iso"]},
            {"feature_id":"feature_subject_meso","level":"meso","description":"主要部件与结构","salience_bps":9000,"evidence_status":"inferred","evidence_regions":[],"affected_part_ids":["part_subject"],"channels":["geometry","normal"],"minimum_acceptance_views":["front"]},
            {"feature_id":"feature_subject_micro","level":"micro","description":"外观材质与表面细节","salience_bps":8000,"evidence_status":"inferred","evidence_regions":[],"affected_part_ids":["part_subject"],"channels":["base_color","roughness"],"minimum_acceptance_views":["iso"]}
        ]
    });
    let contract_sha256 = semantic_sha256(&contract).map_err(|_| {
        ProviderError::schema_mismatch("本机通用视觉作者无法计算限制 contract hash。", false)
    })?;
    Ok(json!({
        "outcome":"limitation",
        "schema_version":"UniversalAuthorOutcome@1",
        "request":request,
        "subject_profile":profile,
        "visual_feature_contract":contract,
        "representation_plan":{
            "schema_version":"RepresentationPlan@1",
            "plan_id":"repplan_local_limited",
            "request_sha256":request_sha256,
            "subject_profile_sha256":profile_sha256,
            "visual_feature_contract_sha256":contract_sha256,
            "capability_manifest_sha256":request["capability_manifest_sha256"],
            "parts":[{"part_id":"part_subject","representation":"deformable","capability_id":"deformable.generic_v1","covered_feature_ids":["feature_subject_macro","feature_subject_meso","feature_subject_micro"],"rationale":"本机过渡作者未启用该表示能力。"}]
        },
        "limitation":{
            "schema_version":"RepresentationLimitation@1",
            "code":"representation_unavailable",
            "message":"本机过渡视觉作者目前只执行银白科幻人形机器人验收样例；没有伪造其他类别的机械臂模板。",
            "affected_part_ids":["part_subject"],
            "missing_capability_ids":["deformable.generic_v1"],
            "suggested_views":["front","side","back"],
            "retryable":true
        }
    }))
}

fn humanoid_profile(request_sha256: &str) -> Value {
    json!({
        "schema_version":"SubjectProfile@1",
        "profile_id":"subject_local_humanoid_robot",
        "request_sha256":request_sha256,
        "identity_label":"银白色科幻人形机器人",
        "category":"fictional humanoid sci-fi robot",
        "category_tags":["humanoid","robot","hard_surface","fictional","game_asset"],
        "silhouette":"高挑双足、前倾头盔、宽肩收腰、分层四肢装甲的完整人形轮廓",
        "negative_space":"头颈间隙、胸腹收束、肩臂与躯干之间以及双腿之间的负空间",
        "pose":"站立、轻微前倾、肩部不对称的展示姿态",
        "visible_views":["front_three_quarter"],
        "occlusions":["背部和关节内侧部分被前方装甲遮挡"],
        "uncertainties":["参考输入主要提供正面三分之二视图，背面细节为推断"],
        "parts":[
            {"part_id":"part_torso","label":"胸腹装甲","semantic_role":"primary_torso_shell","traits":["silver_armor","layered_panel","tapered_waist"],"uncertainty_bps":1800},
            {"part_id":"part_head","label":"头盔式头部","semantic_role":"helmet_head","traits":["forward_visor","inset_neck","smooth_shell"],"uncertainty_bps":2200},
            {"part_id":"part_arms","label":"双臂装甲","semantic_role":"arm_armor","traits":["shoulder_shell","forearm_shell","dark_joint_gap"],"uncertainty_bps":3000},
            {"part_id":"part_legs","label":"双腿装甲","semantic_role":"leg_armor","traits":["hip_joint","knee_shell","shin_shell"],"uncertainty_bps":3200},
            {"part_id":"part_accents","label":"暖金色发光缝","semantic_role":"appearance_accent","traits":["emissive_trim","thin_light"],"uncertainty_bps":4200}
        ],
        "features":[
            {"feature_id":"feature_torso_macro","part_id":"part_torso","level":"macro","description":"胸甲宽度、收腰和腹部整体比例"},
            {"feature_id":"feature_torso_meso","part_id":"part_torso","level":"meso","description":"胸甲分层、中央嵌板与腹部暗色内构"},
            {"feature_id":"feature_torso_micro","part_id":"part_torso","level":"micro","description":"银白涂层、倒角高光和细窄接缝"},
            {"feature_id":"feature_head_macro","part_id":"part_head","level":"macro","description":"前倾头盔和无明显面部的头部轮廓"},
            {"feature_id":"feature_head_meso","part_id":"part_head","level":"meso","description":"面罩、颈部结构与侧向耳部机械件"},
            {"feature_id":"feature_head_micro","part_id":"part_head","level":"micro","description":"面罩边缘高光与暗色缝隙"},
            {"feature_id":"feature_arms_macro","part_id":"part_arms","level":"macro","description":"宽肩、下垂双臂与手臂长度"},
            {"feature_id":"feature_arms_meso","part_id":"part_arms","level":"meso","description":"肩甲、上臂和前臂的分层关系"},
            {"feature_id":"feature_arms_micro","part_id":"part_arms","level":"micro","description":"装甲边缘倒角与关节暗色对比"},
            {"feature_id":"feature_legs_macro","part_id":"part_legs","level":"macro","description":"双足站立、髋膝踝的长腿比例"},
            {"feature_id":"feature_legs_meso","part_id":"part_legs","level":"meso","description":"大腿外壳、膝甲和小腿护板"},
            {"feature_id":"feature_legs_micro","part_id":"part_legs","level":"micro","description":"腿部装甲接缝和局部金属反射"}
        ],
        "materials":[
            {"material_id":"material_silver","label":"银白科幻装甲","part_ids":["part_torso","part_head","part_arms","part_legs"],"appearance_traits":["metallic","silver","painted","high_gloss_edge"]},
            {"material_id":"material_dark","label":"黑色关节内构","part_ids":["part_torso","part_arms","part_legs"],"appearance_traits":["graphite","low_roughness","recessed"]},
            {"material_id":"material_glow","label":"暖金色发光缝","part_ids":["part_accents"],"appearance_traits":["emissive","warm_gold","thin_trim"]}
        ]
    })
}

fn humanoid_feature_contract(request_sha256: &str, profile_sha256: &str) -> Value {
    let levels = [
        (
            "torso",
            "part_torso",
            "胸腹装甲",
            10000_u32,
            vec!["geometry", "normal", "roughness"],
        ),
        (
            "head",
            "part_head",
            "头盔式头部",
            9500_u32,
            vec!["geometry", "normal", "base_color"],
        ),
        (
            "arms",
            "part_arms",
            "双臂分层装甲",
            8500_u32,
            vec!["geometry", "normal", "roughness"],
        ),
        (
            "legs",
            "part_legs",
            "双腿分层装甲",
            8500_u32,
            vec!["geometry", "normal", "roughness"],
        ),
    ];
    let mut requirements = Vec::new();
    for (name, part_id, description, salience, channels) in levels {
        for (level, suffix) in [
            ("macro", "整体形体"),
            ("meso", "分件结构"),
            ("micro", "表面细节"),
        ] {
            requirements.push(json!({
                "feature_id":format!("feature_{name}_{level}"),
                "level":level,
                "description":format!("{description}：{suffix}"),
                "salience_bps":if level == "macro" { salience } else if level == "meso" { salience.saturating_sub(900) } else { salience.saturating_sub(1700) },
                "evidence_status":"inferred",
                "evidence_regions":[],
                "affected_part_ids":[part_id],
                "channels":channels,
                "minimum_acceptance_views":if level == "macro" { json!(["front","iso","side"]) } else { json!(["front","iso"]) }
            }));
        }
    }
    json!({
        "schema_version":"VisualFeatureContract@1",
        "contract_id":"vfcontract_local_humanoid",
        "request_sha256":request_sha256,
        "subject_profile_sha256":profile_sha256,
        "requirements":requirements
    })
}

fn humanoid_representation_plan(
    request_sha256: &str,
    profile_sha256: &str,
    feature_contract_sha256: &str,
    capability_manifest_sha256: Value,
) -> Value {
    json!({
        "schema_version":"RepresentationPlan@1",
        "plan_id":"repplan_local_humanoid",
        "request_sha256":request_sha256,
        "subject_profile_sha256":profile_sha256,
        "visual_feature_contract_sha256":feature_contract_sha256,
        "capability_manifest_sha256":capability_manifest_sha256,
        "parts":[
            {"part_id":"part_torso","representation":"procedural","capability_id":"procedural.generic_hard_surface_v1","covered_feature_ids":["feature_torso_macro","feature_torso_meso","feature_torso_micro"],"rationale":"胸腹硬表面装甲可由 Rust 受限程序化源表达。"},
            {"part_id":"part_head","representation":"procedural","capability_id":"procedural.generic_hard_surface_v1","covered_feature_ids":["feature_head_macro","feature_head_meso","feature_head_micro"],"rationale":"头盔式硬表面外壳可由 Rust 受限程序化源表达。"},
            {"part_id":"part_arms","representation":"procedural","capability_id":"procedural.generic_hard_surface_v1","covered_feature_ids":["feature_arms_macro","feature_arms_meso","feature_arms_micro"],"rationale":"双臂分层装甲可由受限 box、bevel 和 array 表达。"},
            {"part_id":"part_legs","representation":"procedural","capability_id":"procedural.generic_hard_surface_v1","covered_feature_ids":["feature_legs_macro","feature_legs_meso","feature_legs_micro"],"rationale":"双腿装甲可由受限 box、bevel 和 array 表达。"},
            {"part_id":"part_accents","representation":"procedural","capability_id":"procedural.generic_hard_surface_v1","covered_feature_ids":[],"rationale":"发光缝作为可编辑外观部件绑定到同一程序化源。"}
        ]
    })
}

fn humanoid_geometry_program() -> Value {
    json!({
        "schema_version":"ForgeVisualGeometryProgram@2",
        "program_id":"visual_local_humanoid_robot",
        "domain":"generic_hard_surface",
        "units":"millimeter",
        "seed":73101,
        "materials":[
            {"material_id":"mat_silver","base_material_id":"mat_aluminum"},
            {"material_id":"mat_dark","base_material_id":"mat_graphite"},
            {"material_id":"mat_glow","base_material_id":"mat_emissive_blue"}
        ],
        "profiles":[],
        "section_sets":[],
        "nodes":[
            {"kind":"box","node_id":"node_torso","size":[360.0,220.0,560.0],"position":[0.0,0.0,120.0]},
            {"kind":"bevel_approx","node_id":"node_torso_bevel","input_node_id":"node_torso","radius":28.0,"segments":2},
            {"kind":"surface_panel","node_id":"node_torso_panel","input_node_id":"node_torso_bevel","size":[250.0,12.0,150.0],"position":[0.0,0.0,160.0],"axis":"positive_y"},
            {"kind":"part","node_id":"node_torso_part","input_node_id":"node_torso_panel","part_id":"part_torso","role":"primary_torso_shell"},
            {"kind":"material_zone","node_id":"node_torso_zone","input_node_id":"node_torso_part","zone_id":"zone_torso_silver","material_id":"mat_silver"},
            {"kind":"box","node_id":"node_head","size":[260.0,240.0,240.0],"position":[0.0,12.0,535.0],"rotation":[-0.12,0.0,0.0]},
            {"kind":"bevel_approx","node_id":"node_head_bevel","input_node_id":"node_head","radius":34.0,"segments":2},
            {"kind":"part","node_id":"node_head_part","input_node_id":"node_head_bevel","part_id":"part_head","role":"helmet_head_shell"},
            {"kind":"material_zone","node_id":"node_head_zone","input_node_id":"node_head_part","zone_id":"zone_head_silver","material_id":"mat_silver"},
            {"kind":"box","node_id":"node_arms","size":[145.0,180.0,450.0],"position":[-270.0,0.0,125.0]},
            {"kind":"bevel_approx","node_id":"node_arms_bevel","input_node_id":"node_arms","radius":22.0,"segments":2},
            {"kind":"array","node_id":"node_arms_array","input_node_id":"node_arms_bevel","axis":"x","count":2,"spacing":540.0},
            {"kind":"part","node_id":"node_arms_part","input_node_id":"node_arms_array","part_id":"part_arms","role":"arm_armor_pair"},
            {"kind":"material_zone","node_id":"node_arms_zone","input_node_id":"node_arms_part","zone_id":"zone_arms_silver","material_id":"mat_silver"},
            {"kind":"box","node_id":"node_legs","size":[165.0,220.0,610.0],"position":[-105.0,0.0,-430.0]},
            {"kind":"bevel_approx","node_id":"node_legs_bevel","input_node_id":"node_legs","radius":24.0,"segments":2},
            {"kind":"array","node_id":"node_legs_array","input_node_id":"node_legs_bevel","axis":"x","count":2,"spacing":210.0},
            {"kind":"part","node_id":"node_legs_part","input_node_id":"node_legs_array","part_id":"part_legs","role":"leg_armor_pair"},
            {"kind":"material_zone","node_id":"node_legs_zone","input_node_id":"node_legs_part","zone_id":"zone_legs_silver","material_id":"mat_silver"},
            {"kind":"box","node_id":"node_accents","size":[22.0,12.0,170.0],"position":[-170.0,-118.0,190.0]},
            {"kind":"array","node_id":"node_accents_array","input_node_id":"node_accents","axis":"x","count":2,"spacing":340.0},
            {"kind":"part","node_id":"node_accents_part","input_node_id":"node_accents_array","part_id":"part_accents","role":"warm_emissive_trim"},
            {"kind":"material_zone","node_id":"node_accents_zone","input_node_id":"node_accents_part","zone_id":"zone_accents_glow","material_id":"mat_glow"}
        ],
        "outputs":[
            {"output_id":"output_torso","node_id":"node_torso_zone"},
            {"output_id":"output_head","node_id":"node_head_zone"},
            {"output_id":"output_arms","node_id":"node_arms_zone"},
            {"output_id":"output_legs","node_id":"node_legs_zone"},
            {"output_id":"output_accents","node_id":"node_accents_zone"}
        ],
        "budgets":{"schema_version":"GeometryProgramBudget@1","max_profiles":2,"max_section_sets":1,"max_nodes":24,"max_parts":6,"max_materials":4,"max_outputs":6,"max_operations":32,"triangle_budget":100000}
    })
}

fn tool_response(outcome: Value, call_id: &str) -> Result<ProviderResponse, ProviderError> {
    Ok(ProviderResponse {
        content: None,
        tool_calls: vec![ProviderToolCall {
            call_id: call_id.into(),
            name: AUTHOR_TOOL.into(),
            arguments: json!({"outcome":outcome}),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_humanoid_geometry_is_a_bounded_non_csg_program() {
        let source = humanoid_geometry_program();
        let lowering = forgecad_core::lower_visual_runtime_source_v1(&source)
            .expect("local humanoid geometry must lower");
        let operations = lowering.shape_program["operations"]
            .as_array()
            .expect("lowering must expose operations");
        assert!(operations
            .iter()
            .all(|operation| { !matches!(operation["op"].as_str(), Some("union" | "subtract")) }));
        assert_eq!(
            lowering.shape_program["outputs"].as_array().unwrap().len(),
            5
        );
    }

    #[test]
    fn local_humanoid_author_arguments_match_the_product_tool_boundary() {
        let request = json!({
            "schema_version":"UniversalAuthorRequest@1",
            "request_id":"request_local_test",
            "project_id":"project_local_test",
            "turn_id":"turn_local_test",
            "instruction":"银白色科幻人形机器人",
            "input_mode":"text",
            "reference_inputs":[],
            "active_asset":null,
            "selection":{"part_ids":[],"material_zone_ids":[]},
            "locks":{"preserve_geometry":false,"preserve_material_surface":false,"locked_part_ids":[],"locked_material_zone_ids":[]},
            "capability_manifest_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        });
        let outcome =
            executable_humanoid_outcome(request).expect("local author fixture should build");
        let registry = forgecad_app_server::ProductToolRegistry::default();
        let call = ProviderToolCall {
            call_id: "local_test_call".into(),
            name: AUTHOR_TOOL.into(),
            arguments: json!({"outcome": outcome}),
        };
        registry
            .build_execution_request(
                "turn_local_test",
                &call,
                "execution_local_test",
                "cancel_local_test",
                "token_local_test",
            )
            .expect("local author outcome must pass the Product Tool schema");
    }

    #[test]
    fn local_open_categories_produce_distinct_validated_visual_sources() {
        for instruction in [
            "生成一只写实猫",
            "生成一辆银色未来汽车",
            "生成一座临海建筑",
            "生成一棵有层次的科幻树",
            "生成一个现代人物角色",
            "生成一台三关节机械臂",
            "生成一把虚构游戏能量剑",
            "生成一个没有类别标签的抽象物体",
        ] {
            let request = json!({
                "schema_version":"UniversalAuthorRequest@1",
                "request_id":"request_local_open_category",
                "project_id":"project_local_open_category",
                "turn_id":"turn_local_open_category",
                "instruction":instruction,
                "input_mode":"text",
                "reference_inputs":[],
                "active_asset":null,
                "selection":{"part_ids":[],"material_zone_ids":[]},
                "locks":{"preserve_geometry":false,"preserve_material_surface":false,"locked_part_ids":[],"locked_material_zone_ids":[]},
                "capability_manifest_sha256":forgecad_core::representation_capability_manifest_sha256().unwrap()
            });
            let outcome_value = executable_generic_visual_outcome(request, instruction)
                .expect("open category author should return an executable outcome");
            let outcome: forgecad_core::UniversalAuthorOutcome =
                serde_json::from_value(outcome_value).expect("outcome must deserialize");
            outcome
                .validate(&[])
                .expect("open category outcome must pass Rust universal validation");
            let forgecad_core::UniversalAuthorOutcome::Executable {
                request,
                subject_profile,
                visual_feature_contract,
                representation_plan,
                executable_payload,
                ..
            } = outcome
            else {
                panic!("open category outcome must be executable");
            };
            assert!(representation_plan
                .parts
                .iter()
                .all(|part| part.capability_id == "procedural.generic_visual_exterior_v1"));
            forgecad_core::lower_visual_runtime_source_v1(&executable_payload)
                .expect("open category geometry must remain within the bounded lowering contract");
            let source = forgecad_core::UniversalAssetSourceV2::from_runtime_procedural(
                &request,
                &subject_profile,
                &visual_feature_contract,
                &representation_plan,
                executable_payload,
            )
            .expect("open category source should compile into UAS@2");
            source
                .validate()
                .expect("open category UAS@2 must validate");
        }
        assert!(!is_supported_humanoid_brief("生成一台三关节机械臂"));
        assert!(!is_supported_humanoid_brief("生成一只机器狗"));
    }

    #[test]
    fn local_visual_prompt_changes_bounded_geometry_not_only_program_seed() {
        let vehicle_specs = local_visual_parts(LocalVisualArchetype::Vehicle);
        let compact = generic_visual_geometry_program(
            LocalVisualArchetype::Vehicle,
            &vehicle_specs,
            "一辆紧凑、低矮、银色的城市巡检车，短车身、宽轮距、橙色灯带",
        );
        let long_range = generic_visual_geometry_program(
            LocalVisualArchetype::Vehicle,
            &vehicle_specs,
            "一辆细长、抬高底盘、白色的荒漠运输车，长车身、窄舱体、蓝色灯带",
        );

        assert_ne!(compact["seed"], long_range["seed"]);
        assert_ne!(compact["program_id"], long_range["program_id"]);
        assert_ne!(compact["nodes"], long_range["nodes"]);
        forgecad_core::lower_visual_runtime_source_v1(&compact)
            .expect("first prompt must remain within the bounded geometry language");
        forgecad_core::lower_visual_runtime_source_v1(&long_range)
            .expect("second prompt must remain within the bounded geometry language");
    }

    #[test]
    fn local_visual_prompt_changes_reviewed_materials_and_structural_patterns() {
        let vehicle_specs = local_visual_parts(LocalVisualArchetype::Vehicle);
        let silver = generic_visual_geometry_program(
            LocalVisualArchetype::Vehicle,
            &vehicle_specs,
            "一辆银色未来汽车，透明座舱、四个橡胶轮胎和蓝色灯带",
        );
        let red = generic_visual_geometry_program(
            LocalVisualArchetype::Vehicle,
            &vehicle_specs,
            "一辆红色未来汽车，透明座舱、四个橡胶轮胎和蓝色灯带",
        );

        let silver_materials = silver["materials"]
            .as_array()
            .expect("silver program must expose reviewed materials");
        let red_materials = red["materials"]
            .as_array()
            .expect("red program must expose reviewed materials");
        assert!(silver_materials
            .iter()
            .any(|material| material["material_id"] == "mat_aluminum"));
        assert!(red_materials
            .iter()
            .any(|material| material["material_id"] == "mat_signal_red"));
        assert!(silver_materials
            .iter()
            .any(|material| material["material_id"] == "mat_dark_glass"));
        assert!(silver_materials
            .iter()
            .any(|material| material["material_id"] == "mat_rubber"));

        let silver_nodes = silver["nodes"]
            .as_array()
            .expect("silver program must expose nodes");
        assert!(silver_nodes
            .iter()
            .any(|node| node["kind"] == "array" && node["count"] == 2));
        assert_ne!(silver["materials"], red["materials"]);
        forgecad_core::lower_visual_runtime_source_v1(&silver)
            .expect("silver program must remain inside reviewed lowering");
        forgecad_core::lower_visual_runtime_source_v1(&red)
            .expect("red program must remain inside reviewed lowering");
    }

    #[test]
    fn local_visual_parts_keep_non_mechanical_semantic_patterns() {
        let plant = generic_visual_geometry_program(
            LocalVisualArchetype::Plant,
            &local_visual_parts(LocalVisualArchetype::Plant),
            "一棵有四向分枝和自然哑光表面的科幻树",
        );
        let animal = generic_visual_geometry_program(
            LocalVisualArchetype::Animal,
            &local_visual_parts(LocalVisualArchetype::Animal),
            "一只四足动物，柔软表面和四条腿",
        );

        assert!(plant["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| { node["kind"] == "radial_array" && node["count"] == 4 }));
        assert!(animal["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| { node["kind"] == "radial_array" && node["count"] == 4 }));
        assert!(plant["materials"]
            .as_array()
            .unwrap()
            .iter()
            .any(|material| material["material_id"] == "mat_composite"));
        assert!(animal["materials"]
            .as_array()
            .unwrap()
            .iter()
            .any(|material| material["material_id"] == "mat_abs_matte"));
    }

    #[test]
    fn local_open_category_materials_do_not_collapse_into_robotic_surfaces() {
        let plant = generic_visual_geometry_program(
            LocalVisualArchetype::Plant,
            &local_visual_parts(LocalVisualArchetype::Plant),
            "一棵有树皮、绿色叶片和自然哑光表面的树",
        );
        let animal = generic_visual_geometry_program(
            LocalVisualArchetype::Animal,
            &local_visual_parts(LocalVisualArchetype::Animal),
            "一只覆盖棕色毛发的四足动物",
        );
        let character = generic_visual_geometry_program(
            LocalVisualArchetype::Character,
            &local_visual_parts(LocalVisualArchetype::Character),
            "一个穿蓝色织物服装、具有自然肤色的角色",
        );

        let material_ids = |program: &Value| {
            program["materials"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|material| material["material_id"].as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        };
        let plant_ids = material_ids(&plant);
        let animal_ids = material_ids(&animal);
        let character_ids = material_ids(&character);
        assert!(plant_ids.iter().any(|id| id == "mat_composite"));
        assert!(!plant_ids.iter().any(|id| id == "mat_emissive_blue"));
        assert!(animal_ids.iter().any(|id| id == "mat_abs_matte"));
        assert!(!animal_ids.iter().any(|id| id == "mat_rubber"));
        assert!(character_ids.iter().any(|id| id == "mat_composite"));
        assert!(character_ids.iter().any(|id| id == "mat_abs_matte"));
        forgecad_core::lower_visual_runtime_source_v1(&plant)
            .expect("plant visual source must remain within the bounded route");
        forgecad_core::lower_visual_runtime_source_v1(&animal)
            .expect("animal visual source must remain within the bounded route");
        forgecad_core::lower_visual_runtime_source_v1(&character)
            .expect("character visual source must remain within the bounded route");
    }
}
