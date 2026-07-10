# ForgeCAD（原武神 Forge）

ForgeCAD 是一个本地优先的 AI 模块化 3D 设计工作台。底层平台保持通用，第一阶段只交付 **Weapon Concept Pack**：面向未来武器概念、游戏资产、影视道具和非功能性展示模型，提供模块组合、AI 设计修改、版本、检查、渲染与导出闭环。

> 新产品不拒绝武器题材。第一阶段允许外观精密、比例明确、模块细致、装配直观和版本可追踪；但产品不会把概念 Mesh/GLB 冒充为功能性武器工程 CAD，也不会在同一默认流程中混入未经实现的 STEP、完整 DFM、BOM 或生产切片。

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
→ 渲染 / 爆炸图 / GLB 导出
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
| 幂等、取消、重试、恢复 | 已实现 | 直接复用 |
| R1 通用基础设施拆分 | 进行中 | connection、migration、object store、Repository/UoW、Job、Library 等已提取 |
| `#/cad` 工作台原型 | 已切换为五阶段 Weapon Concept Workbench | 当前仍是前端交互与程序化 Three.js 模型，尚未接真实 ModuleGraph |
| 旧 CreativeWeaponGraph / SkillGraph | 已实现 | 冻结并删除，不机械改名 |
| 旧图像/神经 3D Provider | 已实现 | 仅作可选概念或局部组件生成来源 |
| 旧 Unity 导出 | 已实现 | legacy baseline；P0 改为通用 GLB/OBJ/Manifest |
| Concept Project / Version / Profile | migration、Repository/UoW、创建/列表/详情/追加版本 API 已实现 | ModuleGraph 持久化仍在 R2 |
| WeaponConceptSpec / ModuleGraph | 合同、注册校验、持久化和回读 API 已实现 | Version 绑定与桌面真实渲染待实现 |
| Connector / 模块吸附 / 爆炸视图 | Connector 合同、数据表、注册和兼容校验已实现；交互未实现 | P0 核心技术阶段 |
| Brief / A-B-C Variant | Brief 入库、确定性三方案、选择与恢复已实现 | 当前是模板基线，不冒充 R4 AI 生成质量 |
| DesignChangeSet 幽灵预览 | proposed/previewed/confirmed、stale base、锁定保护和子版本提交已实现 | AI Change Planner 仍待 R4 |
| ModelQualityReport | 合同、QualityRun/Findings 持久化与 API 已实现 | Graph/Mesh/Assembly 实际检查器进入 R5 |
| DesignSpec / FeatureGraph / CAD Runtime | 未实现 | 后续 CAD/DFM Engineering Pack |

当前主数据链仍是旧 Weapon/Unity baseline；新工作台只是交互原型，不能据此声称 P0 模块系统或 CAD/DFM 已完成。

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
| AI 结构化输出 | Instructor + Pydantic Schema | Brief、方案推荐、ChangeSet |
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
→ 调整整体长度比例和顶部附件高度
→ 锁定握持模块与配色
→ 生成并预览 ChangeSet
→ 确认后创建 V2
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
