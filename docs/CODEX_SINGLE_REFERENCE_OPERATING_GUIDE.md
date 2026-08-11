# Codex 单张参考图操作手册

版本：2026-08-11
状态：当前 MCP010C source-focused 可执行路线；不是视觉质量或材质质量验收

本手册给 Codex/Luna 一条短而严格的单图调用路线。它适用于用户授权的一张 PNG/JPEG，尤其是机器人三分之四视图。它的结果是可编辑、可回读的结构化候选；当前 C 的 source/raw Gate 和一次真实机器人参考运行已能生成固定渲染、比较和评审证据，但首轮 primitive-only 候选的视觉阈值未通过。在真实 likeness、MCP010D–F 和独立真人门完成前，不得把结果称为像素相似、高质量 PBR 或完整 360°模型。

推荐在 Codex 调用前启用本地 `$forgecad-single-reference-quality-loop` Skill；它是编排层 Skill，不是 Runtime 可执行 Bundle，不会安装插件或改变 Runtime catalog。机器人三分之四参考先读取该 Skill 的 `references/three-quarter-robot-intake-template.md`，用稳定 Part/观察状态模板起草，再进入 V2 hash/readback 流程。

## 1. 先判断当前能力

每次新会话先按顺序读取：

1. `capabilities_get`
2. `runtime_status` 与 `doctor`
3. `operator_catalog_get`，并交叉读取 `forgecad://operators/catalog`
4. `skill_list`

只有 `status: active` 且同时出现在当前 catalog 的 Operator 才能进入 GeometryProgram。当前 MCP010B 的可执行几何 Operator 只有 `forgecad.geometry.primitive@2`，形状为 `box`、`cylinder`、`ellipsoid`、`sphere`。Skill Registry 中的 `hard-surface-detail`、`uv-pbr`、`reference-compare` 等仍是 `unavailable` 或 `partial`，不能仅凭 manifest 名称调用。

如果 Runtime 不是 `Ready`、catalog/resource hash 不一致、或 MCP/Runtime cohort 不一致，立即停止写入，返回实际的 typed error；不要从旧 receipt、文档或 Skill manifest 猜 hash。

若 Runtime 已 `Ready` 但 `build_cohort_match=false`，这是进程 cohort 漂移，不是可以忽略的提示：当前 MCP 不能写入 d9 Runtime。开发包切换时只执行一次 `python3 scripts/stop_forgecad_runtime.py --confirm`（通过 authenticated IPC 停止共享 Runtime，不删除 SQLite/CAS），然后完整退出并重新打开 Codex Desktop；不要反复停止已重新启动的 Runtime，也不要手工 kill 未确认的 MCP 子进程。重开后必须重新验证 MCP/Runtime cohort、Skill 数量和 catalog digest，才进入参考导入。

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

- `structural_pass`：合同、GLB、lineage、UV/tangent、当前受限 PBR factor 检查通过；
- `limited`：仅有 aspect-ratio 或其他 MVP 代理；
- `visual_pass`：需要当前 MCP010C 的固定透视渲染、silhouette/landmark/region 指标，并且必须用真实用户参考运行；source/raw Gate 本身不构成 likeness 通过。

`limited` 报告永远不能被描述为视觉接受；只有 Runtime 的 `hard_gate_passed=true`、reference binding 完整且用户明确批准同一 candidate hash 时，才允许 `candidate_confirm`。隔离 host probe 可以用固定测试 receipt 验证 confirm/export 协议，但那不是用户批准，也不能转化为视觉质量 PASS。`candidate_confirm` 创建不可变 Version；Codex 自己生成的字符串不能冒充密码学人类批准证明。

当前真实单图演练的停止点是：23 个 semantic Parts、9,964 triangles、1,592,884-byte GLB 的 strict readback 通过，但 limited aspect proxy 为 `0.5466 < 0.55`，所以候选保持未确认、未创建 version/export。这是正确的 fail-closed 行为。

MCP010C 首次真实机器人参考运行已经完成固定渲染/比较/typed review：1254×1254 用户授权 PNG 进入隔离 CAS，生成九个 512×512 AOV，使用 `mask-2` 的本地梯度 border flood-fill，silhouette IoU `0.5132`、boundary F1 `0.1441`、bbox edge error `0.1074`、centroid error `0.0169`，`quality_visual_status=QUALITY_TARGET_NOT_MET`。该次候选未确认、未创建 version/export，human receipt 为 `NOT_RUN`；脱敏证据见 `docs/evidence/mcp010c/real-reference-robot.json`。这说明调用路线真实可用，也明确说明当前 primitive blockout 尚未接近参考图。

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

真实运行 receipt 必须同时记录：参考 SHA、catalog/program/candidate/artifact/render/comparison hash、九个 AOV 顺序、Codex turn 数、ForgeCAD MCP 调用数、质量指标和 `QUALITY_TARGET_NOT_MET`（若未达门槛）。`render_pass_get` 返回的 PNG image block 不复制进 receipt；原图路径、图片字节、prompt、token、socket、用户绝对路径都不得写入证据。

探针默认使用 Codex 的 MCP 自动审批工作区，并把非 MCP 事件脱敏分类；只读 `.codex/.../SKILL.md` 查阅可以记录为 `codex_skill_read_only`，任何文件变更、网络命令或未知命令都必须使 receipt 保持 `BLOCKED`。若只测试 read-only 边界，可显式传 `--sandbox read-only`，但该模式不能完成需要写入 Candidate/RenderSet/Review 的完整 C 路线。

2026-08-11 的真实 CLI C receipt `docs/evidence/mcp010c/real-codex-cli-c-attempt13.json` 已完成六个短 turn、32 个 ForgeCAD MCP 调用、27 个 semantic Parts、4100 triangles 和九 AOV；它证明“Codex 能真实调用 C 工具链”，但同一用户机器人参考的 silhouette IoU `0.5132`、boundary F1 `0.1441` 仍低于视觉门槛，因此不能称为高质量模型，也不能自动确认或导出 candidate。

### 5.3 Viewer 只读比较路线

完成 `reference_compare_prepare` 后，Viewer 会通过 authenticated local IPC 读取同一 candidate 的 visual evidence，再按需读取参考图和一个 AOV；它不启动 Runtime、不直接打开 SQLite/CAS，也不写 candidate/version。当前源码支持：

```text
viewer_read_model
→ viewer_visual_evidence(candidate_id)
→ viewer_reference_bytes(reference_id, project_id)
→ viewer_render_pass(render_set_hash, pass)
```

界面提供 `split`、`overlay`、`flicker` 三种临时比较方式、九个固定 AOV 标签、camera-lock 状态、质量指标和 reference/render/hash 摘要。参考图和 PNG 仍由 Runtime 校验 project/reference/hash 后才返回；缺失或不一致时 Viewer 显示 unavailable，不从本机路径补读。当前 source implementation 已通过 Runtime/Viewer Rust 测试、Tauri check、TypeScript typecheck 和前端构建；Part/MaterialZone 隔离、explosion/heatmap 以及 packaged/current-cohort Viewer E2E 尚未通过，不能写成 Viewer PASS。

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

## 6. 局部修正循环

在 MCP010D/E/F 完成前，局部修正仍受当前 catalog 与材质能力限制：

1. 只针对明确的 Part intent 修改 typed 参数；
2. 重新生成完整 GeometryProgram@2 和 canonical hash；
3. 重新 `geometry_prepare` 和 readback；
4. 再跑 `quality_get`；
5. 保留每轮 candidate/artifact/hash，不覆盖历史对象。

不要把当前 `change_prepare` 或四 pass MVP renderer 扩写成通用 mesh delta、像素比较或高质量材质闭环。

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
2. **MCP010D**：受限 profile/loft/revolve/sweep、panel/vent/joint detail 和真实 semantic detail Skill；
3. **MCP010E**：离线 AssetPack、UV atlas、MikkTSpace tangent、纹理颜色空间、PBR provenance；
4. **MCP010F**：Viewer compare、AOV、Part/Material 选择、用户评分，以及五张全身参考后的 `HQ_360_PASS`。

在这些阶段分别通过前，不得通过安装任意 GitHub 插件、Blender/FreeCAD MCP、远程模型或付费材质 API 来绕过 ForgeCAD Runtime 的真值边界。
