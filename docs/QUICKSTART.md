# Quickstart

This guide is the release walkthrough for running Wushen Forge as a local desktop Agent during the first production phase.

The product boundary is unchanged: Wushen Forge creates fictional Unity game-art assets and non-manufacturing descriptions. It must not output real-world weapon blueprints, manufacturing dimensions, material recipes, fabrication steps, or assembly instructions.

本版本建议理解为“先结构解释再重铸生成”：

```text
text/sketch -> structure interpretation -> Creative Recast -> recast/confirm
-> CreativeWeaponGraph -> WeaponDesignSpec -> concept -> patch -> SkillGraph
-> rough 3D exhibition rig -> Unity export ZIP
```

目标态新增约束：

- 不要求用户先选“武器类型”；输入只要包含可解释结构与意图即可进入 interpretation。
- interpretation 返回必须是 2~3 条候选，且候选之间应有明显 `combat_affordances` 差异。
- interpretation 若首次返回 1/0 条候选，或出现三候选全部退化到同一能力型，后端先重采样一次；重采样仍失败时返回 `PROVIDER_BAD_OUTPUT` 并阻断。
- 只有确认候选后才允许继续 concept / patch / 3D / export。

当前门禁说明（与 `release:prompt-quality` 对齐）：

- 本轮仍以 `WeaponDesignSpec@1` 为硬门禁，`creative_graph` / `skill_graph` 字段视为目标态兼容字段。
- `release:prompt-quality` 与 `release:safety-scope` 只要求现状字段不回退制造参数、保持非制造边界。
- `creative_graph` 与 `skill_graph` 的完整性是下一阶段目标，不是本轮阻断条件。

本阶段验收提示词（非分类回归）：

- 防弹裤神炮
- 木棍大炮
- 镜子召唤门
- 椅子王座炮台
- 铃铛封印阵
- 树枝龙骨炮

## Prerequisites

- Node.js 20 or newer
- npm 10 or newer
- Python 3.9 or newer
- Optional for Tauri packaging: Rust with Cargo
- Optional for Unity release gate: Unity Editor with glTFast support
- Optional for real image generation: a running ComfyUI service
- Optional for real 3D generation: a local Wushen 3D runtime wrapper or compatible HTTP service

## Install

From the repository root:

```bash
npm install
python3 -m venv .venv
.venv/bin/pip install -e apps/agent
```

For development checks:

```bash
.venv/bin/pip install -e "apps/agent[dev]"
```

## Run The Local Agent

The desktop app talks only to the local FastAPI Agent. The frontend must not call LLM, ComfyUI, or 3D providers directly.

```bash
PYTHONPATH=apps/agent \
WUSHEN_LIBRARY_ROOT="$PWD/WushenForgeLibrary" \
WUSHEN_MIGRATIONS_DIR="$PWD/migrations" \
.venv/bin/python -m uvicorn wushen_agent.main:create_app --factory --host 127.0.0.1 --port 8000
```

Health check:

```bash
curl http://127.0.0.1:8000/api/health
curl http://127.0.0.1:8000/api/provider-settings
```

## Run The Desktop Workbench

In another terminal:

```bash
npm run desktop:dev
```

Browser/Vite mode reads `VITE_FORGE_API_BASE_URL` when the Agent is not on the default URL:

```bash
VITE_FORGE_API_BASE_URL=http://127.0.0.1:8000 npm run desktop:dev
```

In Tauri mode, Settings can start and stop the local development Agent supervisor described in `docs/M3_DESKTOP_SUPERVISOR.md`.

## Mock Asset Loop

The default providers are mock providers. They are enough to verify the first-stage workflow without external model services:

```text
text/sketch -> structure interpretation -> Creative Recast -> CreativeWeaponGraph -> WeaponDesignSpec -> concept -> patch -> SkillGraph -> rough 3D exhibition rig -> Unity export ZIP
```

示例（目标态）：

```
文本输入: “这是一条防弹裤，给它一个神化解释”
```

系统应返回：先给出 2~3 条结构解释候选（如“腰部环形炮架防御外壳”/“风雷机动神兵”），用户确认后再生成概念图。

建议把每轮手工验收固定为四件事：

1) `/interpretation` 返回候选数量为 `2` 或 `3`。
2) 同一输入至少给出两种不同 `combat_affordances` 方向（例如 `shield + area_control` 与 `mobility + projectile`）。
3) 同一输入重复复跑时，至少保留一个候选 `rank` 与核心能力方向不退化。
4) 未确认前不能直接调用 concept / patch / generate-3d / export-unity。

建议输入样例（用于验证非特化闭环）：

- 防弹裤 -> 腰部炮阵 + 护体领域
- 木棍 -> 符文炮杖
- 椅子 -> 王座炮台 / 折叠盾阵
- 镜子 -> 召唤门 / 反射法器
- 伞 -> 天幕阵 / 针雨伞
- 门 -> 传送门阵 / 传送枢纽炮
- 戒指 -> 玄纹护符场 / 召唤触发环
- 树枝 -> 龙骨锁链刃 / 祭坛触发杖
- 钥匙 -> 选择/传送触发器 / 目标锁定阀
- 花盆 -> 风元素核心 + 守域护甲
- 风车 -> 反射片 + 持续伤害旋场
- 铃铛 -> 警戒域 / 位移扰乱阵
- 书卷 -> 召唤法阵 / 符文弹幕
- 柱墩 -> 区域控制塔 / 护甲支点
- 沙包 -> 冲击缓冲/反制位移

建议每周补一轮对象池：
`椅子 -> 防御炮台`，`贝壳 -> 共鸣护罩`，`竹简 -> 能量导轨器`，`车把 -> 位移桩`，`花环 -> 持续领域`。

更广对象池（建议每周至少新增 3 条）：

- 工具类：锤子、剪刀、锯片、钩爪、缝纫机、吹风机
- 日用品：书签、毛巾、手套、雨伞、围巾、发卡、吊牌、口红
- 家具/空间：长椅、楼梯、栏杆、车把、轮胎、门把手、书架
- 自然/抽象形态：贝壳、藤蔓、网、齿轮、环、弧线框架、悬浮球、风铃

判定逻辑（非特化闭环）：

- 任何对象都必须先通过 `interpretation`，并返回 2~3 条候选；
- 候选必须出现至少 2 条不同的 affordance 主线；
- 选中候选必须调用 `/recast/confirm` 后方可进入概念图；
- `generate-3d` 与 `export-unity` 在未确认前一律 `INTERPRETATION_NOT_CONFIRMED`。
- `PROVIDER_BAD_OUTPUT` 表示解释候选质量不合格，不表示该对象不能被神化；下一步是补充结构标注或重跑解释。

自由度滑块（可选，目标用于平衡美术与游戏可用性）：

- 形态自由度：保守 / 奇异 / 异形 / 超现实
- 神化程度：现实材质感 / 国风神兵 / 仙术机关 / 神话概念
- 玩法复杂度：轻量攻击 / 多段技能 / 变形联动 / 多形态机制
- 资产可用性：概念优先 / 概念平衡 / Unity 可用优先


Core API calls are:

```text
POST /api/weapons
POST /api/weapons/{weapon_id}/interpretation
POST /api/weapons/{weapon_id}/recast/confirm
GET /api/weapons/{weapon_id}/creative-graph
POST /api/weapons/{weapon_id}/skill-graph
POST /api/weapons/{weapon_id}/patch
POST /api/weapons/{weapon_id}/generate-3d
POST /api/weapons/{weapon_id}/export-unity
GET /api/jobs/{job_id}/events
GET /api/weapons/{weapon_id}
```

Every mutating request requires an `Idempotency-Key` header.

The desktop flow is:

1. Open Forge.
2. Enter one idea (any object/shape, including non-weapon objects) or upload a sketch.
3. Run interpretation and pick one `structure_candidate`.
4. Confirm candidate via `/recast/confirm` (writes `creative_graph_id`).
5. Generate one concept from confirmed structure only after confirm lock-in.
6. Use Patch Mode for local edits (brush/lasso + mask).
7. Generate the rough 3D model.
8. Inspect the 360-degree exhibition rig: pedestal, simple character, and held weapon.
9. Export the Unity package from the 3D panel or asset library.

对象池建议（每周扩充）：

- 木棍 / 椅子 / 镜子 / 雨伞 / 钥匙 / 花盆 / 风车 / 树枝 / 书卷 / 花环 / 门把。
- 每个对象至少覆盖两种形态：保守/奇异 或 高现实/玄幻。
- 每周新增至少 2 个对象到 `scripts/check_release_prompt_quality.py` 测试集中。

更多“更高细节目标”用于手工验收：

- 结构解释必须包含：`anchor_points`、`protected_regions`、`risk_tags`
- 候选解释中每一项都要说明与至少一种 `combat_affordances` 的映射关系
- 至少两项候选在 `combat_affordances` 组合上互斥（例如 `mobility+projectile` vs `shield+area_control`）
- 用户确认后，重复同一输入第二次复测时允许轻微波动，但不能出现候选集合全部偏离初始核心能力方向的退化
- 3D 展台验收要求：同一输入可出 `held` 与 `worn` 两种姿态映射提示（角色握持点/穿戴点）

GPT Pro 纵向验收（建议每次执行至少覆盖）：

- 输入为非武器对象时，`interpretation` 必须返回 2~3 条候选。
- 每条候选都需含 `combat_affordances` 与风险标签。
- 未执行 `recast/confirm` 前不得进入概念图/3D。
- 概念图通过质量 gate 后才允许执行 Patch 与 `generate-3d`。
- `generate-3d` 输出要能被展台加载并允许 360 拖拽。
- 导出 Unity ZIP 后需有 manifest、rough_normalized_glb、unity_material_json、quality_report、zip 快照一致性。

目标态还建议补一轮技能重生验证：先生成 `skill-graph`，再按槽位重生成 1~2 张技能卡，观察 `combat_affordances` 映射是否仍稳定。

每周结构解释回归建议（执行清单）：

- 重跑至少 1 条同一非武器输入 2 次，确认至少一项候选 `rank` 不回退。
- 新增 5 条对象样本并要求产生 2~3 个候选。
- 至少一条候选必须绑定 1 个 `protected_regions` + 1 个 `risk_tags`。
- 任意切换候选后，`PATCH` 与 `generate-3d` 端点需强制要求 `creative_graph_id` 一致，否则提示 `INTERPRETATION_NOT_CONFIRMED`。

你可以通过 `POST /api/weapons/{weapon_id}/interpretation` 查看返回的候选数量是否达到 2~3 个，防止系统退回传统“类型分类”模式。

反模式检查点（新手工验收）：

- 不显示“先选分类”入口。
- `interpretation` 返回候选必须是 2~3 条；若为 1 条或 0 条直接视为回归。
- `recast/confirm` 前不能直接调用 `generate-3d` 或 `export-unity`。

## Configure LLM Provider

Mock LLM is the default. For an OpenAI-compatible provider:

```bash
export WUSHEN_LLM_PROVIDER=openai_compatible
export WUSHEN_LLM_BASE_URL=https://api.openai.com/v1
export WUSHEN_LLM_MODEL=<model-name>
export WUSHEN_LLM_API_KEY=<secret>
```

Secrets must come from environment variables or secret files. They must not be committed to source, written into job events, saved in asset files, or exported inside Unity packages.

## Configure ComfyUI

Mock image generation is the default. To use ComfyUI:

```bash
export WUSHEN_IMAGE_PROVIDER=comfyui
export WUSHEN_COMFYUI_BASE_URL=http://127.0.0.1:8188
export WUSHEN_COMFYUI_CHECKPOINT=<checkpoint-name>
export WUSHEN_COMFYUI_WORKFLOW_TEMPLATE=/absolute/path/to/concept_api_workflow.json
export WUSHEN_COMFYUI_PATCH_WORKFLOW_TEMPLATE=/absolute/path/to/patch_api_workflow.json
```

Workflow template rules are documented in `workflows/comfyui/README.md`.

## Configure Local 3D Runtime

Mock 3D is the default. To use a local HTTP 3D service:

```bash
export WUSHEN_3D_PROVIDER=local_http
export WUSHEN_3D_HTTP_BASE_URL=http://127.0.0.1:8787
export WUSHEN_GENERATE3D_ASYNC=1
```

To run the bundled protocol wrapper in mock mode:

```bash
.venv/bin/python scripts/wushen_local_3d_runtime.py --backend mock --host 127.0.0.1 --port 8787
```

Stable Fast 3D and TripoSR manual verification are documented in `docs/LOCAL_3D_RUNTIME.md`.

## Unity Export And Import

Unity ZIP export is available through:

```text
POST /api/weapons/{weapon_id}/export-unity
```

Development preflight:

```bash
npm run unity:preflight
```

Release import gate:

```bash
export WUSHEN_UNITY_EXECUTABLE=/Applications/Unity/Hub/Editor/<version>/Unity.app/Contents/MacOS/Unity
npm run unity:import:gate
```

`unity:import:gate` must fail when Unity is not configured. This prevents claiming release readiness from ZIP preflight alone.

## Release Gates

Before a production release candidate:

```bash
npm run m5:gate
npm run release:safety-scope
npm run release:secrets-files
npm run release:prompt-quality
npm run release:docs-walkthrough
npm run release:packaging-readiness
npm run release:license-sbom
npm run release:gate
```

The aggregate `npm run release:gate` intentionally blocks while license/SBOM, Unity import, real provider evidence, or desktop packaging blockers remain unresolved.
