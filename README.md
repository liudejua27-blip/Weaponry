# Weaponry — CrossFire Weapon Agent-native DCC

> **Weaponry P0 override (2026-08-29):** 本文只有在与 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 和 ADR-0029 一致时才具有当前执行权。ForgeCAD 在本文中解释为 Weaponry 的 Rust Runtime lineage；当前唯一产品主线是由 Codex 生成、修改、验证并交付高质量穿越火线非功能性游戏武器。通用 3D、机器人和原创科幻示例仅作 fixture/历史能力，不得抢占本月主线。 本文中所有 2026-08-28 及更早的“当前”“下一原子”“唯一 `in_progress`”和工具/Schema 数量语句均按历史 cohort 解释，不得覆盖 `WPN-*` successor queue。

Weaponry 的目标不是生成“像枪的 GLB”，而是让 Codex 使用 Rust 原生的可编辑拓扑、
原子建模事务、Modifier/Evaluation Graph、High/Low/UV/Cage/Bake/PBR、FPS 镜头、
引擎回读和独立武器美术人审，生产可迭代、可验证、可回退的穿越火线游戏武器资产。

当前实现仍未达到这个目标：`QUALITY_TARGET_NOT_MET / commercial=NOT_PROVEN`。
最新产品权威从[穿越火线产品宪章](docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md)
开始阅读；历史工具数量和结构回执不能替代真实资产验收。

> 2026-08-28 `FPS-FORM-04BE-C` 当前权威状态：真实 D1 final candidate 已完成同 cohort **6×9=54 AOV**，并将 parent composite proposal、CrossView、proposal FormArt 与 receipt 固化为 Runtime-owned immutable evidence sidecar；same-key replay 与隔离 Runtime 重启 GET hash equality PASS。当前公共面为 **575 schemas / 28 operator entries / 126 read + 94 opt-in write = 220 MCP tools**。质量没有通过：CrossView=`rejected-regression`、FormArt=`BLOCKED_PROPOSAL_FORM_ART_EVIDENCE`、FormQualityV2 未创建，Stage=`camera-calibrated`、secondary=`NOT_CREATED`、confirm/version/export=false、commercial=`NOT_PROVEN`。下一原子 `FPS-FORM-04BE-D` 只生成绑定 exact evidence hashes 的 rear-stock owner-void/left-boundary typed repair plan。

> 2026-08-27 `FPS-FORM-04AL` 当前增量：Runtime-owned durable fresh six-view baseline producer 已接通合同、Store、Runtime 与 MCP `prepare/get`；每个视图绑定 approved registration lineage / RigV2、fresh same-cohort 512×512 九 AOV、camera/mask/compare/quality 与完整 CAS reachability，并以单事务持久化。精确状态为 `PASS_SOURCE_COMPILE_DURABLE_PRODUCER_NOT_RUN_REAL_D1`；真实 D1、orientation approval、fresh baseline、notch、secondary、Stage/confirm/version/export 均未执行。当前公共面 **538 schemas / 118 read + 88 opt-in write = 206 tools**，视觉仍 `QUALITY_TARGET_NOT_MET`。

> 2026-08-27 `FPS-FORM-04AK` 当前权威口径：**533 schemas / 28 operator entries / 117 read + 87 opt-in write = 204 MCP tools**。新增 lineage-bound fresh-baseline read-only preflight，materializer 明确 unavailable，真实 D1 零写；旧 FormArt 不复用，Stage/secondary/quality 仍为 `camera-calibrated / NOT_CREATED / QUALITY_TARGET_NOT_MET`。notch proposal 原子性已收紧，但最终 child commit 失败后的下游补偿仍未闭合。

> 2026-08-26 `FPS-AUTHORING-MESH-V2-03` 历史权威口径：**527 schemas / 28 operator entries / 115 read + 87 opt-in write = 202 MCP tools**。真实 D1 `rear-stock` 已用 stable-ID `MoveVertices` 生成持久 child revision，仅替换一个 GeometryProgram source node，完成 Worker hash/compile/replay/strict GLB readback 与 Runtime restart 后六视图 immutable replay。当前最佳 Z taper 的旧 strict-Pareto 门仍因 top/rear-3q 的明确小回退而拒绝；新 `form-art-secondary-pareto-review@1` 只将其标记为“可审阅权衡”，且因派生 candidate 未生成 fresh proposal-side FormArt owner/void/Part-ID 证据而 `BLOCKED_FORMART_OWNER_EVIDENCE`。不可 confirm/推进 Stage；`secondary-form-approved=NOT_CREATED / QUALITY_TARGET_NOT_MET / commercial=NOT_PROVEN`。

> 2026-08-26 `FPS-FORM-04AF` 当前权威口径：**527 schemas / 28 operator entries / 114 read + 85 opt-in write = 199 MCP tools**。真实 D1 的 `rear-stock-profile-reconstruction-v1` 已完成 Runtime-owned source materialize→readback→六视图 render→CrossView evaluate；front/back/top 不回退，left/right/rear-three-quarter 回退，因此 proposal 被正确拒绝，没有 confirm/version/export。`AuthoringMesh@2` 另完成真实 Runtime 持久化 genesis→`split_edge`→restart exact readback，但仍是 structural-only，尚未绑定真实武器 High。CameraLock 的诊断性 180° 仍缺 orientation-specific 用户回执，不得推断批准。资产状态仍为 `Stage=camera-calibrated / QUALITY_TARGET_NOT_MET / commercial=NOT_PROVEN`。

> 2026-08-26 `FPS-FORM-04AE` 当前权威 source 口径：**523 schemas / 28 operator entries / 112 read + 84 opt-in write = 196 MCP tools**。CameraLock additive child 的 Contracts/Store/Runtime/MCP 已编译通过，但 real D1 仍等 orientation-specific user approval；旧数量仅作历史 cohort。

> 商业 FPS Hero Weapon 的最新产品蓝图见 `docs/FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。当前 ForgeCAD 仍为 `QUALITY_TARGET_NOT_MET`；目标合同、开源研究或 source compile 不能替代真实 Form→Engine→Human 闭环。

> 2026-08-26 Goal 纠偏：ForgeCAD 的商业终点是 Codex 经 MCP 交付同一 lineage/hash 的 `HeroSourceAsset@1 + FpsPresentationPackage@1 + EngineDeliveryPackage@1`，并通过真实 FPS 固定镜头、目标引擎 round-trip 与独立 Hero Art Review。Schema/工具数量、source compile、GLB 可打开和 Three.js smoke 只算基础设施。当前机器状态仍为 `Stage=camera-calibrated / QUALITY_TARGET_NOT_MET`；唯一下一资产动作是让真实 D1 的 FormArt 归因闭合并修正一个 source node，取得 `secondary-form-approved` 后再进入 Authoring/High。完整路线见 `docs/COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md` 与 ADR-0027。

> 2026-08-26 最新实现增量：Formal High 的 public idempotency key 已持久化，并以 `(project_id, session_id, idempotency_key)` 唯一约束实现 exact replay/conflict；`production_weapon_formal_high_get/prepare` 已接入默认读/显式 opt-in write MCP、Ponytail preflight、1 MiB wire budget、闭合输入输出校验和 Runtime IPC。Formal High 的 Worker `source-preserved` 已规范化为生产合同允许的 authoring-topology=`partial`。Legacy FormQuality 也已修正为 blockout/primary/secondary 分别绑定 `camera-calibrated/blockout-reviewed/primary-form-approved` 当前 head。Contracts/Store/Runtime/MCP compile PASS，Store 3/3、FormQuality 14/14、Formal High MCP 4/4 聚焦测试 PASS。完整合法 secondary head 下的 Formal High positive/restart/cleanup 与真实 D1 仍 `NOT_RUN/NOT_PROVEN`，所以不构成 Stage、视觉或商业质量 PASS；D1 继续是 Stage=`camera-calibrated`、`secondary-form-approved=NOT_CREATED`、visual=`QUALITY_TARGET_NOT_MET`、human/engine/distribution=`NOT_RUN`、commercial=`NOT_PROVEN`、HQ360=`BLOCKED_REFERENCE_COVERAGE`。

> 2026-08-26 最新权威 source 口径（取代下方 2026-08-25 的“最新/当前”计数）：**515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**。Formal High public surface 已 source-exposed，Store idempotency 已落地；但完整 source-lineage/CAS positive materialize→drop/reopen/cleanup fixture 仍缺失，真实 D1 也没有合法 secondary head，因此新建 prepare 仍应 fail closed、零写。Low/Hero UV durable、Cage/Bake seam 与 Formal High source surface 都只是 structural/source 能力，不是 artist unwrap、正式 Bake、visual、human、engine、commercial 或 packaged PASS；不推进 Stage、confirm、version 或 export。

> 2026-08-25 历史快照（已由上方 2026-08-26 权威口径取代）—商业资产 source 口径：当前为 **499 schemas / 28 operator entries / 107 read + 79 opt-in write = 186 tools**。Native High 已增加确定性的 1–3 段面内 support-loop chamfer arc，并以新 cohort 完成 Worker **6/6 + 7/7** 与 Runtime restart **1/1 PASS**；Editable Low 新增显式四边面草稿 producer/readback；Hero UV 新增 2K/4K、UV0/UV1、可见性权重、padding/seam/stretch/overlap/Mikk 诊断；Viewer 新增只读 Art Director 生产链矩阵。Low/UV 仍是 Worker-only，High proposal 仍 `registered=false`，没有 packaged/candidate visual/human/engine/distribution PASS，所以 `FPS-HIGH-05=NOT_PASSED`、Stage=`camera-calibrated`、visual=`QUALITY_TARGET_NOT_MET`、HQ360=`BLOCKED_REFERENCE_COVERAGE`，没有 stage/confirm/version/export。证据见 `docs/evidence/mcp010f/commercial-weapon-authoring-slices-source-gate-20260825.json`。

2026-08-25 `CQ-02-TYPED-TOPOLOGY-IDENTITY-LINEAGE`：`authoring_mesh_edit_preview → authoring_mesh_edit_prepare` 的 `split_edge / collapse_edge / dissolve_edge` proof 仍保持 source-element-only；下游 Runtime 现在只从 Store 的 exact candidate→idempotency response 恢复该 proof，并把 parent source identity 物化为 durable `AuthoringMeshIdentityLineage@1` child IDs、单调 tombstone 及 one-to-many/many-to-one relation，不接受 caller identity/proof arrays。真实 split/collapse/dissolve 已分别完成各自独立的完整持久化与 Runtime drop/reopen/get 重启链路，合计 **3/3 PASS**；Store `authoring_mesh_` **12/12**、MCP IdentityLineage **3/3**、490-schema checker与 Contracts/Store/Runtime/MCP 联合 compile PASS，工具数仍 **106 read + 78 write = 184**。general correspondence、evaluated retarget、完整 selection/undo history 与产品级 cross-version editor仍 `NOT_PROVEN`。Stage 保持 `camera-calibrated`，视觉=`QUALITY_TARGET_NOT_MET`，human/engine/distribution=`NOT_RUN`，HQ360=`BLOCKED_REFERENCE_COVERAGE`。新回执：`docs/evidence/mcp010f/authoring-mesh-typed-topology-identity-lineage-materialization-source-gate-20260825.json`；原 source-proof 回执继续作为上游证据。

2026-08-25 Native High 较早 source/transport 快照：stable-ID adapter 与 Worker Protocol 当时只闭合 standalone transport；该段已由顶部 source durable/MCP 1/1 最新口径取代。仍然有效的限制是 proposal `registered=false`、Low 只是 feature-protected triangle collapse、artist-authored quad topology/edge-flow=`NOT_PROVEN`，以及 Stage/visual/human/engine/distribution/HQ360 均未晋级。

ForgeCAD 是由 Codex 通过 MCP 调用的本地、可验证、可回退 3D Runtime。Codex 是外部设计大脑；ForgeCAD 是唯一产品执行器与状态写者，负责类型化建模、High/Low、UV、Cage/Bake、PBR、LOD、渲染、质量、不可变版本、回退和导出。最终用户只安装 ForgeCAD；Blender、Substance、Maya、任意脚本插件或远程 image-to-3D 都不是运行依赖。

## 当前状态

2026-08-26：**MCP001–MCP009 的单用户 MVP host golden path 已完成，MCP010A 已完成，FGC-MCP010F 仍是唯一 `in_progress` 原子任务**。当前为 **527 schemas / 28 operator entries / 115 read + 87 opt-in write = 202 tools**，数量只证明协议表面。真实 D1 已新增 stable-ID `MoveVertices` → durable child revision → 单 source-node lowering → strict GLB readback → 六视图 replay 的纵向切片；当前结果仍是 `REVIEWABLE_TRADEOFF + BLOCKED_FORMART_OWNER_EVIDENCE`。正式 High、editable Low、Hero UV、Cage/Bake、MaterialLayerGraph evaluator、FPS、Unreal/Unity、人审和商业门均未通过；Stage 仍为 `camera-calibrated`，视觉状态保持 `QUALITY_TARGET_NOT_MET`。

商业 FPS Hero Asset 的权威质量组是：`Art Direction → approved Form → AuthoringMesh → Native High → editable Low/Retopo → Hero UV → Cage/Bake → Material Layer Graph → FPS Presentation → LOD/Collision/Socket → commercial engine + independent human review → same-hash export/restart`。这些是商业验收分组；实际 Runtime Stage 只按 19 个 `ProductionStage@3` 值晋级，并要求 `hero-art-review-approved → engine-validated → export-confirmed`。完整定义、当前差距和采用队列见 [COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md](docs/COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md) 与 [ADR-0027](docs/ADR/0027-native-fps-weapon-production-executor.md)。任何 structural/source/compile PASS 都不能被描述为“达到无畏契约级商业资产”。

- 新方向已由 [ADR-0025](docs/ADR/0025-codex-only-mcp-3d-runtime.md) 接受；
- 后续高质量重规划已由 [ADR-0026](docs/ADR/0026-agentic-design-runtime.md) 记录：ForgeCAD 目标升级为 Agentic Design Runtime，让 Codex 通过 SemanticSceneGraph、ReferenceCanvas、DesignSpec、stage gates、Visual Evidence 和 Critic/Repair loop “看得见”并逐步设计；当前已落地第一阶段只读 projection，durable orchestrator/checkpoint/repair 仍是目标，不计入完整 Agentic Runtime 能力；
- 原生 FPS 武器生产执行器由 [ADR-0027](docs/ADR/0027-native-fps-weapon-production-executor.md) 接受；外部 DCC 只作 reference-only 研究，不进入 Runtime allowlist、安装包或产品真值；
- 架构模块边界见 [ARCHITECTURE_MODULE_BOUNDARY.md](docs/ARCHITECTURE_MODULE_BOUNDARY.md)，废弃文档/代码/模块隔离规则见 [DEPRECATED_ISOLATION_PLAN.md](docs/DEPRECATED_ISOLATION_PLAN.md)；
- 当前刀类运行时单屏架构图见 [weaponry-runtime-architecture-overview.html](docs/weaponry-runtime-architecture-overview.html)，逐模块债务与升级顺序见 [WEAPONRY_MODULE_EVALUATION_20260830.md](docs/WEAPONRY_MODULE_EVALUATION_20260830.md)；
- 旧 Provider、聊天工作台、App Server、Python Agent、旧合同和旧脚本已从当前树删除；
- superseded `reference-to-typed-plan@0.1.0`、`hard-surface-detail@0.1.0`、`uv-pbr@0.1.0` Skill provenance 已移到 `packages/forgecad-skills/archive/superseded/`，不属于 active registry 或 Runtime build archive；
- 新 first-party `ponytail-preflight@0.1.0` 强制 Codex 在每个 MCP 设计会话先检查必要性、既有受限能力与最小 typed action；它是 MIT workflow reference 的自有重写，不安装上游 Node package、hook 或 MCP server；
- 新 `forgecad-runtime`、`forgecad-mcp`、Runtime Viewer、worker protocol、Runtime V1 migration、首批 contracts、CAS、单写者和 authenticated local IPC 已通过本地 focused Gate；
- 当前版本可以进行开发构建上的本地 3D 功能评估，但不能宣称“通用高质量 3D”或已完成真实 Codex 视觉验收；
- [FGC-MCP004](docs/CODEX_TASK_INDEX.md) 已按 MVP 基座范围收口，且 `FGC-MCP005–009` 已完成各自功能核心：Runtime/authenticated IPC 候选、审批、restore-as-new-version、path-free diagnostic export、`forgecad-mcp` 内置轻量 supervisor、真实 Codex CLI diagnostic/reference write、PNG/JPEG ReferenceEvidence/CAS、有界 typed 多 Part mesh/GLB、bounded UV/tangent/PBR、fixed render、limited quality、stable-Part change 和 CAS `mvp-glb` receipt 已有 evidence；真实 Codex CLI 十二调用 host golden path 也已 PASS，MCP010A 已通过第二次 Desktop 激活 Gate。像素级参考相似度、人评、完整 Desktop 3D write 和 filesystem/package export 仍未验收；
- `FGC-MCP005–009 done` 的边界、命令和未运行项见 [MVP 交付计划](docs/MVP_DELIVERY_PLAN.md) 与 [证据清单](docs/evidence/)。`npm run mcp008:test` 和 `npm run mcp009:test` 分别覆盖 Appearance/Render/Viewer 与 Quality/Change/Version/Export functional core；签名公证和 packaged release 在 MCP013。真实 host receipt 见 `docs/evidence/mcp007/` 和 `docs/evidence/mcp009/`。

reset 前的未提交成果已归档并验证可恢复；归档不属于仓库，也不改变旧产品已经从当前树退役的决定。

## 目标体验

```text
用户在 Codex 对话并上传参考
        ↓
Codex 通过本地 MCP 调用 ForgeCAD
        ↓
ForgeCAD 编译 bounded 几何、UV/tangent、PBR MaterialZone 和固定视图
        ↓
Quality Compiler 生成结构与视觉证据
        ↓
用户在 ForgeCAD Viewer 查看，在 Codex 提出局部修改/批准
        ↓
Runtime 创建不可变版本，可回退、爆炸查看和导出
```

ForgeCAD Desktop 不包含聊天、图片上传、模型选择、Provider 设置或 API Key。P0 只支持 Codex Desktop 和 Codex CLI；Codex IDE / VS Code / Cursor / Windsurf 仅保留未来兼容基线，不是当前产品入口或发布 Gate。ForgeCAD 不内置任何模型 SDK 或模型调用。

未来目标体验在这个基础上增加：

```text
ReferenceCanvas
  → DesignSpec
  → SemanticSceneGraph / ModelUnderstandingBundle
  → Primary / Secondary / Tertiary stage gates
  → Visual Evidence Bundle
  → Critic / Local Repair
  → Human review / version / export
```

Primary form 未过时不进入细节，visible-view 未过时不解锁 PBR，`QUALITY_TARGET_NOT_MET` 不确认、不导出。

## 文档入口

- [文档地图](docs/DOCUMENTATION_MAP.md)
- [当前状态](docs/DOCUMENTATION_STATUS.md)
- [产品定义](docs/PRODUCT_DEFINITION.md)
- [架构设计](docs/DESIGN.md)
- [Agentic Design Runtime ADR](docs/ADR/0026-agentic-design-runtime.md)
- [Agentic Design Runtime 重规划](docs/FORGECAD_AGENTIC_DESIGN_RUNTIME_PLAN.md)
- [架构与模块边界](docs/ARCHITECTURE_MODULE_BOUNDARY.md)
- [废弃隔离计划](docs/DEPRECATED_ISOLATION_PLAN.md)
- [删除/迁移/升级完整清单](docs/RESET_MIGRATION_PLAN.md)
- [单用户 MVP 交付计划](docs/MVP_DELIVERY_PLAN.md)
- [MCP 合同](docs/MCP_RUNTIME_CONTRACT.md)
- [Codex 集成](docs/CODEX_INTEGRATION.md)
- [编译器与质量管线](docs/COMPILER_PIPELINE.md)
- [Viewer 合同](docs/WORKBENCH_VIEWER.md)
- [Skill Package 标准](docs/SKILL_PACKAGE_STANDARD.md)
- [外部项目采用清单](docs/EXTERNAL_PROJECT_ADOPTION.md)
- [工具/Skill/GitHub 候选目录](docs/MVP_TOOL_CATALOG.md)
- [Codex Ponytail 前置设计流程](docs/CODEX_PONYTAIL_PREFLIGHT_WORKFLOW.md)
- [Luna 执行指南](docs/LUNA_GOAL_EXECUTION_GUIDE.md)
- [原子任务索引](docs/CODEX_TASK_INDEX.md)

## 开发规则

开始实现前必须完整阅读 [AGENTS.md](AGENTS.md)。旧 U004/Provider/Module/Mechanical 文档已失去执行权威，不能用旧测试要求恢复已废弃架构。

本项目只生成合法的非功能性视觉资产，不提供现实武器制造图、制造尺寸、材料配方、加工流程、功能机构或性能建议，也不对交通、建筑、医疗或机械结果给出安全/认证结论。

更明确地说：虚构游戏美术资产只允许非制造说明；ForgeCAD 不输出可用于现实制造武器的精确图纸。
