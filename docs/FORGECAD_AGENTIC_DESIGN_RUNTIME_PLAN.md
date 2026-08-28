# ForgeCAD Agentic Design Runtime 重规划

> 2026-08-27 `FPS-FORM-04AV`：Agentic 编排现可调用 Runtime-owned 只读 `production_weapon_owner_reviewed_void_calibration_get`，但不得自行提交 mask/transform/camera 或改写 projection。投影将 calibration eligible 与 strict owner-void PASS 分开：前者只允许一个 bounded repair draft，后者与六视图/FormArt/用户 secondary approval 仍决定是否能离开 Form。当前 source/compile PASS，real-D1 projection 未运行，所以 Agentic Runtime 不得推进 Stage 或触发 High→Low→UV→Bake。

> 2026-08-26 现行 source：**527 schemas / 115 read + 87 write = 202 tools**。真实 D1 已通过 Runtime-owned bounded stable-ID vertex move 形成派生候选与六视图 replay，但缺 fresh proposal-side FormArt owner evidence，尚未成为 secondary approval；AuthoringMesh/High/Low/UV/Bake/Material/FPS/Engine 仍未组成商业单资产 orchestrator 或视觉 PASS。

> 商业武器 Agentic 编排必须遵循 `Form → AuthoringMesh → High → Low → UV → Bake → Material → FPS → Engine → Human`，并把每个自动算法输出保持为可拒绝 draft。实现蓝图见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

> 2026-08-26 商业 Goal 约束：Agentic Runtime 的价值是让 Codex 观察同一资产、选择一个可解释的高影响修复、生成 typed intent、比较前后证据并等待用户批准；它不是自动给自己打分或批量堆细节。最终编排对象是 `HeroSourceAsset + FpsPresentationPackage + EngineDeliveryPackage`，任何 source test/critic score 都不能替代真实资产、引擎和独立艺术家门。

> 2026-08-26 source synchronization: current public surface is **515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**. Low quad durable now reaches Runtime/Store/MCP but remains candidate-bound with exact provenance, `DRAFT_UNREVIEWED`, structural-only and unpromotable. Hero UV `hero_uv_durable_get/prepare` now reaches Store→Runtime→MCP; real prepare→replay→Runtime drop/reopen→get is **1/1 PASS**, with four Hero CAS roots linked/GC. This is structural/source evidence only, not artist-authored unwrap, visual, human, engine, commercial or packaged acceptance; Stage=`camera-calibrated`, visual=`QUALITY_TARGET_NOT_MET`, `FPS-HIGH-05=NOT_PASSED`, human/engine/distribution=`NOT_RUN`, HQ360=`BLOCKED_REFERENCE_COVERAGE`, and no Stage/confirm/version/export gate advances. Render Core raster attribution is not yet bound into the current Runtime Form diagnostic. Evidence: `docs/evidence/mcp010f/commercial-weapon-hero-uv-durable-restart-source-gate-20260826.json`.

> 2026-08-25 最新投影口径：Native High 已通过公共 MCP source/focused Gate 与同 cohort Runtime durable restart fixture **1/1**；Agentic 只能将它显示为 `PASS_SOURCE_DURABLE_RESTART_MCP / FPS-HIGH-05=NOT_PASSED`。proposal 仍未注册，packaged/candidate visual/human/engine/distribution 未运行，不能自行激活或推进 Stage。

> 2026-08-25 商业质量边界：Agentic observe/critic/repair 负责提出、比较和记录艺术决策，不能自我授予 `PASS_HUMAN_ART_REVIEW`，也不能用更多轮次跨越缺失的 AuthoringMesh、High/Low/Cage、Hero UV、diagnostic bake、商业引擎或独立人审。生产链和 Gate 以 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md` 为准。

## 商业级 FPS 资产 Gate 映射（Agentic 只能编排，不能自我放行）

Agentic loop 的 `Observe → Plan → Act → Inspect → Render → Evaluate → Checkpoint` 只是证据编排。商业 Hero Weapon 仍须按下表顺序由 Runtime/Worker 和独立评审共同闭合；critic、Codex typed review、Viewer 或更多迭代不得把 `NOT_RUN` 改写为 PASS。

| Agentic stage | 必须产出的商业证据 | 当前真实差距 |
|---|---|---|
| Brief / design language | `WeaponArtBrief`、风格支柱、识别标志、material hierarchy、预算和授权/IP边界 | target/missing；没有独立 Art Direction receipt |
| Silhouette / primary | 多视图 CameraLock、比例、negative space、landmark、primary form non-regression | Stage=`camera-calibrated`；CrossView=`QUALITY_TARGET_NOT_MET`，`secondary-form-approved=NOT_CREATED`，depth=`UNKNOWN` |
| Secondary / tertiary | 二三级曲面节奏、panel/vent/groove/seam、倒角密度、高光连续性、可追溯 Part/source map | source Operators 不是商业美术接受；secondary/tertiary `NOT_PROVEN`，不可用细节覆盖形体失败 |
| Authoring topology | original/evaluated 分离、quad/loop/ring/crease/edge flow、局部可编辑性和 stable IDs | split/collapse/dissolve durable/restart `3/3 PASS` 仍是 source structural slice；general correspondence/evaluated retarget/editor `NOT_PROVEN` |
| High/Low/UV/Cage/Bake | `HighMeshArtifact`/`DetailGraph`、artist-authored Low、Hero UV、对应 cage、无 miss/fallback/cross-part 的 8-map bake | Native High source durable PASS但质量未过；Low 为 candidate-bound exact provenance 的 explicit quad draft（六个 Low CAS roots）；Hero UV public durable replay/drop/reopen/get **1/1 PASS**（四个 Hero UV CAS roots），两者仍 structural/source。Formal High internal materializer 与 Cage/Bake Worker/七记录 Store/MCP seam 仅 source/compile/focused，完整 positive restart/public surface/current-D1 receipt 缺失，new prepare 零写返回 `PRODUCTION_WEAPON_HIGH_LOW_BAKE_PRODUCER_UNAVAILABLE` |
| Material Layer / PBR | Layer/Mask/Generator/Wear/Microdetail、roughness hierarchy、通道/色彩空间/provenance，多视图材质证据 | 当前 PBR/embedded texture 只为 structural/consumer evidence；fixed-formula preview，commercial PBR `NOT_PROVEN` |
| FPS/world + engine/performance | first-person hip/ADS/inspect/equip/reload/recoil、third-person/world model、socket/readability、engine import、LOD/collision、frame/memory/draw-call budgets | FPS presentation、commercial engine 和性能 `NOT_RUN`；Three.js/固定 AOV 不能替代真实引擎往返 |
| Human / export / restart | 独立 Art Director 盲审、修订批准、同 export hash 的 confirm/version/export/restart | human=`NOT_RUN`，`PASS_HUMAN_ART_REVIEW` 不存在；export/restart=`NOT_RUN`，不能由 Agentic 自评放行 |

Native High 在 Agentic projection 中仍只能作为未通过的依赖显示：stable-ID/embedded GLB/当前 cohort Runtime CAS/Store/restart与公共 MCP source chain已通过，但 proposal保持`registered=false`，package/candidate quality仍未通过。Low quad draft 保持 candidate-bound exact provenance；Hero UV 已有 public durable get/prepare 与 Runtime replay receipt，但这两项仍仅为 structural/source producer，不是 artist unwrap、visual、human、engine、commercial 或 packaged PASS。任何一项都不可被 Agentic stage plan标为商业 active/PASS或解锁后续门。

2026-08-17 P2 binding follow-up：RenderSet camera CAS 与 RepairIntent observation hash 已纳入 Runtime 生产/回读边界；最新 Dev.app isolated real-reference run 完成 compile/readback/render/compare/evaluate 后按严格视觉门 blocked，保留 staged-only、source unchanged、no confirm/version/export。receipt：`docs/evidence/mcp010f/real-reference-repair-intent-run-observation-cas-20260817.json`。

2026-08-17 historical source/package slice：`repair_intent_run_prepare` 已把 CAS RepairIntent 校验、exact observation/reference/camera/candidate binding 和 bounded compile/readback/render/compare 连接起来；最终 Dev.app 的真实授权参考 packaged transport 已通过，但在 camera evidence gate blocked，只产 staged candidate。Repair apply/confirm、完整 orchestrator 与视觉质量门仍未完成。Fictional Energy Rifle Profile/Plan 仍为 nonfunctional source-only authoring aid。该 slice 当时工具面为 144 Schema、41 read + 33 opt-in write = 74 tools；2026-08-26 当前机器真值为 **515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 tools**，499/186 仅是 2026-08-25 历史切片。Mechanical pose 单 tick/sequence 只是一条 candidate-bound Runtime read projection，不是 Agentic orchestrator、Armature/skin 或持久动画状态。

版本：2026-08-13
状态：目标架构计划；observe/plan projection、嵌套只读 projection producer/consumer conformance、durable session/checkpoint/RepairIntent prepare/readback 与 MCP010F 窄范围 Primary Form 单动作 prepare/evaluate 已实现并通过各自证据；durable/reference/DesignSpec 完整 producer、通用单动作 orchestrator、Repair 应用和完整视觉闭环仍未完成，不改变 MCP010F 的 `QUALITY_TARGET_NOT_MET` 事实

## 1. 目标

ForgeCAD 要从“Codex 调用一组 3D 工具”升级为“Codex 能看见、理解、分阶段设计、每步验证、可回滚修改的 Agentic 3D Design Runtime”。

产品目标保持四个关键词：

- 简单：用户只提供需求、授权参考和必要确认；
- 快捷：每一步只暴露当前阶段真正需要的选择；
- 方便：所有状态、问题、下一步动作都能在 Viewer/Codex 中读懂；
- 高质量：必须由同一 candidate/reference/camera/hash 的多视图证据、质量指标、typed review 和真人门证明。

## 2. 当前诊断

当前 ForgeCAD 已有正确底座：

- Runtime 是 SQLite/CAS/Project/Candidate/Version/Job/Quality 的唯一写者；
- MCP 是薄 `stdio` adapter；
- Worker 执行 bounded typed geometry/appearance/render；
- Viewer 只读 Runtime projection；
- MCP010C/D/E/F 已有九 AOV、reference compare、hard-surface operators、AssetPack/PBR、silhouette target/camera/Rig/Part compare 和 Viewer compare surface。

真正缺口是 authoring loop：

1. Codex 看到的不是完整设计现场，缺少统一 semantic scene graph；
2. 设计流程没有被 Runtime/工具面组织成 stage state machine；
3. 检查和视觉证据出现太晚，容易在失败形体上继续堆 detail/PBR；
4. 外部项目研究没有收敛成 ForgeCAD 自有合同和 Gate；
5. 当前结构 PASS 与视觉质量 PASS 容易被混淆。

因此下一阶段不应先换模型或直接接 Blender/FreeCAD/TRELLIS，而应沿着“观察、语义、阶段门、critic、checkpoint”逐层补齐。

## 2.1 已落地的第一阶段：观察、阶段规划与 durable prepare/readback

MCP010F 已交付两个最小、可验证的 Agentic source slice：

- Runtime 从现有 project/snapshot/candidate、Geometry readback、ReferenceEvidence、RenderSet、Comparison 和 QualityReport 按需派生 `projection/read-only`；投影不创建 candidate/version/job，也不冒充持久状态；
- MCP 暴露 `scene_observe_get`、`design_stage_plan_get`、`critic_report_get`、`visual_evidence_bundle_get` 四个只读工具，均要求明确 project/candidate binding，缺失或跨项目数据 fail closed；
- observation envelope 一次返回语义场景、理解 bundle、参考画布、stage plan、critic 和 lineage/evidence hash，并显式标出 observed/inferred/unknown、allowed/blocked action；
- Runtime/MCP 另提供 `session_create_or_resume`、`session_get`、`checkpoint_prepare`、`checkpoint_get`、`checkpoint_restore_prepare`：受批准的 session/checkpoint 写入 SQLite/CAS，restore 只生成 CAS-bound `RepairIntent@1`，不修改 candidate/version/history；
- MCP010F 另提供窄范围 `primary_form_repair_prepare`：Codex 只提交一次 target/camera/Rig/optimizer typed intent，Runtime 在一个有界动作内完成 fit、typed GeometryProgram、strict readback、隔离 Render Worker 九 AOV 与 candidate-bound compare，返回 staged candidate 和 Runtime `QualityReport@2`；没有严格改善则保持 source candidate unchanged，且不 confirm/version/export；
- Viewer 只消费 authenticated IPC/read model 的归一化 projection，并查询 durable session；不在本地推导质量、Session 或 checkpoint，也不提供写入入口；
- `scripts/probe_agentic_runtime.py` 在临时 Runtime/CAS 上先读取 `ponytail-preflight@0.1.0`，验证工具 manifest、空参考阻断、动作锁定和用户持久数据未触碰。证据：`docs/evidence/mcp010f/agentic-runtime-observe-plan-20260813.json`。
- 同一探针在 Runtime/MCP 重启后读取 session/checkpoint，并由 `scripts/check_agentic_runtime_receipt.py` 校验公开合同、candidate/session binding 和不可变 RepairIntent：`docs/evidence/mcp010f/agentic-runtime-session-checkpoint-20260813.json`。
- `scripts/check_agentic_projection_receipt.py` 已对真实 Runtime 回执中的 `AgenticSceneObserveResult@1` 与 `DesignStagePlanProjection@1` 嵌套对象完成 producer/consumer 校验：`docs/evidence/mcp010f/agentic-runtime-projection-conformance-20260813.json`。该 Gate 只覆盖嵌套只读 projection，不覆盖 durable/reference/DesignSpec 的完整 producer。

本阶段的限制必须保留：durable slice 只覆盖受批准的 session/checkpoint prepare、readback 和 CAS-only RepairIntent；嵌套只读 projection conformance 已通过，但它不覆盖 durable/reference/DesignSpec 的完整 producer。当前仅包含 MCP010F Primary Form 的窄范围 prepare/evaluate slice，不包含通用单动作 compile/readback/render/evaluate orchestrator、Repair 应用、用户确认后的 candidate/version mutation、packaged same-cohort 或视觉质量通过。`AgenticSceneObserveResult@1` 仍是 projection envelope，不能替代这些边界。

## 3. 新主循环

```text
1. Intake
   用户需求 + 授权参考 + coverage/unknown

2. Design Spec
   category / style / primary forms / semantic parts / materials / risks

3. Observe
   SemanticSceneGraph + dimensions + stats + camera + selection + AOV

4. Plan
   DesignSession 根据 stage gate 给出下一步允许动作

5. Act
   只执行一个 bounded Part/MaterialZone/Stage action

6. Inspect + Render
   strict readback + multi-view AOV + reference compare

7. Evaluate
   deterministic metrics + Codex typed critic + optional human review

8. Checkpoint
   pass -> stage advance；fail -> bounded repair；unknown -> ask reference/user
```

## 4. 目标模块

### 4.1 Agent Harness Adapter

Codex/Pi-style harness 只做线性编排，不成为状态真值。它负责把用户意图、Viewer 选择和 Runtime evidence 组织成下一次工具调用。

### 4.2 DesignSession Orchestrator

目标合同：`DesignSession@1`、`DesignStagePlan@1`、`DesignCheckpoint@1`。

职责：

- 当前 stage；
- 当前失败门；
- 下一步允许动作；
- candidate/checkpoint/rollback 关系；
- 禁止跨阶段提前堆 detail/PBR/export。

### 4.3 SemanticSceneGraph / ModelUnderstandingBundle

目标合同：`SemanticSceneGraph@1`、`ModelUnderstandingBundle@1`。

最小字段：

- `parts[]`：id、name、role、parent、children、symmetry_partner、visibility；
- `geometry`：bbox、dimensions、triangle_count、surface_area、source_operator_ids；
- `materials`：material_zone_id、channel、asset/provenance；
- `editability`：allowed_operations、parameters、constraints；
- `evidence`：candidate_id、artifact_hash、render_set_hash、quality_report_hash；
- `cameras`：active/fixed/reference camera hash；
- `uncertainty`：observed/inferred/unknown。

### 4.4 ReferenceCanvas + DesignSpec

目标合同：`ReferenceCanvas@1`、`DesignSpec@1`。

ReferenceCanvas 不只是图片列表，而是 coverage truth：

- front/back/left/right/top/three-quarter/material/detail；
- 每个 reference 的授权、hash、camera claim、visible regions；
- missing/unknown 视图明确阻断 360。

DesignSpec 是 Codex 和 ForgeCAD 共同遵循的设计合同，不是 prompt：

- 主形体、比例、风格语言、语义 Part；
- 材质/颜色/线条/细节层级；
- 禁止猜测的区域；
- 当前 stage exit criteria。

### 4.5 Parametric Design Kit

在当前 `operator_catalog_get` 和 `GeometryProgram@2` 上形成 typed macro：

| Kit | 用途 |
|---|---|
| Housing | 主壳、胸甲、头盔、外骨骼 |
| Panel | 装甲板、分层边线、倒角边 |
| Vent | 散热孔、格栅、开槽 |
| Joint | 关节、铰链、接口层 |
| Sensor | 镜头、灯、雷达、面罩 |
| Frame | 内构、支架、骨架 |
| Handle/Foot/Wheel | 功能化外形但不提供工程结论 |
| Fastener/Cable/Light | tertiary detail 与材质绑定 |

Codex 选择 Kit intent，Runtime 展开 bounded geometry program 并保留 source map。

### 4.6 Visual Evidence Engine

目标工具：`scene_observe_get` 或 `visual_evidence_bundle_get`。

一次返回：

- SceneGraph + dimensions + stats；
- selected Part/MaterialZone；
- fixed camera + active camera；
- reference/render split、overlay、diff；
- 九 AOV；
- failed gate 与 threshold；
- evidence hashes。

### 4.7 Critic / Repair

目标合同：`DesignCriticReport@1`、`RepairIntent@1`。

Critic 必须输出可执行的局部问题，而不是“更像、更高级”：

```text
part_id: chest-shell
stage: primary-form
metric: boundary_f1_4px
observed: 0.328765
threshold: 0.90
evidence: <comparison_hash>
repair: adjust Housing/Panel width/height within bounded range
allowed: true
```

## 5. 分阶段质量门

| Stage | 允许动作 | 解锁条件 | 禁止 |
|---|---|---|---|
| reference-canvas | 导入参考、标注 coverage、列 unknown | reference hash + authorization + coverage | 猜背面/隐藏结构 |
| primary-form | blockout、profile、large housing | silhouette/proportion pass | panel/vent/material/export |
| secondary-structure | joint、sensor、module hierarchy | landmarks/region/form pass | micro detail/PBR/export |
| tertiary-detail | panel、vent、groove、fastener | structure pass | 覆盖失败轮廓 |
| uv-pbr | MaterialZone、UV/tangent/PBR | visible-view pass | 用材质掩盖形体失败 |
| final-review | multi-view/human/export | strict compare + human | 单图冒充 360 |

当前 strict visible-view 门继续使用 MCP010F 账本阈值：silhouette IoU/Boundary F1 `>=0.90`、bbox/centroid `<=0.02`、landmark coverage `>=0.80`、NME `<=0.03`、region/critical IoU `>=0.85`。

## 6. 外部项目吸收策略

| 项目 | 学习点 | ForgeCAD 落地方式 |
|---|---|---|
| Pi Agent | 极简 harness、线性工具循环、可配置技能 | Agent Harness Adapter，不保存产品状态 |
| Omniverse Kit | extension/app 组合、插件生命周期 | Skill/Kit manifest 与 app composition 思想 |
| OpenUSD | scene graph、layer、variant、reference | SemanticSceneGraph/ReferenceCanvas schema 思想 |
| FreeCAD | Document、transaction、parametric recompute | DesignSession/Checkpoint/RecomputePlan |
| build123d/CadQuery | AI 友好参数化 CAD API | Parametric Design Kit typed JSON macro |
| BlenderMCP | scene inspect、screenshot feedback、MCP bridge | 只学习 Observe/Render feedback；拒绝 arbitrary Python |
| Manifold | robust boolean | adoption 后作为 isolated Worker capability |
| Trimesh | mesh analysis/repair | dev/test reference；不做 Runtime truth |
| MaterialX | 标准材质图 | post-MVP material graph；当前仍 glTF PBR subset |
| TRELLIS/Hunyuan3D | draft mesh/image-to-3D | 未来 opt-in draft source；不能直接 confirm/export |

## 7. 文档和任务落地

当前源码为 `515 Schema / 28 operator entries / 111 read + 83 opt-in write = 194 tools`。`RenderSet@2` 已携带 Runtime-authenticated Render Worker cohort/status，Viewer 只读消费；Mechanical pose 单 tick/sequence 与 Boolean Operand Lineage 是 candidate-bound structural read projection。Low durable 有六个 linked/GC CAS roots；Hero UV durable 有四个，并完成 public replay/drop/reopen/get **1/1 PASS**。Cage/Bake public get/prepare 与七记录 Store seam 已存在，但 formal producer unavailable。上述都不等于 artist unwrap、visual、human、engine、commercial 或 packaged pass。Subdivision artifact-lineage 另已有显式 Runtime-owned immutable CAS sidecar/Link，但仍不是跨版本 mesh-element identity。durable prepare/readback、CAS-bound RepairIntentRun 与 Primary Form 窄范围 prepare/evaluate/async-Job slice 已有各自 receipt；唯一 `in_progress` 仍为 `FGC-MCP010F`，Stage/quality truth 不变且不 confirm/version/export。建议下一批文档/代码任务：

1. 为 durable/reference/DesignSpec producer 增加剩余完整 producer/consumer conformance，避免字段漂移；嵌套只读 projection checker 已完成，回执见 `scripts/check_agentic_projection_receipt.py`；
2. 将当前 Primary Form 窄范围链路抽象为通用单动作 `prepare -> compile -> readback -> render -> evaluate` orchestrator，但仍不绕过用户批准；
3. 执行 bounded `RepairIntent`，生成新 candidate prepare，保留旧 checkpoint/version 不变；
4. 接入真实 Codex 的 observe→plan→bounded action→inspect loop，再独立运行 packaged/human/360 Gate；
5. 最后才评估 Parametric Design Kit 的 macro producer 和外部库的 isolated adoption。

## 8. 当前不可宣称

- 不能宣称已能生成高质量 3D；
- 不能宣称 attempt35 是 best benchmark；
- 不能宣称 packaged Viewer 已绑定 same observation；
- 不能宣称 Pi Agent/Omniverse/OpenUSD/FreeCAD/build123d/CadQuery/BlenderMCP 已被采用；
- 不能宣称 TRELLIS/Hunyuan3D 可直接生成最终模型；
- 不能用材质、线条、三角数或单张 beauty 替代 silhouette/form/human gate。

## 9. 成功标准

第一阶段成功不是“模型看起来更复杂”，而是：

- Codex 一次调用能得到完整、hash-bound 的 3D 设计现场；
- 每个 stage 的 pass/fail/unknown 都由 Runtime evidence 支撑；
- 每次修正只有一个 bounded design action；
- 失败不会进入后续阶段；
- 用户能在 Viewer 看到当前阶段、失败门、Part 问题和下一步；
- human/export/restart 只在 strict visible-view 通过后出现。
