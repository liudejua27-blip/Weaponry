# FGC-MCP010C 固定渲染与参考比较实施审计

> 2026-08-25 商业质量解释：固定 renderer、九 AOV、reference compare 和 typed visual review 是必要观察面，但不是商业 Hero Weapon 的充分条件。它们必须消费同 hash 的 approved Form、AuthoringMesh、High/Low/UV/Cage/Bake/Material，并由 commercial engine 与 independent human Gate 收口。当前状态仍为 `QUALITY_TARGET_NOT_MET`；详见 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`。

版本：2026-08-11
状态：`MCP010C in_progress / source-focused gate PASS_WITH_UNRUN_VISUAL_GATES`
前置任务：`MCP010B source-focused PASS；Darwin OS total-memory hard cap NOT_RUN`
范围：记录 C 的当前实现、证据、未运行门和后续收口边界；本文件不把合成参考或结构性证据写成真实机器人视觉质量证据。

## 1. 结论

当前 ForgeCAD 已能在隔离 source-built MCP/Runtime 中完成 C 的结构闭环：授权参考进入 CAS、V2 typed geometry 生成候选、固定相机渲染九个 AOV、生成本地参考 mask、计算比较指标、返回 MCP image block、保存 Codex review/human receipt，并由 `quality_get` 读回同一 candidate-bound QualityReport@2。现在也已用用户授权的机器人 PNG 完成一次真实参考运行；链路和证据绑定通过，但 primitive-only blockout 未达到视觉阈值，因此 `quality_get` 返回 `QUALITY_TARGET_NOT_MET`，不能把它宣传为高质量 likeness。

当前新增的 C renderer 是产品自有、离线、受限的固定路径：从严格 GLB readback 读取顶点/法线/UV/material、应用 scene/node transform，使用 512×512 perspective camera、真实 z-buffer、固定直接光照和确定性 2×采样，输出 `beauty`、`silhouette`、`depth`、`normal`、`ao`、`part-id`、`material-id`、`wireframe`、`uv-stretch` 九个 PNG。MCP008/MCP009 的 `RenderSet@1` 四 pass compatibility path 保持不变。

因此，以下结论仍然成立：

- 23-Part 和 51-Part 单图 V2 演练只证明 typed geometry/readback；增加 primitive 数量不是相似度优化；
- V1 三材质区演练只证明材质 plumbing、UV/tangent 和 GLB 回读；不能证明 PBR 纹理或参考相似度；
- C source Gate 的 IoU/boundary/bbox/centroid/landmark/region 计算已可回读。早期固定相机的历史 receipt 为 silhouette IoU `0.5132`、boundary F1 `0.1441`、bbox edge error `0.1074`；本轮增加默认相机的参考轮廓自动取景并修复 CAS 浮点指标往返后，最新真实 PNG raw receipt 达到 silhouette IoU `0.6623`、boundary F1 `0.2418`、bbox edge error `0.0566`、centroid error `0.0135`，仍低于当前 likeness 门槛。landmark coverage `0`、region median IoU `0` 反映当前 primitive blockout 没有提交可验证的局部标注；局部梯度 flood-fill 已避免原先的棚拍背景污染，但不等于语义分割。
- 单张三分之四参考最多允许 `PARTIAL_VISIBLE_VIEW_PASS`；没有 front/back/left/right/rear-three-quarter 全身参考，`HQ_360_PASS` 必须保持 `BLOCKED_REFERENCE_COVERAGE`。

## 2. 当前实现证据

### 2.1 Runtime/MCP 入口

当前 source-built MCP discovery 已包含 `render_pass_get`（read）、`material_pack_get`（read）以及 `reference_compare_prepare`、`visual_review_submit`、`human_visual_review_submit`（authenticated write opt-in）。当前全局工具面为 28 read + 18 write = 46；C 的历史 raw receipt仍按当时 20+16 结构保存。`appearance_prepare` 仍产生 `RenderSet@1` 四 pass compatibility path；C 的 `RenderSet@2` 只由 `reference_compare_prepare` 生成，避免改变 MCP008/MCP009 历史真值。轮廓目标/相机拟合/边界误差与 `CameraCalibrationRef@1` 属于后续 F source 增量，不改写 C 的历史 receipt。

### 2.2 当前 renderer 的事实

`apps/geometry-worker/src/lib.rs::render_perspective_glb` 当前：

1. 读取严格 GLB meshes/accessors/BIN，并拒绝不在闭合 profile 中的 payload；
2. 应用 scene/node transform 与固定 `CameraCalibration@1` perspective 相机；
3. 在 1024×1024 内部 buffer 做确定性三角形栅格化与深度测试，再降采样为 512×512；
4. 使用固定直接光照、glTF 因子、sRGB 输出和可回读的 part/material lineage；
5. 一次输出九个固定顺序 PNG，Runtime 将每个 pass 写入 CAS 后生成 RenderSet@2；
6. 旧 `render_fixed_glb` 仍只服务 MCP008/MCP009 兼容路径，不被 C 当成质量证据。

这条路径必须继续保持 Runtime-owned、离线、确定性和只读输入；C 实现不能把 Three.js、Blender、任意 Python/JS shader 或远程服务变成产品真值。

### 2.3 当前合同事实

仓库当前源合同为 77 个，其中 C 新增并由 Runtime producer/consumer 使用的七个合同为：

- `ReferenceViewSpec@1`；
- `CameraCalibration@1`；
- `RenderSet@2`；
- `ReferenceComparisonReport@1`；
- `VisualReviewReport@1`；
- `HumanVisualReviewReceipt@1`；
- `QualityReport@2` 生产、校验和 candidate 绑定。

Runtime 还以 `VisualEvidenceRecord` 保存 RenderSet/comparison/review/human/quality CAS 指针；`render_pass_get` 只读 CAS，不隐式重新渲染。

C 当前已新增严格 JSON Schema，并实现 Runtime producer/consumer 的顶层与嵌套输出校验；unknown field、缺失字段、越界数值和视觉评审条目变更会 fail closed。只添加 schema、更新 `MVP_TOOL_CATALOG.md` 或把空工具列入 `tools/list` 都不算实现。

### 2.4 首次真实机器人参考运行（历史固定相机 baseline） — `PASS_WITH_QUALITY_TARGET_NOT_MET`

使用用户授权的 `/Downloads` PNG（字节 SHA-256 `b9cb687e…c1cadd`，1254×1254）在全新临时 Runtime/CAS 中运行：`reference_import → operator_catalog_get → geometry_program_hash → geometry_prepare → reference_compare_prepare → render_pass_get`（九个 PNG）`→ visual_review_submit → quality_get`。Runtime 没有写入用户持久数据，也没有确认 candidate、创建 version 或伪造 human receipt。

该运行证明真实图片字节、candidate、RenderSet、comparison、review 和 QualityReport 的绑定链可用；局部梯度边界 flood-fill 让 bbox/centroid 指标比初版更稳定，但当前 primitive-only 机器人草图仍只是结构 blockout。生成的 beauty/silhouette 预览见临时输出目录，脱敏 receipt 见 `docs/evidence/mcp010c/real-reference-robot.json`。该 receipt 保留为历史 baseline，结论是 `QUALITY_TARGET_NOT_MET`，不是 `PARTIAL_VISIBLE_VIEW_PASS`。

### 2.4.1 默认相机取景与指标 CAS 往返修复 — `PASS_WITH_QUALITY_TARGET_NOT_MET`

当 `reference_compare_prepare` 未提供显式 `CameraCalibration@1` 时，Runtime 先用默认相机渲染一次，再在有界候选集中比较 height-only 与 width/centroid framing，按 silhouette、boundary、bbox 和 centroid 的综合分数选择唯一相机；显式 camera 仍完全由调用方控制，不改变模型或隐藏几何，只有胜出的九个 pass 会写入 CAS。真实用户 PNG 的最新隔离 raw 回归由 `docs/evidence/mcp010c/real-reference-robot-camera-search.json` 记录：IoU `0.6623`、boundary F1 `0.2418`、bbox edge error `0.0566`、centroid error `0.0135`，九个 AOV、typed review 和 `quality_get` 均成功，视觉状态仍为 `QUALITY_TARGET_NOT_MET`。

同一回归暴露并修复了一个数据真值问题：高精度 `f64` 视觉指标在写入/读回 CAS 后可能改变最后几位，导致 `visual_review_submit` 错误拒绝合法 comparison report。Runtime 现在在持久化前将视觉指标量化到 12 位小数，并用 CAS round-trip 回归证明 canonical hash 稳定；这不是放宽质量门。

随后又修复了区域指标的定义：`region-mask-iou-v2` 现在在每个声明的可见区域内比较 reference/model 两个 silhouette mask，unknown 区域不进入 aggregate；不再把整个模型 mask 与区域矩形直接比较。真实 20-Part 用户机器人回归的 region median IoU 从修复前的 `0.1847` 更正为旧 rounded-panel 前基线 `0.8625`、critical region min 为 `0.6509`。当前 linework/material-zoned source baseline 为 26 Parts/4704 triangles，region median IoU `0.8694`、critical region min `0.6663`，silhouette IoU `0.7410`、boundary F1 `0.3288` 和 landmark coverage `0.7333` 仍未过门，整体继续保持 `QUALITY_TARGET_NOT_MET`。8-zone AssetPack refinement 仅增加可审计的材质族分区，不改变这些 comparison 指标。修复只改善比较真值，rounded-panel、linework 和 material-zoned 都只是增量几何/材质改善，不构成 likeness 或人评通过；脱敏 receipts 见 `docs/evidence/mcp010f/rounded-panel-real-reference.json`、`docs/evidence/mcp010f/surface-linework-real-reference.json` 与 `docs/evidence/mcp010f/surface-zones-real-reference.json`。

### 2.5 真实 Codex CLI C 运行 — `PASS_WITH_QUALITY_TARGET_NOT_MET`

同一 source-built MCP/Runtime/geometry Worker cohort 的真实 Codex CLI 已完成六个短 turn：setup 创建/导入与 `reference_get` 回读、V2 capability/catalog/skill/hash/geometry prepare、candidate-bound readback/compare、九个 `render_pass_get`、`visual_review_submit` 和 `quality_get`。共 32 个 ForgeCAD MCP 调用全部 completed，生成 27 个语义 Part、4100 triangles 和 validator-passed GLB；九个 AOV 顺序与 candidate/render/comparison/review hash 绑定一致。脱敏历史 receipt 为 `docs/evidence/mcp010c/real-codex-cli-c-attempt13.json`。

Codex 过程中的两个非 MCP 事件是读取 `.codex/.../SKILL.md` 的只读查阅，已保留事件类型与 SHA-256 摘要；没有文件变更、网络调用或用户持久数据写入。该历史 receipt 保留了自动取景修复前的 `QUALITY_TARGET_NOT_MET`（silhouette IoU `0.5132`、boundary F1 `0.1441`），所以它证明的是“Codex 能真实调用 C 工具链”，不是高质量 likeness。

### 2.5.1 轮廓优先真实 Codex CLI 完整 transport — `PASS_WITH_QUALITY_TARGET_NOT_MET`

attempt32 是保留的 primitive route 历史 receipt。最新 receipt `docs/evidence/mcp010f/real-codex-cli-silhouette-first-20260813-attempt35-detail-camera-ref.json` 在同一隔离 source-built cohort 完成 11 个短 turn：reference/mask、V2 detail geometry hash/prepare、silhouette target、camera fit、Runtime-owned Rig hash、silhouette fit、candidate-bound readback、reference compare、boundary error、九个 AOV、typed visual review 与 quality。26 Parts、4704 triangles、validator-passed GLB 和所有 cohort/hash 绑定均通过；这次携带 15 个 image-derived landmarks/8 个 visible regions。结果为 `PASS_WITH_QUALITY_TARGET_NOT_MET`，silhouette IoU `0.741047`、boundary F1 `0.328765`、bbox edge error `0.007813`、centroid error `0.007878`、landmark coverage `0.733333`、landmark NME `0.134536`、region median `0.869403`、critical-region minimum `0.666289`。Runtime silhouette-fit proposal 为 `status=no_improvement`、IoU `0.698340`，仍是 read-only 参数建议，不是新 candidate。attempt33/34 的完整 CameraCalibration payload 因 canonical hash 漂移被 Runtime 正确拒绝；新增 `CameraCalibrationRef@1` 只传 Runtime-owned `camera_hash + canonical_sha256`，由 Runtime 按 candidate/target 证据解析完整相机，已消除该 transport 阻断。没有 human approval、PBR、confirm/export 或 360 门；未写入用户持久数据。camera search 内部仍为 64 次评估，Codex 只接收有限候选与哈希引用。

### 2.6 Viewer 只读比较面 — `source implementation PASS / packaged C transport PASS / Viewer UI E2E NOT_RUN`

源码现在提供了一个最小但可用的 Viewer 证据面：Runtime 通过 `visual_evidence_get` 返回 candidate-bound RenderSet、comparison、QualityReport 元数据；`reference_bytes_get` 只在项目绑定、CAS 元数据和实际 SHA-256 都匹配时返回参考图；`render_pass_get` 按需返回单个 AOV PNG。Tauri 只增加了对应的只读命令，React Viewer 增加参考图/Render AOV 分屏、透明叠加、闪烁切换、九个 AOV 选择、camera-lock 标识、质量指标与 hash 摘要，不创建 Runtime 状态或写入 CAS。

这一层已通过 `forgecad-runtime` 的九 AOV/视觉证据回读测试、Viewer Rust IPC 测试、Tauri `cargo check`、TypeScript `typecheck` 和生产前端构建。当前 Dev.app 的安装/包验证/隔离探针、九 AOV raw C probe 和 packaged Codex CLI compare/review transport 也已通过；对应脱敏 receipt 见 `docs/evidence/mcp010c/dev-app-*.json`。这证明 packaged Runtime/MCP 的 C 传输路径，不证明 Viewer UI 已在 packaged app 中运行。因此“Viewer compare surface”继续保持 `NOT_RUN`，仅 source implementation 与 packaged C transport 分开记录。

## 3. C 的最小实现顺序

按以下顺序领取一个独立的 MCP010C Goal。每一步都要有单独 receipt，PASS、FAIL、BLOCKED、NOT_RUN 分开记录。

### C1：合同和持久化边界 — `PASS`

1. 新增并注册七个合同，所有对象 `additionalProperties:false`；
2. 把 `candidate_id`、`artifact_sha256`、`program_sha256`、`reference_id/reference_sha256`、`camera_hash`、`render_set_hash` 和 `quality_report_hash` 绑定到同一 candidate；
3. `reference_compare_prepare` 只创建临时比较证据，不创建 version；
4. `quality_get` 只读取 Runtime 已持久化且与当前 candidate hash 一致的报告；
5. `human_visual_review_submit` 保存用户评分证据，但不声称模型身份认证或密码学 approval attestation。

### C2：固定相机和 renderer — `PASS`（source-focused）

1. 固定 512×512 perspective 相机，显式保存 transform、FOV、near/far、up-axis、handedness 和 camera hash；
2. 使用真正的三角形深度测试/z-buffer，处理 node transform 和遮挡；
3. 使用确定性抗锯齿、固定 GGX 直接光照和显式色彩管理；
4. 同一 candidate hash 一次性生成九个 pass：

   `beauty`、`silhouette`、`depth`、`normal`、`AO`、`part-ID`、`material-ID`、`wireframe`、`UV-stretch`。

5. PNG 必须内嵌/进入 CAS，pass 元数据、renderer revision、camera hash 和 candidate hash 必须可回读；
6. renderer 错误、超时、坏 PNG、hash 不一致或 pass 缺失必须在 candidate/version 写入前 fail closed。

### C3：参考比较和视觉 review — `PASS`（synthetic + real transport；真实 likeness threshold 未通过）

1. 当前干净棚拍参考使用 `mask-2` 的本地确定性梯度 border flood-fill/morphology 生成 silhouette mask；不得引入远程分割模型；
2. Codex 提交 normalized landmarks、region、visibility 以及 `observed/inferred/unknown` 标记；
3. 计算 silhouette IoU、4 px boundary F1、bbox edge error、centroid error、landmark coverage/NME 和 region IoU；
4. `visual_review_submit` 只保存绑定具体 pass/region/candidate hash 的 typed issue 和建议；Codex review 不能修改硬门结果；
5. 每个 candidate 最多五轮：`silhouette → structure → form → material/surface → final`；任何一轮未达到目标都返回 `QUALITY_TARGET_NOT_MET`，不能偷偷 confirm 低质量 candidate。
6. 首次真实机器人运行已完成一轮 `silhouette` review，Codex typed issue 要求补充 panel/vent/cable/joint detail；它没有改变硬门结果，candidate 保持未确认。
7. 默认 camera auto-fit 与浮点指标 CAS round-trip 已加入当前 Runtime；最新真实 PNG raw receipt 提升了轮廓指标但仍未达到 likeness 门槛，不能确认或导出。

### C4：工具和返回面 — `PASS`（raw stdio + source/packaged real Codex CLI；Viewer UI 仍未运行）

当前 source gate 已增加并验证以下 MCP tools：

| 工具 | 访问 | 必须证明 |
| --- | --- | --- |
| `render_pass_get` | read | 返回已持久化、hash-bound 的真实 PNG image block，不隐式生成 render |
| `reference_compare_prepare` | write/temporary | camera、render、mask、metrics、diff 全部绑定 candidate/reference，不创建 version |
| `visual_review_submit` | write/evidence | typed region issue、pass、claim、confidence 和建议可回读 |
| `human_visual_review_submit` | write/evidence | 用户评分与 candidate/reference/render hash 绑定，缺字段 fail closed |

## 4. C 的验收矩阵

### 合同和数据真值

- Schema checker、Runtime producer/consumer 和 MCP envelope validator 全部通过；
- unknown field、缺失 hash、跨 candidate/reference、pass 缺失、损坏 PNG、错误 camera、过期 review 全部 fail closed；
- 同一机器同一 candidate 的真实 MCP 比较连续重复五次，render pass、RenderSet、metrics 和 report hash 完全一致；脱敏 receipt 见 `docs/evidence/mcp010c/determinism-5x.json`；
- restore/restart/export 后 render、reference、quality、candidate hash 不漂移。

### Renderer

- 512×512 perspective、camera transform、z-buffer 和 scene/node transform 有合成测试；
- 通过遮挡、深度排序、背面、极薄结构、透明/不透明边界和空 mesh 负向测试；
- 九个 pass 全部真实 PNG、尺寸/色彩空间/通道元数据正确；
- 不把 `RenderSet@1` 的四 pass 或 `fixed_render: passed` 当成 C 的九 AOV PASS。

### 参考比较

- 使用固定合成参考验证 IoU、boundary F1、bbox、centroid、landmark 和 region 指标单位；
- 参考裁切、旋转、不同背景和不可见区域测试不泄漏 inferred 区域；
- 真实用户图片只在用户授权字节已进入 CAS 时运行；本轮已完成一次真实运行但 likeness threshold 失败，下一轮必须先改进轮廓/比例并重新比较；没有 reference evidence 时状态为 `unavailable`；
- 当前机器人图片仍只能产生 `PARTIAL_VISIBLE_VIEW_PASS`，不能生成 `HQ_360_PASS`。

### Codex/Viewer 证据

- raw stdio 与真实 Codex CLI 已完成 `catalog/camera → render → compare → review → quality` 的绑定链；CLI receipt 仍保留 `QUALITY_TARGET_NOT_MET`，不升级为 likeness PASS；
- Viewer 源码已只读显示参考、九 AOV、overlay/flicker、camera lock 和质量指标；隔离 Vite browser DOM smoke 已实际点击 AOV、模式、轮廓画布、heatmap/flicker 控件并验证无 metrics 时的空队列，但当前 Dev.app 的 C renderer/compare/review transport 已通过，正式 Viewer UI 的 packaged/current-cohort E2E、Part/MaterialZone 真实候选筛选、explosion 临时状态和 heatmap 数据态仍未运行；
- Viewer 关闭、重启和 export 不改变 Runtime 真值；Viewer hash 必须与 export hash 相同；
- C 完成前 packaged Viewer compare、真人评分和完整 360°继续 `NOT_RUN/BLOCKED`；packaged Codex C 仍明确为 `QUALITY_TARGET_NOT_MET`，不得升级为 likeness PASS。

## 5. C 当前收口检查清单

用户已显式领取 MCP010C；Luna 后续只能继续本原子 Goal，并保持 C–F 的顺序。当前已完成：

1. 59-contract checker、Worker renderer、Runtime review chain、嵌套 receipt validator、raw stdio receipt 和真实 Codex CLI C receipt 已建立；当前还新增了默认 camera auto-fit、fractional-metric CAS round-trip 和同一 candidate 五次 MCP determinism receipt；
2. MCP010B 的 V2 graph、Worker isolation 和 closed GLB Gate 未被覆盖；
3. synthetic source Gate 和首次真实机器人参考运行均已记录；真实运行只写入隔离临时 CAS，quality target 未通过，未改变用户持久数据；
4. Viewer compare/read-only UI 的 source implementation 已完成并通过本地构建/IPC 测试；五次 render/metrics/report determinism 已通过 source MCP receipt；还需完成同 cohort packaged Viewer probe、export/restart hash；人评仍必须由用户实际提交，不能由脚本代填。

本文件的结论是 `MCP010C in_progress / source-focused PASS_WITH_UNRUN_VISUAL_GATES`，不是“当前已对单张图片生成高质量 3D”的宣传声明。
