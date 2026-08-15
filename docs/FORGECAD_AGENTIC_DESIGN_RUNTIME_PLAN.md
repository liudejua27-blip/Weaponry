# ForgeCAD Agentic Design Runtime 重规划

2026-08-15 staged ActionRun continuation：Stage Batch 已把高层 `parameter_changes + view_spec` 接入现有 Runtime-owned parameter patch 执行器；真实回执 `docs/evidence/mcp010f/design-action-run-real-reference-stage-view-spec-20260815-b37.json` 证明同 cohort 的父批次、子 ActionRun、review candidate、质量阻断和 checkpoint replay 链路。当前仍是一个 bounded independent action；它不等于自动 Critic→Repair、跨阶段状态晋级、Repair 应用/Promotion、完整 DesignSpec producer conformance 或视觉 likeness PASS。

版本：2026-08-15

2026-08-15 高层 ActionRun 自动参数补丁（additive source/real evidence）：`design_action_run_prepare` 现在可用 typed `action.parameter_changes` 与外层 `ReferenceViewSpec` 触发 Runtime-owned `RuntimeParameterPatch@1`；Runtime 选择 `surface-control-points-v1`，生成 `RepairIntent`/review candidate 并完成五阶段 `prepare → compile → readback → render → evaluate`。真实授权参考回执 `docs/evidence/mcp010f/design-action-run-real-reference-runtime-auto-parameter-patch-20260815.json` 在 cohort `a21e448f…f057106` 中 `caller_supplied_full_proposal=false`，但 proposal 为 `rejected-regression`、视觉为 `QUALITY_TARGET_NOT_MET`，source candidate/version/confirm 未变。该 slice 只补齐高层 ActionRun 到 bounded typed authoring 的入口，不代表完整 orchestrator、Repair execution、likeness 或高质量闭环完成。

2026-08-15 SurfaceRig/CADFit v15 additive evidence：真实授权参考上的 `surface-shell@1` chest-shell 使用 Runtime-owned `SilhouetteRig@1` 多控制点参数和 paired-group bounded search 完成 39 次 `32/4/3` 评估；多控制点 candidate/readback、同 camera 和同一 Part Manifold residual lane 均通过，但 Part strict objective 未通过，`proposal_status=blocked-no-improvement`，没有 confirm/version/export。该增量只把 SurfaceProgram 参数搜索接入 Agentic staged loop，不改变 `QUALITY_TARGET_NOT_MET`、`BLOCKED_INCOMPLETE_BINDING`、人评/PBR/export-restart/360 的独立状态。

2026-08-15 最新真实参考闭环（additive evidence）：用户授权的单张硬表面机器人参考已在统一四组件 cohort `61e01276…bf340a` 上完成 `Visual Surface/Critic/PartError → CADFit OptimizationJob → product-owned Manifold Boolean residual → quality gate` 的真实 transport。独立索引为 `docs/evidence/mcp010f/real-reference-quality-closure-20260815.json`；OptimizationJob 成功完成 39 次 `32/4/3` 评估，Boolean residual lane 经过候选 `[1..9]`，ActionRun→CADFit child handoff 也完成到 evaluate。Runtime 现在从持久化 RenderSet 的 `camera_object_sha256` 重建相机，并在写入 CAS 前做 Runtime-owned canonical float normalization，优化与比较使用同一 camera hash。该闭环仍是 proposal/transport 层：视觉比较 `QUALITY_TARGET_NOT_MET`，strict improvement 为 false，proposal 为 `blocked-no-improvement`，没有 confirm/version/export；human review 为 `NOT_RUN`，单张参考的 `HQ_360` 为 `BLOCKED_REFERENCE_COVERAGE`。不得把该 receipt 写成 likeness/high-quality PASS 或通用 mesh Boolean 完成。
2026-08-15 当前源码真实回归（additive evidence）：同一授权参考在当前 SurfaceProgram/ActionRun 源码构建的统一 cohort `613470b6…af04a` 上重新完成 OptimizationJob 与 ActionRun→CADFit child；回执为 `docs/evidence/mcp010f/optimization-job-real-reference-20260815-user-reference-boolean-residual-v12.json` 和 `docs/evidence/mcp010f/design-action-run-real-reference-20260815-user-reference-with-optimization-v13.json`。两条均完成 39 次 `32/4/3` 多保真评估，Manifold residual/readback、camera binding 和 review-only boundary 均通过，但 quality gate 仍为 `QUALITY_TARGET_NOT_MET`、strict improvement=false、proposal blocked；这只证明当前源码没有破坏 staged pipeline，不代表 Visual Surface backend、likeness、高质量、Repair、PBR、人评、export/restart 或 360 已完成。
2026-08-15 SurfaceProgram ActionRun 增量（additive source/test evidence）：`RuntimeParameterPatch@1` 新增 `surface-control-points-v1`，允许在同一 Part 的唯一 `subd-cage@1`/`surface-patch@1`/`surface-shell@1` 节点上提交最多 8 个 `control-point-{index}-{x|y|z}` 有界修改。Runtime 负责 strategy/operator/节点唯一性、单位/范围、stale-before、GeometryProgram Worker hash、RepairIntent 和 review-candidate 的完整 `prepare → compile → readback → render → evaluate`；越界点在 evaluate 阶段 fail closed。证据为 `docs/evidence/mcp010f/runtime-parameter-patch-surface-control-points-20260815.json`。该增量只证明 typed SurfaceProgram 可进入 ActionRun proposal 链，不证明真实参考视觉质量、MaterialZone/UV-PBR、human、export/restart 或 HQ_360。
2026-08-15 真实 SurfaceProgram 质量回路（additive evidence）：当前源码 cohort `613470b6…af04a` 把授权机器人参考的 `chest-shell` 变为 16 控制点 `surface-shell@1`，在同一 camera `8cd20605…a535` 上执行 `RuntimeParameterPatch@1 / surface-control-points-v1`。`control-point-5-z` 的 `0.12m→0.20m` 变更完成 `prepare → compile → readback → render → evaluate`，但 source/proposal composite score `5.681233147408 → 5.688397884520`，`strict_improvement=false`、`non_regressing=false`、promotion=`rejected-regression`；两者都保持 `QUALITY_TARGET_NOT_MET`。证据为 `docs/evidence/mcp010f/design-action-run-real-reference-surface-control-points-v14.json`。这证明 surface-backed authoring 已进入真实 staged loop，但下一步必须由局部目标/参数搜索找出真正非回归控制点组合，不能把单点局部改善误写成高质量或 likeness PASS。
状态：目标架构计划；observe/plan projection、嵌套只读 projection producer/consumer conformance、默认或显式 bounded Runtime-owned ReferenceCanvas/DesignSpec CAS producer/readback、durable session/checkpoint/RepairIntent prepare/readback、primary-form/secondary-structure/tertiary-detail single-Part proposal executor、逐视图 evidence inventory、bounded cross-view render/compare bundle、approval-gated 有界同阶段独立动作 batch、带 `cumulative-program` 合并准备的 ordered composition proposal，以及受限 ActionRun→CADFit child handoff 已实现并通过对应证据；新增完整覆盖的六视图 synthetic cross-view promotion transaction fixture，已证明 promotion/replay/immutable-version 边界；真实参考 Repair 应用/晋级、完整 orchestrator、MaterialZone/UV-PBR executor 和完整视觉闭环仍未完成，不改变 MCP010F 的 `QUALITY_TARGET_NOT_MET` 事实

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
- Runtime/MCP 另提供 `session_create_or_resume`、`session_get`、`checkpoint_prepare`、`checkpoint_get`、`checkpoint_restore_prepare`：受批准的 session/checkpoint 写入 SQLite/CAS，session 生成并在读回时校验最小 `ReferenceCanvas@1`/`DesignSpec@1` authoring context，restore 只生成 CAS-bound `RepairIntent@1`，不修改 candidate/version/history；
- Viewer 只消费 authenticated IPC/read model 的归一化 projection，并查询 durable session；不在本地推导质量、Session 或 checkpoint，也不提供写入入口；
- `scripts/probe_agentic_runtime.py` 在临时 Runtime/CAS 上先读取 `ponytail-preflight@0.1.0`，验证工具 manifest、空参考阻断、动作锁定和用户持久数据未触碰。证据：`docs/evidence/mcp010f/agentic-runtime-observe-plan-20260813.json`。
- 同一探针在 Runtime/MCP 重启后读取 session/checkpoint，并由 `scripts/check_agentic_runtime_receipt.py` 校验公开合同、candidate/session binding 和不可变 RepairIntent：`docs/evidence/mcp010f/agentic-runtime-session-checkpoint-20260813.json`；authoring context 与 ActionRun fail-closed 边界另见 `docs/evidence/mcp010f/agentic-runtime-authoring-context-action-boundary-20260814.json`。
- `scripts/check_agentic_projection_receipt.py` 已对真实 Runtime 回执中的 `AgenticSceneObserveResult@1` 与 `DesignStagePlanProjection@1` 嵌套对象完成 producer/consumer 校验：`docs/evidence/mcp010f/agentic-runtime-projection-conformance-20260813.json`。该 Gate 只覆盖嵌套只读 projection，不覆盖 durable/reference/DesignSpec 的完整 producer。

本阶段的限制必须保留：durable slice 覆盖受批准的 session/checkpoint prepare、默认或显式 bounded authoring context、CAS-only RepairIntent，以及 geometry-stage single-Part proposal；ActionRun 当前只允许 checkpoint、primary-form/secondary-structure/tertiary-detail 和 typed bounded-repair proposal，其余 action kind fail closed。受限 geometry ActionRun 可额外携带 hash-bound `OptimizationIntent@1`，Runtime 只创建同 `run_id` 的 child `OptimizationJob`，不自动 confirm/version；真实回执 `docs/evidence/mcp010f/design-action-run-cadfit-handoff-real-reference-20260815.json` 已完成 32/4/2 搜索但 proposal 因 `blocked-no-improvement` 被拒绝。`bounded-repair` 的 2–8 view render/compare 生成 candidate-bound `CrossViewEvidenceBundle@1`；完整覆盖的 synthetic fixture 已额外通过 `cross_view_promotion_confirm` 并创建不可变版本，但真实参考仍必须独立通过质量/人评门。`design_stage_run_prepare` 只允许最多 6 个同阶段独立 ActionRun，按 RuntimeJob/event 记录并在首个质量门阻断。`design_composition_prepare` 已把 2–6 个 action 的线性依赖、每步 ActionRun、aggregate、stop/replay lineage 封装为 approval-gated proposal；显式 `merge.mode=cumulative-program` 会校验父程序哈希链，并在批次通过后编译一个独立合并候选，但当前 focused fixture 仍在视觉门前阻断，不 confirm/version/export。嵌套只读 projection conformance 已通过；positive merge、MaterialZone/UV-PBR executor、真实参考 Repair 应用、packaged same-cohort 或视觉质量通过仍未完成。`AgenticSceneObserveResult@1` 仍是 projection envelope，不能替代这些边界。

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

当前源码以 Stage 0 marker 为准，为 `123 个 JSON Schema / 40 read + 30 opt-in write = 70 tools`。durable prepare/readback、独立 stage batch、带累计程序链的 composition merge prepare、逐视图 evidence inventory、`CrossViewEvidenceBundle@1`、`cross_view_promotion_confirm` Promotion boundary、`repair_apply_prepare` CAS-backed apply-intent boundary、单视图 `repair_apply_confirm` source boundary、ActionRun→CADFit child handoff 和 `design_action_optimization_proposal_prepare` 独立 review-candidate continuation 已有各自合同/源代码证据；新增 synthetic cross-view promotion receipt `docs/evidence/mcp010f/cross-view-promotion-positive-synthetic-20260815.json`。`cargo check --workspace`、Runtime/MCP 全量测试与 `script/test_mcp010f.sh` 仍需在本轮变更后复跑，真实 child Job 仍因严格多目标门 `blocked-no-improvement`。真实参考 Repair 应用、跨视图 likeness/人评/360 仍未运行。建议下一批文档/代码任务：

1. 为 durable/reference/DesignSpec producer 增加剩余完整 producer/consumer conformance，避免字段漂移；嵌套只读 projection checker 已完成，回执见 `scripts/check_agentic_projection_receipt.py`；
2. 为完整 durable/reference/DesignSpec producer 增加剩余 producer/consumer conformance，定义失败恢复与跨视图 aggregate 的真实参考边界；
3. 在真实授权参考上执行 bounded `RepairIntent`，生成新 candidate prepare，保留旧 checkpoint/version 不变，再由跨视图质量/人评决定是否 promotion；
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
