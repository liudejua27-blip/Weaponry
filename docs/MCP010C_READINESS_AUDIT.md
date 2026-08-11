# FGC-MCP010C 固定渲染与参考比较实施审计

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
- C source Gate 的 IoU/boundary/bbox/centroid/landmark/region 计算已可回读。真实机器人运行的指标为 silhouette IoU `0.5132`、boundary F1 `0.1441`、bbox edge error `0.1074`、centroid error `0.0169`，仍低于当前门槛；landmark coverage `0`、region median IoU `0` 反映当前候选没有提交可验证的局部标注；局部梯度 flood-fill 已避免原先的棚拍背景污染，但不等于语义分割。
- 单张三分之四参考最多允许 `PARTIAL_VISIBLE_VIEW_PASS`；没有 front/back/left/right/rear-three-quarter 全身参考，`HQ_360_PASS` 必须保持 `BLOCKED_REFERENCE_COVERAGE`。

## 2. 当前实现证据

### 2.1 Runtime/MCP 入口

当前 source-built MCP discovery 已包含 `render_pass_get`（read）以及 `reference_compare_prepare`、`visual_review_submit`、`human_visual_review_submit`（authenticated write opt-in）。工具面为 20 read + 16 write = 36。`appearance_prepare` 仍产生 `RenderSet@1` 四 pass compatibility path；C 的 `RenderSet@2` 只由 `reference_compare_prepare` 生成，避免改变 MCP008/MCP009 历史真值。

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

仓库当前有 59 个合同，其中 C 新增并由 Runtime producer/consumer 使用的七个合同为：

- `ReferenceViewSpec@1`；
- `CameraCalibration@1`；
- `RenderSet@2`；
- `ReferenceComparisonReport@1`；
- `VisualReviewReport@1`；
- `HumanVisualReviewReceipt@1`；
- `QualityReport@2` 生产、校验和 candidate 绑定。

Runtime 还以 `VisualEvidenceRecord` 保存 RenderSet/comparison/review/human/quality CAS 指针；`render_pass_get` 只读 CAS，不隐式重新渲染。

C 当前已新增严格 JSON Schema，并实现 Runtime producer/consumer 的顶层与嵌套输出校验；unknown field、缺失字段、越界数值和视觉评审条目变更会 fail closed。只添加 schema、更新 `MVP_TOOL_CATALOG.md` 或把空工具列入 `tools/list` 都不算实现。

### 2.4 首次真实机器人参考运行 — `PASS_WITH_QUALITY_TARGET_NOT_MET`

使用用户授权的 `/Downloads` PNG（字节 SHA-256 `b9cb687e…c1cadd`，1254×1254）在全新临时 Runtime/CAS 中运行：`reference_import → operator_catalog_get → geometry_program_hash → geometry_prepare → reference_compare_prepare → render_pass_get`（九个 PNG）`→ visual_review_submit → quality_get`。Runtime 没有写入用户持久数据，也没有确认 candidate、创建 version 或伪造 human receipt。

该运行证明真实图片字节、candidate、RenderSet、comparison、review 和 QualityReport 的绑定链可用；局部梯度边界 flood-fill 让 bbox/centroid 指标比初版更稳定，但当前 primitive-only 机器人草图仍只是结构 blockout。生成的 beauty/silhouette 预览见临时输出目录，脱敏 receipt 见 `docs/evidence/mcp010c/real-reference-robot.json`。当前结论是 `QUALITY_TARGET_NOT_MET`，不是 `PARTIAL_VISIBLE_VIEW_PASS`。

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

### C4：工具和返回面 — `PASS`（raw stdio；packaged/live Desktop C gate 未运行）

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
- 同一机器重复五次 render、pass、metrics 和 report hash 完全一致；
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

- raw stdio 与真实 Codex CLI 必须完成 `catalog/camera → render → compare → review → quality` 的绑定链；
- Viewer 只读显示参考、九 AOV、overlay/flicker/heatmap、camera lock、Part/MaterialZone 选择和 explosion 临时状态；
- Viewer 关闭、重启和 export 不改变 Runtime 真值；Viewer hash 必须与 export hash 相同；
- C 完成前 Viewer compare、真人评分和完整 360°继续 `NOT_RUN/BLOCKED`。

## 5. C 当前收口检查清单

用户已显式领取 MCP010C；Luna 后续只能继续本原子 Goal，并保持 C–F 的顺序。当前已完成：

1. 59-contract checker、Worker renderer、Runtime review chain、嵌套 receipt validator 和 raw stdio receipt 已建立；
2. MCP010B 的 V2 graph、Worker isolation 和 closed GLB Gate 未被覆盖；
3. synthetic source Gate 和首次真实机器人参考运行均已记录；真实运行只写入隔离临时 CAS，quality target 未通过，未改变用户持久数据；
4. 还需完成真实 Codex CLI C 证据、Viewer compare/read-only UI、同 cohort packaged C probe、五次 render/metrics/report determinism 和 export/restart hash；人评仍必须由用户实际提交，不能由脚本代填。

本文件的结论是 `MCP010C in_progress / source-focused PASS_WITH_UNRUN_VISUAL_GATES`，不是“当前已对单张图片生成高质量 3D”的宣传声明。
