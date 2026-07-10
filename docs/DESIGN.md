# ForgeCAD 系统设计

版本：产品重构 v2（2026-07-10）
状态：R0 已完成，R1 正在把旧武神基线重构为通用 3D 平台；R2 Concept 合同、Project/Profile/Version、Module/Connector registry、ModuleGraph、ChangeSet 和 QualityRun 数据/API 已落地。

## 1. 产品定义

ForgeCAD 是本地优先的 AI 模块化 3D 设计工作台。产品不按“武器”类别拒绝需求，但第一阶段只把以下场景作为正式范围：

- 未来武器概念；
- 游戏 3D 资产；
- 影视道具；
- 非功能性展示与收藏模型。

第一阶段产品形态由两层组成：

```text
ForgeCAD 通用 3D 设计平台
└─ Weapon Concept Pack（首个内容包）

后续扩展
└─ CAD / DFM Engineering Pack（独立工程能力包）
```

因此，“武器优先”描述的是首个垂直场景和内容包，不表示 P0 要完成可工作的武器工程设计或制造验证。

## 2. 首版边界

### 2.1 P0 必须支持

- 用自然语言和参考图描述设计意图；
- 生成结构化 `WeaponConceptSpec`；
- 从受控组件库选择模块，生成 `ModuleGraph`；
- 使用语义连接器完成替换、对齐和组合；
- 在可交互 3D 视口中进行选择、移动、旋转、测量和爆炸查看；
- 生成候选方案和结构化 `DesignChangeSet`；
- 先显示 ghost preview，用户确认后创建新版本；
- 检查网格、连接、相交、浮空、比例、对称、法线和非流形问题；
- 导出 GLB、OBJ、PNG、爆炸图、转台图、Module Manifest 和检查报告。

### 2.2 P0 不承诺

- 可发射、可承压或具有真实工作机构的武器设计；
- 枪机、闭锁、供弹、击发、膛压、热载荷或弹道工程；
- B-Rep、STEP、3MF、结构仿真、专业公差或制造认证；
- 通过外观模型推断实物安全、合法性或制造可行性；
- 让 LLM 直接生成并执行任意建模代码。

### 2.3 后续 Engineering Pack

CAD / DFM Engineering Pack 使用独立的权威链路：

```text
DesignSpec
→ FeatureGraph
→ build123d / OpenCascade
→ validated B-Rep
→ STEP / 3MF / STL / GLB
→ DFM / slicer estimate
```

该链路不与 P0 的 `ModuleGraph` 混称。概念项目只有在用户主动进入工程化流程、补齐尺寸与制造条件后，才能创建新的 Engineering Project。

## 3. 产品原则

### 3.1 通用平台优先，内容包承载垂直语义

项目、版本、任务、资产、预览、选择、导出和审计属于平台层；模块分类、提示模板、连接器规则、检查规则和演示数据属于 Weapon Concept Pack。未来增加机器人、载具或工业设备时，不复制一套应用骨架。

### 3.2 P0 的几何真值是模块图和 GLB

```text
WeaponConceptSpec
→ Module selection
→ ModuleGraph
→ validated module transforms/connectors
→ combined GLB
```

GLB 源模块、连接器元数据、变换矩阵和内容哈希共同构成可重建依据。渲染图不是几何真值；生成式概念图不能静默覆盖模块图。

### 3.3 AI 只提出变更

AI 可以解析 Brief、选择模块、调整比例、提出风格和组合方案，但修改必须落入结构化 `DesignChangeSet`。锁定模块和锁定连接器不能被静默修改；确认前只显示预览。

### 3.4 本地优先、可恢复、可追溯

SQLite 和内容寻址对象存储是本地权威数据。每个版本、任务和导出必须记录输入哈希、资产哈希、算法/Provider 版本、状态、错误和父版本。

## 4. P0 系统结构

```text
┌────────────────────────────────────────────────────────────┐
│ Tauri Desktop                                              │
│ Projects | Workbench | Library | Jobs | Exports | Settings │
└──────────────────────────┬─────────────────────────────────┘
                           │ HTTP + SSE
┌──────────────────────────▼─────────────────────────────────┐
│ FastAPI Local Agent                                        │
│ Routes → Use Cases → Workflow / Jobs                       │
├───────────────────────┬────────────────────────────────────┤
│ Repository / UoW      │ Ports                              │
│ SQLite / Object Store │ LLM / GLB / Renderer / Exporter    │
└───────────────────────┴────────────────────────────────────┘
```

P0 不强制启动独立 CAD Runtime。复杂网格检查或渲染可以先作为隔离 worker 运行；Engineering Pack 再增加 OpenCascade sidecar。

## 5. 代码边界

目标目录：

```text
apps/agent/forgecad_agent/
  api/                       # app factory、dependencies、thin routes、DTO
  application/               # commands、queries、use cases
  domain/
    concepts/                # spec、module graph、change set、versions
    modules/                 # module、connector、pack、compatibility
    quality/                 # checks、reports、rulesets
    jobs/                    # job、step、attempt、events
  ports/                     # LLM、GLB、renderer、exporter、repositories
  infrastructure/            # SQLite、object store、provider adapters
  runtime/                   # worker、workflow、checkpoint、recovery

apps/desktop/src/
  app/
  features/projects/
  features/concept-workbench/
  features/viewport/
  features/module-library/
  features/assistant/
  features/quality/
  features/versions/
  features/exports/

packages/concept-spec/
packages/module-graph/
packages/model-quality/
packages/test-fixtures/

packs/weapon-concept/
  pack.json
  modules/
  thumbnails/
  prompts/
  rules/
```

迁移期间 `wushen_agent` 和 `SQLiteAssetStore` 作为 legacy facade 存在；所有新 Concept 业务必须进入 `forgecad_agent`，不能继续扩大旧聚合类。

## 6. 核心领域合同

### 6.1 WeaponConceptSpec@1

它表达“希望看到什么”，不包含真实武器工作机理。

```json
{
  "schema_version": "weapon-concept-spec/1.0",
  "project_id": "prj_01...",
  "name": "寒地巡逻 S1",
  "archetype": "future_modular_sidearm",
  "intended_use": ["game_asset", "film_prop", "display_model"],
  "style": {
    "keywords": ["寒地", "工业", "紧凑", "硬表面"],
    "palette": ["graphite", "gunmetal", "signal_red"],
    "detail_density": 0.68
  },
  "proportions": {
    "overall_length_mm": 230,
    "body_height_mm": 54,
    "grip_angle_deg": 15
  },
  "required_slots": ["core", "front", "rear", "grip"],
  "optional_slots": ["top", "left", "right", "bottom", "side_panels"],
  "constraints": {
    "symmetry": "mostly_symmetric",
    "max_triangle_count": 180000
  },
  "assumptions": ["非功能性概念模型，不用于真实制造或使用"]
}
```

### 6.2 ModuleAsset@1

```json
{
  "schema_version": "module-asset/1.0",
  "module_id": "core_shell_01",
  "pack_id": "weapon-concept/1",
  "category": "core_shell",
  "asset_id": "ast_01...",
  "sha256": "...",
  "bounds_mm": [148, 56, 42],
  "triangle_count": 28400,
  "connectors": [
    { "id": "core.front", "type": "shell_front", "position": [-74, 0, 0], "rotation": [0, 0, 0], "scale_range": [0.9, 1.1] },
    { "id": "core.grip", "type": "grip_mount", "position": [32, -30, 0], "rotation": [0, 0, 0], "scale_range": [0.92, 1.08] }
  ]
}
```

首包使用九类视觉模块：

1. 核心外壳；
2. 前部外壳；
3. 后部外壳；
4. 握持外壳；
5. 顶部附件；
6. 侧部附件；
7. 下部结构；
8. 能源/储存视觉模块；
9. 装甲或装饰面板。

首批只制作 8–12 个高质量、拓扑可靠的手工 GLB；闭环稳定后扩展到 24–30 个。

### 6.3 ModuleGraph@1

```json
{
  "schema_version": "module-graph/1.0",
  "graph_id": "mg_01...",
  "root_node_id": "node_core",
  "nodes": [
    { "id": "node_core", "module_id": "core_shell_01", "transform": { "position": [0, 0, 0], "rotation": [0, 0, 0], "scale": [1, 1, 1] }, "locked": true },
    { "id": "node_front", "module_id": "front_shell_02", "transform": { "position": [0, 0, 0], "rotation": [0, 0, 0], "scale": [1, 1, 1] }, "locked": false }
  ],
  "edges": [
    { "from": "node_core:core.front", "to": "node_front:front.core", "status": "connected" }
  ]
}
```

图不变量：

- 每个非根节点必须能沿 edge 到达根节点；
- edge 两端连接器类型必须兼容；
- 变换必须是有限值，缩放必须落在模块约束内；
- 同一个非共享连接器只能占用一次；
- 删除节点必须同时处理子节点或明确重连；
- 保存前运行结构验证，失败图不能成为已确认版本。

### 6.4 DesignChangeSet@1

```json
{
  "schema_version": "design-change-set/1.0",
  "base_version_id": "ver_05",
  "summary": "延长顶部轮廓并降低附件高度",
  "operations": [
    { "op": "replace_module", "node_id": "node_top", "module_id": "top_accessory_03" },
    { "op": "set_transform", "node_id": "node_top", "scale": [1.12, 0.9, 1.0] },
    { "op": "set_style", "path": "palette.accent", "value": "signal_red" }
  ],
  "protected_nodes": ["node_core"],
  "status": "proposed"
}
```

允许的操作首版固定为：`add_module`、`remove_module`、`replace_module`、`connect`、`disconnect`、`set_transform`、`set_style`、`set_parameter`。不接受任意脚本。

### 6.5 ModelQualityReport@1

检查项分为三类：

- Graph：连接器不兼容、浮空节点、无根路径、重复占用、非法缩放；
- Mesh：法线、非流形边、退化三角形、隐藏几何、密度、UV、LOD；
- Assembly：模块相交、异常间隙、穿插、对称偏差、包围盒越界。

每项必须包含 `check_id`、severity、status、node ids、实测值、阈值、可读建议和 ruleset version。`passed`、`warning`、`failed`、`not_run` 不得混用。

## 7. 连接器系统

Weapon Concept Pack 的标准槽位：

```text
core.front
core.rear
core.top
core.bottom
core.left
core.right
core.grip
core.side_panel_left
core.side_panel_right
```

连接器定义局部坐标系、兼容类型、允许缩放、占用规则和可选间隙。装配时以连接器矩阵求解子模块世界变换；不依赖名称猜测或人工拖到“差不多”的位置。

## 8. 工作流与版本

### 8.1 主闭环

```text
新建 Weapon Concept 项目
→ 输入 Brief / 参考图
→ AI 生成 WeaponConceptSpec
→ 用户确认风格、比例和必需模块
→ AI 选择 2–3 组 ModuleGraph 方案
→ 用户进入工作台组装/精修
→ AI 提出 DesignChangeSet
→ ghost preview + 影响摘要
→ 用户确认并创建子版本
→ Model Quality 检查
→ 展示与导出
```

### 8.2 版本原则

- 已确认版本不可原地覆盖；
- ChangeSet 基于明确 `base_version_id`；
- stale base 必须重新预览或显式 rebase；
- 预览资产带 TTL，不作为正式版本；
- 每个版本保存 spec、graph、asset references、quality summary 和父版本。

## 9. API 草案

```http
POST   /api/v1/concept-projects
GET    /api/v1/concept-projects/{project_id}
POST   /api/v1/concept-projects/{project_id}/brief:interpret
POST   /api/v1/concept-projects/{project_id}/variants:generate

GET    /api/v1/packs
GET    /api/v1/packs/{pack_id}/modules
GET    /api/v1/modules/{module_id}

GET    /api/v1/concept-versions/{version_id}
POST   /api/v1/concept-versions/{version_id}/changes:propose
POST   /api/v1/change-sets/{change_set_id}:preview
POST   /api/v1/change-sets/{change_set_id}:commit

POST   /api/v1/concept-versions/{version_id}/quality-jobs
POST   /api/v1/concept-versions/{version_id}/render-jobs
POST   /api/v1/concept-versions/{version_id}/export-jobs
GET    /api/v1/jobs/{job_id}
GET    /api/v1/jobs/{job_id}/events
POST   /api/v1/jobs/{job_id}:cancel
```

当前实现已完成 Project/Version、Module registry、ModuleGraph、ChangeSet、QualityRun，并通过 `/brief:interpret`、`/variants` 和 `:select` 完成确定性 A/B/C 数据闭环。当前 Variant generator 是 R2 模板基线，不代表 R4 的 AI Brief/方案质量；Concept JobEvent 与 Export Job 仍是后续契约。

幂等创建请求接受 `Idempotency-Key`；耗时操作一律返回 Job，不让路由持有长事务。

## 10. 数据与资产

P0 主要表：

```text
projects
concept_specs
concept_versions
module_packs
module_assets
module_connectors
module_graphs
module_graph_nodes
module_graph_edges
change_sets
quality_reports
jobs
job_steps
job_attempts
job_events
assets
artifact_links
schema_migrations
```

对象存储保存源 GLB、组合 GLB、缩略图、参考图、渲染图、爆炸图、导出包和报告。数据库只保存元数据、相对对象键和 SHA-256，不保存依赖工作站的绝对路径。

## 11. 前端信息架构

工作台沿用参考图的高密度九区布局，但产品语义调整为五阶段：

```text
概念 → 组装 → 精修 → 检查 → 展示
```

- 左栏：项目、版本、AI 助手和当前 Brief；
- 顶部：文件操作、五阶段导航和视口工具；
- 中央：Three.js 3D 视口；
- 底部抽屉：组件、方案、版本、时间线；
- 右侧检查器：参数、外观、连接、检查；
- 状态栏：阶段、选择、连接状态、单位、任务和提示。

核心交互必须真实可用：模块筛选/选择、阶段切换、参数修改、连接状态、AI 提交、预览/确认、版本切换、检查和导出格式选择。

## 12. AI 设计协议

AI 输出必须过 Schema 校验。推荐分三步：

1. Brief Interpreter：产生 spec、缺失项和 2–3 个方向；
2. Module Planner：根据 pack inventory 产生候选 ModuleGraph；
3. Change Planner：根据当前 graph 和用户指令产生 DesignChangeSet。

Provider 不得看到本地绝对路径或密钥；日志保存清洗后的请求摘要、模型版本、延迟和 token 统计，不保存不必要的原始参考图。

## 13. 导出与交付包

P0 导出：

- `model.glb`：组合模型和材质；
- `model.obj`：通用网格交换；
- `preview.png`：透明或场景预览；
- `exploded.png`：爆炸结构图；
- `turntable/`：转台帧或视频；
- `module-manifest.json`：模块、连接器、变换和哈希；
- `quality-report.json/html`：检查结果和已知限制。

Manifest 中明确 `intended_use` 和“非功能性概念模型”声明。P0 不显示“制造就绪”。

## 14. 验证策略

固定纵向样本为“寒地巡逻 S1”未来模块化短武器概念，至少覆盖：

- 8–12 个模块资产注册；
- 9 个标准槽位中的核心、前部、后部、握持、顶部和侧板；
- 两组候选方案；
- 替换模块、调整比例、锁定核心、预览并提交；
- 人工注入连接器不匹配、浮空、穿插和非法缩放；
- GLB/OBJ/PNG/Manifest/报告导出；
- 重启后恢复项目、版本和 Job 历史。

质量门定义见 [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md)，运行与故障处置见 [OPERATIONS.md](OPERATIONS.md)。

## 15. 已冻结决策

1. P0 是通用平台加 Weapon Concept Pack；
2. 不按武器类别拒绝，但首版只承诺概念/游戏/影视/展示模型；
3. P0 权威模型是 `WeaponConceptSpec + ModuleGraph + GLB`；
4. AI 修改必须通过 `DesignChangeSet` 和用户确认；
5. 首批模块少而精，先做 8–12 个；
6. CAD/DFM 是独立后续 Engineering Pack；
7. Tauri 是产品路径，浏览器只是开发壳；
8. SQLite、对象存储、任务恢复和审计继续作为平台基础。
