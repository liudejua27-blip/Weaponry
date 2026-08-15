# ForgeCAD Runtime Viewer

2026-08-15 Codex-only 入口收口：`apps/desktop/src/App.tsx` 现在只挂载 `RuntimeViewer`，不再提供本地图片上传、聊天式需求输入或断开 Runtime 的“准备生成/让 Codex 检查”假动作；参考附件、设计意图、审批和永久写入仍由 Codex → MCP → Runtime 驱动。新增 `scripts/check_mcp010f_viewer.py` 对 App 入口做负向检查；这只修正产品边界，不升级任何视觉质量、确认或导出 Gate。

2026-08-15 最新源代码覆盖：当前为 120 Schema、38 read + 30 opt-in write = 68 tools。Viewer 仍只读；新增 `visual_surface_get` 只返回 candidate-bound VisualSurfaceResult@1 诊断投影，并可展示同一 RenderSet 的 `VisualSurfaceReadback@1` mask/edge/ROI/AOV 证据，不增加 Viewer 写权限；`repair_apply_confirm` 不向 Viewer 增加写权限，仅由 Runtime 消费单视图 apply intent；`design_action_optimization_proposal_prepare` 生成的候选也只通过 Viewer read model 审查，不增加 Viewer 写权限，多视图仍由 `cross_view_promotion_confirm` 处理。合同/negative gate、真实 CAS readback focused test 与 Critic/CADFit binding 已通过，Viewer/视觉 Gate 不因此升级。

版本：2026-08-14
状态：当前源码口径为 118 Schema、37 read + 30 opt-in write = 67；MCP008–009 已实现只读 GLB canvas，MCP010F 已实现 source Viewer 的九 AOV、reference compare、Part/MaterialZone 筛选、临时 explosion、diff/contour 辅助，并通过 packaged CLI read-model、原生窗口与核心控件 smoke。第一阶段又接入 Runtime-authenticated Agentic projection，Viewer 可归一化显示 stage/gate/action/evidence hash，并按 project/candidate 读取 durable DesignSession/Checkpoint read model；逐视图 Visual Evidence inventory 会对未绑定 view 显式显示 `unknown/not-run`，已新增 hash-bound `CrossViewEvidenceBundle@1` 及跨视图 Promotion fail-closed boundary；唯一 `in_progress` 为 `FGC-MCP010F`。current-cohort provisional observation 的 packaged Viewer CLI read-model binding 已通过，但正式 VoiceOver、真人/PBR/360 与发布级 packaged UI E2E 仍 `NOT_RUN/BLOCKED`；Viewer 不提供 durable 写入，Runtime 已有 bounded single-action geometry ActionRun、独立 stage batch、带 `cumulative-program` 合并准备的 composition proposal、`design_action_optimization_proposal_prepare` 独立 CADFit review-candidate continuation、`repair_apply_prepare` CAS-backed apply-intent 和显式 promotion transaction boundary，但 positive merge、Repair 实际应用、跨视图同 cohort conformance 和 promotion 正向成功仍未完成。

Stage 0 Viewer 证据边界读取 `docs/evidence/mcp010f/current-benchmark-truth.json`：attempt35 只是 provisional retained observation，为 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，fit/compare camera 为 `MISMATCH`；current-cohort packaged Viewer CLI read-model 已绑定 project/candidate/artifact/reference/render-set/comparison lineage，但正式 UI/accessibility E2E 未运行。故已实现的 Viewer surface 和 package smoke 只能证明读取/交互表面，不能证明同一 candidate 的视觉、PBR、human、export/restart 或 360 通过。

<!-- forgecad-stage0: schemas=123 schema_set_sha256=583fd0d2615f09e66d16c58fca8d4ab60f1856d1de427b5b9e390c8c8b137f67 read_tools=40 write_tools=30 total_tools=70 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260815-b37-complete-auto-v3.json latest_completed=real-codex-cli-current-20260815-b37-complete-auto-v3.json -->

## 1. 产品角色

新桌面软件不是 Agent 工作台，而是 ForgeCAD Runtime 的可视化查看器。它回答五个问题：

1. Codex 正在处理哪个项目/候选/Job；
2. 当前 3D 结果、参考和固定视图是什么；
3. 哪些 Part、材质、UV 和质量项有问题；
4. 用户在指哪个局部；
5. 版本、恢复、爆炸图和导出证据是否一致。

Viewer 不包含聊天、prompt、图片上传、模型选择、Provider 配置、API Key、搜索、coding workspace 或 Agent timeline。

ADR-0026 后，Viewer 的目标角色升级为只读 **design stage console**：它应展示 ReferenceCanvas、DesignSpec 摘要、SemanticSceneGraph、当前 stage、失败门、Visual Evidence Bundle、Critic issue 和下一步允许动作。但这些都必须从 Runtime evidence 派生；Viewer 仍不成为写者。

## 2. 页面模型

```text
┌ Project / Runtime / Codex connection / current version ┐
├ Assembly tree ┬──────── single 3D viewport ────────┬ Inspector ┤
│ Part hierarchy│ candidate / version / explode      │ Part      │
│ versions      │ selection / isolate / compare      │ Material  │
│ jobs          │ one WebGL context                  │ UV/Quality│
├───────────────┴─────────────────────────────────────┴───────────┤
│ Reference & fixed views | Quality issues | Job events           │
└─────────────────────────────────────────────────────────────────┘
```

只允许一个交互 WebGL renderer/context。固定视图和 AOV 是 Runtime 生成的 CAS 工件；Viewer 展示而不重新定义质量事实。

目标 stage console 追加只读区域：

```text
ReferenceCanvas | DesignSpec | Stage gates | Critic issues | Next allowed action
```

该区域没有几何写按钮；任何 repair/confirm/export 都回到 Codex 的 prepare/approval/confirm。

## 3. 允许的交互

- orbit/pan/zoom、视图预设、网格/线框/材质/AOV 查看；
- Part/face/source region 选择、隔离、隐藏、透明；
- reference overlay、split、flicker 和 fixed-view 对比；
- 临时 exploded distance、层级折叠和标签；
- 候选/版本切换查看、quality issue 定位、Job 取消请求；
- 复制稳定 Part ID 或让 Codex 读取当前 selection resource。

这些默认是 ephemeral，不创建资产版本。任何几何、材质、纹理、恢复、Skill 安装和导出永久动作都回到 Codex 的 prepare/approval/confirm。

## 4. 状态和一致性

Viewer 只消费 Runtime read model：`RuntimeConnection`、`ProjectSummary`、`ActiveDesignSnapshot`、`CandidateSummary`、`SelectionState`、`JobProjection`、`QualitySummary`。它没有本地版本头，不用 localStorage 恢复产品状态，不把旧请求结果覆盖新候选。

每个屏幕必须显示 project ID、version/candidate ID 和 connection freshness。Runtime 断开、Schema 不兼容或版本漂移时进入明确只读诊断状态；不能展示可以点击却会走 legacy 路径的按钮。

## 5. 当前 Viewer 与 MVP 增量

`FGC-MCP001` 已提供可编译最小 Shell，MCP004 已用 authenticated IPC 读取项目/版本/当前快照；MCP007 读取 prepared candidate、artifact SHA、GLB MIME/size、Part IDs、triangle count 和 validator status；MCP008 的 `viewer_artifact_bytes` 通过 authenticated IPC 读取受限 GLB bytes，Three.js 只建立临时 canvas scene，不写 SQLite/CAS 或第二份产品状态。

MCP007–009 已按顺序增加：

- MCP007：Runtime candidate/artifact readback、Part IDs、candidate/version/hash metadata；
- MCP008：hash-bound PBR metadata、真实 GLB canvas、固定 beauty/silhouette/normal/part-ID render evidence；
- MCP009：`quality_get`/`version_diff`、stable-ID `change_prepare` handoff、immutable version/restore 和 CAS export receipts。

Viewer 始终只读；选择是 ephemeral，永久修改回到 Codex。当前 UI 已实现 candidate-bound AOV/reference compare、Part/MaterialZone 筛选、临时 explosion、diff heatmap、轮廓画布和 correction queue projection；full issue editing、正式 VoiceOver 和 provisional observation 的 packaged visual E2E 尚未完成。任何已显示 GLB/AOV 或交互 smoke 都不能写成 reference similarity PASS。

### 5.1 MCP010F 当前实现与剩余门

- 已实现 source surface：reference/render split、透明 overlay、flicker、diff heatmap；
- 已实现 Runtime/MCP read-only surface：`visual_surface_get` 返回 candidate-bound `VisualSurfaceResult@1` 与 `VisualSurfaceReadback@1` 诊断投影；从同一 `RenderSet@2`/CAS 解码九个 AOV、mask、4px edge/SDF 和 Part-ID ROI，并从同一已通过 ArtifactReadback@2 的 candidate GLB 提供 bounded mesh-derived curvature/feature-line summary；不创建 candidate/version，不设置质量 Gate；SubD/NURBS principal curvature、zebra 和真人视觉门仍未实现；
- 已实现 source surface：beauty/silhouette/depth/normal/AO/part-ID/material-ID/wireframe/UV-stretch 九 AOV；
- 已实现 ephemeral surface：Part/MaterialZone 筛选、临时 explosion、轮廓画布、hash-bound 草图复制与只读 correction queue；
- 已有证据：TypeScript/Vite/Tauri source Gate、packaged CLI read-model、原生窗口以及 AOV/Home/End/overlay/flicker/轮廓/热图/爆炸图核心控件 smoke；
- 尚未关闭：full issue editing/candidate undo-redo、正式 VoiceOver、同一 provisional observation 的 packaged Viewer binding、独立真人评分、PBR likeness、export/restart 同 hash 与 360。

这些 UI 只消费 Runtime 的 `RenderSet@2`、QualityReport 和 selection projection。屏幕图像、Three.js scene 或本地交互状态不能回写质量 PASS。当前 high-quality inspection 路径是 `GeometryProgram@2` detail → strict readback → `RenderSet@2` 九 AOV → candidate-bound strict compare → typed visual review；`[transition-v1]` `GeometryProgram@1` primitive-only / `RenderSet@1` 四 pass 仅用于历史兼容。正式 VoiceOver 与 provisional observation package binding 属 MCP010F 未关闭子门；Developer ID、clean install、发布级 packaged WebView/GPU/Codex E2E 仍属 MCP013。

### 5.2 ADR-0026 目标 Viewer 面

当前 Viewer 已通过 `agentic-design.ts` 消费 Runtime-authenticated read-only projection，显示可用时的 stage、failed gate、下一步允许动作、锁定动作和 candidate-bound evidence hashes；缺失或跨项目 evidence 时显示 unavailable/unknown/locked。它不会在本地创建 SceneGraph、Session 或 QualityReport。

未来 durable Viewer 面仍应通过 `scene_observe_get` / `visual_evidence_bundle_get` 只读显示：

- SceneGraph：Part tree、role、dimensions、symmetry、source map；
- 当前 camera、selection、multi-view AOV；
- ReferenceCanvas coverage 和 missing/unknown views；
- 当前 stage、失败指标、阈值和 evidence hash；
- Critic issue 列表和单 Part/MaterialZone repair intent；
- checkpoint/version/candidate 关系。

当前已实现的是 source/read-only projection surface，真实 Runtime 的 scene/stage 嵌套只读 projection conformance 也已有独立回执，但不是完整 durable target。DesignSession/Checkpoint 虽已具备受批准的持久化 readback，显式 bounded authoring_context 与 primary-form/secondary-structure/tertiary-detail proposal 也已进入 Runtime，但跨阶段写入 orchestrator、跨视图质量评估、MaterialZone/UV-PBR/Repair 应用、同 observation packaged binding 和正式无障碍/真人门仍为 `NOT_RUN` 或后续任务。在这些 Gate 关闭前，Viewer 只能把投影标为可重建观察，不能把本地 UI 推导成 DesignSession 真值。

### 5.3 2026-08-14 前端交互与诊断加固

- 3D viewport 使用懒加载的 Three runtime 与 `OrbitControls`，支持 orbit/pan/zoom；`ResizeObserver` 同步容器宽高、camera aspect、projection matrix 和 renderer size，不再固定为初始化时的 `aspect=1`。
- 候选选择从自动绑定扩展为显式“自动·最新任务 / 手动候选·历史”选择，并按最新/最旧切换；候选选择、GLB、参考比较、质量投影和生成耗时共用同一 candidate ID。
- 候选切换和 GLB 重试会完整释放 controls、scene、geometry、material、texture、renderer/context；轮廓边界与差异热图通过 `compare-worker.ts` 在 Worker 中计算，避免大图像循环阻塞主线程。
- Runtime、GLB、candidate-bound evidence、AOV 和比较资源失败均显示可复制的故障码及重试入口；比较面板增加缩放、亮度、双层透明度、热图敏感度、标尺、平移和当前视图导出。
- Viewer 轮询改为首次完整读取 + 变更摘要读取；仅在 project/head/candidate/version 相关摘要签名变化时重新拉取大 payload，后台页将摘要间隔放宽至 15 秒。
- 生成耗时独立面板按任务 ID 展示平均耗时、候选状态成功率和异常计数；缺失、未来时间、无法解析和超长耗时使用图标、文本和边框共同提示。状态图例统一区分通过、未通过/异常、未运行/未知。

本轮源码/build 验证：`desktop:typecheck`、Vite production build、`check_mcp010f_viewer.py`、Tauri `cargo check --offline` 和 `git diff --check` 均通过；本地浏览器空 Runtime 诊断与比较模式/标尺开关 smoke 通过。正式 packaged WebView/GPU、VoiceOver、真人视觉、PBR likeness、export/restart hash 和 360 仍按上文保持 `NOT_RUN/BLOCKED`。

## 6. 可访问性与性能

- 键盘可完成树、视图预设、issue 定位和 selection；
- 状态不只靠颜色，AOV 和 reference compare 有文字标签；
- 支持 reduced motion，爆炸动画可关闭；
- 1280×720 不横向溢出；大 Assembly 使用虚拟化；
- 交互 renderer 的卡顿不阻塞 Runtime Job；
- Renderer/WebGPU/WebGL 崩溃只影响显示，不影响候选、版本和导出真值。

## 7. 验收

- 源码中无旧 `cad-workbench` import；
- 只有一个 canvas/context；
- Viewer 关闭时完整 Runtime/MCP 流程仍成功；
- candidate、quality、selection、version 和 export 的 ID/hash 与 Runtime 一致；
- 重启恢复不依赖 localStorage；
- 当前已有 `npm run desktop:typecheck`、focused GLB/read-model、source build、packaged CLI read-model、原生窗口和核心控件 smoke evidence；这些不是 attempt35 的 packaged binding。正式 VoiceOver/screen-reader、独立人评、PBR likeness、export/restart hash 与 360 仍 `NOT_RUN/BLOCKED`；发布级 packaged WebView/GPU/签名安装环境属于 MCP013。
