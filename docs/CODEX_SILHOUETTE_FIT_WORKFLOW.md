# ForgeCAD 单参考轮廓优先工作流

版本：2026-08-13 · 任务：FGC-MCP010F contour-first slice

## 目的

单张参考图首先约束“看得见的轮廓和比例”，然后才进入语义 Part、表面细节、UV/PBR 和材质。轮廓不是 Viewer 的临时画线，也不是把二维多边形直接挤压成最终模型；它是 Runtime 保存的、可复核的 `SilhouetteTarget@1`，供相机搜索、固定渲染、边界诊断和下一轮单 Part 修改共同使用。

单张三分之四参考仍然不能证明背面、脚部或完整 360°。未知区域必须标记 `unknown/inferred`，不能由 Codex 猜测后写成观察事实。

## 当前可调用闭环

1. `reference_import`：Codex 先确认用户授权，把图片字节写入 Runtime CAS。不得传原图路径、URL、secret 或未经授权文件。
2. `reference_mask_prepare`：输入 `project_id`、`reference_id`，可选传入归一化 `contour_points`、landmarks、parts。`parts` 是 contour point 数组上的非重叠、闭合区间（`start_index`/`end_index`）；Runtime 会拒绝重复/重叠/越界区间。Runtime 生成 512×512 mask，将 target/mask 写入 CAS，返回 `ReferenceMaskPrepareResult@1`。
3. `silhouette_target_get`：只读回 target，验证 target canonical hash、reference hash、mask hash 和项目范围。目标是唯一的轮廓比较输入。
4. `geometry_program_hash` → `geometry_prepare` → `artifact_readback_get`：构造受限 GeometryProgram@2；先读 live OperatorCatalog，保持 program/catalog/project hash 一致。
5. `camera_fit_prepare`：对现有 candidate 运行固定的轻量相机搜索：产品路径评估 8 个均衡的 yaw/pitch/取景候选（覆盖默认、对称方向、FOV/distance 变化和 framing probe）；37 个候选与前三名各 9 个局部探针仍保留为离线研究/测试参考，不会在一次 MCP 请求中无界运行。它只返回 `CameraFitResult@1`，不改 candidate、不创建 version。
   Codex 后续只应保存 `selected_camera.camera_hash` 与 `selected_camera.canonical_sha256`，组成 `CameraCalibrationRef@1`。不要把完整浮点 `CameraCalibration@1` 从一次 Codex turn 复制到下一次 turn；Runtime 会按 candidate/target 证据重新解析精确的完整相机，并在 hash 不匹配时 fail closed。
6. `silhouette_rig_hash`：对候选绑定的 hash-free `SilhouetteRig@1` draft 做 Runtime-owned canonicalization，返回唯一 `canonical_sha256`；Codex/Luna 不在 prompt、脚本或本地客户端重复实现 canonical JSON 哈希。该工具只读且不创建 candidate/Job/CAS。
7. `silhouette_fit_prepare`：读取已补齐 Runtime hash 的 `SilhouetteRig@1`/`SilhouetteFitIntent@1`，在最多 64 次 128×128 transient batch 评估、8 次迭代内执行 bounded grid 或实际 coordinate-descent 搜索；每轮只从当前最佳相机生成受限邻域，恶化即停止并保留 best-so-far。Transient batch 内部只编码 `silhouette` 与 `part-id`，并在 64×64 栅格上完成二值候选排序后确定性上采样到 128×128 合同；正式 512×512 fixed renderer 和九 AOV 输出不变。若候选有 V2 geometry evidence，Runtime 会读取候选绑定的 GeometryProgram CAS，按 `step_fraction` 生成少量受限 Rig 变体，并逐个经过固定 Worker 编译、严格 GLB 回读和选定相机渲染；返回 `geometry_evaluations`、`parameter_deltas` 和真实最优参数。该调用仍是只读试算，不写 CAS/candidate/version。随后将 mask 上采样到 512×512 计算统一指标，返回实际完成的 `iterations`、camera、参数、SDF/Chamfer 和严格 visible-view thresholds（IoU/F1 `>=0.90`、bbox/centroid `<=0.02`）；最终交付仍由 512×512 fixed renderer 复核。
8. `reference_compare_prepare`：使用选定 camera 生成九个固定 AOV，创建 candidate/reference/render-set 绑定的比较证据。
9. `boundary_error_get`：读取同一 candidate 的 RenderSet 和 SilhouetteTarget，返回最多 64 条最大的边界误差段。每段包含 reference/model 点、像素 delta、`inward/outward/aligned` 径向方向和可解析的 `part_id`；这是下一轮局部修改提示，不是自动修改授权。
10. `silhouette_part_error_get`：读取同一 candidate 的 `part-id` AOV 和 target 的非重叠 Part contour slices，返回每个 semantic Part 的 target/model envelope、像素数、质心偏移、宽高比、边界误差、可见性/状态，并按边界误差给出最多 16 个 `recommended_part_ids`。它是多 Part 归因表，不创建 candidate、Job 或 version；`missing_model_part`、`empty_target_part` 和 `unknown` 必须由 Luna 先修正输入或停止猜测。
11. `part_contour_fit_prepare`：只针对一个 semantic Part，结合 part-ID 和该 Part 对应的 target contour slice（没有 observed slice 时才回退整图诊断）以及 SDF/边界误差返回 bounded parameter adjustments；`width`、`height`、`scale` 使用该 Part 的局部投影包围盒，`offset_x/offset_y` 使用局部质心偏移，`depth/offset_z` 保持中性（单图不可观测）；这是 reviewable intent，不是 mesh mutation。
12. `silhouette_candidate_compare`：把 2–8 个候选绑定到同一 target，返回综合 loss、metrics、delta 和 winner/tie；它不创建版本。
13. `visual_review_submit`：Codex 提交观察到的区域问题和唯一修改意图。轮廓门未通过时，只允许一个 contour-bearing Part/Operator；不得提前进入材质或表面堆料。
14. `reference_mask_refine_prepare`：若用户在画布中修正轮廓，基于旧 target 创建新的不可变 target。旧 target 不覆盖，所有后续比较必须使用新 hash。
15. 对单一 Part 重新执行 Geometry → readback → camera/compare → boundary error → Part error table。最多五轮；没有改善就保留上一候选并记录 `QUALITY_TARGET_NOT_MET`。

Transient camera/candidate ranking uses a fixed lightweight loss: 35% normalized
SDF/Chamfer, 25% `(1 - IoU)`, 15% `(1 - Boundary F1)`, 10% bbox error, 10%
centroid error, and a reserved 5% regularization slot. Landmark and semantic
Part penalties are added only when typed annotations are present; when visible
landmarks are supplied, 10% of the bounded loss is assigned to normalized
landmark reprojection error and the remaining terms are scaled proportionally.
For the current robot vocabulary, known landmark IDs resolve to fixed Part-ID
anchors from the renderer's `part-id` AOV (for example crown→head-shell Top and
chest-center→chest-shell Center); unknown IDs retain the bounded global-silhouette
fallback. This mapping is product-owned and deliberately not a free-form Part
selector. Camera search, Rig/geometry trial ranking, candidate comparison and
the persisted reference comparison use the same transient anchor loss.
The solver never fabricates landmarks from a single mask. The automatic
mask-to-contour aid now traces directed grid boundary edges, selects the
largest deterministic outer loop and downsamples it; the 512×512 binary mask
remains comparison truth, while detached components require explicit user
annotation rather than being interleaved into one polygon.

## 轮廓目标合同

`SilhouetteTarget@1` 固定 `512×512` 和 `normalized_reference_image` 坐标。它绑定：

- `reference_id + reference_sha256`：用户授权参考的身份和字节；
- `mask_sha256`：Runtime 生成或用户轮廓栅格化的 PNG；
- `contour_points`：3–512 个有限归一化点；
- `parts`、`landmarks`：只允许 typed 的 observed/inferred/unknown 标记；
- `parts[].start_index/end_index`：把用户在画布确认的局部轮廓绑定到语义 Part。单 Part 闭合 polygon 可覆盖整个 contour；多 Part 只允许互不重叠的开放 contour chain，未知区域不自动补线；
- `canonical_sha256`：target 自身的内容哈希。

Runtime 拒绝未知字段、越界/非 finite 坐标、自交轮廓、跨项目 reference、错误 target hash 和错误 canonical hash。Viewer 只读 target；Viewer 不启动 Runtime、不写 SQLite/CAS，也不拥有第二套 mask 真值。

## Codex/Luna 的修改纪律

- 先解决 camera/framing，再改 geometry；不要用旋转模型掩盖 camera 错误。
- 优先看 `boundary_error_get` 的长段和 direction，选择一个能影响该段的稳定 Part；不要同时改 torso、shoulder、head 和 legs。
- 如果用户在 Viewer 选择 `chest-shell` 等 Part 后画出局部 polygon，Viewer 复制出的 `ForgeCADViewerContourDraft@2` 会携带同一批点的 `parts` annotation。Codex 应把该点集作为一个 observed target Part slice（`start_index=0`、`end_index=point_count-1`）提交给 `reference_mask_refine_prepare`，再把新 target hash 传给 `part_contour_fit_prepare`；不能继续使用没有 Part annotation 的旧 target。这个局部 target 只用于该 Part 的拟合诊断，整机 compare 仍使用原始全身 target。
- `outward` 表示当前模型边界相对 reference 径向外扩，通常需要收窄/后移；`inward` 表示当前模型缺失，通常需要外扩/前移。方向是径向近似，必须由下一次 comparison 验证，不能当作精确法线。
- 轮廓 gate 未通过时，`surface-detail`、`uv-pbr` 和 emissive 只可作为观察，不得宣称改善 likeness。
- 当前参考裁切了小腿/脚部时，脚部保持 `unknown/deferred`；不要为了增加 triangle 数而推断隐藏结构。
- 每轮只保留一个主变更，保存前一轮 candidate/target/render/comparison hash；失败候选不 confirm/export。

## 参考项目的采用边界

本路线借鉴了 `img2threejs/img2threejs` 的分阶段编排（参考 → 评估 → 参数化 spec → 固定构建 → 对照渲染 → 有界修正），以及 PyTorch3D silhouette fitting、OpenCV distance-transform/morphology、SAM/SAM2 的“mask proposal 与几何真值分离”原则。它们只用于方法研究，不会被 Runtime 远程调用，也不会把 Blender/Python/JS 插件或模型权重安装成产品依赖。

可选的 MobileSAM/SAM2、PyTorch3D 或 CUDA differentiable rasterizer 只能作为未来离线 proposal/研究 Worker；产品当前使用 Runtime 自有的 border flood-fill、polygon rasterizer、固定 renderer 和 typed readback。任何第三方 Library 进入 lockfile 或安装包前，必须固定 revision、许可证、SBOM、恶意输入/确定性 benchmark 和退出方案。

## 当前状态和停止条件

当前 `silhouette_fit_prepare` 已不仅返回 envelope proposal：对有 V2 evidence 的候选，Runtime 会从候选绑定的 GeometryProgram 生成受限参数变体，经过固定 Worker 编译、严格回读和 transient silhouette 渲染后选择 best-so-far，并在结果中记录 `geometry_evaluations` 与 `parameter_deltas`。这一过程仍是只读试算；历史真实图片 receipt 中的 proposal-only 数值不被改写。

2026-08-13 的实现增量把这一步从“只改输出节点”收紧为 DAG-aware materialization：Runtime 会从语义 `part_outputs[].input_node_ids` 递归追踪到真正的源几何节点；镜像/数组/part-output 只作为汇聚边，平移/整体缩放优先落到上游 transform，width/height/depth 落到 profile、panel、vent、joint 等源参数。相同源节点被多次引用时只应用一次，并由固定 Worker 重新生成 GLB；因此 Rig 变体的 `applied` 计数和几何评估不再因为镜像 sink 而出现假改善。该逻辑有 Runtime 回归覆盖，但尚未使真实用户图片达到严格视觉门。

同一增量还把自动轮廓从“按全局质心排序边界像素”改为有向栅格边界追踪：每个前景像素生成确定性边界边，按转向规则闭合 loop，取最大外环并均匀采样 256 点。分离组件不会污染主轮廓，单元测试用重建栅格 IoU `>0.94` 验证顺序和边界。若 target 带有经过 intake 审阅的 landmarks，camera/Rig/geometry 变体排序会临时加入 10% landmark NME；该字段不改变公共 QualityReport schema，也不写入第二套视觉真值。

review 工具漂移的恢复回归也已通过 transport：失败的 `human_visual_review_submit` 不会污染后续合法的 `visual_review_submit`/`quality_get`，但不会凭空生成 human approval。新的 12 参数 Rig 真实回合最终低于 attempt35 baseline（IoU `0.698465`、boundary F1 `0.281074`），因此只记为可复现的诊断证据，保留最佳基线，继续选择单一 silhouette-bearing Part 做局部修正。

真实同 cohort 回归 `docs/evidence/mcp010f/real-codex-cli-rig-fit-20260813.json` 已运行这条新路径：8 次受限评估包含 V2 GeometryProgram 的 Worker 编译/严格 GLB 回读/固定相机渲染，结果 IoU `0.700586`、Boundary F1 `0.278263`，最终比较 IoU `0.698465`、Boundary F1 `0.281074`，仍低于 `0.90` 严格轮廓门。带 15 个图像地标的后续同 cohort 回合见 `docs/evidence/mcp010f/real-codex-cli-landmark-aware-20260813.json`：target/camera/Rig/fit/compare/九 AOV/review/quality 传输完成，最终 IoU `0.685417`、Boundary F1 `0.272115`、landmark coverage `0.666667`、NME `0.134407`，同样低于门且不覆盖 attempt35 基线。两份 receipt 都证明轻量几何试算已接入真实 Codex 闭环，但不是 candidate mutation、质量通过或材质解锁证据。

本 slice 的 source contracts、Runtime target round-trip、bounded camera search、bounded Rig/SDF fit batch、MCP tool dispatch 和原有 F Viewer Gate 已通过；真实用户图片的 likeness 仍不能因此标记 PASS。保留的 detail/material-zoned baseline 约为 26 Parts/4704 triangles、silhouette IoU `0.7410`、boundary F1 `0.3288`，仍 `QUALITY_TARGET_NOT_MET`。attempt32 是保留的历史 primitive route；最新 source-built 真实 Codex attempt35 已通过 `CameraCalibrationRef@1` 完整跑通 11-turn detail target→camera/Rig/fit→compare→boundary→九 AOV→typed review/quality transport，同 cohort 为 `74749cc7…e1c3`，26 Parts/4704 triangles。最终指标为 silhouette IoU `0.741047`、boundary F1 `0.328765`、bbox edge error `0.007813`、centroid error `0.007878`、landmark coverage `0.733333`、landmark NME `0.134536`、region median IoU `0.869403`、critical region min `0.666289`，仍 `QUALITY_TARGET_NOT_MET`。silhouette-fit proposal 的 IoU `0.698340` 且 `status=no_improvement`，只是 read-only 参数建议，不代表新 candidate 已生成或确认。attempt33/34 的完整相机 hash 漂移失败保留为负向证据；当前 compact ref 已消除该 transport 阻断。2026-08-13 的新 primitive blockout 回合在同一授权参考上完成了 target→camera→Rig→fit→compare 运输，Rig 左右 Part 映射实际生效，compare 的 silhouette IoU `0.700430`、boundary F1 `0.242521`、bbox edge error `0.054688`、centroid error `0.013401`、landmark coverage `0.268025`、landmark NME `0.223858`、region median IoU `0.836134`、critical region min `0.569007`；其九 AOV 读取未由 Codex 稳定完成，因此该回合记为 `BLOCKED`，只作为性能/映射诊断，不覆盖 attempt35 的最佳基线，也不构成视觉通过。`HQ_360_PASS` 继续要求 front/back/left/right/rear-three-quarter 五张同一设计的全身参考；未补齐时固定为 `BLOCKED_REFERENCE_COVERAGE`。

下一步应继续复用 `docs/evidence/mcp010e/robot-reference-intake.json` 的 15 个 landmark/8 个 visible region，保持 attempt35 的 `reference/camera-ref/target/catalog` hash 绑定，只修一个由 `boundary_error_get` 指向的 silhouette-bearing Part，再重复 `silhouette_fit_prepare → geometry_prepare → reference_compare_prepare → boundary_error_get → render_pass_get → visual_review_submit → quality_get`。只有严格轮廓/比例门通过，才解锁 semantic-part fill、surface detail 和 UV/PBR；不要用 attempt35 的 read-only fit proposal、Viewer 草图、本地 fixture 或结构 Gate 替代真实图片 likeness 和人评。

当前还提供一个轻量 source-only 验证入口：`scripts/probe_mcp010f_part_correction.py` 读取经过 Viewer 绑定的单 Part contour draft，先调用 `part_contour_fit_prepare`，再对同一 Part 的少量宽/高/偏移变体执行 `geometry_prepare → reference_compare_prepare → silhouette_candidate_compare`。它适合 Luna 在大体量 GeometryProgram authoring 超时后继续做小步搜索；输出仍是 candidate-bound comparison 证据，不会确认、导出或替代真实 Codex 视觉门。2026-08-13 的 chest-shell 运行把 IoU 从 `0.741047` 提升至最高 `0.745895`，仍记录为 `QUALITY_TARGET_NOT_MET`。

Runtime 的 `silhouette_fit_prepare` 还会在 target 含 typed Part slice 时，于选定相机做一次 bounded `part-id` readback：匹配 `part_id` 的 width/height/scale/offset 参数使用局部 target/model envelope 与质心偏移；没有 Part annotation 时保持保守的全身 proposal。V2 候选会把这些参数物化成只读几何变体并输出 `parameter_deltas`/`geometry_evaluations`，但不自动修改 mesh、confirm 或 export。`silhouette_part_error_get` 现在把同一证据扩展为全 Part 误差表，支持 Luna 先按最大局部边界误差选择一个修正对象，再调用 `part_contour_fit_prepare`。局部优先回归、多 Part 合成回归和真实 chest-shell transport 均通过，但真实机器人仍未通过严格 visible-view gate。
