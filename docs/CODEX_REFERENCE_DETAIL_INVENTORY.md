# ForgeCAD 单图参考 Detail Inventory 与质量合同

> 2026-08-25 商业质量补充：单图 inventory 只能标 observed/inferred/unknown 并支持可见视图设计；它不能证明背面、深度、HQ360、High/Low/UV/Bake 或商业交付。进入建模前还需 `WeaponArtBrief`、原创概念变体和 ArtDecision；完整多视图与阶段链见 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`。

版本：`ForgeCADCodexReferenceInventory@1`  
性质：Codex 编排模板；不是 Runtime 合同、不是 Skill Bundle、不会写 SQLite/CAS，也不会把图片字节写入 GeometryProgram。

ADR-0026 后，本模板是未来 `ReferenceCanvas@1` / `DesignSpec@1` 的前身：它帮助 Codex 先把 Reference、coverage、observed/inferred/unknown、Primary/Secondary/Tertiary 阶段目标写清楚，再进入 geometry。它仍不是当前 Runtime durable Schema；当前 Agentic 只读 projection 也不能替代它的 CAS-bound producer。

## 1. 为什么需要这个模板

单张参考图最容易出现的失败不是 Operator 不够多，而是 Codex 在没有明确“看到了什么、没有看到什么、这一轮要修什么”的情况下直接堆节点。这个模板把一次单图建模拆成四个可回读的事实层：

1. `reference_intake`：参考图授权、hash、尺寸和视角范围；
2. `quality_contract`：当前视图允许宣称的质量门和硬停止条件；
3. `detail_inventory`：每个可见细节对应的 semantic Part、Operator、MaterialZone 和验证信号；
4. `correction_state`：最多五轮的单变量修改记录。

它吸收了 [img2threejs](https://github.com/img2threejs/img2threejs) 的 staged pass、detail inventory、per-region confidence 和 side-by-side review 思路，但把执行边界收敛到 ForgeCAD 的 typed GeometryProgram、AppearanceProgram、固定 renderer 和 Runtime readback。上游仓库及其 Skill 只作为研究参考，不进入 Runtime、Worker 或安装包。

本模板也承接用户提出的三条纪律：Reference First、Primary/Secondary/Tertiary 禁止跨级、每步 render/critic/local fix。Primary form 未通过时，`detail_inventory` 中的 tertiary detail 只能记录为 planned，不得进入 `GeometryProgram@2`。

## 2. 最小 intake 记录

Codex 在第一次写入前必须完成下列信息。原图路径、原始字节、prompt、URL、secret 和本机绝对路径不得进入该记录。

```json
{
  "schema_version": "ForgeCADCodexReferenceInventory@1",
  "reference": {
    "reference_sha256": "<ReferenceEvidence sha256>",
    "source": "user-authorized-reference",
    "width": 1254,
    "height": 1254,
    "view": "rear-three-quarter-or-three-quarter",
    "coverage": {
      "front": "visible_or_partial",
      "back": "unknown",
      "left": "partial",
      "right": "visible_or_partial",
      "feet": "cropped_or_unknown"
    }
  },
  "quality_contract": {
    "target": "PARTIAL_VISIBLE_VIEW_PASS",
    "visual_gate": "MCP010C_REFERENCE_COMPARISON",
    "strict_thresholds": {
      "silhouette_iou_min": 0.90,
      "boundary_f1_4px_min": 0.90,
      "bbox_edge_error_max": 0.02,
      "centroid_error_max": 0.02,
      "landmark_coverage_min": 0.80,
      "landmark_nme_max": 0.03,
      "region_median_iou_min": 0.85,
      "critical_region_min_iou_min": 0.85
    },
    "max_correction_rounds": 5,
    "metric_priority": [
      "boundary_f1_4px",
      "silhouette_iou",
      "bbox_edge_error",
      "centroid_error",
      "landmark_coverage_and_nme",
      "region_iou"
    ],
    "stop_on": [
      "QUALITY_TARGET_NOT_MET",
      "reference_binding_incomplete",
      "unknown_operator",
      "readback_integrity_error",
      "catalog_or_candidate_hash_mismatch"
    ],
    "hq_360_status": "BLOCKED_REFERENCE_COVERAGE"
  },
  "detail_inventory": []
}
```

## 3. Detail inventory 字段

每一行只描述一个可观察或明确推断的细节。`confidence` 不能掩盖 unknown：不可见区域必须写成 `unknown` 或 `inferred`，不能用高分替代缺失证据。

| 字段 | 规则 |
|---|---|
| `detail_id` | 稳定的小写 ID，例如 `head-visor-lower-edge` |
| `semantic_part_id` | 对应 GeometryProgram 的 Part；没有 Part 就不能进入最终候选 |
| `feature` | 3D 术语，例如 `tapered_shell`、`recessed_vent`、`joint_ring`、`cable_transition` |
| `evidence` | `observed`、`inferred` 或 `unknown`；可附 normalized bbox/landmark，但不附图片字节 |
| `criticality` | `identity`、`major` 或 `supporting`；identity 特征错误时本轮不能通过 |
| `topology_strategy` | `profile-loft`、`profile-extrude`、`panel`、`vent-array`、`joint-stack`、`revolve`、`tube-sweep` 等当前 active Operator |
| `operator_ids` | 只能填 live `OperatorCatalog@1` 中的 `active` 项；`boolean@1` 当前支持同一 Part 的 bounded union/difference/intersection |
| `material_zone_id` | 必须来自已验证的离线 AssetPack；未知材质停止，而不是自造名称 |
| `line_flow` | 连接该细节的视觉线条，例如 `helmet→visor→neck` |
| `confidence` | `0..1` 的 Codex 观察置信度；不是 Runtime 质量分数 |
| `review_signal` | 预期通过 `boundary`、`landmark`、`material-id`、`normal` 或 `uv-stretch` 哪个 AOV/指标验证 |
| `status` | `planned`、`tested`、`retained` 或 `rejected` |

### 机器人单图示例

对于当前白色硬表面机器人参考，首批 inventory 应覆盖：

- `head-shell`：头盔外壳、前额弧线、下缘；`observed`；`profile-loft`；
- `visor`：深色面罩带和下缘；`observed`；`panel`；`black-anodized-metal`；
- `neck`：颈部环和线缆过渡；`inferred`（由可见下颌/肩甲间隙推断，不能当作完整背面事实）；`joint-stack` + `tube-sweep`；
- `chest-shell`：胸甲主轮廓和中心线；`observed`；`panel`/`profile-loft`；`white-dielectric-clearcoat`；
- `chest-vent` 与 `chest-core`：通风槽、核心环、暖橙指示；`observed`；`vent-array`/`revolve`；
- `shoulder → upper-arm → elbow → forearm`：两侧可见装甲流线；`observed` 或 `inferred`，左右必须保持独立 lineage；
- `hip → thigh → knee → shin`：可见大腿和膝部层次；脚部若被裁切则保持 `unknown`。

## 4. 分阶段执行

每个阶段只解锁下一阶段所需的最小工作。`reference-canvas` 是 Codex 的
瞬时观察层（参考图、silhouette AOV、透明叠加、闪烁和热图），不是第二个
Runtime，也不保存图片字节或模型状态。只有前一层门通过，下一层才会进入
拟合计划；因此材质/特效不能掩盖轮廓错误：

1. `intake`：`capabilities_get → runtime_status → doctor → operator_catalog_get → skill_list → reference_import → reference_get`；冻结 hash、视角、unknown 区域；
2. `silhouette-blockout`：在 canvas 中先看 reference/silhouette overlay，只生成 head/chest/shoulder/arm/pelvis/thigh 的主要外轮廓；跑 geometry readback 和第一张 comparison sheet；
3. `landmark-structure`：只有轮廓门通过后，才用可见 landmark 调整比例和姿态；
4. `semantic-part-fill`：轮廓和结构门通过后，加入 panel、vent、joint ring、cable、emissive housing 等可追踪细节；每个细节回到 inventory；
5. `surface-detail`：保持 camera/reference 不变，只修改一个 Part 或一个 Operator stage；
6. `uv-pbr`：只有 form 门通过后，读取 `material_pack_get`，绑定同一 candidate 的 MaterialZone、UV/tangent/PBR 和九 AOV；材质不能修复错误 silhouette；
7. `final`：运行 `reference_compare_prepare → render_pass_get → visual_review_submit → quality_get`；只有达标且用户批准时才进入 confirm。

每阶段只产生一张临时 comparison sheet：reference、beauty、silhouette 和一个诊断 AOV。sheet 只帮助 Codex 判断，不是 QualityReport，也不得写回 Runtime。

`scripts/build_mcp010f_fit_plan.py` 会把这条顺序写入 `workflow`：它先返回
轮廓失败项；轮廓未通过时不会排入 landmark/form/material 修改，且明确返回
`surface_material_unlocked=false`。这只是 hash-bound 编排提示，最终质量仍以
Runtime 的 candidate-bound `QualityReport` 为准。

在第一次 `geometry_program_hash` 或 `geometry_prepare` 之前，先运行：

```text
python3 scripts/validate_mcp010f_reference_inventory.py \
  --inventory <inventory.json> \
  --operator-catalog <live-operator-catalog.json> \
  --assetpack-manifest <offline-material-pack-manifest.json> \
  --output <temporary-validation-receipt.json>
```

该检查器只读取 inventory、当前 catalog receipt 和离线 AssetPack manifest，不调用 MCP、不写 Runtime/CAS。它会拒绝原图字节/路径/prompt、未知或 inactive Operator、不存在于 AssetPack 的材质 ID、错误的 observed/inferred/unknown、MaterialZone/轮次状态以及任何 confirmation/360 解锁企图。只有 `status=PASS`、`operator_catalog=PASS_LIVE_ACTIVE_OPERATORS` 且 `assetpack_manifest=PASS_ASSETPACK_MATERIALS` 才能继续；输出的 inventory hash 是 Codex 编排证据，不是 Runtime 的 GeometryProgram canonical hash。

校验通过后，再用同一份输入生成流线计划：

```text
python3 scripts/build_mcp010f_surface_lineflow_plan.py \
  --inventory <inventory.json> \
  --operator-catalog <live-operator-catalog.json> \
  --assetpack-manifest <offline-material-pack-manifest.json> \
  --validation <temporary-validation-receipt.json> \
  --output <temporary-lineflow-plan.json>
```

只有 `READY_FOR_SINGLE_PART_FLOW_REVIEW` 才能进入下一轮 Codex 编排。计划
把 `line_flow`、active Operator 和 MaterialZone 绑定到最多五个单 Part 意图；
当前轮廓或结构门未通过时，所有表面材质动作保持锁定，`unknown`/裁切区域进入
deferred 列表。它不含几何参数、不调用 MCP、不写 Runtime/CAS，也不改变
`QualityReport`，只是让 Codex 更快选择一个可解释的轮廓/流线修改。

如果用户在 Viewer 的 `轮廓画布` 中画出了可见外轮廓，Codex 应先复制
`ForgeCADViewerContourDraft@2`，再运行：

```text
python3 scripts/validate_mcp010f_contour_draft.py \
  --draft <viewer-contour-draft.json> \
  --receipt <candidate-bound-comparison-receipt.json> \
  --output <temporary-contour-intent.json>
```

该验证器只接受闭合的 3–128 点归一化多边形，拒绝越界、过小、相邻重复和自交
轮廓，并要求 draft 的 candidate/reference/artifact/render/comparison hash 与
当前 comparison receipt 完全一致。`selected_part_id` 为 null 或 `all` 时，
结果只能是 `CONTOUR_DRAFT_BOUND_PART_SELECTION_REQUIRED`；Codex 不得自行猜
部件。选定已存在的 semantic Part 后，才会得到
`READY_FOR_SINGLE_PART_CONTOUR_EDIT`，随后仍需重新执行
`geometry_program_hash → geometry_prepare → artifact_readback_get →
reference_compare_prepare → quality_get`。该产物是临时编排 intent，不是
Runtime mask、GeometryProgram、QualityReport 或确认凭证。

## 5. 修正和停止规则

- 每轮最多一个 semantic Part、一个 Operator stage 或一个 MaterialZone；不得同时改变 camera、pose、geometry 和材质；
- 以 boundary F1 优先，其次 silhouette、bbox/centroid、landmark，最后才看 region IoU；
- 任意关键 identity feature 错误时，即使整体 IoU 上升也必须 `rejected`；
- `QUALITY_TARGET_NOT_MET` 是有效的质量证据，不是失败的 Runtime；保留 candidate 和 receipt，不能偷偷 confirm；
- 只有当前用户批准同一个 candidate hash，才允许 `candidate_confirm`；Codex 不得伪造人评 receipt；
- 单张三分之四参考最多只能产生 `PARTIAL_VISIBLE_VIEW_PASS`；补齐 front/back/left/right/rear-three-quarter 且每视图过门之前，`HQ_360_PASS` 固定为 `BLOCKED_REFERENCE_COVERAGE`。

## 6. 与 ForgeCAD 工具的对应

| 目标 | Runtime/MCP 真值 | Codex 编排产物 |
|---|---|---|
| 参考授权 | `reference_import` / `reference_get` / CAS hash | `reference_intake` |
| 可编辑几何 | `geometry_program_hash` / `geometry_prepare` / `artifact_readback_get` | `detail_inventory` + GeometryProgram draft |
| 材质与纹理 | `material_pack_get` / `appearance_prepare` / ArtifactReadback | MaterialZone 与 channel recipe |
| 视觉比较 | `reference_compare_prepare` / `quality_get` | comparison sheet + typed issue list |
| 局部修改 | `change_prepare` | 单变量 `correction_state` |
| 永久版本 | `candidate_confirm` / `restore_confirm` / `export_confirm` | 用户明确批准记录 |

该模板的职责是让 Codex 更快、更少重复地作出正确调用；它不能扩大当前 Operator、AssetPack 或视觉门的能力，也不能把单图推断冒充完整 360°事实。
