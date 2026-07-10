# ForgeCAD 系统设计

版本：产品重构 v2（2026-07-10）
状态：R0–R3 当前纵向切片已落地；R4 已完成 Brief/Module/Change Planner、A/B/C、ghost preview 与显式确认链，真实 Provider 指标仍待评测。

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

当前交付按未来概念、影视道具和虚构游戏美术资产处理。项目不生成现实可制造武器的精确图纸，也不输出制造尺寸、材料配方或加工流程；这项非制造边界不妨碍对武器外观、比例、模块、材质和展示细节进行精密设计。

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

迁移期间 `wushen_agent` 和 `SQLiteAssetStore` 作为 legacy facade 存在；所有新 Concept 业务必须进入 `forgecad_agent`，不能继续扩大旧聚合类。旧 Create、Patch、Generate-3D、Worker Runtime 与 Unity Export 已分别迁入 application services；facade 只做依赖组装、代理、旧错误映射，以及共享资产/质量/事件 adapter。AST 门要求 10 个 workflow facade 方法均不超过 30 行，并禁止高层 Provider/ZIP 编排回流。这些仍是冻结兼容链，不是 Concept 新架构。

## 6. 核心领域合同

### 6.1 WeaponConceptSpec@1

它表达“希望看到什么”，不包含真实武器工作机理。

```json
{
  "schema_version": "WeaponConceptSpec@1",
  "project_id": "prj_arctic_patrol_s1",
  "profile_id": "profile_weapon_concept_v1",
  "name": "寒地巡逻 S1",
  "archetype": "future_modular_sidearm",
  "intended_uses": ["game_asset", "film_prop", "non_functional_display"],
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
  "schema_version": "ModuleAssetManifest@1",
  "module_id": "module_core_shell_01",
  "pack_id": "pack_weapon_concept_v1",
  "category": "core_shell",
  "asset_id": "asset_core_shell_01",
  "sha256": "<64-char lowercase sha256>",
  "bounds_mm": [148, 56, 42],
  "triangle_count": 28400,
  "material_slots": ["primary", "secondary", "accent"],
  "connectors": [
    {
      "connector_id": "connector_core_front",
      "slot": "core.front",
      "connector_type": "shell_mount",
      "transform": { "position": [-74, 0, 0], "rotation": [0, 0, 0], "scale": [1, 1, 1] },
      "scale_range": [0.9, 1.1],
      "exclusive": true
    }
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

### 6.2.1 ModulePackManifest@1

`ModuleAssetManifest@1` 描述单个不可变模块，`ModulePackManifest@1` 描述可分发资产包。包合同固定 `millimeter` 业务单位、GLB 的 `Y-up / -Z-forward / right-handed` 导出约定、许可证和文件索引；P0 导入仅接受 `LOD0`。

```json
{
  "schema_version": "ModulePackManifest@1",
  "pack_id": "pack_weapon_concept_v1",
  "profile_id": "profile_weapon_concept_v1",
  "name": "Weapon Concept Pack v1",
  "version": "0.1.0",
  "description": "Future concept, game, film-prop and non-functional display modules.",
  "intended_uses": ["visual_asset", "game_asset", "film_prop"],
  "non_functional_only": true,
  "units": "millimeter",
  "up_axis": "Y",
  "forward_axis": "-Z",
  "handedness": "right",
  "license": {
    "spdx_expression": "LicenseRef-Proprietary",
    "license_path": "LICENSES/PACK.txt"
  },
  "modules": [
    {
      "module_id": "module_core_shell_01",
      "manifest_path": "modules/module_core_shell_01/module.json",
      "glb_path": "modules/module_core_shell_01/model.glb",
      "thumbnail_path": "modules/module_core_shell_01/thumbnail.png",
      "license_path": "modules/module_core_shell_01/LICENSE.txt",
      "lod": "LOD0"
    }
  ]
}
```

机器校验、Blender 命名和显式导入流程见 `docs/MODULE_ASSET_GUIDE.md`。`ModulePackManifest` 不改变 Module registry API，也不把视觉 Connector 提升为真实机械接口。

### 6.3 ModuleGraph@1

```json
{
  "schema_version": "ModuleGraph@1",
  "graph_id": "mg_arctic_patrol_v1",
  "project_id": "prj_arctic_patrol_s1",
  "root_node_id": "node_core",
  "nodes": [
    { "node_id": "node_core", "module_id": "module_core_shell_01", "transform": { "position": [0, 0, 0], "rotation": [0, 0, 0], "scale": [1, 1, 1] }, "locked": true, "visible": true },
    { "node_id": "node_front", "module_id": "module_front_shell_02", "transform": { "position": [0, 0, 0], "rotation": [0, 0, 0], "scale": [1, 1, 1] }, "locked": false, "visible": true }
  ],
  "edges": [
    {
      "edge_id": "edge_core_front",
      "from_node_id": "node_core",
      "from_connector_id": "connector_core_front",
      "to_node_id": "node_front",
      "to_connector_id": "connector_front_01_core",
      "status": "connected"
    }
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
  "schema_version": "DesignChangeSet@1",
  "change_set_id": "change_top_profile_01",
  "project_id": "prj_arctic_patrol_s1",
  "base_version_id": "ver_arctic_patrol_v1",
  "summary": "延长顶部轮廓并降低附件高度",
  "operations": [
    { "operation_id": "op_replace_top", "op": "replace_module", "node_id": "node_top", "module_id": "module_top_accessory_03" },
    { "operation_id": "op_scale_top", "op": "set_transform", "node_id": "node_top", "transform": { "position": [0, 0, 0], "rotation": [0, 0, 0], "scale": [1.12, 0.9, 1.0] } },
    { "operation_id": "op_accent", "op": "set_style", "path": "style.palette", "value": ["graphite", "gunmetal", "signal_red"] }
  ],
  "protected_node_ids": ["node_core"],
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

当前服务端规则集 `weapon-concept-geometry/1.3` 读取 Version 绑定的不可变 WeaponConceptSpec、ModuleGraph 与内容寻址 GLB，不接受客户端替它声明“已通过”。它解码内嵌 glTF accessor，检查索引范围、三角形计数、退化面、法线长度、UV0、焊接后的开放/非流形边、清单 bounds、重复面、内嵌封闭组件、密度离群、总三角预算和 P0 LOD0 名称；装配层复用 Connector 世界 frame 计算 `0.1 mm / 0.1°` 对齐误差，并检查 Spec 对称目标。`1.0` 是历史 AABB 筛查；`1.1` 增加精确穿插；`1.2` 增加 provenance 与间隙；包含新策略规则的报告必须记录为 `1.3`。

未直接相连的节点会把模块局部三角形按 Graph TRS 与 mirror 转到毫米世界空间，以每叶最多 8 个三角形的确定性 BVH 做 broad phase；候选对用三角形法向量、边叉积与共面分离轴做 SAT narrow phase，接触按相交处理，最多记录 128 个表面对。两个网格均无开放/非流形边且没有表面交叉时，再以三条稳定射线的多数奇偶规则检查完整包含。Finding 除 node ids、表面对数、containment、实际窄相位次数和截断状态外，还记录最多 16 组每节点的 triangle index 与毫米世界坐标。直接 Connector 相连的模块可能在接口处有设计允许的接触或嵌合，不进入穿插规则；在 Connector 对齐之外，`1.2` 以两个世界 AABB 的分离距离作为保守表面间隙，超过 `2 mm` 生成 warning。这个距离不是精确网格最近点，不能解释为装配公差或制造结论。

隐藏几何首版只对两个可证明的情况下结论：焊接后顶点集合完全相同的重复 triangle，以及一个断开的封闭组件被另一个断开的封闭组件严格包裹且没有表面相交。密度定义为 `triangle / 1000 mm²` 实际表面积；至少三个模块时，超过装配中位数 8 倍生成 warning，所有情况下都执行 Spec `max_triangle_count` 总预算。P0 LOD 规则验证 `MESH_/GEO_<module_id>_LOD0[_NN]`，LOD1/LOD2 仍被拒绝，不代表多 LOD 切换已经实现。

对称首版是模块占位代理：以 root 的局部 Z 中面为基准，中心跨中面的 AABB 自配对，离开中面的 AABB 只与同 category、尺寸和镜像中心均在容差内的模块配对；`symmetric` 最多允许 5% 未配对模块，`mostly_symmetric` 允许 35%，`asymmetric` 跳过。它不能发现 AABB 内部的细节不对称。桌面 Finding 点击会选择首个有效关联节点、框选全部关联节点，并以红色 emissive 与不受深度遮挡的线框叠加高亮双方和局部相交三角形。强度/制造分析不在该规则集内。

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
POST   /api/v1/projects
GET    /api/v1/projects
GET    /api/v1/projects/{project_id}
POST   /api/v1/projects/{project_id}/versions
GET    /api/v1/versions/{version_id}
POST   /api/v1/projects/{project_id}/brief:interpret
POST   /api/v1/projects/{project_id}/variants
POST   /api/v1/projects/{project_id}/variants/{variant_id}:select

POST   /api/v1/module-assets
GET    /api/v1/module-assets
GET    /api/v1/module-assets/{module_id}/file
POST   /api/v1/module-graphs/{graph_id}/validate
GET    /api/v1/module-graphs/{graph_id}

POST   /api/v1/versions/{version_id}/change-sets
POST   /api/v1/versions/{version_id}/change-sets:plan
GET    /api/v1/projects/{project_id}/change-sets
POST   /api/v1/change-sets/{change_set_id}:preview
POST   /api/v1/change-sets/{change_set_id}:reject
POST   /api/v1/change-sets/{change_set_id}:confirm

POST   /api/v1/versions/{version_id}/quality-runs
POST   /api/v1/versions/{version_id}/quality-runs:inspect
GET    /api/v1/quality-runs/{quality_run_id}
POST   /api/v1/versions/{version_id}/exports
GET    /api/v1/exports/{export_id}
GET    /api/v1/exports/{export_id}/file
GET    /api/v1/exports/{export_id}/combined.glb
GET    /api/v1/exports/{export_id}/combined.obj
GET    /api/v1/exports/{export_id}/combined.mtl
GET    /api/v1/exports/{export_id}/preview.png
GET    /api/v1/exports/{export_id}/exploded.png
GET    /api/v1/exports/{export_id}/views/{front|side|top}.png
GET    /api/v1/exports/{export_id}/turntable/{0..7}.png
GET    /api/v1/exports/{export_id}/turntable.mp4
GET    /api/v1/exports/{export_id}/renders.zip
GET    /api/v1/jobs/{job_id}
GET    /api/v1/jobs/{job_id}/events
```

ChangeSet 审计查询使用：

```http
GET /api/v1/projects/{project_id}/change-sets
    ?limit=20
    &cursor=<opaque>
    &q=<id|summary|node|diagnostic>
    &status=<proposed|previewed|confirmed|rejected|stale>
    &operation=<ChangeOperationType>
```

权威排序是 `updated_at DESC, change_set_id DESC`。cursor 同时绑定 query/status/operation 的 hash，不能跨过滤条件复用。migration `0015` 为每条记录增加 `user|planner` actor；Planner 行同时保存原始 instruction、rationale、`ConceptPlannerProvenance` 和 `concept_change_plan` Job ID。preview 的合同、锁定节点、Connector remap/snap 或 Graph validation 失败会把已持久化 ChangeSet 更新为 `rejected`，并保存 code/message/stage/operation_ids/node_ids/recorded_at；用户放弃 ghost preview 保存 `CHANGE_SET_DISCARDED`，confirm 前 current Version 漂移保存为 `stale` 与 confirm-stage diagnostic。HTTP 错误不是唯一审计来源。

桌面 `#/cad` 的“检查”面板已调用 `quality-runs:inspect`，显示规则集状态、Finding 消息和测量值；带 node ids 的 Finding 可点击选择节点并重新框选相机，这不是仅在 API 中存在的占位能力。

当前实现已完成 Project/Version、Module registry、ModuleGraph、ChangeSet、QualityRun 和 Concept Export；Brief、Variant、Change Planner、Graph validate、QualityRun 与 Export 均写入 Concept JobEvent@2。桌面 `#/cad` 已加载版本 Spec、Graph 与不可变 GLB，支持 raycast 选择、隐藏、聚焦、Connector overlay、显式 X 镜像和爆炸视图。组件可拖到视口目标节点形成替换候选；自然语言修改也可生成受限 DesignChangeSet，但两者都必须先 preview，AI 链路以半透明青色 ghost 显示，显式确认后才创建子版本，放弃只更新审计状态。Undo/Redo 是不可变 parent/child 版本导航。替换 preview 会先按 `slot + connector_type` remap，再以 root 为基准重定位被替换节点和后代；镜像也通过 `set_mirror` 形成子版本并进入 Export Manifest。额外循环约束无法同时满足，或自动重定位会移动 locked 后代时，preview 拒绝。正式资产成功率仍属于后续 R3。

Project ChangeSet 时间线从 `design_change_sets` 权威记录读取完整 actor、Provider provenance、instruction、operation、base/result Version、状态、诊断与时间戳；桌面时间线直接调用服务端 cursor/search/filter 并加载更多，不把 Version summary 或客户端数组过滤冒充操作审计。

### 9.1 坐标与 Connector 吸附

```text
GLB mesh POSITION = meters (glTF 2.0)
viewport / ModuleGraph / Connector position = millimeters
rotation = radians, Euler XYZ
scale = dimensionless
```

桌面加载器在 GLB asset scene 上应用固定 `×1000`，Graph node Transform 仍保持毫米。服务端吸附使用 Connector 局部毫米坐标计算世界 frame：非 root 替换固定其父节点，root 替换固定 root；后代沿确定性 BFS tree 递归重定位。`mirror_axis` 是独立的 `none/x/y/z` Graph 状态，视口将其转换为渲染 scale 符号，Connector 位置使用同一镜像轴参与吸附；Transform 本身继续只允许正 scale。树外约束边必须在 `0.1 mm / 0.1°` 容差内同时成立，否则 ChangeSet preview 失败。该算法不修改父版本，只写入 preview Graph 和确认后的子版本。

视口卸载必须释放 cloned material、geometry、GLTF texture、SkinnedMesh skeleton、OrbitControls、renderer 和 WebGL context。浏览器压力门连续切换 V3/V4，读取 renderer memory、DOM canvas/context 计数和 GC 后 JS heap；它证明合成 fixture 的释放行为，不替代正式资产或 Tauri 窗口中的 GPU profiling。

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

桌面应用组合边界固定为：

```text
App.tsx（route-level composition，21 行）
├─ useLegacyAppController（旧工作台应用状态与任务命令）
│  ├─ useAppRouting（Hash URL 与唯一 hashchange listener）
│  ├─ jobPersistence（最近任务与桌面通知持久化）
│  └─ assetSelectors（Version/Asset 派生选择）
├─ LegacyWorkbench（无本地 state/effect 的页面渲染组合）
└─ CadWorkbenchPanel（独立 lazy route）
```

`Preview3DPanel` 继续由 `LegacyWorkbench` 动态导入，避免把 Three.js 查看器并回基础 bundle。`App.tsx` 不得持有任务轮询、恢复、通知、`localStorage` 或页面业务状态；`scripts/smoke_r1_foundation.py` 对该边界做静态断言，桌面 E2E 再验证行为连续性。

## 12. AI 设计协议

AI 输出必须过 Schema 校验。推荐分三步：

1. Brief Interpreter：产生 spec、缺失项和 2–3 个方向；
2. Module Planner：根据 pack inventory 产生候选 ModuleGraph；
3. Change Planner：根据当前 graph 和用户指令产生 DesignChangeSet。

Provider 不得看到本地绝对路径或密钥；当前 provenance 只保存 provider/model、清洗后输入/输出 hash、registry ids 与 warning，不保存密钥、绝对路径或原始参考图。真实 Provider 评测时还需补延迟与 token 统计。

当前 Brief/Module/Change Planner 共用 `ConceptPlannerProvider` 边界。默认 `deterministic_rules` 不是 AI：它只把有限视觉词汇映射到有界 Spec 参数，生成三个可重复结构方案，并将明确数值/有限相对词、展示配色、选中候选替换或镜像映射为受限操作。配置 `openai_compatible` 后，Brief 只返回 style/proportions/symmetry patch，Module Planner 只返回 rank/name/summary、已存在 target node、`0.85–1.15` scale、注册 module ids 和 rationale；Change Planner 只能返回 `replace_module/set_mirror/set_style/set_parameter`，路径白名单固定，所有 nullable 字段在 strict JSON Schema 中仍为 required。服务端固定 project/profile/id、安全假设与 Graph 不变量，不直接执行模型文本。

每条 Brief/Variant/Planner ChangeSet 保存 `ConceptPlannerProvenance`：实际 generator/provider/model、auto fallback 前尝试的 provider/model、fallback 标记、清洗后输入/输出 SHA-256、当时 registry module ids 和 warning。migration `0014` 为旧 Brief/Variant 模板行写入明确的 legacy provenance；migration `0015` 为 ChangeSet 增加 actor/instruction/rationale/provenance/Job。`auto` 可以显式降级；`configured_provider` 失败必须向调用方暴露，不能用规则结果伪装 AI 成功。Variant 选择只切换桌面预览并更新 selected/rejected，不创建 Version；Change Planner 则产生 proposed 记录并复用既有 `preview → confirm`，只有 confirm 才创建子版本。

## 13. 导出与交付包

当前已实现 `ConceptExportManifest@1` 源包、`Model/combined.glb`、`Model/combined.obj/.mtl` 与 `Renders/preview.png/exploded.png`：ZIP 中包含不可变模块 GLB、WeaponConceptSpec、ModuleGraph、可选最新质量报告、README、组合/交换/渲染工件和逐文件 SHA-256；数据库保存 package asset、artifact link 与 completed JobEvent。

R5 的正式 P0 导出目标：

- `model.glb`：组合模型和材质；
- `model.obj`：通用网格交换；
- `preview.png`：透明或场景预览；
- `exploded.png`：爆炸结构图；
- `turntable/`：转台帧或视频；
- `module-manifest.json`：模块、连接器、变换和哈希；
- `quality-report.json/html`：检查结果和已知限制。

Manifest 中明确 `intended_use` 和“非功能性概念模型”声明。P0 不显示“制造就绪”。

combined GLB 第一切片合并静态 GLB 的 bufferView/accessor/mesh/material/node，去重完全相同材质，将 Graph `position(mm)` 转成 glTF `translation(m)`，Euler XYZ 转 quaternion，并将 `mirror_axis` 写入 wrapper node 有符号 scale。wrapper 使用稳定 `NODE_{node_id}__{module_id}` 名称和 provenance extras。skin、animation、纹理和 required/compression extension 当前结构化拒绝；后续由 glTF Transform/Meshopt 与纹理管线扩展，不能静默丢数据。

combined OBJ 第一切片以同一份 combined GLB 为输入，递归扁平化 scene graph，将节点 matrix/TRS、非均匀缩放和镜像烘焙进顶点；法线使用逆转置矩阵并归一化，负行列式变换翻转三角面序。OBJ 固定声明米制，保留 `NODE_{node_id}__{module_id}` 路径、`v/vt/vn/f` 和稳定 material 名；PBR factor 确定性投影为配套 MTL。OBJ/MTL 进入同一不可变 ZIP 和 Manifest，并提供独立下载。该转换不支持 sparse accessor、非 TRIANGLES primitive、morph、skin、animation 或贴图搬运；MTL 是有损交换格式，不替代源 GLB。

PNG 管线以同一份 combined GLB 为输入，经确定性 OBJ flatten 后在 Agent 内软件光栅化。固定输出 640×640 RGBA8、透明背景、正交投影、自动取景、z-buffer、基础材质颜色和方向光。exploded render 只复制 GLB JSON，在临时 wrapper translation 上按装配中心径向增加确定性距离；中心重合时用 node ID hash 生成稳定方向，不修改 ModuleGraph、Version 或源 GLB。固定 `front(+Z) / side(+X) / top(+Y)` 三个正交视图和绕 Y 轴均匀采样的 8 帧 turntable 与 preview/exploded 共 13 张图。

展示交付切片在透明轮廓外缘增加固定 coverage 像素，并在非 top 相机下绘制确定性半透明软接触阴影；算法和模式进入 Export metadata/JobEvent，避免把技术预览写成照片级渲染。请求显式设置 `include_turntable_video=true` 时，Agent 通过配置的 FFmpeg 以固定 8 fps、单线程 H.264 参数和移除时间元数据的方式生成 `Renders/turntable.mp4`；视频和 13 张 PNG 一同进入 `render-set.zip`、主 ZIP 与 Manifest。旧请求默认不依赖 FFmpeg；视频请求在编码器缺失时返回结构化 `VIDEO_ENCODER_UNAVAILABLE`。桌面在 Version 未变化且所需工件存在时复用最近 Export，避免各格式形成不同交付真相。

`scripts/check_dcc_roundtrip.py` 是只读交付门：发现 Blender/Assimp 后将显式输入的 combined GLB 导入再导出，校验源 SHA-256 未变化、输出 GLB 2.0 可读且 flatten 后 vertex/triangle count 一致。没有 DCC 时只返回 `blocked_dcc_not_configured`；这不构成 round-trip 证据。当前渲染器仍不含贴图、PBR 环境光或照片级材质。

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
