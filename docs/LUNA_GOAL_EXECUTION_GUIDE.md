# Luna Goal 执行指南：ForgeCAD 单用户 MVP

版本：2026-08-13
状态：Luna 强制执行协议；MVP host golden path 已收口；MCP010 质量轨道已批准
当前任务：`FGC-MCP010A done`；`FGC-MCP010B blocked/deferred（Darwin OS memory hard cap NOT_RUN）`；`FGC-MCP010C done（source-focused PASS_WITH_UNRUN_VISUAL_GATES）`；`FGC-MCP010D/E source + packaged structural PASS（Manifold/xatlas/Validator/视觉子门 NOT_RUN）`；唯一 `in_progress` 为 `FGC-MCP010F`（Viewer source、packaged CLI read-model、原生窗口与核心控件 smoke PASS；provisional observation package binding/正式 VoiceOver/人评/360 `NOT_RUN/BLOCKED`）

## 1. Goal 目标

Luna 是仓库开发执行者，不是 ForgeCAD 运行时 Agent、Provider 或状态真值。当前 Goal 的代码主线和真实 Codex CLI 已完成一条用户授权图片 → typed 3D → PBR/fixed render → quality → approval/version → CAS GLB receipt 的 MVP host golden path；MCP010C source 已完成固定 renderer、九 AOV、reference compare、Codex typed visual review 以及 human review 合同/工具接口，MCP010D/E source 已完成真实硬表面 Operator、离线 AssetPack、UV atlas、MikkTSpace 和嵌入式 PBR，MCP010F source 已完成只读 Viewer 的 AOV/对比/部件筛选/材质区筛选/爆炸图/热图辅助。真实用户 likeness、同一 candidate 的 packaged Viewer、独立真人评分、PBR likeness、xatlas/Validator、export/restart hash 和 360 仍按独立证据推进，不继续堆复杂后台治理。

Stage 0 唯一机器可读真值为 `docs/evidence/mcp010f/current-benchmark-truth.json`：attempt35 只是 provisional retained observation，不是合格 benchmark；它为 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，fit/compare camera 为 `MISMATCH`；packaged Viewer 来自不同 cohort/artifact，未绑定该 candidate。任何 Luna 状态更新都必须保留这些失败/未运行边界。

<!-- forgecad-stage0: schemas=102 schema_set_sha256=8d3644fb8169157584cf844a21588d4b6a49c7852600de4698205a7da6050cdb read_tools=36 write_tools=23 total_tools=59 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json latest_completed=real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json -->

不要把 Goal 写成“完善整个软件”后无边界并行修改。一次只领取一个 `FGC-MCPxxx`，先完成退出 Gate，再进入下一项。MCP005–009 是已完成的 functional core；MCP010A–F 严格串行，MCP011–013 保留可靠性、分发和正式发布职责。

## 2. 每次启动完整阅读

1. `/Users/liuchongjiang/Documents/武神/AGENTS.md`
2. `docs/DOCUMENTATION_MAP.md`
3. `docs/DOCUMENTATION_STATUS.md`
4. `docs/CODEX_HANDOFF.md`
5. `docs/ADR/0025-codex-only-mcp-3d-runtime.md`
6. `docs/ADR/0026-agentic-design-runtime.md`
7. `docs/FORGECAD_AGENTIC_DESIGN_RUNTIME_PLAN.md`
8. `docs/ARCHITECTURE_MODULE_BOUNDARY.md`
9. `docs/DEPRECATED_ISOLATION_PLAN.md`
10. `docs/RESET_MIGRATION_PLAN.md`
11. `docs/CODEX_EXECUTION_PLAN.md`
12. `docs/CODEX_TASK_INDEX.md`
13. `docs/MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`
14. `docs/AUTHORITATIVE_STATE.md`
15. `docs/MVP_DELIVERY_PLAN.md`
16. `docs/MVP_TOOL_CATALOG.md`
17. `docs/EXTERNAL_PROJECT_ADOPTION.md`
18. `docs/LUNA_GITHUB_REPLICATION_PLAYBOOK.md`（处理 GitHub 研究或选择性源文件复刻时必读）
19. 本文件
20. 当前任务对应的 MCP/Schema/Compiler/Viewer/Skill/Test/Packaging 合同。

若冲突，按 `DOCUMENTATION_MAP.md` 解决；没有明确权威时先修文档，不自行混合两套架构。

ADR-0026、Agentic plan、模块边界和废弃隔离计划都是当前权威阅读链的一部分。当前已有 Agentic contract family、4 个 read-only projection tool、5 个 durable session/checkpoint prepare/readback tool 和隔离 Runtime/MCP/Viewer evidence；真实 Runtime 的嵌套只读 projection producer/consumer conformance 另已通过 `scripts/check_agentic_projection_receipt.py` 校验。Luna 必须把它们分别标为 `source/read-only projection PASS`、`nested projection conformance PASS` 与 `durable prepare/readback PASS`，不能写成 durable/reference/DesignSpec 完整 schema conformance、单动作 orchestrator、Repair execution 或高质量 PASS。

## 3. Goal 建议文本

用户已批准并显式继续以下 Goal；010A–E 的已完成/source 状态按账本保留，当前只执行唯一 `in_progress` 的 FGC-MCP010F：

```text
按照 AGENTS.md、docs/MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md、docs/CODEX_TASK_INDEX.md 和本指南，保护 dirty worktree，一次只执行一个原子任务。当前源码固定为 102 Schema、36 read + 23 opt-in write = 59，唯一 `in_progress` 是 FGC-MCP010F。Agentic 设计调用必须先 `skill_get(ponytail-preflight@0.1.0)`；观察路径为 `scene_observe_get/design_stage_plan_get`，durable 路径为 `session_create_or_resume → checkpoint_prepare → checkpoint_get/session_get`，恢复只允许 `checkpoint_restore_prepare` 生成 CAS-only RepairIntent，不能绕过 approval 或直接改 candidate/version。高质量路径只允许 `GeometryProgram@2` detail → strict readback → 九 AOV strict compare → typed visual review；`[transition-v1]` primitive-only 仅为历史兼容。Viewer source 和 packaged read-model/window/core-control smoke 已通过，但 attempt35 仍为 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，fit/compare camera `MISMATCH`，packaged Viewer 未绑定 provisional observation。正式 VoiceOver、真人、PBR likeness、export/restart hash 和 360 仍独立记录。禁止旧 Provider、付费 API、远程 image-to-3D、任意 Python/BlenderMCP、手工 GLB、heartbeat 或插件市场。
```

若用户要求大调整或架构重规划，应追加：

```text
同时遵循 ADR-0026、FORGECAD_AGENTIC_DESIGN_RUNTIME_PLAN、ARCHITECTURE_MODULE_BOUNDARY 和 DEPRECATED_ISOLATION_PLAN。目标是让架构和模块边界清晰：先隔离 superseded 文档/代码/模块，再按 ReferenceCanvas → DesignSpec → SemanticSceneGraph → stage gates → Visual Evidence → Critic/Repair 拆分后续任务。不得在当前脏工作树直接删除未知文件；废弃项先进入 archive/quarantine 并保持可恢复证据。
```

用户已经授权的 GitHub 研究必须追加：

```text
仅按 LUNA_GITHUB_REPLICATION_PLAYBOOK.md 研究 build123d、BlenderMCP、CadQuery、Manifold、MaterialX 的冻结 revision。每个项目先写 research-authorized receipt，选择性文件只进隔离缓存，随后做许可证/依赖/动态代码/网络/文件系统审查。默认输出是 ForgeCAD 自有 Schema 和 Rust rewrite；不运行 Python、Blender addon、socket、上游安装脚本或模型下载。未取得 approval: accepted、SBOM、恶意输入、确定性、资源和平台证据前，不改 lockfile、安装包、Runtime allowlist 或 active Skill。
```

真实 host 证据按下面的顺序运行；`<AUTHORIZED_REFERENCE>` 必须是用户明确授权的本地 PNG/JPEG，命令输出不得写入 Git、日志或 receipt：

```bash
cd /Users/liuchongjiang/Documents/武神
export FORGECAD_MCP005_REFERENCE="<AUTHORIZED_REFERENCE>"
export FORGECAD_MCP007_REFERENCE="$FORGECAD_MCP005_REFERENCE"
export FORGECAD_MCP007_CODEX_E2E=1
export FORGECAD_MCP009_REFERENCE="$FORGECAD_MCP005_REFERENCE"
export FORGECAD_MCP009_CODEX_E2E=1
npm run release:mvp
script/test_mcp005.sh
script/test_mcp007.sh
script/test_mcp009.sh
```

MCP005/007/009 的真实 Codex receipts 已为 `PASS`；复核时必须严格观察
`project_create → reference_import → geometry_prepare → artifact_readback_get → appearance_prepare → artifact_readback_get → quality_get → candidate_confirm → version_list → export_prepare → export_confirm → version_list`。Codex 的缺参重试、额外调用、OAuth 无工具、超时或 Desktop attachment 不可验证时，保留 `BLOCKED/NOT_RUN`，并把 observed prefix、Runtime/host 错误和下一条安全动作写入对应 evidence；不要放宽 probe 来制造完整闭环 PASS。

## 4. 任务领取协议

修改文件前记录：

```text
Task ID:
Dependency status:
Base commit / branch:
git status -sb:
git diff --check:
Existing dirty files in owned paths:
Owned paths:
Forbidden paths:
Current capability:
Target capability:
Baseline commands and exit codes:
Exit gates:
External dependency decisions:
Destructive actions / user approval:
```

若任务索引没有 `ready`，不得自行跳任务。领取时只把该任务设 `in_progress`；任何其他任务保持 `blocked`。

### 4.1 MCP010 当前领取规则

- 当前 010A 已由用户批准并完成真实 Desktop Gate，标为 `done`；
- 成功 receipt 必须保留，第一次失败 receipt 也不得改写；
- 010A 已 done；010B structural source Gate已通过但 Darwin OS memory hard cap deferred；010C source-focused Gate 已完成但视觉子门仍未运行；D/E source Gate 已完成；当前只允许 F 保持 `in_progress`，其 packaged/human/360 子门独立记录；之后每次只将直接后继改为 ready/in_progress；
- 当前工作树有 102 Schema（MCP006 历史 44 + MCP010B/C/D/E/F 与 Agentic contract family）、36 read + 23 opt-in write = 59 个工具、12 个 Skill（包含每个设计 MCP session 必须先读的 `ponytail-preflight@0.1.0`；历史 `0.1.0` + `primitive-blockout@0.2.0`、`hard-surface-detail@0.2.0`、`uv-pbr@0.2.0` active）；C source Gate 已通过 contracts、fixed renderer/九 AOV、comparison/typed visual review、MCP image block 和 deterministic raw stdio，D/E source Gate 已通过真实 Operator、AssetPack、UV/PBR/MikkTSpace/embedded-texture 和九 AOV，并有同 cohort packaged D/E structural probes，F source Gate 另通过哈希绑定轮廓目标、Runtime-owned camera reference、方向性边界误差、多 Part `silhouette_part_error_get` 归因表、只读 Viewer 的 AOV/compare/selection/explosion/heatmap 及构建边界，packaged Viewer 另有 read-model/window/core-control smoke；Agentic projection 与 durable session/checkpoint/RepairIntent prepare/readback 另有合同、preflight、空参考 fail closed、Runtime/MCP 重启和隔离持久化检查。attempt35 likeness/receipt/camera 仍失败或不完整；provisional observation package binding、正式 VoiceOver、人评阈值、xatlas/Validator、PBR likeness、export/restart hash和360只写 `NOT_RUN/BLOCKED`。

## 5. FGC-MCP005 已完成记录

### 5.1 已完成 Gate

- `MVP_DELIVERY_PLAN.md` 的 MCP005；
- `MCP_RUNTIME_CONTRACT.md` 的参考导入；
- `SCHEMAS.md`、`DATABASE.md`、`TEST_STRATEGY.md`；
- CAS/Runtime/MCP 当前实现和 `docs/evidence/mcp004/manifest.json`；
- `EXTERNAL_PROJECT_ADOPTION.md` 中 `image-rs/image`、`img2threejs`、`img2css` decision。

MCP005 已完成：

1. `ReferenceEvidence@1`、import/get request/result Schema 与 Runtime records/migration；
2. PNG/JPEG decoder limits、MIME/魔数/截断/超限/hash mismatch、authorized root/outside-root/symlink negative tests；
3. Store/Runtime CAS admission；migration 在 OS 文件锁之后；永久状态不保存原路径；
4. MCP `reference_import/reference_get` 与 `supports_reference_import=true`；
5. `image-rs/image` 只启用 PNG/JPEG，依赖锁已更新；
6. 真实 Codex CLI 隔离 Runtime 导入用户授权 PNG，源 SHA-256 与 CAS object hash 一致；证据只留 hash/尺寸/MIME/授权；
7. Desktop 当前 bridge 诚实记录 `NOT_RUN / unavailable`；
8. 证据、状态账本、handoff 和下一任务已更新。

### 5.2 禁止扩展

图片进入 CAS 不是建模完成。当前 MCP007–009 已打开受限 geometry/GLB、UV/tangent/PBR、固定渲染、limited quality、stable-Part change、immutable version/restore 和 CAS export；仍禁止 Blender、资产下载和远程 Provider。

## 6. MCP006–009 执行摘要

### MCP006（已完成）

先 Schema/validator，再 first-party Skills。MCP006 已完成 44 个 contracts schema、十项 historical registry manifest、十个独立标准 Bundle、trust hash、`skill_list/get` 和只读 resource、DAG/单位/finite/预算 validator、负向 fixture、benchmark receipt、LICENSE/NOTICE/SBOM/provenance 绑定；当前总数为 102 Schema，`primitive-blockout@0.2.0`、`hard-surface-detail@0.2.0` 与 `uv-pbr@0.2.0` 均有 active Runtime consumer。Codex 提交 typed program，ForgeCAD 不调用 LLM。MVP Bundle 用 canonical hash + first-party trust root；不省略许可证/SBOM/provenance，但分发签名/撤销延后。它只证明声明式能力可审计，不证明通用视觉质量。

### MCP007（已完成）

`[transition-v1]` MCP007 已通过 `npm run mcp007:test`：Geometry Worker library/binary 接受 canonical `GeometryProgram@1` primitive-only program，生成确定性 glTF 2.0 GLB；Runtime 生成 geometry candidate/quality report，MCP 通过 authenticated IPC 暴露 `geometry_prepare` 和 `artifact_readback_get`，Viewer read model 读取候选与 artifact lineage。该历史 fixture/receipt 不等于当前 `GeometryProgram@2` detail、九 AOV strict compare、像素相似度或真人高质量结论。

### MCP008（已完成功能核心）

`[transition-v1]` `npm run mcp008:test` 已通过：hash-bound 三种 MaterialZone、UV/tangent/glTF PBR lowering、四个固定 PNG、Runtime readback 和 Three.js GLB canvas。Viewer 只读，不复制状态；该历史四-pass receipt 不替代当前 `AppearanceProgram@2`、九 AOV、strict compare、PBR likeness 或视觉评分。证据：`docs/evidence/mcp008/`、`docs/evidence/mcp009/`。

### MCP009（MVP host golden path 已完成）

`[transition-v1]` `npm run mcp009:test` 已通过 24 个 Runtime tests + 16 个 MCP tests；真实 Codex CLI 已完成十二调用 reference→geometry→appearance→readback→quality→candidate_confirm→version_list→CAS-only export。该历史 `QualityReport@1` 只含明确 `limited` aspect-ratio；`change_prepare` 要求当前 base version、稳定 Part ID、allowlisted operation 和新 typed programs；confirm/reject/restore 保持 immutable/idempotent；`mvp-glb` export 只消费 confirmed quality-passing CAS GLB，返回 output hash 和 receipt，不写任意本机路径。当前 MCP010C/F 已实现 candidate-bound silhouette/landmark/region compare 和 Codex typed visual review，但 attempt35 仍 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，独立真人评分和 retained-candidate packaged Viewer E2E 未运行。证据：`docs/evidence/mcp009/` 与 Stage 0 真值。

## 7. GitHub / Skill / Plugin 纪律

用户允许 GitHub 研究和配置，不等于允许任意安装。必须遵循：

```text
search/read source + release + license
→ classify library/tool/asset/reference-only
→ pin exact revision
→ adoption receipt + LICENSE hash + transitive SBOM
→ isolated malicious/resource/determinism benchmark
→ accepted decision
→ only then edit lockfile/build/package
```

允许优先评估：image-rs/image、gltf-rs/gltf、Manifold、xatlas、mikktspace、glTF-Validator、glTF-Transform。`approved-for-evaluation` 不等于 `adopted`。

禁止安装：BlenderMCP、FreeCAD MCP、任意 Python CAD MCP、远程 image-to-3D Provider、自动下载模型权重、GitHub prompt/Skill pack。MCP010E 仅允许 Codex 将计划点名的 CC0 文件一次性下载到本机 adoption cache；逐资产 hash/license/SBOM/provenance 通过后才能编入 first-party 离线 AssetPack。Runtime、安装器和 Viewer 不联网、不调用素材 API。

## 8. 实施纪律

- 保留 dirty worktree，不 reset/clean/checkout 用户修改；
- 文件修改用 patch；不 commit/push/merge，除非用户明确要求；
- 新公开合同先 Schema、生成类型、validator、negative tests；
- Runtime 唯一写库；MCP/Viewer/Worker 不开 SQLite；
- 永久写必须绑定 project/base/candidate/artifact/quality/approval/idempotency；
- 不记录 secret、prompt、原图副本、用户名、绝对路径；
- 不开网络服务、8000、Provider、任意 shell/Python/JavaScript；
- 失败路径不创建版本，不以 fallback 假成功；
- 任何质量数字同时记录阈值、实测值、fixture 和 limitation。

## 9. 验证分类

```text
PASS      当前工作树实际运行成功
FAIL      已运行且失败
BLOCKED   权限、宿主、硬件或外部状态阻断
NOT_RUN   本轮没有运行
```

focused ≠ aggregate ≠ real Codex ≠ packaged ≠ visual ≠ human。必须分别写。历史 receipt 不覆盖当前二进制，文档总结不覆盖 GLB/render/raw report。

共同 Gate：

```bash
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run release:license-sbom
npm run contracts:check
git diff --check
```

任务专属命令在对应 evidence manifest 中固定；依赖变更后必须证明 offline build。

## 10. 每轮 handoff

必须更新：

- `CODEX_HANDOFF.md`：实际做了什么、命令/exit、真实运行、blocked/not-run、下一动作；
- `CODEX_TASK_INDEX.md`：只有满足退出条件才变状态；
- `DOCUMENTATION_STATUS.md` 和 capability matrix：当前能力；
- 对应合同、用户指南和 evidence manifest；
- 外部依赖 receipt、THIRD_PARTY_LICENSES/SBOM（若采用）。

## 11. Goal 状态句

完成单个任务：

```text
FGC-MCPxxx completed: all listed exit gates passed on <commit/worktree>; next ready task is FGC-MCPyyy.
```

未完成：

```text
FGC-MCPxxx not complete: <PASS/FAIL/BLOCKED/NOT_RUN evidence>; next safe action is <one action>.
```

MVP 关闭：

```text
ForgeCAD MVP completed for the first hard-surface reference benchmark on <commit/worktree>; universal high-quality image-to-3D and production distribution remain out of scope.
```

## 12. 当前可执行的高质量工具顺序

真实 Codex host 具备 MCP write opt-in 后，按下面顺序调用；每一步把返回的 ID/hash传给下一步，禁止从模型自由猜测 hash。新的 MCP stdio 设计会话必须先成功读取 first-party `ponytail-preflight@0.1.0`；只有 `capabilities_get`、`runtime_status`、`doctor` 可作为无状态诊断例外，不能代替前置读取：

1. `skill_get(ponytail-preflight@0.1.0) → capabilities_get → runtime_status → doctor → operator_catalog_get → skill_list`：先保存 Skill manifest/knowledge canonical hash，再要求 Runtime Ready、同 cohort、catalog digest 一致；当前口径必须是 36 read + 23 opt-in write = 59。未完成 `skill_get` 时不得调用 `project_create`、参考、Geometry、Appearance、比较、评审或其他 Skill。
2. `project_create → reference_import → reference_get`：只提交用户授权 PNG/JPEG，记录 project/reference/object hash 和 observed/inferred/unknown coverage。
3. 构造 project/catalog-bound `GeometryProgram@2` detail draft；`geometry_program_hash → geometry_prepare → job_get → candidate_get → artifact_readback_get`，要求 strict `ArtifactReadback@2` 的完整 lineage 和零 integrity failure。
4. `reference_mask_prepare → silhouette_target_get → camera_fit_prepare → silhouette_rig_hash → silhouette_fit_prepare`；fit 返回的 camera 必须与后续 compare 的 camera hash/canonical hash 一致，不一致即停止。
5. 仅在轮廓/结构门允许时提交 `AppearanceProgram@2` 并运行 `appearance_prepare`；AssetPack/UV/tangent/PBR readback 不等于 PBR likeness。
6. `reference_compare_prepare` 必须绑定同一 project/reference/candidate/artifact/camera，生成 `RenderSet@2` 九 AOV strict compare；逐项 `render_pass_get`，再 `visual_review_submit → quality_get`。
7. 任一 strict metric 失败时保留 receipt，只做一次单 Part/detail 受限修正并重跑 readback/九 AOV/strict compare；`QUALITY_TARGET_NOT_MET` 禁止 confirm/export。
8. 只有 strict visible-view 通过后才进入独立 `human_visual_review_submit`；正式真人门当前仍 `NOT_RUN`，不得由 Codex typed review 代替。
9. 真人批准精确 candidate/version 后才可 `candidate_confirm → version_diff → restore_prepare/restore_confirm → export_prepare/export_confirm`，并验证 preview/export/restart 同 hash；这些门当前仍未完成。

`[transition-v1]` `GeometryProgram@1` primitive-only + `AppearanceProgram@1` + `RenderSet@1` 四 pass 只保留 MCP007–009 structural MVP/历史导出兼容。它不是当前高质量路径，不能产出 strict likeness、PBR、human、packaged Viewer 或 360 结论。

真实验收必须另外记录：Codex host 类型、MCP initialize 版本、参考源字节 hash、Geometry/Appearance canonical hash、GLB hash、RenderSet hash、QualityReport、approval receipt、version DAG、Viewer readback、重启后的 hash 和真人评分。任何一项没有运行，都写 `NOT_RUN`；宿主不可用写 `BLOCKED`。
