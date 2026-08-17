# Codex 单张参考图操作手册

版本：2026-08-13
状态：当前 MCP010C/D/E source Gate 已完成、MCP010F Viewer source 与 packaged read-model/原生窗口结构路线可执行；不是视觉质量或材质质量验收。ADR-0026 要求后续单图流程升级为 ReferenceCanvas/DesignSpec/SemanticSceneGraph/stage gates；当前本手册仍只描述已存在工具链。

本手册给 Codex/Luna 一条短而严格的单图调用路线。它适用于用户授权的一张 PNG/JPEG，尤其是机器人三分之四视图。它的结果是可编辑、可回读的结构化候选；当前 C 的 source/raw Gate、D/E 的 Operator/AssetPack source Gate 和 F 的 Viewer source surface 已通过，一次真实机器人参考运行已生成固定渲染、比较和评审证据，但首轮 primitive-only 候选的视觉阈值未通过。在真实 likeness、packaged Viewer、独立真人门和完整 360°门完成前，不得把结果称为像素相似、高质量 PBR 或完整 360°模型。

ADR-0026 的“Codex 必须看得见”原则在本手册中的当前做法是：进入 silhouette-first 视觉回合后，先用同一 candidate/project 调用一次 `scene_observe_get`，由它返回 Scene Graph、object metadata、dimensions、geometry stats、current camera、selected objects 和 quality/evidence projection，再做 bounded camera/Rig action；不要让 Codex 用零散 project、artifact、quality 读取拼接连续设计判断。该 projection 仍是 Runtime-owned、只读、可重建现场，不是持久 DesignSpec 或视觉 PASS。

推荐在 Codex 调用前启用本地 `$forgecad-single-reference-quality-loop` Skill；它是编排层 Skill，不是 Runtime 可执行 Bundle，不会安装插件或改变 Runtime catalog。机器人三分之四参考先读取该 Skill 的 `references/three-quarter-robot-intake-template.md` 与仓库的 `docs/CODEX_REFERENCE_DETAIL_INVENTORY.md`，先生成 `ForgeCADCodexReferenceInventory@1`（授权 hash、视角覆盖、identity/major/supporting 细节、observed/inferred/unknown、单轮修正目标），再进入 V2 hash/readback 流程。该 inventory 只帮助 Codex 编排，不写 Runtime/CAS；当前用户图片的脱敏实例见 `docs/evidence/mcp010f/reference-detail-inventory-real-reference.json`。

## 1. 先判断当前能力

每次新会话先按顺序读取：

1. `capabilities_get`
2. `runtime_status` 与 `doctor`
3. `operator_catalog_get`，并交叉读取 `forgecad://operators/catalog`
4. `skill_list`

只有 `status: active` 且同时出现在当前 catalog 的 Operator 才能进入 GeometryProgram。当前 catalog 有 16 项：`primitive@2`、`profile-extrude@1`、`profile-loft@1`、`subd-cage@1`、`surface-patch@1`、`surface-shell@1`、`revolve@1`、`tube-sweep@1`、`transform@2`、`mirror@1`、`array@1`、`panel@1`、`vent-array@1`、`joint-stack@1`、`part-output@1` 和 `boolean@1`；`boolean@1` 允许同一 Part scope 的 bounded union/difference/intersection，通用 mesh Boolean 不开放。`hard-surface-detail@0.2.0` 只有在 Runtime 验证其 manifest、recipe、operator lock、benchmark、provenance 和 trust 后才返回 active。`uv-pbr@0.2.0` 与 `forgecad-hard-surface-robot@1.0.0` AssetPack 已有 source-focused 离线验证；Codex 仍必须从当前 `skill_list`/AssetPack manifest 读取实际 hash，不能仅凭计划或 GitHub 项目名称调用。

如果 Runtime 不是 `Ready`、catalog/resource hash 不一致、或 MCP/Runtime cohort 不一致，立即停止写入，返回实际的 typed error；不要从旧 receipt、文档或 Skill manifest 猜 hash。

若 Runtime 已 `Ready` 但 `build_cohort_match=false`，这是进程 cohort 漂移，不是可以忽略的提示：当前 MCP 不能写入旧 Runtime。开发包切换时只执行一次 `python3 scripts/stop_forgecad_runtime.py --confirm`（通过 authenticated IPC 停止共享 Runtime，不删除 SQLite/CAS），然后完整退出并重新打开 Codex Desktop；不要反复停止已重新启动的 Runtime，也不要手工 kill 未确认的 MCP 子进程。重开后必须重新验证 MCP/Runtime cohort、Skill 数量和 catalog digest，才进入参考导入。

## 2. 参考图进入 CAS

参考图必须由用户明确授权。优先使用 Codex 已取得的附件字节，通过 `reference_import` 的 `inline_content` 传入：

```json
{
  "project_id": "<project_id>",
  "source": {"kind": "inline_content", "mime": "image/png", "content_base64": "<bytes>"},
  "authorization": {
    "user_authorized": true,
    "declaration": "User authorized this reference for ForgeCAD modeling"
  },
  "expected_sha256": "<sha256>"
}
```

不要把本机路径、原始图片字节、prompt 或 secret 写入 receipt。`reference_import` 只建立 ReferenceEvidence，不会自动生成几何。若是 `codex_local_file`，必须满足产品配置的 authorized root 和 symlink 边界；失败时不能通过修改配置绕过。

导入后用 `reference_get` 复核 MIME、尺寸、CAS hash 和 `user_authorized=true`。同时写一份短 intake note：

- `observed`：图片直接可见的结构；
- `inferred`：根据对称、遮挡或设计常识推断的结构；
- `unknown`：背面、远侧、裁切脚部或被装甲遮挡的区域；
- `material_zone`：只作为 lineage 标签，不宣称已具备纹理。

单张三分之四图不能证明背面、脚部或完整 360°；未观察区域必须保持 `unknown` 或 `inferred`。

## 3. 构造 GeometryProgram@2

先使用真实 catalog 的 operator schema，再构造 hash-free draft。所有长度用米，角度用弧度，up-axis 为 `Y`，右手坐标；参数只能是 catalog 允许的封闭对象，不得塞入任意脚本、URL、路径或动态表达式。

建议先建立稳定的语义 Part：`head`、`visor`、`neck`、`chest`、`core`、`shoulder_left/right`、`arm_left/right`、`hand_left/right`、`pelvis`、`thigh_left/right`、`knee_left/right`、`shin_left/right`、`foot_left/right`。左右结构应共享同一套规划参数；当前 primitive catalog 没有 `mirror@2`，因此不要假装镜像 Operator 已激活。

每个 node 必须有唯一 `node_id` 和 `operator_id`。每个 node 只能被一个有序 `part_outputs[].input_node_ids` semantic-Part sink 消费一次；空数组、未知 node、重复 node 或跨 Part 重用都必须被拒绝。每个 sink 保留 `part_id`、`material_zone_id` 和 `solid`，以便 GLB 回读保留 part/source/material lineage。

典型草稿流程：

```text
catalog digest
  ↓
hash-free GeometryProgram@2 draft
  ↓ geometry_program_hash
canonical_sha256 + catalog digest
  ↓ 填回 draft
geometry_prepare(project_id, reference_id)
```

`geometry_program_hash` 是只读工具；它不创建 Candidate、Job、CAS、Event 或 Version。若它拒绝 unknown key、预填 hash、错误 catalog、非法单位或超预算，停止并修 draft，不要在 Codex 端自行实现另一套 hash。

## 4. 编译后的硬检查

`geometry_prepare` 返回 `GeometryPrepareResult@2` 和 Job。先 `job_get` 等到终态，再调用 `candidate_get` 与 `artifact_readback_get`。必须同时满足：

- `program_sha256` 等于 Runtime 返回的 canonical hash；
- artifact、candidate、readback、catalog digest 相互绑定；
- invalid index、non-finite、退化三角形、boundary/non-manifold、winding、UV、tangent、external URI、metadata mismatch 全部为 `0`；
- part/source/material coverage 为 `1.0`；
- GLB 是自包含的，所有 accessor、BIN、scene/node lineage 都通过 closed profile；
- Worker 退出、超时、崩溃或 accepted peak-RSS 超预算时，不能产生 CAS/Candidate 写入。

任何一项失败都停止，不要用 root extras、Skill receipt 或 `validator_status: passed` 代替真实 BIN/accessor readback。

## 5. 质量与确认策略

对候选调用 `quality_get`，并把报告区分为：

- `structural_pass`：合同、GLB、lineage、UV/tangent、嵌入 PBR 通道回读与当前受限 beauty sampling 检查通过；
- `limited`：仅有 aspect-ratio 或其他 MVP 代理；
- `visual_pass`：需要当前 MCP010C 的固定透视渲染、silhouette/landmark/region 指标，并且必须用真实用户参考运行；source/raw Gate 本身不构成 likeness 通过。

`limited` 报告永远不能被描述为视觉接受；只有 Runtime 的 `hard_gate_passed=true`、reference binding 完整且用户明确批准同一 candidate hash 时，才允许 `candidate_confirm`。隔离 host probe 可以用固定测试 receipt 验证 confirm/export 协议，但那不是用户批准，也不能转化为视觉质量 PASS。`candidate_confirm` 创建不可变 Version；Codex 自己生成的字符串不能冒充密码学人类批准证明。

当前真实单图演练的停止点是：23 个 semantic Parts、9,964 triangles、1,592,884-byte GLB 的 strict readback 通过，但 limited aspect proxy 为 `0.5466 < 0.55`，所以候选保持未确认、未创建 version/export。这是正确的 fail-closed 行为。

MCP010C 首次真实机器人参考运行已经完成固定渲染/比较/typed review：1254×1254 用户授权 PNG 进入隔离 CAS，生成九个 512×512 AOV，使用 `mask-2` 的本地梯度 border flood-fill；该固定相机历史 baseline 的 silhouette IoU `0.5132`、boundary F1 `0.1441`、bbox edge error `0.1074`、centroid error `0.0169` 保留在 `docs/evidence/mcp010c/real-reference-robot.json`。当前 Runtime 已在未提供显式 camera 时自动按参考/模型 silhouette 包围盒取景，并将视觉指标量化后再写入 CAS；最新有界 camera-search raw receipt `docs/evidence/mcp010c/real-reference-robot-camera-search.json` 达到 IoU `0.6623`、boundary F1 `0.2418`、bbox edge error `0.0566`、centroid error `0.0135`，但仍为 `QUALITY_TARGET_NOT_MET`。两次候选均未确认、未创建 version/export，human receipt 为 `NOT_RUN`；这说明调用路线和数据往返真实可用，也明确说明当前 primitive blockout 尚未达到参考 likeness。

随后按稳定 Part intake brief 运行的当前候选为 12 个 semantic Parts、13 个 source nodes、896 triangles、161104-byte GLB；strict ArtifactReadback@2 的 integrity counters 全为 0，Part/source/material coverage 均为 `1.0`，bounded aspect proxy 为 `0.65517`。该报告仍明确标记 `limited`，候选保持 `reviewable`、未确认、未创建 version/export；它是当前 primitive-only 路线的结构证据，不是 silhouette、PBR 或真人视觉通过。

用户重启后的 d9 Dev.app 又完成了一次隔离真实 Codex CLI 结构演练：同一授权 PNG 经过 `project_create → reference_import → reference_get → capabilities_get → operator_catalog_get → geometry_program_hash → geometry_prepare(reference_id) → job_get → candidate_get → artifact_readback_get → quality_get(reference_id)`，MCP/Runtime/Worker cohort 完全一致；`reference_get`、`candidate_get`、`quality_get.reference_compare.reference_id` 都与导入的 reference_id/项目一致，Job 为 succeeded/100%。输出 12 Parts、896 triangles、161104-byte 自包含 GLB，`chest-shell` 保留 `chest-shell → chest-panel` 有序来源。候选未确认，未触碰正式用户数据；该 receipt 仍只证明结构回读和参考绑定，不证明参考相似度、PBR、人工评分或 360°，证据见 `docs/evidence/mcp010b/dev-app-primitive-knowledge-codex-cli-v2-reference-bound-readbacks.json`。

### 5.2 MCP010C 真实 Codex CLI 固定渲染路线

要验证 Codex 是否真正能驱动 C 的视觉工具链，使用仓库内的 `scripts/probe_mcp010c_codex_cli.py`，不要手写另一套本地 JSON-RPC 或在 Codex 端计算 GeometryProgram hash。探针要求同一 cohort 的 `forgecad-mcp`、`forgecad-runtime` 和 sibling `forgecad-geometry-worker`，并把 Runtime 数据写入临时目录：

```text
project_create → reference_import → reference_get
→ capabilities_get → runtime_status → doctor → operator_catalog_get → skill_list
→ geometry_program_hash → geometry_prepare → job_get → candidate_get → artifact_readback_get
→ reference_compare_prepare → render_pass_get × 9
→ visual_review_submit → quality_get
```

在同一 candidate 需要稳定比较时，重复调用 `reference_compare_prepare`（保持
`project_id`、`candidate_id`、`reference_id`、`view_spec` 完全不变）最多五次，要求
RenderSet、comparison report 和九个 pass artifact hash 全部一致；任何漂移都停止
视觉迭代并保留 `DETERMINISM_FAILED` 证据，不要继续 confirm。

真实运行 receipt 必须同时记录：参考 SHA、catalog/program/candidate/artifact/render/comparison hash、九个 AOV 顺序、Codex turn 数、ForgeCAD MCP 调用数、质量指标和 `QUALITY_TARGET_NOT_MET`（若未达门槛）。`render_pass_get` 返回的 PNG image block 不复制进 receipt；原图路径、图片字节、prompt、token、socket、用户绝对路径都不得写入证据。

完成大轮廓后，Codex 可执行一次受限的表面线流 pass：用 `panel@1`、`vent-array@1`、`joint-stack@1` 和 `mirror@1` 组成 `visor-edge → chest-ridge → shoulder-trim → forearm-rail → hip-flank → knee-cap` 六类可追踪层。每层必须对应观察到的接缝、凹槽、灯带或材质断点，保持外轮廓不变，并单独绑定 Part/MaterialZone。当前真实参考的 26-Part/4704-triangle linework receipt 已在 `docs/evidence/mcp010f/surface-linework-real-reference.json`，其全局指标相对 20-Part/3944-triangle rounded-panel baseline 全部改善或持平，但仍是 `QUALITY_TARGET_NOT_MET`，不能确认或导出。胸甲浅斜切的单变量实验虽把 boundary F1 提升到 0.3338，却降低可见 landmark/region 覆盖，已拒绝并记录在 `docs/evidence/mcp010f/chest-wedge-linework-real-reference-rejected.json`；新增一个胸甲上缘 cap 也未改善全局 silhouette，见 `chest-top-cap-real-reference-not-promoted.json`。

组合曲面实验也没有被提升为默认：`curved-tapered` 同时把胸甲换成 `profile-loft`、把下肢外壳做渐缩，虽然真实 Worker、AssetPack、九个 AOV 和严格回读均通过，但 20 Parts/3320 triangles 的 silhouette IoU `0.7359`、boundary F1 `0.2786`、region median IoU `0.8553` 均不如当前 26-Part linework 基线，渲染仍是 blockout-like。它只作为负向证据保留在 `docs/evidence/mcp010f/experiment-curved-tapered-bb5c01.json`；Codex 默认仍使用 `surface-linework → armor-shell-zones`，避免把“曲面 Operator 已执行”误当成“外观更像参考”。

为了减少 Codex 每轮重复读取和截图，九个 AOV 完成后可在临时目录生成一张固定 review sheet：

```text
python3 scripts/make_mcp010f_comparison_sheet.py \
  --reference <authorized-reference.png> \
  --render-dir <candidate-render-dir> \
  --output <temporary-review-sheet.png> \
  --manifest <temporary-review-sheet.json>
```

它只使用 Python 标准库，把 `reference / beauty / silhouette / material-id` 排成固定 2×2 图，并在 manifest 中保存输入和输出 SHA-256；它不计算 IoU、不写 Runtime/CAS、不创建 candidate/version，也不替代 `QualityReport@2`。review sheet 可能含用户原图，必须留在临时目录，不能提交到仓库或 evidence；提交的只能是脱敏 hash-only manifest。每轮仍只改变一个可解释 Part、相机或材质区，并保留上一轮 sheet/metrics 作为对照。

在需要把指标转成下一轮 Codex 操作时，可在同一临时目录运行本地 fit-plan 辅助器：

```text
python3 scripts/build_mcp010f_fit_plan.py \
  --comparison <comparison-report.json> \
  --view-spec <reference-view-spec.json> \
  --operator-catalog <operator-catalog.json> \
  --output <temporary-fit-plan.json>
```

它只读取已经由 Runtime 产生的 `ReferenceComparisonReport@1`、`ReferenceViewSpec@1` 和可选的 `OperatorCatalog@1`；会验证 canonical hash，按 `reference-canvas → silhouette-blockout → landmark-structure → semantic-part-fill → surface-detail → uv-pbr → final` 的门控顺序输出最多五轮单一 Part/MaterialZone 意图。轮廓门（silhouette IoU、boundary F1、bbox、centroid）未通过时只输出 `silhouette` 动作；landmark/form/material 不会提前进入队列，且 `workflow.gates.surface_material_unlocked=false`。它还把已知 image-space region 映射到稳定的 `primary_part_ids`、只读 `supporting_part_ids`、`material_zone_hints` 和按 Part 分组的 `part_operator_hints`；每条动作只保留一个主 Part，未知 region 进入 `unmapped_region_ids`，不能被 Codex 当成可执行目标。它不会估计新的 landmark、写 GeometryProgram、调用 Operator、写 Runtime/CAS 或改变质量状态；`operator_hints` 只是当前活动目录中的候选提示，Codex 仍必须重新读取 live catalog 并自己构造 hash-bound draft。输出是临时编排证据，`QualityReport@2` 和 candidate-bound comparison 仍是唯一质量真值。

#### 5.2.1 视觉 intake 完整性门

真实 Codex CLI 的比较探针可以证明参考图已经进入隔离 CAS、几何/渲染/比较/评审调用完成，但如果 `ReferenceViewSpec@1` 没有提交任何归一化 landmark 或 visible region，`landmark_coverage=0` 与 `region_median_iou=0` 只说明“没有可比较的标注”，不能直接诊断为模型的 landmark/region 几何错误。当前 packaged contour receipt 就属于这个情况：`boundary_f1_4px=0.2418`、`silhouette_iou=0.6623`、`bbox_edge_error=0.0566` 真正暴露了轮廓问题，而 landmark/region 需要先补 intake。

可用标准库辅助器把这条停止规则固化为 hash-only 计划：

```text
python3 scripts/build_mcp010f_contour_correction.py \
  --receipt docs/evidence/mcp010f/real-codex-cli-current-20260812-packaged-contour.json \
  --output <temporary-contour-plan.json>
```

它不会读取原图、调用 MCP、写 Runtime/CAS 或创建质量结果；只验证 receipt 中的 candidate/reference/program/catalog/render/comparison hash 和指标，输出 `visual_intake=incomplete_visual_intake` 时，先要求 Codex 从用户授权图片提取 normalized visible landmarks、visible regions 以及 observed/inferred/unknown 标记。轮廓失败时只解锁一个 contour-bearing Part 或一个活动 Operator stage；landmark、semantic-part fill、surface detail、UV/PBR 保持锁定。不要把零 coverage 当成“脚部/肩部一定做错”，也不要猜测遮挡或裁切区域。

带 intake 的真实运行应优先用于决定“先改哪里”。当前用户机器人 PNG 的 15 个 landmark/8 个 region intake 已通过隔离 packaged Codex CLI 复测：区域 IoU 已明显高于全局轮廓门，因此不要先堆材质、发光或更多三角形；应优先选择一个影响头盔、肩甲、胸甲或四肢外缘的 contour-bearing Part，保持 camera、reference、catalog 和 candidate 绑定不变，再重新跑 readback→comparison→review。相反，MCP010D/E 的 detail route 可作为第二阶段验证：同级 Worker 的 profile/panel/vent/joint/sweep 与离线 AssetPack 复测得到 26 Parts/4704 triangles、7 个 MaterialZone 和 `silhouette_iou=0.7410`，但仍未达到整体门；它证明“细节 Operator 有价值”，不证明“材质能修复轮廓”。

真实 Codex 要调用这条路线时，可在隔离诊断探针中显式选择 `--geometry-route detail`，并保留 `--geometry-variant surface-linework --material-variant armor-shell-zones`；探针会先读取 live catalog，再把受限 Operator 组合交给 `geometry_program_hash → geometry_prepare`。这只是 Codex 编排入口，appearance、human review、confirm/export 仍需分别调用并通过各自硬门；默认不应把 detail route 作为 Runtime 隐式默认。

首次 Geometry 写入前，Codex 还应运行 `scripts/validate_mcp010f_reference_inventory.py` 校验 `ForgeCADCodexReferenceInventory@1`，同时传入当前 catalog 和离线 AssetPack manifest。它要求所有 Operator 都是 active、所有材质 ID 都存在于 AssetPack，并拒绝原图路径/字节、错误的 observed/inferred/unknown、轮次不一致和任何 confirmation/360 解锁；只有 `status=PASS`、`operator_catalog=PASS_LIVE_ACTIVE_OPERATORS`、`assetpack_manifest=PASS_ASSETPACK_MATERIALS` 才进入 `geometry_program_hash`。该校验产生的 hash 只属于 Codex 编排证据，不能代替 Runtime canonical hash。

校验通过后，再运行 `scripts/build_mcp010f_surface_lineflow_plan.py`：

```text
python3 scripts/build_mcp010f_surface_lineflow_plan.py \
  --inventory <inventory.json> \
  --operator-catalog <live-operator-catalog.json> \
  --assetpack-manifest <offline-material-pack-manifest.json> \
  --validation <inventory-validation.json> \
  --output <temporary-lineflow-plan.json>
```

它必须返回 `READY_FOR_SINGLE_PART_FLOW_REVIEW`，并把每个可见细节的
`line_flow`、活动 Operator 和 MaterialZone 绑定成最多五个单 Part 意图。
当前轮廓/结构/form 任一失败时，材质、UV/PBR 和隐藏区域保持锁定；计划不含
几何参数、不调用 MCP、不写 Runtime/CAS。当前用户图的实跑结果记录在
`docs/evidence/mcp010f/surface-lineflow-plan-20260812.json`，其五个首选动作
均处于 `silhouette-blockout`，下肢/脚部因参考裁切保持 deferred。

探针默认使用 Codex 的 MCP 自动审批工作区，并把非 MCP 事件脱敏分类；只读 `.codex/.../SKILL.md` 查阅可以记录为 `codex_skill_read_only`，任何文件变更、网络命令或未知命令都必须使 receipt 保持 `BLOCKED`。若只测试 read-only 边界，可显式传 `--sandbox read-only`，但该模式不能完成需要写入 Candidate/RenderSet/Review 的完整 C 路线。

2026-08-11 的真实 CLI C receipt `docs/evidence/mcp010c/real-codex-cli-c-attempt13.json` 已完成六个短 turn、32 个 ForgeCAD MCP 调用、27 个 semantic Parts、4100 triangles 和九 AOV；它保留了 camera auto-fit/指标 CAS 修复前的 silhouette IoU `0.5132`、boundary F1 `0.1441`。最新 raw source receipt 已验证修复后的 IoU `0.6623` 与 review/quality 读回，但尚未重跑完整 CLI turn，因此两者都不能称为高质量模型，也不能自动确认或导出 candidate。

### 5.3 Viewer 只读比较路线

完成 `reference_compare_prepare` 后，Viewer 会通过 authenticated local IPC 读取同一 candidate 的 visual evidence，再按需读取参考图和一个 AOV；它不启动 Runtime、不直接打开 SQLite/CAS，也不写 candidate/version。当前源码支持：

```text
viewer_read_model
→ viewer_visual_evidence(candidate_id)
→ viewer_reference_bytes(reference_id, project_id)
→ viewer_render_pass(render_set_hash, pass)
```

界面提供 `split`、`overlay`、`flicker` 三种临时比较方式、九个固定 AOV 标签、camera-lock 状态、Part/MaterialZone 筛选、爆炸图和差异热图辅助，以及质量指标和 reference/render/hash 摘要。AOV 使用标准 tablist/tabpanel 语义，支持 Arrow 键循环切换、Home/End 跳到首尾并保持 roving focus；这只改善检查效率，不改变 Runtime 真值。参考图和 PNG 仍由 Runtime 校验 project/reference/hash 后才返回；缺失或不一致时 Viewer 显示 unavailable，不从本机路径补读。当前 source implementation 已通过 Runtime/Viewer Rust 测试、Tauri check、TypeScript typecheck 和前端构建；同 cohort Dev.app CLI read-model 与原生 `ForgeCAD Runtime Viewer` 窗口结构探针也已通过（窗口 1296×803）；这些控件只修改 ephemeral UI state，热图不写 QualityReport。原生窗口探针不等于 DOM 控件交互或 Accessibility E2E，后两者仍未运行，不能写成完整 Viewer/视觉 PASS。

#### 5.3.1 轮廓画布与下一轮修正

当目标是先把图片外轮廓做准时，Codex 在 Viewer 中点 `轮廓画布`，或手动选择 `silhouette` AOV + `overlay`。这只是把 Runtime 已生成的 silhouette PNG 与授权参考图叠加在固定 camera 上；它不会改变 camera、candidate、GeometryProgram 或 QualityReport。Viewer 还可以在浏览器内存生成一层橙色的 `REFERENCE CONTOUR AID`，用于帮助人眼寻找参考图边缘，并允许用户在画布上描绘最多 128 个归一化轮廓点。现在复制的是 `ForgeCADViewerContourDraft@2`：除点集外还绑定 project、candidate、reference、artifact、RenderSet、comparison hash、silhouette pass、当前 Part/MaterialZone 选择；如果没有 candidate-bound 证据，按钮保持不可用。点集仍只存在 Viewer 内存，可清除或复制给 Codex，明确标记 `transient_only`、`runtime_write:false`，不写 SQLite/CAS/evidence，也不替代 Runtime 的 reference mask。该辅助层采用与 Runtime `mask-2` 同源的有界边界连通 flood-fill 和局部颜色差阈值，能够穿过平滑棚拍渐变并在高对比主体边缘停止；它仍不是分割模型、不是比较指标。Codex 不得用它替代 `ReferenceComparisonReport@1` 的 IoU/F1，而应先通过 `scripts/validate_mcp010f_contour_draft.py` 生成 hash-bound、单 Part 的 `ForgeCADContourCorrectionIntent@1`。

验证命令：

```text
python3 scripts/validate_mcp010f_contour_draft.py \
  --draft <ForgeCADViewerContourDraft@2.json> \
  --receipt <candidate-bound-comparison-receipt.json> \
  --output <temporary-contour-intent.json>
```

只有 `READY_FOR_SINGLE_PART_CONTOUR_EDIT` 才能进入 `change_prepare`；没有选择 Part 时返回 `CONTOUR_DRAFT_BOUND_PART_SELECTION_REQUIRED`，仍属于观察输入。验证器拒绝自交/过小/越界点、旧 candidate/reference/render/comparison hash 和任何 Runtime 写字段；它不读原图、不调用 MCP、不创建质量结果。

轮廓画布下方的 `CODEX NEXT ACTION` 是只读、hash-bound 的编排提示。如果当前仍处于 `reference-canvas` 或 comparison metrics 尚未由 Runtime 绑定到 candidate，队列保持为空；Codex 应先完成 `reference_compare_prepare → render_pass_get → visual_review_submit → quality_get`，不能凭空把模型判为轮廓失败。metrics 已存在后，Codex 只取队列中的第一条意图：

1. `fit-silhouette`：只修改一个已存在的主要 Part，优先处理 `boundary_f1_4px` 和 silhouette；保持 project/candidate/reference/camera/program/catalog hash 不变，随后重新执行 geometry readback → comparison → quality。
2. `fit-landmarks` 或 `fit-regions`：只有前一层轮廓门已通过才允许进入；仍然一次只改一个 Part 或一个 Operator stage。
3. `review-surface`：表示当前几何门已过，但不能把它当成视觉通过；仍需读取固定 AOV、提交 typed review，并等待用户的人评批准。

Viewer 不会替 Codex 构造 GeometryProgram、调用写工具或自动确认。视觉解锁只信任同一 candidate 的 `viewer_visual_evidence.quality_report`（`QualityReport@2.visual_status` 与 `hard_gate_passed`）；candidate 上独立的 `quality_hard_gate_passed` 或通用 quality 投影只能表示结构/缓存状态，不能清空视觉修正队列或提前解锁材质。若 `viewer_visual_evidence` 不可用，Viewer 必须显示 `not-run`，不得回退到通用 quality 或 render hash。若候选 hash、参考 hash、camera lock 或 catalog digest 发生变化，应丢弃该队列并重新生成 comparison；若队列为空或质量状态为 `QUALITY_TARGET_NOT_MET`，保留候选和证据，不得 confirm/export。这样“画布先定轮廓、再填充结构、最后做材质”的顺序可被 Codex 重复执行，同时不会把临时视觉辅助误报成产品质量真值。

兼容材质基线也已实际运行：V1 `AppearanceProgram@1` 生成 15 Parts、548 triangles、三种 material zone 和四个 256x256 fixed-render metadata pass，ArtifactReadback@1 的 UV/tangent/validator 均通过；但 limited aspect proxy 为 `0.4662 < 0.55`，Runtime 将候选拒绝。它证明的是材质 plumbing 和回读，不是 V2 PBR、纹理质量或视觉相似度；证据见 [`real-reference-v1-appearance-baseline.json`](evidence/mcp010b/real-reference-v1-appearance-baseline.json)。

### 5.1 现有 MVP 的完整 appearance/export 路线

如果目标是验证 Codex 与现有 MVP 的完整事务链，而不是宣称视觉质量，可按以下顺序运行一次隔离回归：

```text
project_create
→ reference_import
→ geometry_prepare
→ artifact_readback_get
→ appearance_prepare
→ artifact_readback_get
→ quality_get
→ candidate_confirm（仅同一 candidate hash + 明确批准）
→ version_list
→ export_prepare
→ export_confirm
→ version_list
```

V1 兼容 probe 必须先取得真实 `project_id`，再把它写入 GeometryProgram 并重新计算 canonical hash；写死旧 project ID 会被当前 B 的 project-binding 硬门拒绝。2026-08-10 最新 `primitive-blockout` Dev.app 的真实 Codex CLI 隔离回归已完成上述 12 调用：15 Parts、548 triangles、geometry/appearance validator 均通过，并完成 version 与 CAS-backed MVP GLB export。该 receipt 只证明 Runtime/MCP/Worker/approval/export plumbing；程序是确定性测试输入，图片只用于授权参考 admission，不是图像条件生成或视觉相似度证据。

当前 D/E route 也已用用户授权的机器人 PNG 在隔离 Runtime 和同 cohort Dev.app 中跑通：`operator_catalog_get → geometry_program_hash → geometry_prepare` 使用 MCP010D 的 profile/panel/vent/joint/tube-sweep/mirror/array 组合，先得到历史 20 个 semantic Parts/3944 triangles，再通过六类受限表面线流层得到当前 26 Parts/4704 triangles。随后 `appearance_prepare` 绑定 `forgecad-hard-surface-robot@1.0.0`、7 个嵌入纹理、UV/tangent 与九个固定 AOV，`render_pass_get` 返回真实 PNG image block；当前 material-zoned refinement 进一步把同一几何绑定到 8 个显式 MaterialZone（white clearcoat、dark painted、black anodized、brushed steel、engineering plastic、joint rubber、micro-scratch、warm emissive）。针对参考图中“白色外壳、深色内构”的观察，`armor-shell-zones` 配方保留可见上臂/前臂外壳为白色，只把凹槽、关节、线缆和发光通道分配到深色/钢/橡胶/琥珀区；它不改变 geometry 或 comparison 指标，是当前材质 recipe 候选。固定 renderer 现在实际采样 embedded baseColor、normal、metallic-roughness、AO、emissive，并应用固定 key/fill/rim GGX-like lighting、clearcoat 和 emissive strength；这一实现有 geometry-worker 单测覆盖，但仍不是 PBR likeness 证据。当前 linework/material-zoned `region-mask-iou-v2` 指标为 silhouette IoU `0.7410`、boundary F1 `0.3288`、bbox edge error `0.0078`、centroid error `0.0079`、landmark coverage `0.7333`、region median IoU `0.8694`、critical region min `0.6663`，整体仍为 `QUALITY_TARGET_NOT_MET`。历史圆角面板证据为 `docs/evidence/mcp010f/rounded-panel-real-reference.json`，线流证据为 `docs/evidence/mcp010f/surface-linework-real-reference.json`，当前 8-zone 材质证据为 `docs/evidence/mcp010f/surface-zones-real-reference.json`，外壳优先配方证据为 `docs/evidence/mcp010f/armor-shell-zones-real-reference.json`，旧 3368-triangle receipt 仍保留用于历史对照。这证明细节几何/离线材质/比较链路可用，同时明确当前模型还不像参考，不是 likeness、PBR 视觉保真、真人批准或 360°完成。

材质口径补充：完整 `surface-zones` 配方使用 AssetPack 的 8 个材质族；当前正式候选 `armor-shell-zones` 实际绑定其中 7 个 used zones，不能把“8 个材质族”误报成当前候选的 8 个绑定区。材质/流线调用的 Codex 编排说明已单独放入本机 `forgecad-material-surface-design` Skill：先读 AssetPack 与 MaterialZone，再按 helmet→visor→neck、shoulder→arm→joint、chest→vent→core、hip→thigh→knee 的可见线条顺序添加受限 Operator，最后用 `material-id`/`normal`/`uv-stretch` AOV 复核。它只负责编排，不是 Runtime Skill Bundle；缺少 live AssetPack 时不得伪造材质能力。

本轮几何调优纪律也已留下脱敏 receipt：椭球肩甲、profile-loft 肩甲、渐缩胸甲、加宽头盔/面罩、统一三分之四 yaw、“缩小头盔并拉长四肢”、sleek-armor 和 visible-thigh 比例假设，都因 boundary、全局轮廓、region 或 landmark 权衡退化而拒绝；这些旧实验以 20 Parts/3368 triangles 作为历史基线。随后 panel@1 增加固定四段圆角 profile，得到历史 20 Parts/3944-triangle rounded-panel receipt；在其上增加六类表面线流层，当前 source baseline 为 26 Parts/4704 triangles。两者质量门均未通过；region-mask-iou-v2 的区域真值修复、圆角面板和 linework 结果分别见 `docs/evidence/mcp010f/fit-plan-real-reference-region-fix.json`、`docs/evidence/mcp010f/rounded-panel-real-reference.json` 和 `docs/evidence/mcp010f/surface-linework-real-reference.json`。这说明 Codex 应优先改变一个可解释 Part 并回读比较，不应以增加节点、旋转、拉长比例或三角形替代参考分区校准。

第二轮真实单变量实验继续遵循同一纪律：`head-wedge`、`tapered-lower`、圆角化 `asymmetric-armor` 和 `chest-profile` 均被拒绝；`asymmetric-stance` 通过拆分肩臂左右 Part 让右肩上提、左臂下落，取得当前局部最佳 silhouette IoU `0.7490`、region median IoU `0.8684`，但 boundary F1 仅 `0.2381`、bbox edge error `0.0195`、landmark coverage `0.7333`，仍为 `QUALITY_TARGET_NOT_MET`。`chest-profile` 只替换 chest-shell 一个 Operator，却把 boundary F1 降到 `0.3064`、silhouette IoU 降到 `0.7044`，因此也保留为拒绝证据。随后对整机只施加 `-0.1 rad` 的轻微 yaw，silhouette IoU 降到 `0.7251`、boundary F1 降到 `0.2786`、landmark coverage 降到 `0.6667`，同样拒绝；证据见 `docs/evidence/mcp010f/pose-yaw-mild-real-reference-rejected.json`。对详细 Part sinks 施加完整 `-0.30 rad` 三分之四 yaw 时，silhouette IoU 进一步降至 `0.6578`、boundary F1 `0.2821`、landmark coverage `0.6000`，也予以拒绝；证据见 `docs/evidence/mcp010f/three-quarter-detail-real-reference-rejected.json`。只旋转 head-shell `+0.12 rad` 时 IoU 仅升到 `0.7413`，但更高优先级的 boundary F1 由 `0.3288` 降至 `0.3234`，也予以拒绝；证据见 `docs/evidence/mcp010f/head-turn-mild-real-reference-rejected.json`。仅拆分左右肩甲外轮廓则把 landmark NME 从 `0.1345` 降到 `0.0700`、region median IoU 提升到 `0.8764`，但 boundary F1 仍轻微降至 `0.3275`，所以保留为局部候选而不晋级；更小幅度的 shoulder-contour-tiny 也未恢复 boundary F1，故拒绝。证据见 `docs/evidence/mcp010f/shoulder-contour-mild-real-reference-not-promoted.json` 与 `shoulder-contour-tiny-real-reference-rejected.json`。材质方面，保留当前 linework geometry，并新增 8-zone AssetPack mapping；它不改变 comparison 指标，只改善 `material-id`/beauty 的表面可读性，证据见 `docs/evidence/mcp010f/surface-zones-real-reference.json`。这些实验都不替换当前 linework/material-zoned 基线，详见 `docs/evidence/mcp010f/geometry-experiment-round-2.json`。下一轮仍应针对真实轮廓边界的单一 Part 原因继续验证，不得因为局部 IoU 上升或材质区增加就确认或导出。

本次 intake 的 observed/inferred/unknown 清单与归一化标注冻结在 `docs/evidence/mcp010e/robot-reference-intake.json`；它只保存参考 SHA、结构化坐标和覆盖边界，不保存原图字节、本机路径或隐藏侧猜测。

## 6. 局部修正循环

在 MCP010F packaged/human 子门完成前，局部修正仍受当前 catalog 与材质能力限制：

1. 只针对明确的 Part intent 修改 typed 参数；
2. 重新生成完整 GeometryProgram@2 和 canonical hash；
3. 重新 `geometry_prepare` 和 readback；
4. 再跑 `quality_get`；
5. 保留每轮 candidate/artifact/hash，不覆盖历史对象。

### 6.0 轮廓优先的画布门

先在 Codex 的临时 canvas 中把参考图与 `silhouette` AOV 做 split、overlay
或 flicker，再决定是否修改 Runtime。canvas 只负责观察，不是第二套模型真值；
它不写 SQLite/CAS、不保存原图字节，也不能生成新的质量数字。拟合计划的解锁顺序固定为：

`reference-canvas → silhouette-blockout → landmark-structure → semantic-part-fill → surface-detail → uv-pbr → final`

当前轮廓门由 `silhouette_iou >= 0.90`、`boundary_f1_4px >= 0.90`、
`bbox_edge_error <= 0.02`、`centroid_error <= 0.02` 组成；完整可见视图门
还要求 landmark coverage `>= 0.80`、NME `<= 0.03`、region median IoU
`>= 0.85`、critical region IoU `>= 0.85`。轮廓任一项失败时，计划只输出 `silhouette` 修正，
不会把 landmark、form 或 MaterialZone 问题提前当作可执行动作。只有轮廓、
结构和 form 门都通过，`surface_material_unlocked` 才为 true。这样可以避免
用纹理、粗糙度、发光或更多面数掩盖错误的外轮廓。

当前 D Operator 仍是有界 typed recompile，不是通用 mesh delta；E 已提供 source-focused 离线 AssetPack、UV atlas、MikkTSpace 和 embedded PBR，但它们不等于视觉 likeness。F Viewer 仍只读，热图只是辅助，不写 Runtime 真值；不要把这些 source Gate 扩写成用户图片高质量闭环。

### 6.1 质量驱动的迭代纪律

当前 primitive-only catalog 下，Codex 不应把更多节点、更多 triangles 或更多 material zone 当成质量目标。真实单图实验已经得到：23-Part V2 blockout 的 limited aspect proxy 为 `0.5466`，增加到 51-Part 后反而降为 `0.4604`；V1 材质基线加入三种材质区后仍只有 `0.4662`。因此每轮只允许一个可解释的 Part/比例变化，并在 `quality_get` 后比较上一轮的 aspect 值、readback counters 和候选 hash：

1. 若结构 readback 失败，先修合同/拓扑，不做视觉判断；
2. 若结构通过但 limited aspect 下降，保留证据并回退该轮，不继续堆细节；
3. 若只验证材质 plumbing，使用 V1 兼容路线并明确 `AppearanceProgram@1`，不能把它附着到 V2 候选上；
4. 当前 C 已提供 render/compare 工具，可把 silhouette、landmark、region 作为 typed 修正目标；但只有真实用户参考、独立视觉门和相应 receipt 通过后，才能把它们写成 likeness 或 material-fidelity 结论；
5. `candidate.quality_hard_gate_passed` 只代表 Runtime 已完成的结构硬检查，最终是否可确认必须以同一 candidate 的 `quality_get.hard_gate_passed` 和用户批准为准。

本规则把“更复杂”与“更接近参考”分开，避免 Codex 在当前能力不足时制造看似精细但更不相似的模型。

## 7. 失败映射

| 现象 | Codex 动作 |
|---|---|
| Runtime `Starting/Degraded/Busy` | 只读等待或报告 retryable；不重复写入、不猜 endpoint |
| catalog hash mismatch | 丢弃 draft，重新读取 catalog |
| hash/program/project mismatch | 停止，重新绑定 project 和 canonical hash |
| readback 任一 integrity counter 非零 | 丢弃候选；不要确认或导出 |
| Skill `unavailable/partial` | 不调用缺失 Operator；降级为当前 active primitive 或停止 |
| quality limited / hard gate failed | 保留候选供比较，禁止 confirm/export |
| 参考图区域不可见 | 标记 `unknown/inferred`，不能把单图当 360°证据 |

## 8. 后续阶段交接

本手册不会激活未领取的能力。真正提升图片相似度的顺序是：

1. **MCP010C**：固定 perspective/z-buffer renderer、九 AOV、camera calibration、silhouette/landmark/region comparison、typed visual review；
2. **MCP010D**：已通过 source-focused 的 profile/loft/revolve/sweep、panel/vent/joint detail、transform/mirror/array 和真实 semantic detail Skill；同 cohort packaged D raw structural probe 已通过；Manifold boolean 与视觉门仍 NOT_RUN；
3. **MCP010E**：离线 AssetPack、UV atlas、MikkTSpace tangent、纹理颜色空间、PBR provenance；
4. **MCP010F**：Viewer compare、AOV、Part/Material 选择、爆炸图/热图辅助、用户评分，以及五张全身参考后的 `HQ_360_PASS`；当前 source slice 已通过，packaged/human/360 子门仍未运行。

在这些阶段分别通过前，不得通过安装任意 GitHub 插件、Blender/FreeCAD MCP、远程模型或付费材质 API 来绕过 ForgeCAD Runtime 的真值边界。
