# ForgeCAD（原武神 Forge）

ForgeCAD 是一个本地优先的 AI 模块化 3D 设计工作台。底层平台保持通用，第一阶段只交付 **Weapon Concept Pack**：面向未来武器概念、游戏资产、影视道具和非功能性展示模型，提供模块组合、AI 设计修改、版本、检查、渲染与导出闭环。

> 新产品不拒绝武器题材。第一阶段允许外观精密、比例明确、模块细致、装配直观和版本可追踪；但产品不会把概念 Mesh/GLB 冒充为功能性武器工程 CAD，也不会在同一默认流程中混入未经实现的 STEP、完整 DFM、BOM 或生产切片。

边界不是拒绝武器设计，而是明确交付类型：当前生成物属于未来概念、影视道具和虚构游戏美术资产，导出包含非制造说明；产品不输出可用于现实制造武器的精确图纸。

正式品牌尚未冻结，文档暂用 **ForgeCAD**。代码包名和环境变量仍保留 `wushen` / `WUSHEN_*`，直到迁移完成。

## 产品结构

```text
通用 3D 设计平台
├── 项目 / 版本 / 资产 / Job / Agent / 视口 / 导出
├── Weapon Concept Pack（第一阶段）
├── Game Prop / Robot / Vehicle Pack（后续领域包）
└── CAD / DFM Engineering Pack（独立后续轨）
```

第一阶段不是“万能 AI CAD”，也不是“输入文字后每次重新生成整把模型”。核心价值是：

> 用人工可控的高质量模块库，让用户在五分钟内完成一款可修改、可回退、可检查、可展示和可导出的武器概念设计。

## 第一阶段目标用户

- 游戏独立开发者与 3D 美术；
- 影视道具与概念设计人员；
- 硬表面建模人员；
- 武器概念设计爱好者；
- 需要快速组合与比较造型方案的小型创意团队。

北极星指标：

> 每周完成并成功导出的有效武器概念设计数量。

不是聊天次数、Prompt 数或完整模型生成次数。

## 第一阶段闭环

```text
描述 Brief
→ 选择三个轮廓方案之一
→ 组合和替换模块
→ 调整比例、材质和配色
→ AI 生成 DesignChangeSet
→ 幽灵预览与冲突检查
→ 用户确认并创建子版本
→ 模型质量检查
→ 渲染 / 爆炸图 / ZIP、GLB、OBJ、PNG 导出
```

工作台使用五个阶段：

```text
概念 / 组装 / 精修 / 检查 / 展示
```

## P0 边界

P0 只实现一个完整场景：

> **未来模块化短型武器概念设计工作台。**

支持：

- 自然语言 Brief、参考图和风格参数；
- 三种明显不同的组合方案；
- 8–12 个首批模块，逐步扩展到 24–30 个；
- 模块选择、高亮、隐藏、替换、镜像、锁定和爆炸视图；
- 语义 Connector、吸附、父子关系和对称规则；
- 比例、位置、旋转、材质、配色和细节密度；
- `WeaponConceptSpec@1`、`ModuleGraph@1`、`DesignChangeSet@1`；
- `ModuleAssetManifest@1`、`ModulePackManifest@1` 与显式 dry-run/import 资产门；
- 追加式版本、Undo/Redo、操作时间线；
- 网格、法线、穿插、接口、对称、UV 和 LOD 检查；
- GLB、OBJ、PNG、爆炸图、组件 Manifest 与项目报告。

P0 导出 Profile：

```text
visual_asset
game_asset
film_prop
non_functional_display
```

P0 不承诺：

- 内部击发、闭锁、膛室、弹药或消声功能设计；
- 生产级关键机械参数与功能安全结论；
- STEP、工程 BOM、完整 DFM、切片或制造就绪状态；
- 任意 Python/脚本执行；
- 每次 AI 修改都重新生成整把模型。

这些是阶段排序，不是对武器题材的类别拒绝。需要精确 B-Rep、STEP、3MF 和制造检查的工作进入独立 CAD/DFM Engineering Pack，并使用另一套合同和质量门。

## 第一阶段领域模型

```text
WeaponConceptSpec
└── ModuleGraph
    ├── Module
    ├── Connector
    ├── Transform / symmetry / lock
    └── material slots / provenance

DesignChangeSet
├── before / after
├── affected modules
├── locked modules
└── ghost preview / confirmation

ModelQualityReport
└── findings with module and viewport location
```

首批九类视觉模块：

```text
核心主体壳体 / 前端外壳 / 后部外壳 / 握持外壳
顶部视觉附件 / 侧面附件 / 下部结构
能源或存储造型模块 / 表面装甲与装饰面板
```

首批语义接口：

```text
core.front / core.rear / core.top / core.bottom
core.left / core.right / core.grip
core.side_panel_left / core.side_panel_right
```

## 当前真实完成度

| 能力 | 当前状态 | 决策 |
| --- | --- | --- |
| Tauri + React 桌面壳 | 已实现 | 保留 |
| FastAPI 本地 Agent | 已实现 | 保留并继续薄化路由 |
| SQLite、WAL、迁移 | 已实现 | 泛化为 Project/Version/Module 数据 |
| 内容寻址资产与 SHA-256 | 已实现 | 直接复用 |
| Job / Step / Event / SSE | 已实现 | 直接复用 |
| Concept JobEvent@2 | 独立 Job/Event 表、JSON replay、Last-Event-ID/SSE 已实现；Brief、Variant、Change Planner、Graph validate、QualityRun、Export 均已留痕 | 异步取消/重试继续在后续 worker 化 |
| 幂等、取消、重试、恢复 | 已实现 | 直接复用 |
| R1 通用基础设施拆分 | 当前退出边界完成 | `asset_store.py` 的完整 workflows 已迁入 application services；`App.tsx` 已从约 706 行缩为 21 行组合根，路由、控制器、持久化、选择器和旧工作台渲染已分层，并由 `r1:gate` 固定 |
| `#/cad` Concept 工作台 | 已读取真实 Project/Version/ModuleGraph/GLB，支持选择、隐藏、聚焦、Connector overlay、拖拽候选、ChangeSet 替换/吸附/镜像、版本 Undo/Redo、爆炸视图、实际几何检查及 ZIP/GLB/OBJ/MTL/PNG/MP4 导出 | 已用 10 模块参考 Pack 跑 E2E；人工 Blender 最终质量与正式替换矩阵待完成 |
| 视口 GPU 生命周期 | geometry/material/texture/skeleton、controls、renderer 与 WebGL context 显式释放；版本压力 smoke 已实现 | 20 轮 V3↔V4 保持 1 canvas/1 context；正式资产和 Tauri 压力仍待验证 |
| Module Pack 与正式资产晋级门 | `ModulePackManifest@1`、`ForgeCADModuleNaming@1`、目录/许可证/GLB/UV/材质/三角数/包围盒校验、dry-run、幂等批量导入、重启恢复已实现；`FormalModuleReview@1` 锁定 `.blend`、module Manifest、GLB、thumbnail 与 Pack/Module license hash，要求独立人工审阅、全部评分 ≥4、Blender generator、anti-placeholder 三角下限、最终许可证和基线 ID/Connector 不漂移 | Blender 4.2.22 LTS 已真实生成 core/front01/front02 的 `.blend`/GLB/thumbnail，只读 re-export、source hash 不变、Connector 基线和 core GLB DCC 往返已验证；它们仍是 starter，core 940 三角低于正式下限 1000，且无最终许可证/独立人工批准 |
| Arctic Patrol S1 参考 Pack | 10 GLB、九类、17 Connector、UV0/normal/三材质、缩略图、许可证、确定性生成与 9 节点 Graph 已实现 | 可用于产品闭环和 DCC 交接，不冒充最终高质量美术 |
| 旧 CreativeWeaponGraph / SkillGraph | 已实现 | 冻结并删除，不机械改名 |
| 旧图像/神经 3D Provider | 已实现 | 仅作可选概念或局部组件生成来源 |
| 旧 Unity 导出 | 已实现 | legacy baseline；P0 改为通用 GLB/OBJ/Manifest |
| Concept Project / Version / Profile | migration、Repository/UoW、创建/列表/详情/追加版本 API、桌面项目管理与版本切换已实现 | 打包桌面的干净机器恢复仍待 C10 |
| WeaponConceptSpec / ModuleGraph | 合同、注册校验、持久化、Version 绑定、回读 API 与桌面真实 GLB 渲染已实现 | 正式资产和 Tauri 性能仍待验证 |
| Connector / 模块吸附 / 镜像 / 爆炸视图 | Connector 合同、注册、兼容校验、替换 remap、rooted 子树自动重定位、显式镜像 ChangeSet、冲突/lock 拒绝、overlay 和爆炸视图已实现 | 合成 100 组为 100%；正式资产替换矩阵 ≥95% 仍待验证 |
| Brief / A-B-C Variant | Brief Interpreter、注册表约束 Module Planner、确定性规则 Provider、OpenAI-compatible strict JSON Schema Adapter、显式 auto fallback、provenance/hash、三种结构方案、桌面生成/选择预览与重启恢复已实现 | 当前只证明 Provider 边界与确定性/fake-provider truth set；真实模型解析成功率和方案质量指标尚未达标 |
| DesignChangeSet 幽灵预览 | 自然语言 Change Planner、受限 `replace_module/set_mirror/set_style/set_parameter`、proposed/previewed/confirmed/rejected/stale、半透明 ghost、锁定/注册表/Connector 校验、显式确认与子版本提交已实现 | 确定性/fake-provider/API/桌面链通过；真实模型修改成功率 ≥85% 与锁定保持率 ≥95% 尚未评测 |
| R4 Planner 评测 | 固定 20 Brief + 20 Variant + 20 Change + 20 lock probes、truth-set hash、阈值、逐例结果、p50/p95 延迟、token 汇总与严格 live CLI 已实现；明确数值 fallback 已补齐 | deterministic baseline 为 100%/100%/100%/100%，但 `real_provider_evidence_eligible=false`；当前环境未配置真实 Provider，不能作为 AI 指标证据 |
| ChangeSet 操作时间线与审计归档 | Project 级逆序 cursor API、搜索、状态/操作过滤、user/planner actor、Provider provenance、instruction、operation/node/result Version、rejected/stale/discarded diagnostic、桌面加载更多已实现；当前筛选可批量导出 JSONL/CSV、逐文件哈希清单与不可变 ZIP，数据库和对象在 Agent 重启及隔离整库恢复后均可回读 | `project_lifetime` 不提供单包删除入口，并随 Project 数据保留；它不是法规级 WORM、legal hold 或加密异地灾备，正式规模恢复耗时仍待测 |
| Library 备份、恢复与演练 | SQLite Backup API 快照、`ForgeCADLibraryBackupManifest@1`、legacy/Concept 对象引用合并、SHA-256/size/FK/integrity 校验、容量/去重/未引用候选统计、禁止覆盖式恢复已实现；`ForgeCADLibraryRecoveryDrillReport@1` 可连续测量备份/独立校验/恢复/Agent 回读、吞吐及相对基线容量增长，并逐个回读 Module GLB hash；正式资产声明还必须携带 `formal_release_10_12` 晋级报告且 hash 集合完全一致 | 10 模块参考 Pack 已有可重复时间/容量基线且会被识别为 fixture；备份不含 Provider secret/config、WAL/SHM、trash 或未引用对象，且不是加密异地备份；正式 Blender/代表性用户库仍须独立运行，reference-aware GC 尚未启用 |
| ModelQualityReport | `weapon-concept-geometry/1.3` 从版本绑定的 Spec、ModuleGraph 与不可变 GLB 检查 Mesh、重复/内嵌隐藏几何、组件间密度离群、项目三角预算、P0 LOD0 命名、根中面对称占位、Connector 对齐/间隙，以及未直连组件的世界三角形 BVH/SAT/containment；局部 triangle provenance、工作台高亮、JobEvent 与重启恢复已实现 | 当前仍是确定性概念资产筛查；正式 Blender truth set、Tauri 大网格阈值和多 LOD 运行时尚未完成，不代表强度/DFM/安全证明 |
| Concept Export | 源 GLB/Spec/Graph/Quality ZIP、combined GLB、OBJ/MTL、透明/爆炸 PNG、front/side/top、8 帧 turntable、确定性 MP4、轮廓抗锯齿/软接触阴影、render-set ZIP、Manifest hash、JobEvent、独立下载与重启恢复已实现 | Blender 4.2.22 已对 starter core（2178 顶点/940 三角）和工作台导出的 10 模块 reference combined GLB（840 顶点/420 三角）完成真实往返；纹理交换和正式资产渲染性能仍待验证 |
| DesignSpec / FeatureGraph / CAD Runtime | 未实现 | 后续 CAD/DFM Engineering Pack |

新 Concept 数据链已经独立存在，桌面工作台已消费真实 Project、Version、ModuleGraph 和源 GLB；Brief/A-B-C 与自然语言修改均通过同一 Provider 边界，修改先进入半透明 ghost preview，确认后才由既有 ChangeSet 服务创建子版本。操作记录可按当前筛选归档为带 SHA-256 清单的 JSONL/CSV ZIP。combined GLB、OBJ、正交图、8 帧 turntable 和 MP4 都读取同一 Graph/资产真相，首版 Mesh/Assembly 检查也已落地。旧 Weapon/Unity 主链继续作为回归 baseline；当前仍不能证明真实 AI 指标、最终资产矩阵、法规级审计存储、照片级渲染、正式 Blender 资产的全装配往返、完整 R5 检查或 CAD/DFM 已完成。

正式模块的 Blender 坐标、单位、命名、材质、UV、Connector、缩略图、许可证、校验与导入步骤见 [Weapon Concept Module Pack 资产制作规范](docs/MODULE_ASSET_GUIDE.md)。

## P0 架构

```text
Tauri Desktop
  ├─ Projects / Concept / Assembly / Refine / Inspect / Showcase
  └─ HTTP + SSE
          ↓
FastAPI Local API
  ├─ Thin Routes / Use Cases / Jobs
  ├─ DesignDomainProfile
  └─ Weapon Concept Pack
          ↓
SQLite + Content-addressed Objects
  ├─ Project / Version / ModuleGraph / ChangeSet
  └─ GLB modules / thumbnails / reports / exports
          ↓
Three.js Workbench
  ├─ SceneGraph / Selection / Transform / Connector
  ├─ Exploded View / Comparison / Quality Overlay
  └─ GLB / OBJ / PNG / Manifest export
```

后续 Engineering Pack 独立增加：

```text
DesignSpec → FeatureGraph → build123d/OpenCascade B-Rep
→ STEP/3MF/STL → DFM/Print Doctor
```

GLB ModuleGraph 与 B-Rep FeatureGraph 是两种不同的权威模型，不能机械互转或长期双写。

## P0 技术选择

| 层 | 选择 | 用途 |
| --- | --- | --- |
| 桌面端 | Tauri + React + TypeScript | 本地工作台和 sidecar 生命周期 |
| 本地 API | FastAPI + Pydantic | 合同、用例、Job 与事件 |
| 数据 | SQLite + 内容寻址对象存储 | Project、Version、Module、工件 |
| 视口 | Three.js + three-mesh-bvh | 模块选择、变换、空间查询和检查定位 |
| 模块资产 | GLB + Blender 管线 | 人工高质量模块、材质、UV、LOD |
| 网格检查 | trimesh；Manifold 按需 | 非流形、法线、包围盒和组件检查 |
| AI 结构化输出 | Pydantic + strict JSON Schema / OpenAI-compatible Adapter | Brief、模块方案、受限 ChangeSet Planner；真实 Provider 指标待评测 |
| 图标 | Phosphor Icons | 统一技术工作台图标体系 |

第一阶段不引入 build123d、lib3mf 或 PrusaSlicer 作为主链依赖；它们保留在 Engineering Pack 技术决策中。

## 第一个验收纵向切片

示例 Brief：

> 创建“寒地巡逻 S1”未来短型武器概念。整体紧凑、厚重、可靠、低调，工业感高、未来感中低；主色深石墨灰，辅色黑色金属，暗红作为识别色。用途为游戏与影视道具概念。

系统必须完成：

```text
创建项目
→ 返回 A/B/C 三种模块组合
→ 选择方案 B
→ 替换前端外壳
→ 用自然语言调整整体长度和细节密度
→ 保护已锁定核心模块与注册表边界
→ 生成并检查半透明 ghost ChangeSet
→ 明确确认后创建不可变子版本
→ 运行 ModelQualityReport
→ 生成爆炸图
→ 导出 GLB + Manifest
→ 重启后恢复项目与版本
```

退出指标：

- 新用户首次有效设计时间小于 5 分钟；
- 模块吸附和替换成功率不低于 95%；
- 锁定模块保持率不低于 95%；
- Undo/Redo、版本回退和崩溃恢复正确率 100%；
- GLB 导出成功率不低于 98%；
- 严重网格问题提示率 100%。

## 开始工作

当前 baseline 安装与门禁：

```bash
npm install
python3 -m venv .venv
.venv/bin/pip install -e "apps/agent[dev]"
npm run r1:gate
npm run r2:contracts-gate
npm run r2:gate
npm run r4:planner-gate
npm run release:gate
```

当前新工作台开发入口：

```bash
npm --workspace apps/desktop run dev -- --host 127.0.0.1
# http://127.0.0.1:1420/#/cad
```

它只是 Vite 开发壳；Tauri 才是最终本地桌面交付路径。完整运行、备份和故障处理见 [操作手册](docs/OPERATIONS.md)。

## 文档地图

- [实施计划](docs/IMPLEMENTATION_PLAN.md)：R0–R6、PR 顺序、C01–C10 和近期行动。
- [系统设计](docs/DESIGN.md)：平台、领域包、合同、API、数据库、视口、检查与后续 CAD/DFM 轨。
- [操作手册](docs/OPERATIONS.md)：当前真实命令、Tauri/Vite 区别、数据和故障处理。
- [Quickstart](docs/QUICKSTART.md)：操作手册统一入口。
- [架构决策](docs/ADR/)：产品范围、领域模型、许可证和迁移决策。
- [执行证据](docs/evidence/)：阶段门禁与回归结果。

## 贡献约束

- 不在 `asset_store.py` 或 `App.tsx` 继续堆积新职责；
- 不把旧 WeaponDesignSpec/CreativeWeaponGraph 机械改名成新合同；
- P0 新业务进入通用 Project/Version 基础设施和 Weapon Concept Pack；
- AI 只能引用已存在 Module ID，所有修改先生成 ChangeSet；
- 原始模块资产和父版本不可覆盖；
- 所有耗时动作使用 Job、幂等、取消、恢复和事件 replay；
- 依赖必须固定版本、记录许可证并进入 SBOM；
- 概念资产不得被 UI 或文档声称为生产级 CAD/DFM 结果。

完整顺序以 [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md) 为准。
