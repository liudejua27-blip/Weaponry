# ForgeCAD Runtime Viewer

2026-08-22 `CandidateMaterialSurfaceQuality@1` public positive fixture：`Geometry → CandidateTopologyQuality@1 → AppearanceProgram@3 → TextureBuild@2 → SurfaceBake@1 → AppearanceSourceLineage@1 → CandidateMaterialSurfaceQuality@1` 的 `prepare → same-key replay → get → Runtime drop/reopen → restart get` 通过 **1/1（111.72s）**；Runtime focused **5/5**、Store full **74/74**、Contracts **350**。CAS inventory unchanged；stable `artifact_id` 与 GLB object SHA-256、MaterialPack CAS kind 精确区分，合法 UV/tangent rebuild 不计入 geometry-preservation 漂移。该结果仅为 `structural_only`；V2 animated-socket-particles 仍无完整 public `prepare → Store → restart get`，durable end-to-end=`NOT_RUN`/`BLOCKED_FIXTURE_CHAIN`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/candidate-material-surface-quality-public-positive-source-gate-20260822.json`。

最终同 cohort 修订口径：强制 build cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` 下 material focused **5/5（112.63s）**；Runtime full **310 passed / 0 failed / 20 ignored**（330 total，201.91s），且 public material fixture 明确在该 full run 内执行。此前 **111.72s** 仅为 public fixture 单测时长；两者都只支持 `structural_only`，不提升 visual/commercial、human/engine 或 stage/confirm/version/export 状态。

数值口径：当前 source 为 **375 schemas / 26/26 active operators / 85 read + 64 write = 149 tools**；本文的 291/118、284/116、271/112、264/110、257/108、231/100、229/99、227/98、221/96、215/94、210/92、204/91、201/90、197/90、195/90、193/90、191/90、187/89、177/84、175/83、173/82、170/80、168/79、166/78、164/78、162/77、160/76 仅作 historical prior slice 保留。

2026-08-22 `FictionalEnergyVfxAnimatedSocketParticlesSequence@2` 双候选 source slice：Contracts **350**；Store V2 focused **2/2**、Store full **74/74**；Runtime V2 仅低层 focused **6/6**、cargo check **PASS**；MCP V2 **3/3**；同 cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` Runtime full **309 passed / 0 failed / 20 ignored**（191.06s）、MCP full **128 passed / 0 failed / 0 ignored**（1.93s），这些是全量回归，不是 V2 public `prepare → Store → restart get` 正向 fixture。V1/V2 隔离；V2 仅证明 1..16 frame、geometry/appearance 双 candidate/delivery/AnchorSet bridge 以及 Store FK/reachability/idempotence/conflict/rollback 的结构面。完整双候选 public Runtime `prepare → Store → restart get` 正向 fixture 尚不存在，durable end-to-end=`NOT_RUN` / `BLOCKED_FIXTURE_CHAIN`，不能声称正向 durable。该 slice 为 `structural_only`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/fictional-energy-vfx-animated-socket-particles-v2-dual-candidate-source-gate-20260822.json`。

2026-08-20 `recessed-channel@1` 本轮只增加 Geometry/Runtime/MCP/Skill source capability，没有新增 Viewer 写入口或把 channel 解释为宿主编辑历史。Viewer 仍按现有 Part/MaterialZone/readback 投影结果；package/live UI、VoiceOver 与视觉/人评状态不变。

2026-08-20 Mechanical Animation Viewer source slice 现有三个认证、只读 Tauri/Runtime IPC 入口：inventory、verified clip detail，以及 immutable schedule 内的 single-tick Runtime frame preview。Viewer 只把 Runtime 双 Worker 已验证的 rigid Part delta 应用到 GLB 唯一 identity Part owner；embedded animation、Bone/SkinnedMesh、未知/重复/嵌套/nonidentity owner 全部 fail closed，动画与 explosion 在一次 baseline-reset effect 中组合。playhead、展开和选择都是临时 UI 状态；无自动连续播放、Viewer 本地 pose 求值、prepare/confirm 或永久写入。该 slice 是 `structural_only`，不是 Armature、skin、IK、NLA/F-Curve、GLB animation、Blender/Python parity 或视觉 PASS；package/live UI E2E、正式 VoiceOver 与真人门仍未运行。

2026-08-19 Authoring Mesh Edit Prepare 只新增 Runtime/MCP approval-gated staging surface；Viewer 本轮没有新增直接 mesh 编辑器或写入口。新 candidate 仍通过既有只读 candidate/read-model 查看，确认必须回到 Codex 审批链；不得把它展示成 Blender/BMesh/Python/plugin parity 或视觉质量通过。

2026-08-19 Render Evidence Replay 本轮只新增 Runtime/MCP 只读 surface，没有新增 Viewer 重放按钮或第二质量门。未来 Viewer 若展示，只能呈现 same-cohort/profile、九 AOV byte/pixel exact 和 structural-only limitations；不得从该结果推导 visual/PBR/human PASS。package/live Viewer 与 VoiceOver 状态未升级。

2026-08-19 Mechanical Pose Geometry Preview 目前只存在于 Runtime/MCP source read surface；本轮未新增 Viewer timeline、playback、armature、skin 或持久 artifact UI。未来 Viewer 只能把它标为 caller-authored rigid pose 的 transient structural preview，不能把 hash/readback当 durable candidate/version 或原资产 rig provenance；package/live Viewer、VoiceOver、visual/human Gate 未升级。

2026-08-19 当前 Subdivision artifact-lineage 已有 Runtime-owned immutable CAS sidecar 与只读 getter，但本轮未新增 Viewer 写入口或展示 UI。未来 Viewer 只能经 read model 显示 candidate-local、source-primitive-local triangle lineage，并明确 no glTF V/E/C、cross-version false、structural-only；package/live Viewer、VoiceOver、视觉/人评状态未升级。

2026-08-19 当前 Subdivision artifact-lineage 仍是 Runtime/MCP 只读 source slice；Viewer 本轮未新增写入口，也不把 reconstructed projection 冒充 durable sidecar。未来 Viewer 若显示它，只能标注 source-primitive-local triangle identity、no glTF V/E/C identity、cross-version false 和 structural-only；package/live Viewer、VoiceOver、视觉/人评状态不变。

2026-08-19 historical Subdivision root-lineage source slice：本轮未改 Viewer，也未把 mapping 持久化到 artifact/GLB 或 Viewer read model。Viewer 不得把 preview ID 当 artifact/cross-version ID；package/live Viewer、正式 VoiceOver、visual/human Gate 不因本 structural PASS 升级。

2026-08-19 historical Boolean Operand Lineage Runtime/MCP 只读 source slice 当时为 164 schemas、19/19 active operators、45 read + 33 opt-in write = 78 tools。本轮未改 Viewer，也未把 lineage 持久化进 GLB 或 Viewer read model；package/live Viewer、正式 VoiceOver、visual/human Gate 不因该 source structural PASS 而升级。source receipt：`docs/evidence/mcp010f/blender-boolean-operand-lineage-source-gate-20260819.json`。

2026-08-19 Render Evidence Integrity 是 Runtime/MCP 只读 historical source slice，该 slice 当时为 162 schemas、19/19 active operators、44 read + 33 opt-in write = 77 tools。本轮未改 Viewer；Viewer 仍消费既有 RenderSet/quality read model，不自行重算 CAS/PNG/threshold 结论。package/live Viewer、正式 VoiceOver、visual/human Gate 不因该 source structural PASS 而升级。

2026-08-19 Mechanical Pose Sequence Preview 是 Runtime/MCP 只读 historical source slice，该 slice 当时为 160 schemas、19/19 active operators、43 read + 33 opt-in write = 76 tools。本轮未改 Viewer，未生成 candidate、timeline、playback、armature 或 skin UI；package/live Viewer、正式 VoiceOver、visual/human Gate 均不因 source structural PASS 而升级。receipt：`docs/evidence/mcp010f/blender-mechanical-pose-sequence-preview-source-gate-20260819.json`。

2026-08-18 historical Parametric Group v2 是 Runtime/MCP 只读 source slice；该 slice 当时为 158 schemas。本轮未改 Viewer，未生成 candidate、timeline 或 node editor UI。receipt：`docs/evidence/mcp010f/blender-parametric-group-v2-source-gate-20260818.json`。

2026-08-18 historical source slice 当时为 160 Schema、43 read + 33 opt-in write = 76 tools。Mechanical pose、Modifier evaluation v2 与 Subdivision evaluation v2 均由 Codex/MCP/Runtime 只读消费；Viewer 当前不新增姿态时间轴、骨骼编辑器或持久 pose 状态，也不重算 hierarchy/hash。`RenderProfile@1`/AOV lineage 由 Worker 和 Runtime 生产、Viewer 只读消费；`topology_snapshot_get` 仍是 Runtime/MCP read model。既有 Viewer source/package smoke 与视觉 Gate 继续分开记录。

2026-08-17 历史 Stage 0 覆盖为 138 Schema；该阶段随后达到 144 Schema、41 read + 33 opt-in write = 74 tools；Viewer 仍只读。`repair_intent_run_prepare` 为 Codex/Runtime 的 CAS-bound staged-run source slice，Viewer 不调用它，也不提供 Repair apply/confirm。

版本：2026-08-17
状态：当前源码口径为 290 Schema、26/26 active operators、69 read + 49 opt-in write = 118；MCP010F source Viewer 在原有九 AOV、reference compare、Part/MaterialZone、explosion、diff/contour 和 mechanical clip inspector 之上，新增 candidate-state-bound Provenance Graph。图以紧凑纵向树显示 GeometryProgram/Operator DAG/ArtifactReadback/GeometryQuality，并且只在 Runtime 完整验证后显示 visual 与 animation 分支；不加入任何写按钮。唯一 `in_progress` 仍为 `FGC-MCP010F`；本轮 package/live UI E2E、正式 VoiceOver、真人/PBR/export-restart/360 仍 `NOT_RUN/BLOCKED`，Viewer 不提供 durable 写入。

Stage 0 Viewer 证据边界读取 `docs/evidence/mcp010f/current-benchmark-truth.json`：attempt35 只是 provisional retained observation，为 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，fit/compare camera 为 `MISMATCH`；现有 packaged Viewer receipt 又来自不同 cohort/artifact，未绑定 attempt35。故已实现的 Viewer surface 和 package smoke 只能证明读取/交互表面，不能证明同一 candidate 的视觉、PBR、human、export/restart 或 360 通过。

<!-- forgecad-stage0: schemas=404 schema_set_sha256=a2517bd579b3caf769182c87aab9252323c8cfc9a5acd9ae0a779911c80d963a read_tools=91 write_tools=69 total_tools=160 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260815-b37-complete-auto-v3.json latest_completed=real-codex-cli-current-20260815-b37-complete-auto-v3.json -->

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
┌ ForgeCAD / project ─── Modeling | Compare | Review ─── Codex / export ┐
├ Structure | References ┬────── one primary 3D viewport ─────┬ Inspector ┤
│ searchable Part tree   │ layout / camera / shading / aids   │ tabs      │
│ reference thumbnails   │ compare or review mode when chosen │ read-only │
├────────────────────────┴─────────────────────────────────────┴───────────┤
│ Versions | Codex tasks | Quality issues | Activity log · collapsed rail │
└──────────────────────────────────────────────────────────────────────────┘
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

Viewer 允许把左右面板宽度、底部抽屉展开状态和抽屉标签保存为本机界面偏好；这些字段不包含 project/candidate/version/hash，也不能恢复或覆盖 Runtime 产品状态。

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
- 已实现 source surface：beauty/silhouette/depth/normal/AO/part-ID/material-ID/wireframe/UV-stretch 九 AOV；
- 已实现 ephemeral surface：Part/MaterialZone 筛选、临时 explosion、轮廓画布、hash-bound 草图复制与只读 correction queue；
- 已有证据：TypeScript/Vite/Tauri source Gate、packaged CLI read-model、原生窗口以及 AOV/Home/End/overlay/flicker/轮廓/热图/爆炸图核心控件 smoke；
- 尚未关闭：full issue editing/candidate undo-redo、正式 VoiceOver、同一 provisional observation 的 packaged Viewer binding、独立真人评分、PBR likeness、export/restart 同 hash 与 360。

这些 UI 只消费 Runtime 的 `RenderSet@2`、QualityReport 和 selection projection。`RenderSet@2` 内的 Render Worker cohort/status 只作为 Runtime 认证后的只读绑定信息展示，Viewer 不重算或晋级质量门。屏幕图像、Three.js scene 或本地交互状态不能回写质量 PASS。当前 high-quality inspection 路径是 `GeometryProgram@2` detail → strict readback → `RenderSet@2` 九 AOV → candidate-bound strict compare → typed visual review；`[transition-v1]` `GeometryProgram@1` primitive-only / `RenderSet@1` 四 pass 仅用于历史兼容。正式 VoiceOver 与 provisional observation package binding 属 MCP010F 未关闭子门；Developer ID、clean install、发布级 packaged WebView/GPU/Codex E2E 仍属 MCP013。

### 5.2 ADR-0026 目标 Viewer 面

当前 Viewer 已通过 `agentic-design.ts` 消费 Runtime-authenticated read-only projection，显示可用时的 stage、failed gate、下一步允许动作、锁定动作和 candidate-bound evidence hashes；缺失或跨项目 evidence 时显示 unavailable/unknown/locked。它不会在本地创建 SceneGraph、Session 或 QualityReport。

未来 durable Viewer 面仍应通过 `scene_observe_get` / `visual_evidence_bundle_get` 只读显示：

- SceneGraph：Part tree、role、dimensions、symmetry、source map；
- 当前 camera、selection、multi-view AOV；
- ReferenceCanvas coverage 和 missing/unknown views；
- 当前 stage、失败指标、阈值和 evidence hash；
- Critic issue 列表和单 Part/MaterialZone repair intent；
- checkpoint/version/candidate 关系。

当前已实现的是 source/read-only projection surface，真实 Runtime 的 scene/stage 嵌套只读 projection conformance 也已有独立回执，但不是完整 durable target。DesignSession/Checkpoint 虽已具备受批准的持久化 readback，跨阶段写入 orchestrator、durable/reference/DesignSpec 完整 producer、Critic/Repair 执行、同 observation packaged binding 和正式无障碍/真人门仍为 `NOT_RUN` 或后续任务。在这些 Gate 关闭前，Viewer 只能把投影标为可重建观察，不能把本地 UI 推导成 DesignSession 真值。

### 5.3 2026-08-14 前端交互与诊断加固

- 3D viewport 使用懒加载的 Three runtime 与 `OrbitControls`，明确采用左键选中、右键拖动旋转、中键拖动平移、滚轮缩放；`ResizeObserver` 同步容器宽高、camera aspect、projection matrix 和 renderer size，不再固定为初始化时的 `aspect=1`。
- 候选选择从自动绑定扩展为显式“自动·最新任务 / 手动候选·历史”选择，并按最新/最旧切换；候选选择、GLB、参考比较、质量投影和生成耗时共用同一 candidate ID。
- 候选快照卡片同时展示耗时、GLB/参考/RenderSet/比对/质量绑定、Part/材质区、GLB 回读 canonical、Part→MaterialZone 绑定、UV、切线、PBR 材质区 check 和 Validator；当前候选与上一候选的 diff 快览不再只比较时间和状态，缺少 QualityReport check 时显示“未运行”。
- 候选切换和 GLB 重试会完整释放 controls、scene、geometry、material、texture、renderer；不在仍复用的 canvas 上强制丢失 WebGL context，避免历史候选切换后 renderer 无法重新初始化。轮廓边界与差异热图通过 `compare-worker.ts` 在 Worker 中计算，避免大图像循环阻塞主线程。
- Scene Tree 只在左侧工作台列渲染一份；Part/MaterialZone 选择按钮自身承担 `treeitem` 语义，支持搜索、过滤、显隐、锁定和方向键导航。GLB 回读时为每个 Mesh 隔离临时 Material 实例（纹理仍共享），避免共享材质导致跨部件高亮或重复 DOM/ID；Shift+左键框选可同时高亮多个对象，主对象仍用于 Inspector/聚焦；悬停 Raycaster 按 `requestAnimationFrame` 节流，避免大模型 pointermove 触发连续 React 更新。
- Runtime、GLB、candidate-bound evidence、AOV 和比较资源失败均显示可复制的故障码及重试入口；比较面板增加缩放、亮度、双层透明度、热图敏感度、标尺、平移和当前视图导出。
- Error Console 对候选缺载荷/证据未就绪同时提供“刷新当前候选”和“切换自动候选（放弃当前查看）”动作；GLB/比较故障提供重试与切换动作。Viewer 不执行 reject/confirm/删除等 Runtime 写入，“放弃当前查看”只是切换临时选择，不改变候选数据。
- Error Console 默认只保留摘要，详情可展开；真正的阻断错误自动展开。中心工作区保持 Scene / Viewport / Inspector 的固定关系，中心列在小窗口中纵向滚动以保证参考比较不会被裁切；场景树声明真实多选语义，框选取消会恢复 OrbitControls。
- 新手基础模式将右侧 Inspector 限定为当前选中对象、当前候选和三项基础状态；耗时、证据哈希、Agentic 阶段和质量细节收进“专业检查”抽屉。参考比较默认只显示状态与基础画面，AOV、热图、标尺、筛选和导出收进可展开的“专业对比”区域。
- 基础模式的工作台术语改为“模型、3D 视口、对象信息、当前版本、检查结果”；Part/MaterialZone、RenderSet 等内部字段只在专业面板或任务详情中出现。无项目/无候选时隐藏无效的版本排序和视角按钮，视口提供“描述需求 → 等待版本 → 查看确认”的三步空状态引导。
- 项目首页、创建、检查确认和导出页沿用同一新手路径：先用自然语言描述，再查看模型版本，最后检查和导出；candidate-bound、confirmed head、Camera lock 等内部状态在普通页面改为用户可读文案。
- 差异热图与轮廓 Worker 的图像解码、512×512 重采样和像素循环优先在 `OffscreenCanvas/createImageBitmap` Worker 路径执行；旧 WebView 自动回退到主线程解码后仍把差异/轮廓循环放在 Worker。热图敏感度滑杆有 120ms 有界防抖，避免连续拖动启动无效 Worker。失败也会投影到全局 Error Console；成功重试会清除旧的辅助计算故障码，原始 AOV 与 Runtime `QualityReport` 仍保持独立。
- Viewer 轮询改为首次完整读取 + 变更摘要读取；仅在 project/head/candidate/version 相关摘要签名变化时重新拉取大 payload，后台页将摘要间隔放宽至 15 秒。
- 生成耗时独立面板按任务 ID 展示平均耗时、候选状态成功率和异常计数；缺失、未来时间、无法解析和超长耗时使用图标、文本和边框共同提示。状态图例统一区分通过、未通过/异常、未运行/未知。

本轮源码/build 验证：`desktop:typecheck`、Vite production build、`check_mcp010f_viewer.py`、Tauri `cargo check --offline` 和 `git diff --check` 均通过；本地浏览器空 Runtime 诊断与比较模式/标尺开关 smoke 通过。正式 packaged WebView/GPU、VoiceOver、真人视觉、PBR likeness、export/restart hash 和 360 仍按上文保持 `NOT_RUN/BLOCKED`。

### 5.4 2026-08-17 Canvas-first 布局升级

- 顶部改为项目切换、`建模 / 对比 / 审查` 三模式和只读批准边界；确认按钮保持禁用并明确回到 Codex，导出继续进入受保护页面。
- 左侧固定为 `结构 / 参考图` 两个标签，默认宽度 248px；右侧固定为 `属性 / 几何 / 材质 / 检查` 四个标签，默认宽度 320px。两侧支持 220–360px / 280–420px 有界拖动与键盘方向键、Home、End 调整，只持久化界面宽度。
- 中央视口把版本、`1/2/4` 布局、相机、实体/材质/线框、参考图/网格/Gizmo 和重置收进单一紧凑工具栏；建模模式不再常驻参考比较，避免压缩主视口。
- 参考比较只在对比模式出现，保留分屏、叠加、闪烁、AOV、热图、标尺和筛选；无候选、参考图或固定视角渲染时显示逐项前置条件与对应恢复动作，不再只显示空白占位。审查模式保留 3D 视口，且只有候选绑定问题才显示空间标记。
- Runtime 断连时，顶部状态、中央恢复横幅、对象信息和底部“连接问题”保持同一语义；重试和诊断均可直接操作。导出仅在 Runtime 已连接、存在候选版本且权威质量门通过时可用。
- 版本、Codex 任务、连接/检查问题和活动日志改为默认 42px 的底部抽屉，展开为 238px；问题行提供真实恢复动作。900px 以下不再删除左右面板，而由顶部“结构 / 对象”按钮打开覆盖式抽屉，并支持 Esc 或遮罩关闭。
- 后续按“低信息密度、低视觉噪音”收敛默认态：无模型时隐藏搜索、筛选、四类对象标签和成排禁用视图控件，只保留版本入口、连接恢复和单句空状态；模型可用后再显示相机与显示模式，布局和辅助显示收进“视图选项”。
- 桌面键盘路径新增 `⌘K` 命令菜单，支持搜索、上下键和 Enter 执行；`⌘1/2/3` 切换建模/对比/审查，`⌘B` 聚焦结构搜索，`⌘I` 打开对象信息，`⌘J` 展开或收起底部面板。鼠标路径仍保留，不要求用户记忆快捷键。
- 用户可见的基础路径统一为简体中文；Runtime、Codex、GLB、AOV、PBR 等保留为产品名或专业缩写，内部原始状态仅用于无障碍/诊断，不作为主文案。
- 1672×941、900×900 与 720×800 空 Runtime 浏览器证据已验证恢复、对比前置条件、导出禁用、命令菜单、紧凑抽屉、Esc 关闭、面板键盘缩放和文字无重叠；控制台无 warning/error。该证据只证明 current-source UI 布局与交互，不替代当前安装包、真实模型质量或 reference likeness。

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
