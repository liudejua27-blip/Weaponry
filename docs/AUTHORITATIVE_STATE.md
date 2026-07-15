# ForgeCAD 唯一权威状态设计

版本：2026-07-15
状态：S001–S008、D001–D005、F001–F006、T001–T004、G801–G826、R001–R004、M101–M107、C101–C104 与 Q002–Q003 的当前原子任务已按各自边界完成；F001/T002/T003 已在本机 Chrome 验证启动、澄清、预览不写盘、Agent 提交、Snapshot/导出一致、重启和单 WebGL canvas。当前 Agent 路径的恢复、选择、预览、质量、回退/前进和 GLB 导出已读取同一 Snapshot；R003 的爆炸概念图和 R004 的 PNG/manifest 图包均是条件式只读派生物；Q002 将质量写入收紧为 ETag + Idempotency-Key 重放，任务级 CAS 竞争已有 smoke，广泛多客户端压力矩阵仍未完成。

2026-07-15 增量真值：G819 已将 ShapeProgram 操作接受/拒绝收敛到单一 manifest；Q003 已将质量与导出的 triangle、bounds、hash、operation/output/material 证据收敛到当次 `GeometryCompileReadback@1`。G825 再把每个有序 ShapeProgram node 的输入/结果/参数/provenance hash、runtime/kernel version 和 CSG surface/material 来源收敛到同次 GLB 回读的不可变 `feature_history`；该历史是资产内容的派生证据，不是第二个 Project/Version/FeatureGraph 真值。旧估算报告和缺少 Feature History 的新编译均不是当前资产质量真值。

D005 增量真值：Style Token 和语义比例 Recipe 是版本化只读目录，不保存当前参数值。可用选项每次从当前活动 `AgentAssetVersion` 的 AssemblyGraph、G808 binding 和同一 ShapeProgram 的 G826 GLB readback 重新解析；当前比例值来自 AssemblyGraph transform。点击配方仍创建普通 ChangeSet preview，只有 confirm 创建不可变子版本并更新 Snapshot。配方选择本身不写 localStorage/Snapshot，也不能扩大路径、范围、步长或 G819 operation manifest。

## 1. 历史问题与当前边界

在 S001–S008 之前，工作台曾同时读取旧 `ConceptProject/ConceptVersion/ModuleGraph` 和新 `AgentAssetVersion/AssemblyGraph`，可能出现：

- Agent 面板显示资产 v3，状态栏显示 Concept v2；
- 旧 ModuleGraph 选择和新 Agent Part 选择同时生效；
- Agent GLB 导出走新资产，其他格式回退旧 Concept；
- 质量报告、撤销和恢复指向不同版本链。

这是数据正确性问题，不是单纯文案问题。S001–S008 已用唯一 `ActiveDesignSnapshot` 收敛 Agent-first 主路径；以下旧现象只作为迁移历史保留，不能再被描述为当前 Agent-first 运行时的正常行为。仍未完成的是 legacy 兼容 UI 完全退出、广泛多客户端压力验证和 packaged 安装恢复。

## 2. ActiveDesignSnapshot

正式合同：

```text
ActiveDesignSnapshot@1
├── project_id
├── active_design（判别联合，只能二选一）
│   ├── agent_asset: project_id + asset_version_id + assembly_graph_id
│   └── legacy_concept_read_only: project_id + legacy_version_id + module_graph_id
├── selected_part_id?
├── selected_material_zone_id?（必须属于 selected_part_id 的真实材质区）
├── preview?（project_id + change_set_id + base_asset_version_id）
├── quality?（project_id + quality_report_id + asset_version_id）
├── export（source + project_id + source_version_id）
├── render_preset?（ActiveDesignRenderPreset@1：camera_view + light_preset）
├── part_display?（ActiveDesignPartDisplay@1：locked_part_ids + hidden_part_ids + isolated_part_id）
├── revision
└── updated_at
```

`active_design` 的嵌套 `project_id` 用于合同层拒绝跨 Project 引用。Agent source 下 preview 的 base、quality 的 asset 和 export source version 必须等于 active Agent asset version；legacy source 下 preview/quality/Agent selection 必须为空，export 只能指向 active legacy version。前端只消费完整 Snapshot，不单独拼接多个 hook 的“当前”状态。

`active-design-snapshot.schema.json`、`ActiveDesignSnapshot` Pydantic model、生成 TypeScript、SQLite Snapshot 表、repository、revision CAS、旧库/空库迁移、GET/select/转换授权 API、桌面 client/reducer 均已完成。Snapshot 已在 Agent blockout 提交、GLB 导入和 ChangeSet preview/拒绝/确认时随 head 同事务更新；工作台的 Agent 恢复、部件选择、视口高亮、质量、GLB 导出及回退/前进已接入该 Snapshot。legacy 兼容 UI 只能只读并通过显式重建授权进入 Agent 路径；核心 CAS 竞争已有 smoke，广泛多客户端压力矩阵仍待完成。

## 3. 各对象的唯一拥有者

| 状态 | 唯一真值 | 允许缓存 | 禁止行为 |
| --- | --- | --- | --- |
| Project | SQLite `projects`/目标通用 Project repository | 前端只读摘要 | 用 localStorage 创建第二个 Project 真值 |
| AgentAsset | SQLite `agent_asset_versions` + 内容寻址对象 | GLB/缩略图缓存 | 把旧 ConceptVersion 当作同一个资产版本号 |
| Version Head | 服务端原子更新的 Agent asset head + Snapshot | 前端缓存 Snapshot revision | 前端自行推断最新版本 |
| Selection | Snapshot 的 `selected_part_id` + `selected_material_zone_id` | 视口临时 hover | 同时保存 ModuleGraph node 和 Agent part/zone 为活动选择 |
| Preview | 单个未确认 ChangeSet | 视口 ghost 几何 | 直接改写父版本或存在多个活动预览 |
| Quality | 指向活动资产版本的最新报告 | UI 摘要 | 显示旧 Concept 报告为 Agent 资产报告 |
| Export | Snapshot 的 `export.source_version_id` | 下载状态 | 根据文件格式切换到另一版本链 |
| Camera / light | Snapshot 的 `render_preset`（Agent asset only） | localStorage 仅作首次加载前的 UI 偏好 | 把 localStorage 当版本真值或给 legacy Snapshot 写入 Agent preset |
| Part display / protection | Snapshot 的 `part_display`（Agent asset only） | 视口临时 hover | 用组件 local state 伪造锁定、让隐藏部件保持选中，或把显示动作变成几何版本 |
| Concept scope | Agent Kernel 在每次 Turn 内本地计算的 `ConceptScopeDecision@1` | 当前 Turn 的 Item 展示 | 将它当作 Project、Version、Selection、Quality、Export 或 Snapshot 真值 |

## 4. 状态转换

```text
NoProject
  → ProjectReady
  → DirectionPreview
  → SegmentationCandidate
  → EditableAsset(version N)
  → ChangePreview(base N)
  → EditableAsset(version N+1)
```

规则：

1. Agent-first 主路径每个状态只有一个 `revision`；legacy 兼容读取不与 Agent 版本合并；
2. ChangeSet 必须声明 base version；
3. base 不是活动版本时标记 stale；
4. confirm 在事务中创建子版本并更新 head；
5. selection 在版本切换后必须重新验证；
6. quality 和 export 必须显式携带 source version；
7. 重启后只从服务端恢复 Snapshot，localStorage 只允许保存无害 UI 偏好。
8. 撤销或重做不会原地重激活历史版本：服务端从目标内容创建新的不可变 AgentAssetVersion，原 head 变为 `superseded`，并在同一事务中清空 selection、preview 和 quality。
9. 相机视图（`iso/front/top/right`）和灯光预设（`cad_neutral/soft_studio/concept_contrast`）属于活动 Agent Snapshot 的视觉状态；切换经过 revision/ETag/Idempotency-Key CAS，资产版本切换会重置到默认 `iso/cad_neutral`。它们只控制同一个主视口，不代表工程照明或照片级渲染。R002/R003 的四视图及条件式爆炸 PNG，以及 R004 由同一 fingerprint 生成的 PNG/manifest ZIP，均是绑定当前 AgentAssetVersion 的派生只读 artifact，不能成为新的版本、质量、装配或导出真值；切换资产后旧 render-set 必须丢弃，指纹不匹配的图包下载必须拒绝。
10. 部件显示与保护（`part_display`）只属于活动 Agent Snapshot：`locked_part_ids` 会在服务端阻止相关 ChangeSet，`hidden_part_ids` 与 `isolated_part_id` 只控制同一个主视口可见性。隐藏或隔离使选中部件不可见时，服务端会原子清空 selection；资产版本切换、撤销/重做时只保留仍存在的稳定 part ID，其余显示状态必须丢弃。该状态不是工程装配约束、制造锁定或新几何版本。

## 5. 领域与概念范围预检

目标状态机不得把未知领域默认映射为武器包：

```text
recognized → 创建三方向计划
ambiguous  → waiting_for_clarification
unsupported → completed scope stop（不调用 Planner 或 Provider）
```

`DomainInferenceResult@1` 只负责识别/澄清领域；随后 `ConceptScopeDecision@1` 才以 `allowed`、`clarification_required` 或 `unsupported` 决定是否可进入 Planner。范围停止只允许写入可读的 Thread/Turn/Item/幂等记录，不能触及 Plan、blockout、AgentAssetVersion、Snapshot、质量或导出；它不是版本状态，也不改变当前选择。澄清只问一个问题，例如“这是汽车、飞机、机械臂，还是未来概念道具？”澄清前不得生成 blockout 或创建版本。

当前规则只覆盖明确的现实武器/制造、加工或材料配方、工程性能，以及车辆安全、适航/飞行、机器人控制/扭矩/认证请求。它是可测试、可解释的产品范围预检，而不是完整内容安全系统；其余安全边界仍由受限 ShapeProgram、工具权限、确认和导出合同共同保证。

## 6. Legacy 读取规则

迁移期允许：

```text
design_source=legacy_concept_read_only
```

此状态可以查看、导出旧交付或触发显式“转换为 Agent 资产候选”，但不能：

- 与 AgentAssetVersion 共用版本号；
- 在同一编辑动作中同时写两套图；
- 把旧质量报告附到新资产；
- 依据格式隐式切换导出源。

## 7. API 要求

S003 已实现：

```text
GET  /api/v1/projects/{project_id}/active-design
POST /api/v1/projects/{project_id}/active-design:select
POST /api/v1/projects/{project_id}/active-design:convert-legacy
POST /api/v1/projects/{project_id}/active-design:undo
POST /api/v1/projects/{project_id}/active-design:redo
GET  /api/v1/projects/{project_id}/active-design:navigation
POST /api/v1/projects/{project_id}/active-design:render-preset
POST /api/v1/projects/{project_id}/active-design:part-display
```

`GET /active-design` 与 S003 POST 返回 `ETag: W/"active-design-{revision}"`，并固定 `Cache-Control: no-store`。首次 GET 只会从有效 Agent head 或 legacy current version创建一个兼容 Snapshot；空项目不创建 Snapshot。navigation 是派生读模型，同样 `no-store` 且不提供独立 ETag，客户端必须刷新 Snapshot 后再写。选择、legacy hand-off、撤销、重做、render-preset 和 part-display 至少提交 `snapshot_revision` 或 ETag；质量检查必须同时提交 `Idempotency-Key` 和当前 ETag，重试重放原报告、旧 revision 返回 `ACTIVE_DESIGN_STALE`。part-display 只允许 Agent Snapshot；legacy 返回 `ACTIVE_DESIGN_LEGACY_READ_ONLY`，preview 存在时返回可恢复冲突，并按请求幂等重放。Agent ChangeSet preview 会绑定 `preview.change_set_id/base_asset_version_id`；确认子版本会清除 preview、quality 与 selection，拒绝 preview 会清空该引用。S007 将 hand-off 持久化为只含 source/revision 的转换授权；它不创建或修改 legacy 版本。撤销/重做只接受当前 Agent head 的服务端历史目标，在新版本中复制目标内容，不会改写或重新标记历史版本。只有获得授权后确认的新 Agent 资产才能原子替换活动设计，旧数据继续保留。

## 8. 前端要求

- 一个 reducer/state machine 持有 Snapshot；
- Agent 面板、视口、选择卡、质量抽屉和导出抽屉只读取同一 selector；
- 状态栏只显示 `active_asset_version_id` 对应版本；
- 旧 Concept UI 进入只读兼容模式；
- 任何版本不一致都阻止导出并显示可恢复错误。

## 9. 验收

- 工作台任意时刻只显示一个活动版本号；
- 任意时刻只有一个活动选择和一个预览 ChangeSet；
- 质量、导出、组件保存均引用活动资产版本；
- 重启恢复 Snapshot、选择和 head，无 localStorage 版本漂移；
- 从 legacy 转换不会修改原数据；
- E2E 覆盖预览、确认、拒绝、并发冲突、重启和导出版本一致性。
