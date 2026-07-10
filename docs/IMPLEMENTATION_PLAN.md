# ForgeCAD 路线图与实施计划

状态：R0 已完成；R1 通用基础设施解耦进行中；产品主线已校准为 **Weapon Concept Pack first**。

本文保持 R0–R6 的可回滚阶段结构。P0 先完成模块化武器概念设计闭环；参数化 CAD/DFM 作为独立后续 Engineering Pack，不再阻塞第一阶段产品验证。

## 1. 计划基线

截至 2026-07-10：

- 已有 Tauri、React、FastAPI、SQLite、内容寻址资产、Job/Step/Event/SSE、幂等、恢复和追加式版本；
- `asset_store.py` 已从约 5210 行降至约 3052 行；connection、migration、object store、Repository/UoW、Job query/command/recovery、asset upload、library/version、Creative Recast、同步 Create Weapon，以及 Generate-3D 同步/排队入口已提取；
- `App.tsx` 约 706 行；AppShell、Hash route、Runtime/JobEvent/Selection Providers 和懒加载工作台已提取；
- `main.py` 约 54 行；legacy route groups 和 app factory 已拆分；
- `#/cad` 已按九区布局切换到“概念/组装/精修/检查/展示”，并接入真实 Project/Version/ModuleGraph、GLB、Connector 与 Concept 源包导出；
- 新 Concept 合同、Project/Profile/Version、Module/Connector registry、ModuleGraph、ChangeSet、QualityRun、确定性 Brief/Variant、JobEvent@2/SSE、可追溯源包、combined GLB、OBJ/MTL、透明/爆炸 PNG、front/side/top 与 8 帧 turntable 已实现；最终高质量资产、AI 方案质量和转台视频尚未实现；
- `ModulePackManifest@1`、资产目录/许可证/GLB 结构校验、release 覆盖门和幂等批量导入已实现；正式 Blender 资产本身尚未完成；
- 服务端 `weapon-concept-geometry/1.1` 已从版本绑定的内容寻址 GLB 计算 Mesh/Assembly Findings，包含未连接组件 triangle BVH/SAT/containment，并由桌面 Finding 触发节点聚焦；异常间隙、对称/隐藏几何/LOD 和局部 triangle 高亮尚未完成；
- build123d、OpenCascade、FeatureGraph、STEP/3MF 和 DFM 尚未实现，且不再属于 P0 主链。

旧代码是迁移输入；当前工作台是参考实现，不代表新领域完成。

### 当前证据

| 项目 | 状态 | 证据 |
| --- | --- | --- |
| R0 tag / branch | 完成 | `legacy-wushen-v0.1`、`codex/refactor-cad-dfm-agent` |
| R0 ADR / baseline | 完成 | `docs/ADR`、`docs/evidence/R0_BASELINE.md` |
| R1 infrastructure | 主要通用切片完成 | `forgecad_agent/infrastructure` |
| R1 application services | 进行中 | Job query/command/recovery、Asset、Library、Creative Recast、Create Weapon、Generate-3D entry services；worker 执行、Patch/export 仍待提取 |
| R1 API factory | 完成当前切片 | legacy routes + base app factory |
| R1 frontend shell | 完成当前切片 | router、AppShell、Providers |
| R1 workbench reference | 五阶段语义已完成 | `#/cad`、`design-qa.md`；真实 ModuleGraph/Connector 进入 R2–R3 |
| R2 concept contracts | 第一切片完成 | `packages/concept-spec`、`forgecad_agent.domain.concepts`、`r2:contracts-gate` |
| R2 project/version data + API | 第一切片完成 | migration `0009`、Concept repositories/service/routes、`r2:gate` |
| R2 module registry + graph | 第一切片完成 | immutable GLB registration、Connector compatibility、Graph persistence、restart smoke |
| R2 ChangeSet + child Version | 第一切片完成 | proposed/previewed/confirmed、protected node、stale base、parent immutability smoke |
| R2 QualityRun/Findings | 第一切片完成 | version-scoped report ingestion、finding persistence、idempotency、round-trip |
| R2 Brief/Variant | 模板基线完成 | interpreted brief、A/B/C graph variants、selection、restart recovery |
| R2 Concept JobEvent@2 | 同步主链完成 | Brief、Variant、Graph validate、QualityRun、Export jobs/events、cursor、SSE、restart |
| R2 Concept Export | 源包闭环完成 | `ConceptExportManifest@1`、ZIP、source GLB/spec/graph/quality、hash、artifact link、JobEvent、restart smoke |
| R3 workbench data binding | 四个纵向切片完成 | 米制 GLB→毫米视口、加载/选择/隐藏/聚焦/overlay、drag candidate、ChangeSet replace+snap、Undo/Redo、explode、restart |
| R3 Module Pack tooling | 完成 | 九类/8–12 release 门、UV/material/triangle/bounds/hash/license 校验、dry-run/import、idempotency/restart smoke |
| R3 reference assets | 完成可运行基线 | 10 GLB、九类、17 Connector、三材质/UV0/normal/thumbnail/license、9-node Graph、desktop E2E；最终 Blender art 待完成 |
| R3 Connector snap/mirror | 合成与 API 基线完成 | 100/100 含镜像数学样本、root/child 子树重定位、remap、mirror Version/Export、cycle conflict、lock、idempotency/restart；正式资产指标待测 |
| R3 viewport lifecycle | 浏览器压力基线完成 | 20 轮 V3↔V4、1 canvas/1 active context、GC heap 与 renderer resource 上限；正式资产/Tauri 待测 |
| R3 operation timeline | 第一切片完成 | Project ChangeSet list API、operation/node/status/result Version、桌面回读与 restart smoke |
| R5 combined GLB | 第一切片完成 | static GLB merge、mm→m、Euler→quaternion、mirror scale、stable wrapper nodes、Manifest/hash、ZIP/direct download/restart、desktop E2E |
| R5 combined OBJ/MTL | 第一切片完成 | scene flatten、TRS/nonuniform scale/mirror、normal/winding、UV/material、meter units、Manifest/hash、ZIP/direct download/restart、desktop E2E |
| R5 deterministic PNG render | 第一切片完成 | 640×640 RGBA、auto-fit isometric、z-buffer、material color/light、preview/exploded、Manifest/hash、ZIP/direct download/restart、desktop E2E/visual QA |
| R5 multiview/turntable | 第一切片完成 | front/side/top、8 distinct frames、render-set ZIP、single Export reuse、API negatives/restart、desktop E2E/visual QA |
| R5 Mesh/Assembly quality | 精确穿插切片完成 | immutable GLB decode、indices/degenerate/normal/UV/topology/bounds、Connector alignment、未直连组件 triangle BVH/SAT/closed-mesh containment、Finding 点击聚焦、JobEvent/restart、desktop E2E |

## 2. 执行硬规则

1. 先冻结旧 baseline，再迁移领域。
2. 禁止继续扩张 `asset_store.py` 和 `App.tsx`。
3. 平台层保持通用，武器能力进入 `Weapon Concept Pack`。
4. P0 权威模型是 `WeaponConceptSpec + ModuleGraph + GLB modules`，不是 B-Rep。
5. 不把旧 WeaponDesignSpec、CreativeWeaponGraph 或 SkillGraph 机械改名。
6. 首版 AI 优先解析 Brief、选择模块、调整参数；不默认整模重生成。
7. 所有自然语言修改先形成结构化 `DesignChangeSet` 和幽灵预览。
8. Module、Connector、Version、Asset 必须使用稳定 ID；父版本和原始资产不可覆盖。
9. UI、报告和导出不得把概念 Mesh 声称为生产级 CAD 或制造就绪。
10. CAD/DFM Engineering Pack 使用独立合同、迁移、运行时和质量门，不能与 P0 长期双写。

## 3. 里程碑总览

| 阶段 | 目标 | 关键产物 | 退出门 |
| --- | --- | --- | --- |
| R0 | 冻结与决策 | tag、branch、ADR、baseline evidence | 旧版本可恢复 |
| R1 | 通用基础设施与产品校准 | Repository/UoW、app factory、frontend shell、双轨文档 | 旧 smoke 不回归，P0 边界一致 |
| R2 | Concept 合同与数据 | DomainProfile、WeaponConceptSpec、ModuleGraph、Connector、Version、ChangeSet | 合同/数据库门通过 |
| R3 | 模块系统与工作台 | 8–12 modules、选择/隐藏/替换/吸附/爆炸/保存 | 模块 E2E 与 GPU 生命周期通过 |
| R4 | AI Brief 与自然语言修改 | 三方案、模块推荐、ChangeSet、幽灵预览 | AI/锁定模块指标通过 |
| R5 | 检查、渲染与导出 | ModelQualityReport、GLB/OBJ/PNG/Manifest、爆炸图 | 检查与导出门通过 |
| R6 | Beta、资产扩展与发布 | 24–30 modules、用户测试、打包、旧域清理 | C01–C10 全部通过 |

CAD/DFM Engineering Pack 在 R6 首轮 Beta 证明产品价值后进入独立路线；其 DesignSpec、FeatureGraph、build123d、STEP/3MF 和 DFM 架构边界继续保留在 `DESIGN.md`，但不占用 P0 退出门。

## 4. 阶段细化

### R0：冻结旧版本与决策

已完成：

- 创建 `legacy-wushen-v0.1` 和重构分支；
- 保存旧门禁；
- 建立产品、内核、安全、许可证、数据迁移 ADR；
- 证明旧 baseline 可恢复。

新增范围修订必须使用 superseding ADR，不改写历史决策。

### R1：通用基础设施与产品校准

后端：

- 提取 connection、migration、content-addressed store；
- 提取 Job、Asset、Idempotency、Checkpoint repositories 和 UoW；
- 提取 legacy Job、Library、Asset Upload、Creative Recast、同步 Create Weapon 和 Generate-3D 同步/排队入口 services；
- 将剩余 Generate-3D worker、Patch/provider/export workflows 移出 `asset_store.py`；
- 保持 route handler 不写 SQL、不组文件、不直接调用 Provider。

前端：

- AppShell、router、Runtime/JobEvent/Selection Providers；
- 将旧工作台业务控制器从 `App.tsx` 提出；
- 将 `#/cad` 更名和改造成 Weapon Concept Workbench；
- 五阶段：概念 / 组装 / 精修 / 检查 / 展示；
- URL、server state、viewport state、未提交表单状态归属明确。

退出条件：

- `asset_store.py` 不再承担完整业务工作流；
- `App.tsx` 只做应用组合；
- README、计划、设计、操作文档使用一致的 Concept-first 边界；
- `npm run r1:gate` 通过。

### R2：Concept 合同、数据库与 API

新增合同：

- `DesignDomainProfile@1`；
- `WeaponConceptSpec@1`；
- `ModuleGraph@1`；
- `ModuleAssetManifest@1`；
- `DesignChangeSet@1`；
- `ModelQualityReport@1`；
- 通用 `JobEvent@2`。
- `ConceptExportManifest@1`。

新增表：

```text
projects / project_versions / domain_profiles
module_assets / module_connectors / module_graphs
design_briefs / design_variants / design_change_sets
quality_runs / quality_findings / export_packages_v2
```

新增 API：

```http
POST   /api/v1/projects
GET    /api/v1/projects
GET    /api/v1/projects/{project_id}
POST   /api/v1/projects/{project_id}/versions
GET    /api/v1/module-assets
POST   /api/v1/module-graphs/{graph_id}/validate
POST   /api/v1/projects/{project_id}/variants
POST   /api/v1/versions/{version_id}/change-sets
POST   /api/v1/change-sets/{change_id}/confirm
POST   /api/v1/versions/{version_id}/quality-runs
POST   /api/v1/versions/{version_id}/exports
```

退出条件：

- fresh/repeat migration 通过；
- 可创建 `weapon_concept` Project 和追加 Version；
- Module/Connector 稳定 ID 与引用完整；
- Python、TypeScript、OpenAPI 生成物无漂移；
- 新表不引用旧 CreativeWeaponGraph/SkillGraph 主键。

### R3：模块系统与桌面工作台

首个固定项目：`寒地巡逻 S1`。

首批 8–12 个静态 GLB 模块覆盖：

```text
core_shell / front_shell / rear_shell / grip_shell
top_accessory / side_accessory / lower_structure
storage_visual / armor_panel
```

实现：

- 统一坐标、朝向、原点、比例、材质槽、UV、LOD 和缩略图；合同、CLI 与模板已落地，正式资产制作按 `MODULE_ASSET_GUIDE.md` 执行；
- Connector overlay、拖放/替换、自动吸附、镜像、Transform、锁定；
- 模块树与视口同步选择、高亮、隐藏和聚焦；
- 爆炸视图、资源释放、Project 保存和恢复；
- 版本追加、Undo/Redo 和操作时间线。

退出条件：

- 能加载由 8–12 个模块构成的模型；
- 模块吸附和替换成功率 ≥95%；
- 更换主体后子模块可按 Connector 重新定位；
- 锁定模块不被普通操作改变；
- 连续加载/卸载无明显 GPU 泄漏；
- 重启后完整恢复 Project、Version 和 ModuleGraph。

### R4：AI Brief、方案与自然语言修改

工具：

```text
parse_design_brief
recommend_template
recommend_modules
create_variant
set_style_parameters
set_global_proportions
plan_change_set
```

工作流：

```text
Brief → Schema validation → A/B/C variants
→ 用户选择 → 指令 → ChangeSet → ghost preview
→ conflict/lock check → confirm → child version
```

AI 只能引用 registry 中存在的 Module/Connector ID。组件库无法满足需求时，局部生成进入单独 Job，原始资产不覆盖。

退出条件：

- Brief 解析成功率 ≥90%；
- 同一 Brief 返回三种明显不同方案；
- AI 修改成功率 ≥85%；
- 锁定模块保持率 ≥95%；
- 所有 AI 修改可解释、可撤销并创建子版本。

### R5：模型检查、渲染与导出

检查：

- 模块穿插、悬空模块、Connector 错位、非法缩放；
- 法线、非流形、对称差异、隐藏几何、网格密度、UV、LOD；
- Finding 点击后相机聚焦并高亮对应模块/区域。

已完成精确穿插切片：索引、三角形数量、退化面、法线、UV0、开放/非流形边、清单 bounds、Connector `0.1 mm / 0.1°` 对齐；未直连组件先走世界 AABB/BVH broad phase，再走 triangle SAT narrow phase，封闭且无表面交叉时补 containment。Finding 可点击选择并聚焦首个关联 Graph 节点。直接 Connector 相连的组件允许在接口区接触或嵌合，因此不进入穿插规则，仍由 Connector 对齐规则约束。异常间隙、隐藏几何、密度、对称、LOD 和相交三角形局部高亮仍待实现。

展示与导出：

- 三分之四、正视、侧视、透明背景；
- 爆炸图、简单工作室灯光和转台动画；
- GLB、OBJ、PNG、Module Manifest 与 Project Report。

退出条件：

- 严重网格问题提示率 100%；
- GLB 导出成功率 ≥98%；
- 导出 Manifest 的 asset/version/hash 可追溯；
- 原始资产不被修复或导出覆盖。

### R6：Beta、资产扩展与发布

实现：

- 模块库扩展到 24–30 个高质量资产；
- 新手六步流程与专业参数渐进展开；
- 10–20 名目标用户完成固定任务；
- 统计完成时间、失败步骤、AI 修改失败和导出意愿；
- 清理旧 Weapon/Skill/Unity 生产入口，保留 importer 和 baseline tag；
- Tauri sidecar 打包、许可证、SBOM、干净机器安装/卸载。

退出条件：

- 新用户首个有效设计时间 <5 分钟；
- 首次 Project 完成率 ≥70%；
- Undo/Redo、版本回退、崩溃恢复正确率 100%；
- C01–C10 全部通过；
- 安装包可脱离源码运行。

## 5. 推荐 PR 顺序

1. **Concept-first ADR 与文档**：双轨产品、P0 输出和范围。
2. **R1 剩余边界**：Provider workflow、App controller、通用 services。
3. **Concept contracts**：DomainProfile、Spec、ModuleGraph、ChangeSet、QualityReport。
4. **Concept database/API**：Project/Version/Module/Connector 与 `/api/v1`。
5. **Module fixtures**：第一套 8–12 个 GLB、Manifest、缩略图和许可证。
6. **Workbench IA**：五阶段、左 ContextPanel、右 Inspector、底部 Drawer。
7. **Module interaction**：选择、隐藏、替换、吸附、锁定、爆炸。
8. **Version/ChangeSet**：Undo/Redo、ghost preview、child version。
9. **Quality/Export**：检查定位、GLB/OBJ/PNG/Manifest。
10. **AI/Beta**：Brief、三方案、修改指标、打包和旧域清理。

每个 PR 必须有独立退出证据，禁止把 2–10 合成一个大重构。

## 6. C01–C10 新质量门

| Gate | 内容 | 阻断条件 |
| --- | --- | --- |
| C01 Contracts | Schema、Python/TS、OpenAPI、unknown field | 漂移、非法引用或不兼容 |
| C02 Database | fresh/repeat migration、FK、Version DAG | 失败、孤儿、环或父版本覆盖 |
| C03 Module Assets | 坐标、原点、比例、材质、UV、LOD、hash | 任一发布模块不合规范 |
| C04 Connectors | 类型、吸附、镜像、重定位、lock | 成功率 <95% 或锁定被破坏 |
| C05 Viewport | 选择、高亮、隐藏、爆炸、资源释放 | 错选、状态不同步或 GPU 泄漏 |
| C06 ChangeSet | before/after、ghost preview、Undo/Redo、Version | 变更不可解释或父版本被改写 |
| C07 Quality | 穿插、悬空、错位、法线、非流形、UV/LOD | 严重问题漏报 |
| C08 Jobs | 幂等、取消、超时、恢复、SSE replay | 重复副作用、丢事件或不可恢复 |
| C09 Export | GLB/OBJ/PNG/Manifest、hash、回读 | GLB 成功率 <98% 或工件不可追溯 |
| C10 Desktop E2E | Brief 到导出、重启恢复、打包 | 主链路或干净机器运行失败 |

现有 `m6:gate` 和 `release:gate` 只证明 legacy baseline；不得作为新产品发布证据。

CAD/DFM Engineering Pack 将另设 E01–E10：DesignSpec、FeatureGraph、B-Rep、STEP/3MF round-trip、DFM truth set 和 CAD sandbox，不与以上 P0 gate 混用。

## 7. Definition of Done

功能只有同时满足以下条件才完成：

- 合同、迁移、API、UI、错误码和领域 Profile 一致；
- 单元、集成、回读或 E2E 证据与风险匹配；
- Job 可取消、超时、重试或明确声明不适用；
- 失败有结构化 Finding/diagnostic；
- 工件可追溯输入、版本、资产 hash、Provider 和规则版本；
- 原始资产、父版本和锁定模块不可被静默覆盖；
- 操作手册包含真实命令、备份和恢复路径；
- 依赖、模型资产和素材许可证已记录；
- 文档不把尚未实现的功能写成已完成。

## 8. 最近十个可执行动作

1. 在已提取 Create Weapon 与 Generate-3D 同步/排队入口基础上，继续将 Generate-3D worker 执行、Patch 和 Unity export workflow 从 `asset_store.py` 提取为 application services。
2. 将旧工作台业务控制器从 `App.tsx` 提出，完成 R1 边界。
3. 制定 Module/Connector/材质/UV/LOD 命名规范。
4. 由 10 模块确定性参考 Pack 进入人工 Blender 最终资产：保持 ID/Connector/Manifest 不变，逐个替换 GLB、缩略图并运行正式替换矩阵。
5. 用正式资产测量 Connector 替换/镜像矩阵 ≥95%；显式镜像、自动吸附、root/child 子树重定位、拖拽候选、加载、选择、隐藏、聚焦、overlay、兼容替换、版本 Undo/Redo 与爆炸视图已完成合成/API/桌面基线。
6. 增强 ChangeSet 操作时间线的分页、搜索与 rejected diagnostic；基础 API/桌面回读、版本时间线和浏览器 GPU 生命周期压力门已完成，正式资产/Tauri profiling 待补。
7. 在已完成 combined GLB、OBJ/MTL、preview/exploded、front/side/top 与 8 帧 turntable 基础上补转台视频、抗锯齿/阴影，并评估 glTF Transform/Meshopt 优化及 Blender/Assimp round-trip。
8. 在已完成 triangle BVH/SAT/containment 与 Finding 节点聚焦基础上补异常间隙、对称/隐藏几何/LOD、相交三角形局部高亮，并形成完整 C07 truth set。
9. 接入 AI Brief/Module Planner/Change Planner 并验证三方案差异度。
10. 将 Concept jobs worker 化，补取消、重试、partial success 与 readiness。

第 9 步的数据闭环通过后再执行第 10 步；AI 指标达标后才进入局部组件生成与 Beta。CAD/DFM Engineering Pack 不提前占用 P0 主链。
