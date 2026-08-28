# ForgeCAD 架构与模块边界

> 2026-08-26 04AE：当前公共面为 **525 schemas / 112 read + 84 write = 196 tools**。Native High 与 MaterialLayerGraph 均通过专用 isolated Worker seam；它们不能写 Runtime/CAS/candidate，Runtime 仍是唯一产品状态写者。

> 2026-08-26 商业生产边界：Manifold/OpenSubdiv/QuadriFlow/xatlas/Embree/OIIO/OCIO/MaterialX/meshoptimizer/glTF Transform 只能位于固定版本、typed、离线 Worker 内；Rust AuthoringMesh、stable lineage、correspondence、Cage owner policy、MaterialLayerGraph、Stage/approval/CAS 仍为 ForgeCAD 产品真值。详见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

2026-08-26 FormArt attribution 边界：MCP只在现有read-only get上承载闭合请求/结果并验证scope/hash/zero-write；Runtime独占 durable evidence resolution、camera/mask派生、Worker启动与canonical receipt；Render Worker只接收bounded GLB+Runtime-derived camera，返回瞬态triangle/source map，不读写SQLite/CAS/Stage。Viewer不拥有该真值。Source/focused PASS不等于real-D1、visual或commercial PASS。

2026-08-26 Formal High 边界同步：Contracts 定义 closed public wrappers，Runtime 负责从 durable Stage/source lineage 派生 scope、candidate state、readback 与 receipt；Store 是唯一持久化写者；MCP 只有在幂等与完整正向 Gate 通过后才可作为薄适配器公开；Worker 与 Viewer 均不得补写、修复或晋级 Stage。当前 Runtime adapter compile 不改变上述边界，也不构成公共能力。

2026-08-26 最新边界实现：Form Stage 证据策略属于 Store 单写者，Runtime 负责深读编排。Formal High 由 Runtime factory 生成 distinct derived candidate，经 Store 单事务提交，MCP 仅暴露闭合 get/explicit-write prepare。当前 seam 为 `PASS_SOURCE_COMPILE_FOCUSED`；完整 positive restart/cleanup 与 capability acceptance 未通过，真实 D1 零写。

> 2026-08-26 最新权威 source 口径（取代下方 2026-08-25 的“最新/当前”计数）：**515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**。Low quad draft 已接入 Contracts、Store、Runtime 与公共 `low_quad_draft_durable_get/prepare`，但仍为 candidate-bound exact provenance 的 `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`；Hero UV 的 public `hero_uv_durable_get/prepare` 已完成 Store→Runtime→MCP，真实 prepare→同键重放→Runtime drop/reopen→get 为 **1/1 PASS**，四个 Hero CAS roots 已 linked/GC。该结果仅为 structural/source pass，不是 artist-authored unwrap、visual、human、engine、commercial 或 packaged pass；`FPS-HIGH-05=NOT_PASSED`、Stage=`camera-calibrated`、visual=`QUALITY_TARGET_NOT_MET`、human/engine/distribution=`NOT_RUN`、commercial=`NOT_PROVEN`、packaged acceptance=`NOT_RUN`、HQ360=`BLOCKED_REFERENCE_COVERAGE`，不推进 Stage、confirm、version 或 export。证据：`docs/evidence/mcp010f/commercial-weapon-hero-uv-durable-restart-source-gate-20260826.json`。

> 2026-08-25 Native High 实现附记：ForgeCAD-owned High Worker 可以在内存中确定性产生 embedded GLB，Runtime 本地 strict readback 后把派生对象写入 CAS/Store；同 cohort Runtime drop/reopen/get fixture **1/1 PASS**，公共 MCP source/focused Gate PASS。Worker 仍无网络/路径/脚本/SQLite/CAS 权限，Runtime 仍是唯一状态写者；该 source slice 不是 active Skill 或 Commercial High PASS。

> 2026-08-25 目标模块补充：商业资产链将拆为固定、无网络、无任意脚本的 Art Direction、Authoring Mesh、High、Low/Retopo、UV、Cage/Bake、Surface、FPS Presentation、LOD/Collision/Socket、Engine Validation executors。它们只能由 Runtime prepare/approve/persist，不能各自成为状态写者；当前尚未实现的模块必须标 `unavailable`。具体输入输出和顺序见 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`。

商业目标模块状态矩阵：

| Executor | 目标输入/输出 | 持久化与晋级权 | 当前 producer 状态 |
|---|---|---|---|
| Art Direction | Brief/ReferenceViewSet → ArtDecision | Runtime 保存；独立人审决定通过 | target / unavailable |
| Authoring Mesh | approved form → original/evaluated AuthoringMesh | Runtime candidate/Stage；Worker 只求值 | typed split/collapse/dissolve 独立 durable/restart 3/3 PASS；direct stable-edge bevel@2 Runtime/MCP read-only lowering PASS；general correspondence/evaluated/editor 未完成 |
| High | AuthoringMesh + DetailGraph → High artifact | Runtime 绑定审批与 hash | NativeHigh durable 与 Formal High factory/Store/MCP/Runtime public seam source/focused PASS；完整 source-lineage/CAS positive materialize→drop/reopen/cleanup 仍 `NOT_RUN`，proposal=`registered=false`、`FPS-HIGH-05=NOT_PASSED`；High Worker commercial integration unavailable |
| Low/Retopo | approved High + constraints → editable Low | Runtime 保留 High↔Low correspondence | explicit quad draft 已接 Contracts/Store/Runtime/MCP；当前 Low 保持 candidate-bound exact provenance，仍 `DRAFT_UNREVIEWED`，artist edge-flow/promotion 未通过；既有 triangle collapse 不被替换 |
| UV | approved Low + visibility policy → Hero UV/tangent | Runtime CAS/quality receipt | 7 个合同与 public `hero_uv_durable_get/prepare` 已完成 Store→Runtime→MCP；真实 prepare/replay/drop/reopen/get **1/1 PASS**，四个 CAS roots linked/GC；仅 structural/source，不是 artist unwrap、visual、人评、engine、commercial 或 packaged PASS |
| Cage/Bake | High + Low + UV → Cage/maps/diagnostics | Runtime 绑定逐 Part ray lineage | fixed Worker 与七记录 Store/MCP public seam source PASS；正式 Runtime producer unavailable、当前 D1 无正向 receipt，quality failed |
| Material Layer Graph / Surface | maps + typed MaterialLayerGraph → Hero material pack | Runtime 物化/版本化 | fixed-formula preview only；商业 layer/mask/wear/microdetail gate unavailable |
| FPS Presentation | asset + cameras/actions → review set | Runtime 绑定 evidence；Viewer 只读 | fragments / no commercial Gate |
| Art Director Viewer | candidate/artifact/reference → AOV、compare、Part/MaterialZone 与阶段矩阵 read model | Viewer 只读；不批准、不写 Stage | source/read-model surface；正式 Art Director gate `NOT_RUN` |
| LOD/Delivery | approved Low → LOD/collision/socket/export | Runtime 唯一写入与同 hash 验证 | partial structural slices |
| EngineValidation | immutable export → importer/readback receipt | 外部引擎只返回证据；Runtime 收录 | commercial Unreal/Unity `NOT_RUN` |
| HeroArtReview | same export hash → independent review receipt | 独立人评；Codex/Viewer 无批准权 | `NOT_RUN` |

任何 executor 即使被拆成 sibling process，也不得直接读写 SQLite/CAS、联网、执行任意脚本或自行推进 Stage。

商业链使用五类真值，禁止混用：`DesignTruth`（Brief/Reference/decision）、`AuthoringTruth`（可编辑拓扑和历史）、`ProductionDerivative`（High/Low/UV/Cage/Bake/Texture）、`PresentationTruth`（FPS rig/clip/cue/fixed shots）、`DeliveryTruth`（GLB/LOD/collision/socket/engine receipts）。GLB 只能是 `ProductionDerivative/DeliveryTruth`，不能回写或冒充 AuthoringMesh。

新增目标 sibling 边界固定为：

| Worker/Adapter | 输入 → 输出 | 特别禁止 |
| --- | --- | --- |
| `retopology-worker` | approved High + locks → editable quad draft + correspondence | 直接 promotion、丢失 Part/孔洞/硬边/UV seam、把 triangle simplify 写成 Low |
| `cage-bake-worker` | approved High/Low/UV + CageField → 8-map BakeSet + heatmaps | self-surface bake 冒充 High→Low、nearest fallback 静默成功 |
| `surface-worker` | BakeSet + MaterialLayerGraph → texture set/material package | 任意 shader/plugin、动态路径/config、用材质掩盖 Form 失败 |
| `fps-presentation-worker` | approved asset + PresentationRig → fixed shots/clip/cue diagnostics | 改几何真值、用任意相机通过、生成 gameplay/现实武器性能结论 |
| `packaging-worker` | approved production objects → canonical GLB/KTX2/LOD package | 用户脚本、无 allowlist 的 glTF Transform 操作、覆盖 canonical source GLB |
| `engine-validation-adapter` | exact export hash + engine profile → read-only engine receipt | 写 ForgeCAD DB、把 preflight/Three.js/Khronos Validator 当真实引擎 PASS |

正式 Cage/Bake resolver 只能读取 Runtime/Store 已持有、同一 `project_id + session_id + Stage transition/head + candidate/artifact/hash` 的 durable High、Low、Hero UV、Cage 与 correspondence truth。caller 只能提交 closed identity/request，不得通过 inline JSON 提供或替换 artifact、topology、map、diagnostic 或 canonical hash。Hero UV 现只允许经 `project + candidate + Low artifact semantic hash` 的唯一 Store 反向索引读取，缺失或多条均拒绝。High resolver 采用两候选模型：Stage source candidate 绑定 AuthoringMesh source GLB 与 NativeHigh，distinct derived High candidate 绑定其 High GLB；同一候选同时冒充 source/high 必须拒绝。resolver 本身只读、不写 CAS/Store；缺失、跨候选、stale head、hash/provenance/cohort 不匹配必须以机器可判定 blocker code 稳定 fail closed。Formal High internal materializer 已负责创建派生候选与正式记录，但完整 source-lineage/CAS positive restart 尚未运行。只有该正向 receipt、fixed Worker 双回放和全部 readback 均通过后，Runtime 才能一次性提交七记录 bundle。

### 商业级能力与研究能力的边界（2026-08-25）

商业级生产能力必须由 ForgeCAD 内建并由同一条可验证链路持有：Form/Reference evidence、AuthoringMesh、High、editable Low/Retopo、Hero UV、Cage/Bake、Material/AssetPack、FPS review、LOD/Collision/Socket、commercial-engine readback，以及 candidate/artifact/GLB/hash/lineage/CAS、固定 Worker、质量门、独立人评、confirm/version/export/restart。它们必须有闭合 typed Schema、预算、strict readback 和 Runtime receipt；外部算法即使被采用，也只能作为产品注册的受限 Worker 内部实现，不能建立第二真值。

Blender、Substance Designer/Painter 和其他 DCC 只能用于学习数据组织、材质通道、烘焙/AOV 和交互方法。它们的 binary、`.blend`/工程文件、graph、插件、脚本、模型权重和会话状态不得进入 Runtime、active Skill、CAS 真值或 Stage 晋级；ForgeCAD 自有的 typed MaterialLayerGraph、AssetPack、GLB 与 readback 才是产品边界。GitHub 研究同样只能产出 reference、Schema/test 设计或 clean-room Rust rewrite。

Native High / GLB / durable slice 的边界：`apps/high-worker` 的 `HighMeshArtifact@1` 与 closed one-shot protocol 是 ForgeCAD-owned typed source/transport；`HighMeshArtifactGlb@1` sibling 只生成 embedded-only、meter-unit、稳定 Part/name/lineage 的 GLB，并由 strict local readback 校验 header/chunks/triangle/source hash。Runtime 的 `NativeHighDurable*` prepare/get 只有在 exact durable AuthoringMesh binding 后，才可分别双回放 High 与 GLB、校验 cohort/bytes/hash，并通过 Store/CAS 写入 derived artifact/link；getter 只读并重校 hash。该链路仍是 source/structural/durable evidence，不能写 candidate geometry、推进 Stage、confirm、创建 version 或 export。

上述 slice 的代码存在、编译、协议回放、GLB 可打开或 CAS 重读都不等于商业 High Gate、active Skill 或视觉 PASS。proposal 继续保持 `registered=false`、Runtime integration=`unavailable`；visual/commercial、human、engine 和 distribution 仍分别记录 `NOT_RUN`/`NOT_PROVEN`，既有 receipt 的历史 source/transport 事实不得被重写。

版本：2026-08-26
状态：模块权责文档；描述当前已实现边界与 ADR-0026 目标模块。Agentic observe/plan projection、嵌套只读 projection conformance、受批准的 durable session/checkpoint/RepairIntent prepare/readback，以及 MCP010F 的窄范围 Primary Form 单动作 prepare/evaluate、bounded action-run/readback 已进入 Runtime/MCP/Viewer；通用 durable/reference/DesignSpec producer、完整单动作 orchestrator 和 Repair 应用仍未进入当前边界。

## 1. 总体边界

ForgeCAD 的边界固定为：

```text
External Agent Harness
  Codex Desktop / Codex CLI / future Pi-style harness
        |
        | MCP stdio
        v
forgecad-mcp
        |
        | authenticated local IPC
        v
forgecad-runtime
        |
        +-- SQLite V1 + CAS
        +-- Geometry / Appearance / Render Worker
        +-- Quality / Evidence / Versioning
        |
        v
Read-only Viewer
```

Codex/Agent 负责理解、规划、设计判断、选择工具和迭代。ForgeCAD 负责几何、约束、布尔/拓扑、单位、材质、渲染、版本、撤销、回读和质量证据。

## 2. 当前已实现模块

| 模块 | Owned state | 允许 | 禁止 |
|---|---|---|---|
| `forgecad-mcp` | 无数据库状态 | MCP initialize、tool/resource manifest、typed request validation、连接 Runtime | 打开 SQLite/CAS、执行模型、运行脚本、保存 Provider/API Key |
| `forgecad-runtime` | SQLite/CAS/Project/Candidate/Version/Job/Quality | 唯一写者、candidate/version/approval/export、Skill registry、QualityReport | 让 MCP/Viewer/Worker 写库、接受任意路径/URL/脚本 |
| Runtime Primary Form repair prepare | Runtime-owned same-camera acceptance、staged candidate/CAS、RenderSet、Comparison、QualityReport | 接收一次 bounded typed intent，完成 Runtime-owned fit→GeometryProgram→strict readback→最终 camera 上 source/proposal 512px retention→Render Worker→compare；只有 `PrimaryFormAcceptance@1` 严格改善才返回 staged candidate，failed quality 仍失败 | Codex 连续参数搜索、Viewer 重算质量、相机补偿晋级、confirm/version/export、任意脚本 |
| Runtime Primary Form repair Job | Runtime-owned `RuntimeJob@1`、JobEvent 与 CAS-backed result；Store 是 SQLite 事务边界 | `primary_form_repair_job_prepare` 只同步创建 queued Job；后台复用同一 repair prepare、Geometry/Render Worker、strict readback 和 same-camera acceptance，`job_get`/`job_events_read`/`job_result_get` 提供进度、终态和 hash-bound 结果 | MCP/Viewer 持有连续搜索、跨重启续跑、网络/脚本调度，或直接 confirm/version/export；单进程异步解耦不等于 durable scheduler 或质量 PASS |
| Runtime Agentic action run | Runtime-owned SQLite/CAS `AgenticActionRunRecord` 与 action result projection | 接收一个已批准、单 Part、`primary-form` bounded action；复用 Runtime repair pipeline，返回可重放的 `DesignActionRun@1`，并通过 `design_action_run_get` 精确回读 | 多阶段自动推进、Repair 应用、candidate/version mutation、confirm/export、Viewer 本地质量推导、任意脚本 |
| Runtime Render Worker adapter | 无持久状态；只返回 typed render passes 与 Worker identity | `render_worker.rs` 是 Runtime 侧 fixed/perspective/batch 协议解析唯一入口；只把 bounded GLB + typed camera 交给固定 sibling Render Worker，并把返回的 cohort 绑定进 `RenderSet@2`。Primary Form 的 framing/ranking/refit 共用此入口 | 接收 GeometryProgram、编译几何、写 SQLite/CAS、在 Runtime/Viewer 内联 renderer、网络/路径/脚本；丢弃 Worker cohort |
| Contracts | JSON Schema + canonical hash | 定义跨进程对象、版本、negative gates | 空 Schema 冒充能力、未实现 producer 就宣传 PASS |
| Geometry Worker | 临时 worker process | bounded typed Operator、GLB lowering、strict readback | 网络监听、任意 Python/JS/shell、下载资产、写 Runtime DB、渲染 AOV |
| Render Core / Render Worker | `apps/render-core` 无状态 renderer + `apps/render-worker` 一次性 worker process | 只接受 bounded self-contained GLB 与 typed camera，生成 fixed/perspective/batch AOV；Render Worker 不依赖 Geometry Worker crate；`RenderSet@2` 明确 `same_cohort_verified` 或 `cohort_unavailable` | 编译 GeometryProgram、写 Runtime DB、网络/路径/脚本/模型调用 |
| Appearance/Render path | Worker/Runtime evidence | MaterialZone、UV/tangent、PBR、九 AOV、reference compare | 用 beauty/截图替代 QualityReport |
| Hero UV durable path | Runtime-owned Store/CAS link | `hero_uv_durable_get` 默认读、`hero_uv_durable_prepare` 显式 opt-in 写；绑定 current Low exact provenance、四根 CAS root linked/GC、重启 get | 仅 structural/source evidence；不等于 artist unwrap、visual、human、engine、commercial 或 packaged pass；不推进 Stage/confirm/version/export |
| Agentic projection | Runtime 按需派生的临时 projection；不持久化 | `scene_observe_get`、`design_stage_plan_get`、`critic_report_get`、`visual_evidence_bundle_get`；输出 observed/inferred/unknown、stage、gate、action 和 hash binding | 写 SQLite/CAS/candidate/version/checkpoint；把 projection 当 durable DesignSession 或视觉 PASS |
| Agentic durable prepare | Runtime-owned SQLite/CAS session/checkpoint/RepairIntent prepare/readback | `session_create_or_resume`、`session_get`、`checkpoint_prepare`、`checkpoint_get`、`checkpoint_restore_prepare`；要求 approval、project/candidate/reference/evidence binding，restore 只生成 CAS-only intent | 执行 orchestrator/Repair、直接改 candidate/version/history、把 prepare receipt 当视觉 PASS |
| Viewer | ephemeral UI state | 只读 GLB/AOV/compare/selection/explosion/heatmap | 创建版本、写 SQLite/CAS、保存产品状态到 localStorage |
| Skills/AssetPack | first-party manifests + receipts | 声明式 recipe、operator lock、validator、SBOM/provenance | 可执行插件、第三方仓库直接安装、模型权重 |
| Evidence | hash-only receipts | PASS/FAIL/BLOCKED/NOT_RUN 分层记录 | 用历史 receipt 证明当前 binary，或用结构 PASS 证明视觉 PASS |

## 3. ADR-0026 目标模块

以下模块仍是目标设计或后续 durable work；当前 Agentic projection 与 durable prepare/readback slice 已实现，不得把它扩展为完整 orchestrator：

| 目标模块 | 责任 | 落地要求 |
|---|---|---|
| Agent Harness Adapter | 线性 `Observe -> Plan -> Act -> Inspect -> Evaluate -> Checkpoint` 编排 | 不保存产品状态；所有动作仍走 MCP/Runtime |
| DesignSession | stage、checkpoint、失败门、下一步允许动作 | 当前已由 Runtime 受批准写入 SQLite/CAS 并可跨重启 readback；嵌套只读 projection conformance 已通过，但 durable/reference/DesignSpec 完整 producer、单动作 orchestrator 和 candidate/version mutation 仍未完成 |
| SemanticSceneGraph | Part tree、role、dimensions、symmetry、source map、editable parameters | 从 readback/RenderSet/Quality 派生，不由 Codex 本地猜 |
| ReferenceCanvas | reference coverage、views、observed/inferred/unknown | 绑定 CAS reference hash，缺失视图阻断 360 |
| DesignSpec | category、style、primary/secondary/tertiary goals、material language | 是设计合同，不是 prompt |
| Visual Evidence Bundle | 多视图 AOV、camera、selection、metrics、failed gate | 当前只读取既有 Viewer evidence；完整目标合同 conformance 和同 cohort packaged evidence 仍未完成 |
| Critic/Repair Loop | evidence-bound Part/MaterialZone issue 与 bounded repair | 不直接改几何，必须重新 compile/readback/render/compare |
| Parametric Design Kit | Housing/Panel/Vent/Joint/Sensor/Frame 等 intent | 展开为 typed bounded program，保留 source map |

## 4. 模块化目录原则

活动产品目录只放当前能力：

- `apps/desktop/src-tauri/crates/forgecad-runtime/**`
- `apps/desktop/src-tauri/crates/forgecad-mcp/**`
- `apps/geometry-worker/**`
- `apps/render-core/**`
- `apps/render-worker/**`
- `apps/desktop/src/features/runtime-viewer/**`
- `packages/forgecad-contracts/schemas/**`
- `packages/forgecad-skills/bundles/**`
- `packages/forgecad-assets/**`
- `docs/evidence/mcp*/**`

隔离目录只放历史或废弃材料：

- `docs/evidence/archive/**`
- `packages/forgecad-skills/archive/**`
- reset/private archive 路径，例如 `/tmp/forgecad-mcp001-20260807`

任何废弃代码、文档或模块不得继续留在活动目录根部；必须移动到 archive/quarantine，或删除前保留可恢复 receipt。当前脏工作树不得无证据删除用户数据或未提交修改。

## 5. 清晰架构验收

每个新增模块必须在文档里回答：

1. 谁是唯一写者；
2. 输入/输出 Schema 是什么；
3. 是否持久化；
4. 是否可重建；
5. 是否允许网络、脚本、路径、模型调用；
6. 对应 Gate 和 evidence 在哪里；
7. 与旧模块的隔离关系是什么。

如果回答不清楚，不允许进入 active capability。
